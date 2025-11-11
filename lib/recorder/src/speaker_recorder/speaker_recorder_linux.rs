use crate::{
    audio_level::{apply_gain, calc_rms_level},
    speaker_recorder::{SpeakerRecorder, SpeakerRecorderConfig, SpeakerRecorderError},
};
use crossbeam::channel::Sender;
use hound::WavSpec;
use pipewire::{
    context::ContextRc,
    core::CoreRc,
    main_loop::MainLoopRc,
    spa::{
        param::audio::{AudioFormat, AudioInfoRaw},
        pod::Pod,
    },
    stream::{StreamBox, StreamFlags, StreamListener},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};

pub struct SpeakerRecorderLinux {
    config: SpeakerRecorderConfig,
    core: CoreRc,
    mainloop: MainLoopRc,
    device_info: Option<(u32, String)>,
}

impl SpeakerRecorderLinux {
    pub fn new(config: SpeakerRecorderConfig) -> Result<Self, SpeakerRecorderError> {
        pipewire::init();

        let mainloop = MainLoopRc::new(None).map_err(|e| {
            SpeakerRecorderError::PipewireError(format!("MainLoop new failed: {e}"))
        })?;
        let context = ContextRc::new(&mainloop, None)
            .map_err(|e| SpeakerRecorderError::PipewireError(format!("Context new faield: {e}")))?;
        let core = context.connect_rc(None).map_err(|e| {
            SpeakerRecorderError::PipewireError(format!("context connect failed: {e}"))
        })?;

        let mut recoder = Self {
            config,
            core,
            mainloop,
            device_info: None,
        };

        let output_device = recoder.find_default_output()?;
        recoder.device_info = output_device.clone();
        Ok(recoder)
    }

    // pub fn stop(&self) {
    //     self.config.stop_sig.store(true, Ordering::Relaxed);
    // }

    fn create_stream<'a>(&'a self) -> Result<StreamBox<'a>, SpeakerRecorderError> {
        let stream_props = pipewire::properties::properties! {
            "node.name" => "wayshot-speaker-recorder",
            "media.class" => "Stream/Input/Audio",
            "audio.channels" => "2",
            "audio.rate" => "48000",
            "stream.monitor" => "true"
        };

        log::info!("Create audio stream...");
        let stream =
            StreamBox::new(&self.core, "wayshot-speaker-recorder", stream_props).map_err(|e| {
                SpeakerRecorderError::PipewireError(format!("New StreamBox failed: {e}"))
            })?;
        log::info!("Successfully create audio stream");

        Ok(stream)
    }

    fn stream_register(
        stream: &StreamBox,
        frame_sender: Option<Sender<Vec<f32>>>,
        level_sender: Option<Sender<f32>>,
        gain: Option<Arc<AtomicI32>>,
    ) -> Result<StreamListener<()>, SpeakerRecorderError> {
        let stream_listener = stream
            .add_local_listener::<()>()
            .process(move |stream, _| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    log::warn!("No available audio buffer");
                    return;
                };

                for data in buffer.datas_mut() {
                    let chunk_size = data.chunk().size() as usize;

                    // log::debug!("chunk_size: {chunk_size} bytes");

                    let Some(samples) = data.data() else {
                        log::warn!("can not get audio data");
                        continue;
                    };

                    let f32_samples: &[f32] = unsafe {
                        std::slice::from_raw_parts(
                            samples.as_ptr() as *const f32,
                            chunk_size / std::mem::size_of::<f32>(),
                        )
                    };

                    let mut f32_samples_gained = Vec::with_capacity(f32_samples.len());
                    let f32_samples = if let Some(ref gain) = gain {
                        f32_samples_gained.extend_from_slice(f32_samples);
                        apply_gain(&mut f32_samples_gained, gain.load(Ordering::Relaxed) as f32);
                        &f32_samples_gained[..]
                    } else {
                        f32_samples
                    };

                    if let Some(ref tx) = frame_sender
                        && let Err(e) = tx.try_send(f32_samples.to_vec())
                    {
                        log::warn!("try send speaker audio frame failed: {e}");
                    }

                    if let Some(ref tx) = level_sender
                        && let Some(db) = calc_rms_level(f32_samples)
                        && let Err(e) = tx.try_send(db)
                    {
                        log::warn!("try send speaker audio db level data failed: {e}");
                    }
                }
            })
            .register()
            .map_err(|e| {
                SpeakerRecorderError::PipewireError(format!("stream register failed: {e}"))
            })?;

        Ok(stream_listener)
    }

    fn stream_connect(stream: &StreamBox, node_id: u32) -> Result<(), SpeakerRecorderError> {
        let mut audio_info = AudioInfoRaw::new();
        audio_info.set_format(AudioFormat::F32LE);
        audio_info.set_rate(48000);
        audio_info.set_channels(2);

        let obj = pipewire::spa::pod::object!(
            pipewire::spa::utils::SpaTypes::ObjectParamFormat,
            pipewire::spa::param::ParamType::EnumFormat,
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::MediaType,
                Id,
                pipewire::spa::param::format::MediaType::Audio
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::MediaSubtype,
                Id,
                pipewire::spa::param::format::MediaSubtype::Raw
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::AudioFormat,
                Id,
                AudioFormat::F32LE
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::AudioRate,
                Int,
                48000
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::AudioChannels,
                Int,
                2
            ),
        );

        let values: Vec<u8> = pipewire::spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            &pipewire::spa::pod::Value::Object(obj),
        )
        .map_err(|e| SpeakerRecorderError::PipewireError(format!("PodSerializer failed: {e}")))?
        .0
        .into_inner();

        let mut params = [Pod::from_bytes(&values)
            .ok_or("Pod from bytes is none")
            .map_err(|e| SpeakerRecorderError::PipewireError(e.to_string()))?];

        log::info!("connet to audio device...");
        stream
            .connect(
                pipewire::spa::utils::Direction::Input,
                Some(node_id),
                StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
                &mut params,
            )
            .map_err(|e| {
                SpeakerRecorderError::PipewireError(format!("stream connect failed: {e}"))
            })?;
        log::info!("Successfully connect to audio device");

        Ok(())
    }
}

