use crate::audio_level::{apply_gain, calc_rms_level};
use crossbeam::channel::Sender;
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
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
    time::Duration,
};
use thiserror::Error;

type WavWriterType = hound::WavWriter<std::io::BufWriter<std::fs::File>>;

/// Error types for speaker output recording operations.
///
/// This enum represents errors that can occur during speaker output
/// recording using PipeWire, including file writing and PipeWire API errors.
///
/// # Examples
///
/// ```no_run
/// use recorder::{SpeakerRecorder, SpeakerError};
/// use std::sync::Arc;
/// use std::sync::atomic::AtomicBool;
///
/// let stop_sig = Arc::new(AtomicBool::new(false));
/// let recorder = SpeakerRecorder::new("speaker.wav".into(), stop_sig, None, false);
///
/// match recorder {
///     Ok(_) => println!("Speaker recorder created"),
///     Err(SpeakerError::PipewireError(e)) => eprintln!("PipeWire error: {}", e),
///     Err(SpeakerError::WriterError(e)) => eprintln!("File error: {}", e),
/// }
/// ```
#[derive(Debug, Error)]
pub enum SpeakerError {
    /// WAV file creation or writing failed
    #[error("Write WAV error: {0}")]
    WriterError(String),

    /// PipeWire API operation failed
    #[error("Pipewire error: {0}")]
    PipewireError(String),
}

#[derive(Clone)]
struct RecordingSession {
    writer: Arc<Mutex<Option<WavWriterType>>>,
    mainloop: MainLoopRc,
}

impl RecordingSession {
    fn stop_recording(&self) -> Result<(), SpeakerError> {
        log::info!("Stop recording speaker and close WAV file...");

        if let Ok(mut writer_opt) = self.writer.lock() {
            if let Some(writer) = writer_opt.take() {
                writer
                    .finalize()
                    .map_err(|e| SpeakerError::WriterError(format!("Finalize WAV failed: {e}")))?;
                log::info!("Successfully close WAV file");
            }
        }
        self.mainloop.quit();
        Ok(())
    }
}

/// Speaker output recorder for capturing system audio using PipeWire.
///
/// This struct provides speaker output recording capabilities on Wayland systems
/// using the PipeWire audio server. It can capture system audio output (what you hear)
/// and save it to WAV files with optional real-time audio level monitoring.
///
/// # Features
///
/// - System audio output recording using PipeWire monitor ports
/// - Automatic discovery of default output devices
/// - Real-time audio level monitoring with RMS calculation
/// - WAV file output with CD-quality settings (48kHz, 32-bit float, stereo)
/// - Preview mode for processing without file output
///
/// # Examples
///
/// ```no_run
/// use recorder::SpeakerRecorder;
/// use std::sync::Arc;
/// use std::sync::atomic::AtomicBool;
///
/// let stop_sig = Arc::new(AtomicBool::new(false));
/// let recorder = SpeakerRecorder::new("speaker.wav".into(), stop_sig, None, false).unwrap();
///
/// // Start recording in a separate thread
/// // recorder.start_recording().unwrap();
/// ```
#[derive(Debug)]
pub struct SpeakerRecorder {
    /// PipeWire main loop for event processing
    mainloop: MainLoopRc,
    /// PipeWire core for API operations
    core: CoreRc,
    /// Path where the WAV file will be saved
    save_path: PathBuf,
    /// Signal to stop recording when set to true
    stop_sig: Arc<AtomicBool>,
    /// Optional sender for audio level data (if monitoring enabled)
    level_sender: Option<Arc<Sender<f32>>>,
    /// Whether to run in preview mode (no file output)
    disable_save_file: bool,
    // [0, infinity]
    amplification: Option<Arc<AtomicI32>>,

    device_info: Option<(u32, String)>,
}

