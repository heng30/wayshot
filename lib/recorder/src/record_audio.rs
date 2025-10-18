use crate::{apply_gain, calc_rms_level};
use cpal::{
    Device, Host, InputCallbackInfo, SampleFormat, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam::channel::{Receiver, Sender, bounded};
use hound::{WavSpec, WavWriter};
use std::{
    fs::File,
    io::BufWriter,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, Ordering},
    },
};
use thiserror::Error;

/// Audio recording error types.
///
/// This enum represents all possible errors that can occur during
/// audio recording operations, including device discovery, stream
/// creation, file operations, and encoding.
///
/// # Examples
///
/// ```no_run
/// use recorder::{AudioRecorder, AudioError};
///
/// match AudioRecorder::new(None) {
///     Ok(recorder) => println!("Audio recorder created"),
///     Err(AudioError::HostError(e)) => eprintln!("Host error: {}", e),
///     Err(AudioError::DeviceError(e)) => eprintln!("Device error: {}", e),
///     Err(e) => eprintln!("Other error: {}", e),
/// }
/// ```
#[derive(Debug, Error)]
pub enum AudioError {
    /// Audio host initialization or discovery failed
    #[error("Audio host error: {0}")]
    HostError(String),
    /// Audio device access or configuration failed
    #[error("Audio device error: {0}")]
    DeviceError(String),
    /// Audio stream creation or operation failed
    #[error("Audio stream error: {0}")]
    StreamError(String),
    /// Audio configuration parameters are invalid
    #[error("Audio configuration error: {0}")]
    ConfigError(String),
    /// Audio file creation or writing failed
    #[error("Audio file error: {0}")]
    FileError(String),
    /// Audio sample encoding or format conversion failed
    #[error("Audio encoding error: {0}")]
    EncodingError(String),
}

/// Information about an audio device for recording.
///
/// This struct contains metadata about available audio devices,
/// including their names, default configurations, and supported formats.
///
/// # Examples
///
/// ```no_run
/// use recorder::{AudioRecorder, AudioDeviceInfo};
///
/// let recorder = AudioRecorder::new(None).unwrap();
/// let devices = recorder.get_available_devices().unwrap();
///
/// for device in devices {
///     println!("Device: {}", device.name);
///     if let Some(config) = device.default_config {
///         println!("  Sample rate: {} Hz", config.sample_rate.0);
///         println!("  Channels: {}", config.channels);
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    /// Device name as reported by the audio system
    pub name: String,
    /// Default stream configuration for this device
    pub default_config: Option<StreamConfig>,
    /// Supported sample formats (F32, I16, etc.)
    pub supported_formats: Vec<SampleFormat>,
}

/// Audio recorder for capturing audio input from microphones and other input devices.
///
/// This struct provides high-level audio recording capabilities using the CPAL library.
/// It supports device discovery, configuration, and real-time audio capture with
/// optional audio level monitoring.
///
/// # Features
///
/// - Audio device discovery and enumeration
/// - Real-time audio recording with configurable parameters
/// - Audio level monitoring with RMS calculation
/// - WAV file output with proper formatting
/// - Thread-safe audio level channel for real-time monitoring
///
/// # Examples
///
/// ```no_run
/// use recorder::AudioRecorder;
///
/// // Create audio recorder with audio level monitoring
/// let recorder = AudioRecorder::new(Some(1024)).unwrap();
///
/// // Get available devices
/// let devices = recorder.get_available_devices().unwrap();
/// for device in devices {
///     println!("Found device: {}", device.name);
/// }
///
/// // Get audio level receiver for real-time monitoring
/// if let Some(receiver) = recorder.get_audio_level_receiver() {
///     // Monitor audio levels in another thread
///     // while let Ok(level) = receiver.recv() {
///     //     println!("Audio level: {:.1} dB", level);
///     // }
/// }
/// ```
pub struct AudioRecorder {
    /// CPAL audio host for device management
    host: Host,

    /// Optional sender for audio level data (if monitoring enabled)
    audio_level_sender: Option<Arc<Sender<f32>>>,

    /// Optional receiver for audio level data (if monitoring enabled)
    audio_level_receiver: Option<Arc<Receiver<f32>>>,

    // [0, infinity]
    amplification: Option<Arc<AtomicI32>>,
}

