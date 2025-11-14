use crate::error::{Error, Result};
use ashpd::desktop::{
    PersistMode,
    screencast::{CursorMode, Screencast, SourceType, Stream as ScreencastStream},
};
use derive_setters::Setters;
use pipewire::{self as pw, spa::param::format};
use pw::{properties::properties, spa};
use screen_capture::{LogicalSize, Position, ScreenInfo};
use std::{
    os::fd::OwnedFd,
    sync::{Arc, Mutex},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

struct UserData {
    format: spa::param::video::VideoInfoRaw,
    frame_count: AtomicU64,
    total_frames: AtomicU64,
    last_time_ns: AtomicU64,
    last_fps_print_ns: AtomicU64,
}

#[derive(Debug, Clone, Setters)]
#[setters(prefix = "with_")]
pub struct PortalCapturer {
    #[setters(prefix = "with_")]
    pub screen_info: ScreenInfo,

    pub include_cursor: bool,
}

impl PortalCapturer {
    pub fn new(screen_info: ScreenInfo) -> Self {
        Self {
            screen_info,
            include_cursor: true,
        }
    }

    pub async fn open_portal(&self) -> Result<(ScreencastStream, OwnedFd)> {
        let proxy = Screencast::new().await?;
        let session = proxy.create_session().await?;
        proxy
            .select_sources(
                &session,
                if self.include_cursor {
                    CursorMode::Embedded
                } else {
                    CursorMode::Hidden
                },
                SourceType::Monitor.into(),
                false,
                None,
                PersistMode::DoNot,
            )
            .await?;

        let response = proxy.start(&session, None).await?.response()?;
        let stream = response
            .streams()
            .first()
            .ok_or(crate::Error::NoStream(
                "no stream found / selected".to_string(),
            ))?
            .to_owned();

        let fd = proxy.open_pipe_wire_remote(&session).await?;

        Ok((stream, fd))
    }

    pub async fn start_streaming(&mut self, node_id: u32, fd: OwnedFd) -> Result<()> {
        pw::init();

        let mainloop = pw::main_loop::MainLoopBox::new(None)?;
        let context = pw::context::ContextBox::new(mainloop.loop_(), None)?;
        let core = context.connect_fd(fd, None)?;

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        
        let data = UserData {
            format: Default::default(),
            frame_count: AtomicU64::new(0),
            total_frames: AtomicU64::new(0),
            last_time_ns: AtomicU64::new(now),
            last_fps_print_ns: AtomicU64::new(now),
        };

        let stream = pw::stream::StreamBox::new(
            &core,
            "wayshot-portal",
            properties! {
                *pw::keys::MEDIA_TYPE => "Video",
                *pw::keys::MEDIA_ROLE => "Screen",
                *pw::keys::MEDIA_CATEGORY => "Capture",
            },
        )?;

        let err_msg = Arc::new(Mutex::new(None));
        let err_msg_clone = err_msg.clone();
        let screen_size = self.screen_info.logical_size;

        let _listener = stream
            .add_local_listener_with_user_data(data)
            .state_changed(|_, _, old, new| {
                log::info!("State changed: {:?} -> {:?}", old, new);
            })
            .param_changed(move |_, user_data, id, param| {
                let Some(param) = param else {
                    return;
                };
                if id != pw::spa::param::ParamType::Format.as_raw() {
                    return;
                }

                let (media_type, media_subtype) =
                    match pw::spa::param::format_utils::parse_format(param) {
                        Ok(v) => v,
                        Err(_) => return,
                    };

                if media_type != pw::spa::param::format::MediaType::Video
                    || media_subtype != pw::spa::param::format::MediaSubtype::Raw
                {
                    return;
                }

                if let Err(e) = user_data.format.parse(param) {
                    log::warn!("Failed to parse param changed to VideoInfoRaw: {e}");
                    return;
                }

                if screen_size
                    != LogicalSize::new(
                        user_data.format.size().width as i32,
                        user_data.format.size().height as i32,
                    )
                {
                    let msg = format!(
                        "selected screen size: {}x{}. Found {}x{}",
                        screen_size.width,
                        screen_size.height,
                        user_data.format.size().width,
                        user_data.format.size().height
                    );

                    *err_msg.lock().unwrap() = Some(msg);
                    return;
                }

                log::info!("got video format:");
                log::info!(
                    "\tformat: {} ({:?})",
                    user_data.format.format().as_raw(),
                    user_data.format.format()
                );
                log::info!(
                    "\tsize: {}x{}",
                    user_data.format.size().width,
                    user_data.format.size().height
                );
                log::info!(
                    "\tframerate: {}/{}",
                    user_data.format.framerate().num,
                    user_data.format.framerate().denom
                );
            })
            .process(|stream, user_data| {
                match stream.dequeue_buffer() {
                    None => log::warn!("out of buffers"),
                    Some(mut buffer) => {
                        let datas = buffer.datas_mut();
                        if datas.is_empty() {
                            return;
                        }

                        // copy frame data to screen
                        let data = &mut datas[0];
                        
                        // 帧率检测代码
                        let frame_count = user_data.frame_count.fetch_add(1, Ordering::Relaxed) + 1;
                        let total_frames = user_data.total_frames.fetch_add(1, Ordering::Relaxed) + 1;
                        let now_ns = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_nanos() as u64;
                        
                        let last_fps_print_ns = user_data.last_fps_print_ns.load(Ordering::Relaxed);
                        
                        // 每秒打印一次FPS
                        if now_ns - last_fps_print_ns >= 1_000_000_000 {
                            let last_time_ns = user_data.last_time_ns.load(Ordering::Relaxed);
                            let duration_s = (now_ns - last_time_ns) as f32 / 1_000_000_000.0;
                            
                            if duration_s > 0.0 {
                                let fps = frame_count as f32 / duration_s;
                                println!("=== FPS STATISTICS ===");
                                println!("Current FPS: {:.1}", fps);
                                println!("Total frames received: {}", total_frames);
                                println!("Frame size: {} bytes", data.chunk().size());
                            }
                            
                            // 重置计数器
                            user_data.frame_count.store(0, Ordering::Relaxed);
                            user_data.last_time_ns.store(now_ns, Ordering::Relaxed);
                            user_data.last_fps_print_ns.store(now_ns, Ordering::Relaxed);
                        }
                        
                        // 每50帧打印一次帧信息
                        if frame_count % 50 == 0 {
                            println!("Frame #{} (size: {} bytes)", total_frames, data.chunk().size());
                        }
                    }
                }
            })
            .register()?;

        log::debug!("Created stream {:#?}", stream);

        if let Some(ref msg) = *err_msg_clone.lock().unwrap() {
            return Err(Error::ScreenInfoError(msg.clone()));
        }

        let obj = pw::spa::pod::object!(
            pw::spa::utils::SpaTypes::ObjectParamFormat,
            pw::spa::param::ParamType::EnumFormat,
            pw::spa::pod::property!(
                pw::spa::param::format::FormatProperties::MediaType,
                Id,
                pw::spa::param::format::MediaType::Video
            ),
            pw::spa::pod::property!(
                pw::spa::param::format::FormatProperties::MediaSubtype,
                Id,
                pw::spa::param::format::MediaSubtype::Raw
            ),
            pw::spa::pod::property!(
                pw::spa::param::format::FormatProperties::VideoFormat,
                Choice,
                Enum,
                Id,
                pw::spa::param::video::VideoFormat::RGBA,
                pw::spa::param::video::VideoFormat::RGBx,
            ),
            pw::spa::pod::property!(
                pw::spa::param::format::FormatProperties::VideoSize,
                Choice,
                Range,
                Rectangle,
                // Default/resolution
                pw::spa::utils::Rectangle {
                    width: 1920,
                    height: 1080
                },
                // Minimum supported resolution
                pw::spa::utils::Rectangle {
                    width: 1,
                    height: 1
                },
                // Maximum supported resolution
                pw::spa::utils::Rectangle {
                    width: 4096,
                    height: 4096
                }
            ),
            pw::spa::pod::property!(
                pw::spa::param::format::FormatProperties::VideoFramerate,
                Choice,
                Range,
                Fraction,
                // Default framerate
                pw::spa::utils::Fraction { num: 25, denom: 1 },
                // Minimum framerate
                pw::spa::utils::Fraction { num: 0, denom: 1 },
                // Maximum framerate
                pw::spa::utils::Fraction { num: 60, denom: 1 }
            ),
        );
        let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            &pw::spa::pod::Value::Object(obj),
        )
        .unwrap()
        .0
        .into_inner();

        let mut params = [spa::pod::Pod::from_bytes(&values).unwrap()];

        stream.connect(
            spa::utils::Direction::Input,
            Some(node_id),
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )?;

        log::info!("Portal connected stream sucessfully");

        mainloop.run();

        Ok(())
    }
}