impl SpeakerRecorder {
    /// Create a new speaker recorder with specified parameters.
    ///
    /// This constructor initializes the PipeWire context and prepares
    /// the recorder for capturing system audio output.
    ///
    /// # Arguments
    ///
    /// * `save_path` - Path where the WAV file will be saved
    /// * `stop_sig` - Atomic boolean signal to stop recording
    /// * `level_sender` - Optional sender for audio level monitoring
    /// * `disable_save_file` - If true, no file will be written
    ///
    /// # Returns
    ///
    /// `Ok(SpeakerRecorder)` if initialization succeeded, or `Err(SpeakerError)` if failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::SpeakerRecorder;
    /// use std::sync::Arc;
    /// use std::sync::atomic::AtomicBool;
    ///
    /// let stop_sig = Arc::new(AtomicBool::new(false));
    /// let recorder = SpeakerRecorder::new("speaker.wav".into(), stop_sig, None, false).unwrap();
    /// ```
    pub fn new(
        save_path: PathBuf,
        stop_sig: Arc<AtomicBool>,
        level_sender: Option<Arc<Sender<f32>>>,
        disable_save_file: bool,
    ) -> Result<Self, SpeakerError> {
        pipewire::init();

        let mainloop = MainLoopRc::new(None)
            .map_err(|e| SpeakerError::PipewireError(format!("MainLoop new failed: {e}")))?;
        let context = ContextRc::new(&mainloop, None)
            .map_err(|e| SpeakerError::PipewireError(format!("Context new faield: {e}")))?;
        let core = context
            .connect_rc(None)
            .map_err(|e| SpeakerError::PipewireError(format!("context connect failed: {e}")))?;

        let mut recoder = Self {
            mainloop,
            core,
            save_path,
            stop_sig,
            level_sender,
            disable_save_file,
            amplification: None,
            device_info: None,
        };

        let output_device = recoder.find_default_output()?;
        recoder.device_info = output_device.clone();
        Ok(recoder)
    }

    pub fn with_amplification(mut self, v: Arc<AtomicI32>) -> Self {
        self.amplification = Some(v);
        self
    }

    /// Start recording system audio output to WAV file.
    ///
    /// This method begins capturing system audio output using PipeWire monitor ports.
    /// It automatically discovers the default output device, creates a WAV file,
    /// and starts the recording session with real-time audio level monitoring.
    ///
    /// # Returns
    ///
    /// `Ok(())` if recording started successfully, or `Err(SpeakerError)` if failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::SpeakerRecorder;
    /// use std::sync::Arc;
    /// use std::sync::atomic::AtomicBool;
    ///
    /// let stop_sig = Arc::new(AtomicBool::new(false));
    /// let recorder = SpeakerRecorder::new("speaker.wav".into(), stop_sig, None, false).unwrap();
    ///
    /// // Start recording (this will block until stop_sig is set)
    /// // recorder.start_recording().unwrap();
    /// ```
    pub fn start_recording(&mut self) -> Result<(), SpeakerError> {
        let Some((node_id, ref node_name)) = self.device_info else {
            return Err(SpeakerError::PipewireError(format!(
                "No found output speaker device (None)"
            )));
        };

        log::info!(
            "Start record speaker. device: {} (ID: {})",
            node_name,
            node_id
        );

        let writer = if self.disable_save_file {
            Arc::new(Mutex::new(None))
        } else {
            let spec = hound::WavSpec {
                channels: 2,
                sample_rate: 48000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };
            let writer = hound::WavWriter::create(&self.save_path, spec)
                .map_err(|e| SpeakerError::WriterError(format!("crate WavWriter faile: {e}")))?;
            Arc::new(Mutex::new(Some(writer)))
        };

        let session = RecordingSession {
            writer: writer.clone(),
            mainloop: self.mainloop.clone(),
        };

        // Create an input stream to record the monitoring port of the output device.
        let stream = self.create_stream()?;
        let _stream_listener = Self::stream_register(
            &stream,
            writer.clone(),
            self.level_sender.clone(),
            self.amplification.clone(),
        )?;
        Self::stream_connect(&stream, node_id)?;

        while !self.stop_sig.load(Ordering::Relaxed) {
            self.mainloop.loop_().iterate(Duration::from_millis(100));
        }

        session.stop_recording()?;

        if !self.disable_save_file {
            log::info!(
                "Successfully save speaker file: {}",
                self.save_path.display()
            );
        }

        Ok(())
    }