impl AudioRecorder {
    /// Create a new audio recorder with optional audio level monitoring.
    ///
    /// # Arguments
    ///
    /// * `level_channel_size` - Optional size for audio level monitoring channel.
    ///   If `Some(size)`, enables audio level monitoring with the specified queue size.
    ///   If `None`, audio level monitoring is disabled.
    ///
    /// # Returns
    ///
    /// `Ok(AudioRecorder)` if initialization succeeded, or `Err(AudioError)` if failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::AudioRecorder;
    ///
    /// // Create recorder without audio level monitoring
    /// let recorder = AudioRecorder::new(None).unwrap();
    ///
    /// // Create recorder with audio level monitoring (1024 sample queue)
    /// let recorder_with_monitoring = AudioRecorder::new(Some(1024)).unwrap();
    /// ```
    pub fn new(level_channel_size: Option<usize>) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let (audio_level_sender, audio_level_receiver) = if let Some(size) = level_channel_size {
            assert!(size > 0);
            let (tx, rx) = bounded(size);
            (Some(Arc::new(tx)), Some(Arc::new(rx)))
        } else {
            (None, None)
        };

        Ok(Self {
            host,
            audio_level_sender,
            audio_level_receiver,
            amplification: None,
        })
    }

    pub fn with_amplification(mut self, v: Arc<AtomicI32>) -> Self {
        self.amplification = Some(v);
        self
    }

    /// Get the audio level receiver for real-time monitoring.
    ///
    /// This method returns the receiver end of the audio level channel,
    /// which provides real-time RMS audio level measurements in decibels.
    ///
    /// # Returns
    ///
    /// `Some(Arc<Receiver<f32>>)` if audio level monitoring is enabled,
    /// `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use recorder::AudioRecorder;
    ///
    /// let recorder = AudioRecorder::new(Some(1024)).unwrap();
    ///
    /// if let Some(receiver) = recorder.get_audio_level_receiver() {
    ///     // Monitor audio levels in another thread
    ///     // while let Ok(level) = receiver.recv() {
    ///     //     println!("Audio level: {:.1} dB", level);
    ///     // }
    /// }
    /// ```
    pub fn get_audio_level_receiver(&self) -> Option<Arc<Receiver<f32>>> {
        self.audio_level_receiver.clone()
    }

    /// Get all available audio devices
    pub fn get_available_devices(&self) -> Result<Vec<AudioDeviceInfo>, AudioError> {
        self.get_input_devices()
    }

    /// Get all available input devices
    pub fn get_input_devices(&self) -> Result<Vec<AudioDeviceInfo>, AudioError> {
        let devices = self
            .host
            .input_devices()
            .map_err(|e| AudioError::HostError(e.to_string()))?;

        let mut input_devices = Vec::new();
        for device in devices {
            if let Ok(info) = self.get_device_info(&device) {
                input_devices.push(info);
            }
        }

        Ok(input_devices)
    }

    /// Get the default input device
    pub fn get_default_input_device(&self) -> Result<Option<AudioDeviceInfo>, AudioError> {
        if let Some(device) = self.host.default_input_device() {
            self.get_device_info(&device)
                .map(Some)
                .map_err(|e| AudioError::DeviceError(e.to_string()))
        } else {
            Ok(None)
        }
    }

    fn get_device_info(&self, device: &Device) -> Result<AudioDeviceInfo, AudioError> {
        let name = device
            .name()
            .map_err(|e| AudioError::DeviceError(e.to_string()))?;

        let default_config = device
            .default_input_config()
            .map(|config| config.config())
            .ok();

        let supported_formats = device
            .supported_input_configs()
            .map(|configs| configs.map(|c| c.sample_format()).collect())
            .unwrap_or_else(|_| vec![]);

        Ok(AudioDeviceInfo {
            name,
            default_config,
            supported_formats,
        })
    }

    /// Find an audio device by name
    pub fn find_device_by_name(&self, name: &str) -> Result<Option<AudioDeviceInfo>, AudioError> {
        let devices = self.get_available_devices()?;
        Ok(devices.into_iter().find(|device| device.name == name))
    }

    /// Get the current audio configuration
    pub fn get_config(&self, name: &str) -> Result<(StreamConfig, Vec<SampleFormat>), AudioError> {
        let device_infos = self.find_device_by_name(name)?;
        let Some(device_info) = device_infos else {
            return Err(AudioError::DeviceError(format!("no found device `{name}`")));
        };

        let Some(default_config) = device_info.default_config else {
            return Err(AudioError::DeviceError(format!(
                "no found default_config for device `{name}`"
            )));
        };

        Ok((default_config, device_info.supported_formats))
    }

    /// Start input recording with the specified device
    pub fn start_input_recording(
        &self,
        device_name: &str,
        callback: impl FnMut(&[f32], &InputCallbackInfo) + Send + 'static,
    ) -> Result<Arc<Mutex<Option<Stream>>>, AudioError> {
        let (stream_config, _) = self.get_config(device_name)?;

        let physical_device = self
            .host
            .input_devices()
            .map_err(|e| AudioError::HostError(e.to_string()))?
            .find(|d| d.name().map(|name| name == device_name).unwrap_or(false))
            .ok_or_else(|| {
                AudioError::DeviceError(format!("Device '{}' not found", device_name))
            })?;

        let stream = physical_device
            .build_input_stream(
                &stream_config,
                callback,
                |err| eprintln!("Audio stream error: {}", err),
                None,
            )
            .map_err(|e| AudioError::StreamError(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;

        Ok(Arc::new(Mutex::new(Some(stream))))
    }
}

/// WAV file writer for audio samples
pub struct AudioFileWriter {
    writer: Option<WavWriter<BufWriter<File>>>,
}

impl AudioFileWriter {
    /// Create a new WAV file writer
    pub fn new<P: AsRef<Path>>(
        file_path: P,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self, AudioError> {
        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let writer =
            WavWriter::create(file_path, spec).map_err(|e| AudioError::FileError(e.to_string()))?;

        Ok(Self {
            writer: Some(writer),
        })
    }

    /// Write audio samples to the WAV file
    pub fn write_samples(&mut self, samples: &[f32]) -> Result<(), AudioError> {
        if let Some(writer) = &mut self.writer {
            for &sample in samples {
                writer
                    .write_sample(sample)
                    .map_err(|e| AudioError::EncodingError(e.to_string()))?;
            }
            Ok(())
        } else {
            Err(AudioError::FileError("Writer is not available".to_string()))
        }
    }

    /// Finalize and close the WAV file
    pub fn finalize(&mut self) -> Result<(), AudioError> {
        if let Some(writer) = self.writer.take() {
            writer
                .finalize()
                .map_err(|e| AudioError::FileError(e.to_string()))?;
        }
        Ok(())
    }
}

impl Drop for AudioFileWriter {
    fn drop(&mut self) {
        let _ = self.finalize();
    }
}

/// Streaming audio recorder that writes to file in real-time
pub struct StreamingAudioRecorder {
    file_writer: Arc<Mutex<Option<AudioFileWriter>>>,
    recording_session: Arc<Mutex<Option<Stream>>>,
    audio_level_receiver: Option<Arc<Receiver<f32>>>,
}

impl StreamingAudioRecorder {
    pub fn start<P: AsRef<Path>>(
        recorder: AudioRecorder,
        device_name: &str,
        file_path: P,
        disable_save_file: bool,
    ) -> Result<Self, AudioError> {
        let (stream_config, _) = recorder.get_config(device_name)?;

        let file_writer = if disable_save_file {
            Arc::new(Mutex::new(None))
        } else {
            Arc::new(Mutex::new(Some(AudioFileWriter::new(
                file_path,
                stream_config.sample_rate.0,
                stream_config.channels,
            )?)))
        };

        let file_writer_clone = file_writer.clone();
        let audio_level_sender = recorder.audio_level_sender.clone();
        let amplification = recorder.amplification.clone();
        let recording_session = recorder.start_input_recording(
            device_name,
            move |f32_samples: &[f32], _info: &_| {
                let mut f32_sample_amplification = Vec::with_capacity(f32_samples.len());
                let data = if let Some(ref amplification) = amplification {
                    f32_sample_amplification.extend_from_slice(f32_samples);

                    apply_gain(
                        &mut f32_sample_amplification,
                        amplification.load(Ordering::Relaxed) as f32,
                    );

                    &f32_sample_amplification[..]
                } else {
                    f32_samples
                };

                if let Some(ref tx) = audio_level_sender
                    && let Some(db) = calc_rms_level(data)
                    && let Err(e) = tx.try_send(db)
                {
                    log::warn!("try send input audio db level data failed: {e}");
                }

                if let Ok(mut writer) = file_writer_clone.lock()
                    && let Some(ref mut writer) = *writer
                {
                    _ = writer.write_samples(data);
                }
            },
        )?;

        Ok(Self {
            file_writer: file_writer,
            recording_session,
            audio_level_receiver: recorder.get_audio_level_receiver(),
        })
    }

    /// Stop the streaming audio recording
    pub fn stop(self) -> Result<(), AudioError> {
        {
            if let Some(stream) = self.recording_session.lock().unwrap().take() {
                drop(stream);
            }
        }

        if let Some(mut writer) = self.file_writer.lock().unwrap().take() {
            writer.finalize()?;
        }

        Ok(())
    }

    pub fn get_audio_level_receiver(&self) -> Option<Arc<Receiver<f32>>> {
        self.audio_level_receiver.clone()
    }
}
