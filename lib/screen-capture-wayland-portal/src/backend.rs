use crate::error::{Error, Result};
use ashpd::desktop::{
    PersistMode,
    screencast::{CursorMode, Screencast, SourceType, Stream as ScreencastStream},
};
use crossbeam::channel::Sender;
use derive_setters::Setters;
use pipewire as pw;
use pw::{properties::properties, spa};
use screen_capture::{LogicalSize, ScreenInfo};
use spin_sleep::SpinSleeper;
use std::{
    os::fd::OwnedFd,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

struct UserData {
    format: spa::param::video::VideoInfoRaw,
    total_frames: AtomicU64,
    sleeper: SpinSleeper,
    start_time: Instant,
}

#[derive(Debug, Clone, Setters)]
#[setters(prefix = "with_")]
pub struct PortalCapturer {
    #[setters(skip)]
    pub screen_info: ScreenInfo,

    pub fps: u32,
    pub include_cursor: bool,
    pub stop_sig: Arc<AtomicBool>,
    pub sender: Option<Sender<Vec<u8>>>,
}

impl PortalCapturer {
    pub fn new(screen_info: ScreenInfo) -> Self {
        Self {
            screen_info,
            fps: 25,
            include_cursor: true,
            stop_sig: Arc::new(AtomicBool::new(false)),
            sender: None,
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

        let data = UserData {
            format: Default::default(),
            total_frames: AtomicU64::new(0),
            sleeper: SpinSleeper::default(),
            start_time: Instant::now(),
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
        let sender = self.sender.clone();
        let interval_ms = 1000.0 / self.fps as f64;

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
            .process(move |stream, user_data| match stream.dequeue_buffer() {
                None => log::warn!("out of buffers"),
                Some(mut buffer) => {
                    let datas = buffer.datas_mut();
                    if datas.is_empty() {
                        return;
                    }

                    let data = &mut datas[0].data().unwrap_or_default();

                    if !data.is_empty() {
                        let index = user_data.total_frames.fetch_add(1, Ordering::Relaxed);

                        if let Some(ref sender) = sender
                            && let Err(e) = sender.try_send(data.to_vec())
                        {
                            log::warn!("portal try send frame failed: {e:?}");
                        }

                        let target_time = user_data.start_time
                            + Duration::from_millis((interval_ms * (index + 1) as f64) as u64);
                        user_data.sleeper.sleep_until(target_time);
                    }

                    // log::debug!("frame size: {}", data.len());
                }
            })
            .register()?;

        log::debug!("Created stream {:#?}", stream);

        if let Some(ref msg) = *err_msg_clone.lock().unwrap() {
            return Err(Error::ScreenInfoError(msg.clone()));
        }

        stream.connect(
            spa::utils::Direction::Input,
            Some(node_id),
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut [spa::pod::Pod::from_bytes(&self.init_pipewire_pod()).unwrap()],
        )?;

        log::info!("Portal connected stream sucessfully");

        while !self.stop_sig.load(Ordering::Relaxed) {
            let _fd_counts = mainloop.loop_().iterate(Duration::from_millis(10));
            // log::debug!("mainloop processed fd counts: {_fd_counts}");
        }

        mainloop.quit();
        log::info!("exit Portal mainloop");

        Ok(())
    }

    fn init_pipewire_pod(&self) -> Vec<u8> {
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
                    width: self.screen_info.logical_size.width as u32,
                    height: self.screen_info.logical_size.height as u32
                },
                // Minimum supported resolution
                pw::spa::utils::Rectangle {
                    width: 1,
                    height: 1
                },
                // Maximum supported resolution
                pw::spa::utils::Rectangle {
                    width: self.screen_info.logical_size.width as u32,
                    height: self.screen_info.logical_size.height as u32
                }
            ),
            pw::spa::pod::property!(
                pw::spa::param::format::FormatProperties::VideoFramerate,
                Choice,
                Range,
                Fraction,
                // Default framerate
                pw::spa::utils::Fraction {
                    num: self.fps,
                    denom: 1
                },
                // Minimum framerate
                pw::spa::utils::Fraction { num: 0, denom: 1 },
                // Maximum framerate
                pw::spa::utils::Fraction {
                    num: self.fps * 2,
                    denom: 1
                }
            ),
        );

        let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            &pw::spa::pod::Value::Object(obj),
        )
        .unwrap()
        .0
        .into_inner();

        values
    }
}