impl SpeakerRecorder for SpeakerRecorderLinux {
    fn spec(&self) -> WavSpec {
        WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        }
    }

    fn get_device_info(&self) -> Option<(u32, String)> {
        self.device_info.clone()
    }

    fn start_recording(self) -> Result<(), SpeakerRecorderError> {
        let Some((node_id, ref node_name)) = self.device_info else {
            return Err(SpeakerRecorderError::PipewireError(format!(
                "No found output speaker device (None)"
            )));
        };

        log::info!(
            "Start record speaker. device: {} (ID: {})",
            node_name,
            node_id
        );

        // Create an input stream to record the monitoring port of the output device.
        let stream = self.create_stream()?;
        let _stream_listener = Self::stream_register(
            &stream,
            self.config.frame_sender.clone(),
            self.config.level_sender.clone(),
            self.config.gain.clone(),
        )?;
        Self::stream_connect(&stream, node_id)?;

        while !self.config.stop_sig.load(Ordering::Relaxed) {
            self.mainloop.loop_().iterate(Duration::from_millis(100));
        }

        self.mainloop.quit();
        Ok(())
    }

    fn find_default_output(&self) -> Result<Option<(u32, String)>, SpeakerRecorderError> {
        log::info!("Start search output audio devices...");

        let registry = self.core.get_registry().map_err(|e| {
            SpeakerRecorderError::PipewireError(format!("Get registry failed: {e}"))
        })?;
        let output_info = Arc::new(std::sync::Mutex::new(None));
        let output_info_clone = output_info.clone();

        let _listener = registry
            .add_listener_local()
            .global(move |global| {
                // log::debug!(
                //     "Find audio device: type={:?}, ID={}, attr: {:?}",
                //     global.type_,
                //     global.id,
                //     global.props
                // );

                if global.type_ == pipewire::types::ObjectType::Node
                    && let Some(props) = &global.props
                    && let (Some(media_class), Some(node_name)) =
                        (props.get("media.class"), props.get("node.name"))
                    && (media_class == "Audio/Sink" || media_class.starts_with("Audio/Sink"))
                {
                    // log::debug!(
                    //     "Find audio output device : {} (ID: {}), type={:?}, attr: {:?}",
                    //     node_name,
                    //     global.id,
                    //     global.type_,
                    //     global.props
                    // );

                    if let Some(priority) = props.get("priority.session")
                        && let Ok(priority) = priority.parse::<i32>()
                    {
                        let mut info = output_info_clone.lock().unwrap();
                        match info.take() {
                            None => *info = Some((global.id, priority, node_name.to_string())),
                            Some((_, old_priority, _)) if old_priority < priority => {
                                *info = Some((global.id, priority, node_name.to_string()))
                            }
                            Some(old_item) => *info = Some(old_item),
                        }
                    }
                }
            })
            .register();

        log::info!("Wait enumerate devices...");
        for _ in 0..10 {
            self.mainloop.loop_().iterate(Duration::from_millis(100));
        }

        log::info!("Find default device: {:?}", output_info);

        let result = output_info
            .lock()
            .unwrap()
            .take()
            .map(|(id, _, name)| (id, name.to_string()));

        if result.is_some() {
            return Ok(result);
        } else {
            return Err(SpeakerRecorderError::PipewireError(format!(
                "No found output speaker device"
            )));
        }
    }
}