    pub fn get_device_info(&self) -> Option<(u32, String)> {
        self.device_info.clone()
    }

    /// Stop the speaker recording session.
    ///
    /// This method signals the recording thread to stop and clean up resources.
    /// It sets the stop signal which will cause the recording loop to exit,
    /// finalize the WAV file, and close the PipeWire connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::SpeakerRecorder;
    /// use std::sync::Arc;
    /// use std::sync::atomic::AtomicBool;
    ///
    /// let stop_sig = Arc::new(AtomicBool::new(false));
    /// let recorder = SpeakerRecorder::new("speaker.wav".into(), stop_sig.clone(), None, false).unwrap();
    ///
    /// // In another thread, start recording
    /// // std::thread::spawn(move || {
    /// //     recorder.start_recording().unwrap();
    /// // });
    ///
    /// // Later, stop the recording
    /// recorder.stop();
    /// ```
    pub fn stop(&self) {
        self.stop_sig.store(true, Ordering::Relaxed);
    }

    pub fn find_default_output(&self) -> Result<Option<(u32, String)>, SpeakerError> {
        log::info!("Start search output audio devices...");

        let registry = self
            .core
            .get_registry()
            .map_err(|e| SpeakerError::PipewireError(format!("Get registry failed: {e}")))?;
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
            return Err(SpeakerError::PipewireError(format!(
                "No found output speaker device"
            )));
        }
    }

    fn create_stream<'a>(&'a self) -> Result<StreamBox<'a>, SpeakerError> {
        let stream_props = pipewire::properties::properties! {
            "node.name" => "wayshot-speaker-recorder",
            "media.class" => "Stream/Input/Audio",
            "audio.channels" => "2",
            "audio.rate" => "48000",
            "stream.monitor" => "true"
        };

        log::info!("Create audio stream...");
        let stream = StreamBox::new(&self.core, "wayshot-speaker-recorder", stream_props)
            .map_err(|e| SpeakerError::PipewireError(format!("New StreamBox failed: {e}")))?;
        log::info!("Successfully create audio stream");

        Ok(stream)
    }

    fn stream_register(
        stream: &StreamBox,
        writer: Arc<Mutex<Option<WavWriterType>>>,
        level_sender: Option<Arc<Sender<f32>>>,
        amplification: Option<Arc<AtomicI32>>,
    ) -> Result<StreamListener<()>, SpeakerError> {
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

                    let mut f32_sample_amplification = Vec::with_capacity(f32_samples.len());
                    let f32_samples = if let Some(ref amplification) = amplification {
                        f32_sample_amplification.extend_from_slice(f32_samples);

                        apply_gain(
                            &mut f32_sample_amplification,
                            amplification.load(Ordering::Relaxed) as f32,
                        );

                        &f32_sample_amplification[..]
                    } else {
                        f32_samples
                    };

                    if let Ok(mut writer_opt) = writer.lock()
                        && let Some(ref mut wav_writer) = *writer_opt
                    {
                        for &sample in f32_samples {
                            if wav_writer.write_sample(sample).is_err() {
                                break;
                            }
                        }
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
            .map_err(|e| SpeakerError::PipewireError(format!("stream register failed: {e}")))?;

        Ok(stream_listener)
    }

    fn stream_connect(stream: &StreamBox, node_id: u32) -> Result<(), SpeakerError> {
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
        .map_err(|e| SpeakerError::PipewireError(format!("PodSerializer failed: {e}")))?
        .0
        .into_inner();

        let mut params = [Pod::from_bytes(&values)
            .ok_or("Pod from bytes is none")
            .map_err(|e| SpeakerError::PipewireError(e.to_string()))?];

        log::info!("connet to audio device...");
        stream
            .connect(
                pipewire::spa::utils::Direction::Input,
                Some(node_id),
                StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
                &mut params,
            )
            .map_err(|e| SpeakerError::PipewireError(format!("stream connect failed: {e}")))?;
        log::info!("Successfully connect to audio device");

        Ok(())
    }
}
