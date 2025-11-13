use crate::error::{Error, Result};
use ashpd::desktop::{
    PersistMode,
    screencast::{CursorMode, Screencast, SourceType, Stream as ScreencastStream},
};
use derive_setters::Setters;
use pipewire as pw;
use pw::{properties::properties, spa};
use screen_capture::{LogicalSize, Position, ScreenInfo};
use std::os::fd::OwnedFd;

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

        log::warn!("======== {:?}", stream.source_type());

        Ok((stream, fd))
    }

    pub async fn start_streaming(&mut self, node_id: u32, fd: OwnedFd) -> Result<()> {
        struct UserData {
            format: spa::param::video::VideoInfoRaw,
        }

        pw::init();

        let mainloop = pw::main_loop::MainLoopBox::new(None)?;
        let context = pw::context::ContextBox::new(mainloop.loop_(), None)?;
        let core = context.connect_fd(fd, None)?;

        let data = UserData {
            format: Default::default(),
        };

        let stream = pw::stream::StreamBox::new(
            &core,
            "wayshot-portal",
            properties! {
                *pw::keys::MEDIA_TYPE => "Video",
                *pw::keys::MEDIA_CATEGORY => "Capture",
                *pw::keys::MEDIA_ROLE => "Screen",
            },
        )?;

        let _listener = stream
            .add_local_listener_with_user_data(data)
            .state_changed(|_, _, old, new| {
                log::info!("State changed: {:?} -> {:?}", old, new);
            })
            .param_changed(|_, user_data, id, param| {
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
            .process(|stream, _| {
                match stream.dequeue_buffer() {
                    None => log::warn!("out of buffers"),
                    Some(mut buffer) => {
                        let datas = buffer.datas_mut();
                        if datas.is_empty() {
                            return;
                        }

                        // copy frame data to screen
                        let data = &mut datas[0];
                        println!("got a frame of size {}", data.chunk().size());
                    }
                }
            })
            .register()?;

        log::debug!("Created stream {:#?}", stream);

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
                // Default framerate (30 FPS)
                pw::spa::utils::Fraction { num: 30, denom: 1 },
                // Minimum framerate (1 FPS)
                pw::spa::utils::Fraction { num: 0, denom: 1 },
                // Maximum framerate (60 FPS)
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
