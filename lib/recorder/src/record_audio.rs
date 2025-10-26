use crate::{RealTimeDenoise, apply_gain, calc_rms_level, denoise_model};
use cpal::{
    Device, Host, InputCallbackInfo, SampleFormat, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam::channel::Sender;
use derive_setters::Setters;
use hound::WavSpec;
use nnnoiseless::RnnModel;
use once_cell::sync::Lazy;
use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};
use thiserror::Error;

static DENOISE_MODEL: Lazy<RnnModel> = Lazy::new(|| denoise_model());

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Audio host error: {0}")]
    HostError(String),

    #[error("Audio device error: {0}")]
    DeviceError(String),

    #[error("Audio stream error: {0}")]
    StreamError(String),

    #[error("Audio denoise error: {0}")]
    DenoiseError(String),
}

#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub default_config: Option<StreamConfig>,
    pub supported_formats: Vec<SampleFormat>,
}

#[derive(Setters)]
#[setters(prefix = "with_")]
pub struct AudioRecorder {
    #[setters(skip)]
    host: Host,

    #[setters(skip)]
    stream: Option<Stream>,

    level_sender: Option<Sender<f32>>,
    frame_sender: Option<Sender<Vec<f32>>>,

    enable_denoise: bool,
    gain: Option<Arc<AtomicI32>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            host: cpal::default_host(),
            stream: None,
            level_sender: None,
            frame_sender: None,
            enable_denoise: false,
            gain: None,
        }
    }

    pub fn get_available_devices(&self) -> Result<Vec<AudioDeviceInfo>, AudioError> {
        self.get_input_devices()
    }

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

    pub fn find_device_by_name(&self, name: &str) -> Result<Option<AudioDeviceInfo>, AudioError> {
        let devices = self.get_available_devices()?;
        Ok(devices.into_iter().find(|device| device.name == name))
    }

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

    fn stream_play(
        &self,
        device_name: &str,
        callback: impl FnMut(&[f32], &InputCallbackInfo) + Send + 'static,
    ) -> Result<Stream, AudioError> {
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

        Ok(stream)
    }

    pub fn spec(&self, device_name: &str) -> Result<WavSpec, AudioError> {
        let (stream_config, _) = self.get_config(device_name)?;

        Ok(WavSpec {
            channels: stream_config.channels,
            sample_rate: stream_config.sample_rate.0,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        })
    }

    pub fn start_recording(&mut self, device_name: &str) -> Result<(), AudioError> {
        // Note:
        //  Without calling `denoise.flush` is not a problem.
        //  Just losing the last frame of real-time samples.
        let mut denoiser = if self.enable_denoise {
            let spec = self.spec(device_name)?;
            let denoiser = RealTimeDenoise::new(&DENOISE_MODEL, spec)
                .map_err(|e| AudioError::DenoiseError(e.to_string()))?;
            Some(denoiser)
        } else {
            None
        };

        let gain = self.gain.clone();
        let level_sender = self.level_sender.clone();
        let frame_sender = self.frame_sender.clone();

        let stream = self.stream_play(device_name, move |f32_samples: &[f32], _info: &_| {
            let mut denoise_samples = None;
            let f32_samples = if let Some(ref mut denoiser) = denoiser {
                match denoiser.process(f32_samples) {
                    Ok(v) => denoise_samples = v,
                    Err(e) => log::warn!("denoise audio samples failed: {e}"),
                };

                if denoise_samples.is_some() {
                    &denoise_samples.unwrap()
                } else {
                    f32_samples
                }
            } else {
                f32_samples
            };

            let mut f32_sample_amplification = Vec::with_capacity(f32_samples.len());
            let data = if let Some(ref gain) = gain {
                f32_sample_amplification.extend_from_slice(f32_samples);

                apply_gain(
                    &mut f32_sample_amplification,
                    gain.load(Ordering::Relaxed) as f32,
                );

                &f32_sample_amplification[..]
            } else {
                f32_samples
            };

            if let Some(ref tx) = frame_sender
                && let Err(e) = tx.try_send(f32_samples.to_vec())
            {
                log::warn!("try send speaker audio frame failed: {e}");
            }

            if let Some(ref tx) = level_sender
                && let Some(db) = calc_rms_level(data)
                && let Err(e) = tx.try_send(db)
            {
                log::warn!("try send input audio db level data failed: {e}");
            }
        })?;

        self.stream = Some(stream);

        Ok(())
    }

    pub fn stop(self) {
        drop(self);
        log::debug!("Stop recording audio...");
    }
}
