use audio_utils::audio_level::calc_rms_level;
use cpal::{
    Device, Host, SampleFormat, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam::channel::Sender;
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AudioRecorderError>;

#[derive(Error, Debug)]
pub enum AudioRecorderError {
    #[error("No input devices available")]
    NoInputDevices,

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Failed to create audio stream: {0}")]
    StreamCreation(String),

    #[error("Recording not in progress")]
    NotRecording,

    #[error("Already recording")]
    AlreadyRecording,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("WAV encoding error: {0}")]
    WavEncoding(#[from] hound::Error),
}

#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub channels: u16,
    pub sample_rate: u32,
}

#[derive(Debug, Clone)]
pub struct RecordedAudio {
    pub samples: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
}

impl RecordedAudio {
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let spec = WavSpec {
            channels: self.channels,
            sample_rate: self.sample_rate,
            bits_per_sample: 32,
            sample_format: WavSampleFormat::Float,
        };
        let mut writer = WavWriter::create(path, spec)?;
        for sample in &self.samples {
            writer.write_sample(*sample)?;
        }
        writer.finalize()?;
        Ok(())
    }

    pub fn duration_secs(&self) -> f64 {
        if self.channels == 0 || self.sample_rate == 0 {
            return 0.0;
        }
        self.samples.len() as f64 / (self.channels as f64 * self.sample_rate as f64)
    }

    pub fn rms_level_db(&self) -> Option<f32> {
        calc_rms_level(&self.samples)
    }

    pub fn apply_gain(&mut self, gain_db: f32) {
        if gain_db == 0.0 {
            return;
        }

        // Convert dB to linear factor: linear = 10^(dB/20)
        let linear_gain = 10.0_f32.powf(gain_db / 20.0);
        for sample in &mut self.samples {
            *sample *= linear_gain;
            *sample = sample.clamp(-1.0, 1.0);
        }
    }

    pub fn convert_to_mono(&mut self) {
        if self.channels <= 1 {
            return;
        }

        let num_channels = self.channels as usize;
        let num_frames = self.samples.len() / num_channels;
        let mut mono_samples = Vec::with_capacity(num_frames);

        for frame_idx in 0..num_frames {
            let start = frame_idx * num_channels;
            let end = start + num_channels;
            let frame_samples = &self.samples[start..end.min(self.samples.len())];
            let avg = frame_samples.iter().sum::<f32>() / num_channels as f32;
            mono_samples.push(avg);
        }

        self.samples = mono_samples;
        self.channels = 1;
    }
}

pub struct AudioRecorder {
    host: Host,
    stream: Option<Stream>,
    level_sender: Option<Sender<(f32, f32)>>,
    recorded_samples: Arc<Mutex<Vec<f32>>>,
    channels: u16,
    sample_rate: u32,
    gain_db: f32,
    mono: bool,
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioRecorder {
    pub fn new() -> Self {
        let host = cpal::default_host();
        Self {
            host,
            stream: None,
            level_sender: None,
            recorded_samples: Arc::new(Mutex::new(Vec::new())),
            channels: 2,
            sample_rate: 48000,
            gain_db: 0.0,
            mono: false,
        }
    }

    pub fn with_level_sender(mut self, sender: Sender<(f32, f32)>) -> Self {
        self.level_sender = Some(sender);
        self
    }

    pub fn with_gain(mut self, gain_db: f32) -> Self {
        self.gain_db = gain_db;
        self
    }

    pub fn with_mono(mut self, mono: bool) -> Self {
        self.mono = mono;
        self
    }

    pub fn get_input_devices(&self) -> Result<Vec<AudioDeviceInfo>> {
        let devices = self
            .host
            .input_devices()
            .map_err(|e| AudioRecorderError::StreamCreation(e.to_string()))?;

        let mut result = Vec::new();
        for device in devices {
            if let Ok(id) = device.id()
                && let Ok(config) = device.default_input_config()
            {
                result.push(AudioDeviceInfo {
                    name: id.to_string(),
                    channels: config.channels(),
                    sample_rate: config.sample_rate(),
                });
            }
        }

        if result.is_empty() {
            return Err(AudioRecorderError::NoInputDevices);
        }

        Ok(result)
    }

    pub fn get_default_input_device(&self) -> Result<(Device, StreamConfig)> {
        let device = self
            .host
            .default_input_device()
            .ok_or(AudioRecorderError::NoInputDevices)?;

        let config = device
            .default_input_config()
            .map_err(|e| AudioRecorderError::StreamCreation(e.to_string()))?;

        let stream_config: StreamConfig = config.into();
        Ok((device, stream_config))
    }

    pub fn start_recording(&mut self, device_name: Option<&str>) -> Result<()> {
        if self.stream.is_some() {
            return Err(AudioRecorderError::AlreadyRecording);
        }

        let (device, config) = if let Some(name) = device_name {
            self.find_device_by_name(name)?
        } else {
            self.get_default_input_device()?
        };

        self.channels = config.channels;
        self.sample_rate = config.sample_rate;

        if let Ok(mut samples) = self.recorded_samples.lock() {
            samples.clear();
        }

        let samples = self.recorded_samples.clone();
        let level_sender = self.level_sender.clone();
        let channels = self.channels;
        let mono = self.mono;

        let linear_gain = if self.gain_db == 0.0 {
            1.0
        } else {
            10.0_f32.powf(self.gain_db / 20.0)
        };

        // Buffer for level calculation
        let level_buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let level_buffer_clone = level_buffer.clone();

        let stream = device
            .build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let processed_samples: Vec<f32> = if mono && channels > 1 {
                        let num_channels = channels as usize;
                        let num_frames = data.len() / num_channels;
                        let mut mono_samples = Vec::with_capacity(num_frames);

                        for frame_idx in 0..num_frames {
                            let start = frame_idx * num_channels;
                            let end = (start + num_channels).min(data.len());
                            let frame_samples = &data[start..end];
                            let avg = frame_samples.iter().sum::<f32>() / num_channels as f32;
                            mono_samples.push((avg * linear_gain).clamp(-1.0, 1.0));
                        }
                        mono_samples
                    } else {
                        data.iter()
                            .map(|s| (s * linear_gain).clamp(-1.0, 1.0))
                            .collect()
                    };

                    if let Ok(mut s) = samples.lock() {
                        s.extend_from_slice(&processed_samples);
                    }

                    if let Some(sender) = &level_sender
                        && let Ok(mut buf) = level_buffer_clone.lock()
                    {
                        buf.extend_from_slice(&processed_samples);

                        // Calculate and send level every ~100ms worth of samples
                        let threshold = if mono && channels > 1 {
                            config.sample_rate as usize / 10
                        } else {
                            (config.sample_rate as usize / 10) * channels as usize
                        };
                        if buf.len() < threshold {
                            return;
                        }

                        // For level display, always show left/right even if recording mono
                        let levels: (f32, f32) = if channels > 1 {
                            let parts = split_audio_channels(buf.clone(), channels as usize);

                            let left_db = match parts.first().and_then(|p| calc_rms_level(p)) {
                                Some(db) => db,
                                _ => -200.0,
                            };

                            let right_db = match parts.get(1).and_then(|p| calc_rms_level(p)) {
                                Some(db) => db,
                                _ => -200.0,
                            };
                            (left_db, right_db)
                        } else {
                            let level = match calc_rms_level(&buf) {
                                Some(db) => db,
                                _ => -200.0,
                            };
                            (level, level)
                        };

                        _ = sender.send(levels);

                        buf.clear();
                    }
                },
                |err| {
                    log::error!("Audio input stream error: {}", err);
                },
                None,
            )
            .map_err(|e| AudioRecorderError::StreamCreation(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioRecorderError::StreamCreation(e.to_string()))?;

        self.stream = Some(stream);
        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<RecordedAudio> {
        let stream = self.stream.take().ok_or(AudioRecorderError::NotRecording)?;

        drop(stream);

        let samples = if let Ok(mut s) = self.recorded_samples.lock() {
            std::mem::take(&mut *s)
        } else {
            Vec::new()
        };

        let output_channels = if self.mono && self.channels > 1 {
            1
        } else {
            self.channels
        };

        Ok(RecordedAudio {
            samples,
            channels: output_channels,
            sample_rate: self.sample_rate,
        })
    }

    pub fn is_recording(&self) -> bool {
        self.stream.is_some()
    }

    pub fn recorded_sample_count(&self) -> usize {
        if let Ok(samples) = self.recorded_samples.lock() {
            samples.len()
        } else {
            0
        }
    }

    pub fn recorded_duration_secs(&self) -> f64 {
        let count = self.recorded_sample_count();
        let channels = if self.mono && self.channels > 1 {
            1
        } else {
            self.channels
        };
        if channels == 0 || self.sample_rate == 0 {
            return 0.0;
        }
        count as f64 / (channels as f64 * self.sample_rate as f64)
    }

    fn find_device_by_name(&self, name: &str) -> Result<(Device, StreamConfig)> {
        let devices = self
            .host
            .input_devices()
            .map_err(|e| AudioRecorderError::StreamCreation(e.to_string()))?;

        for device in devices {
            if let Ok(id) = device.id()
                && id.to_string() == name
            {
                let supported_config = device
                    .default_input_config()
                    .map_err(|e| AudioRecorderError::StreamCreation(e.to_string()))?;

                let sample_format = supported_config.sample_format();
                let config: StreamConfig = supported_config.into();

                return match sample_format {
                    SampleFormat::F32 => Ok((device, config)),
                    _ => Err(AudioRecorderError::StreamCreation(
                        "Only f32 sample format is supported".to_string(),
                    )),
                };
            }
        }

        Err(AudioRecorderError::DeviceNotFound(name.to_string()))
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        self.stream = None;
    }
}

pub fn split_audio_channels(buf: Vec<f32>, channels: usize) -> Vec<Vec<f32>> {
    if channels == 0 {
        return vec![];
    }

    let mut parts = vec![vec![]; channels];

    for chunk in buf.chunks(channels) {
        for channel in 0..channels {
            if let Some(v) = chunk.get(channel) {
                parts[channel].push(*v);
            }
        }
    }

    parts
}
