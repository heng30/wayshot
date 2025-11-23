use crate::{
    audio_level::{apply_gain, calc_rms_level},
    speaker_recorder::{SpeakerRecorder, SpeakerRecorderConfig, SpeakerRecorderError},
};
use crossbeam::channel::Sender;
use hound::WavSpec;
use std::{
    ptr,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};
use windows::{
    Win32::{
        Media::{Audio::*, KernelStreaming::*},
        System::{Com::*, Ole::*},
    },
    core::*,
};

pub struct SpeakerRecorderWindows {
    config: SpeakerRecorderConfig,
    device_info: Option<(u32, String)>,
    device_id: Option<String>,
}

impl SpeakerRecorderWindows {
    pub fn new(config: SpeakerRecorderConfig) -> Result<Self, SpeakerRecorderError> {
        unsafe {
            CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED).map_err(|e| {
                SpeakerRecorderError::WasapiError(format!("COM initialization failed: {e}"))
            })?;
        }

        let mut recorder = Self {
            config,
            device_info: None,
            device_id: None,
        };

        let output_device = recorder.find_default_output()?;
        recorder.device_info = output_device.clone();
        Ok(recorder)
    }

    fn create_audio_client(
        &self,
    ) -> Result<(IAudioClient, IAudioCaptureClient), SpeakerRecorderError> {
        let device_enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };

        let device = if let Some(ref device_id) = self.device_id {
            unsafe {
                device_enumerator.GetDevice(device_id).map_err(|e| {
                    SpeakerRecorderError::DeviceError(format!("Failed to get device: {e}"))
                })?
            }
        } else {
            unsafe {
                device_enumerator
                    .GetDefaultAudioEndpoint(eRender, eConsole)
                    .map_err(|e| {
                        SpeakerRecorderError::DeviceError(format!(
                            "Failed to get default audio endpoint: {e}"
                        ))
                    })?
            }
        };

        let audio_client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None)? };

        // Configure audio format - match Linux implementation (48kHz, float32, stereo)
        let wave_format = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_EXTENSIBLE,
            nChannels: 2,
            nSamplesPerSec: 48000,
            nAvgBytesPerSec: 48000 * 2 * 4, // sample_rate * channels * bytes_per_sample
            nBlockAlign: 2 * 4,             // channels * bytes_per_sample
            wBitsPerSample: 32,
            cbSize: 22, // size of WAVEFORMATEXTENSIBLE
        };

        let mut wave_format_extensible: WAVEFORMATEXTENSIBLE = unsafe { std::mem::zeroed() };
        wave_format_extensible.Format = wave_format;
        wave_format_extensible.SubFormat = KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
        wave_format_extensible.Samples.wValidBitsPerSample = 32;
        wave_format_extensible.dwChannelMask = KSAUDIO_SPEAKER_STEREO;

        let p_format = &mut wave_format_extensible.Format as *mut _ as *mut WAVEFORMATEX;

        // Initialize audio client for loopback recording
        unsafe {
            audio_client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    0,
                    0,
                    p_format,
                    ptr::null(),
                )
                .map_err(|e| {
                    SpeakerRecorderError::WasapiError(format!(
                        "Audio client initialization failed: {e}"
                    ))
                })?;
        }

        let capture_client: IAudioCaptureClient = unsafe {
            audio_client.GetService().map_err(|e| {
                SpeakerRecorderError::WasapiError(format!("Failed to get capture client: {e}"))
            })?
        };

        Ok((audio_client, capture_client))
    }

    fn process_audio_buffer(
        buffer: &[u8],
        frame_sender: Option<&Sender<Vec<f32>>>,
        level_sender: Option<&Sender<f32>>,
        gain: Option<&Arc<AtomicI32>>,
    ) -> Result<(), SpeakerRecorderError> {
        let f32_samples: &[f32] = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr() as *const f32,
                buffer.len() / std::mem::size_of::<f32>(),
            )
        };

        let mut f32_samples_gained = Vec::with_capacity(f32_samples.len());
        let processed_samples = if let Some(ref gain) = gain {
            f32_samples_gained.extend_from_slice(f32_samples);
            apply_gain(&mut f32_samples_gained, gain.load(Ordering::Relaxed) as f32);
            &f32_samples_gained[..]
        } else {
            f32_samples
        };

        if let Some(ref tx) = frame_sender {
            if let Err(e) = tx.try_send(processed_samples.to_vec()) {
                log::warn!("try send speaker audio frame failed: {e}");
            }
        }

        if let Some(ref tx) = level_sender {
            if let Some(db) = calc_rms_level(processed_samples) {
                if let Err(e) = tx.try_send(db) {
                    log::warn!("try send speaker audio db level data failed: {e}");
                }
            }
        }

        Ok(())
    }
}

impl SpeakerRecorder for SpeakerRecorderWindows {
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
        let Some((_, ref node_name)) = self.device_info else {
            return Err(SpeakerRecorderError::DeviceError(
                "No output speaker device found".to_string(),
            ));
        };

        log::info!("Start recording speaker. device: {}", node_name);

        let (audio_client, capture_client) = self.create_audio_client()?;

        // Start recording
        unsafe {
            audio_client.Start().map_err(|e| {
                SpeakerRecorderError::WasapiError(format!("Failed to start audio client: {e}"))
            })?;
        }

        log::info!("Successfully connected to audio device");

        let buffer_duration = Duration::from_millis(100); // 100ms buffer
        let mut buffer_frame_count = 0u32;

        unsafe {
            audio_client
                .GetBufferSize(&mut buffer_frame_count)
                .map_err(|e| {
                    SpeakerRecorderError::WasapiError(format!("Failed to get buffer size: {e}"))
                })?;
        }

        log::info!("Audio buffer size: {} frames", buffer_frame_count);

        while !self.config.stop_sig.load(Ordering::Relaxed) {
            std::thread::sleep(buffer_duration);

            let mut data_ptr: *mut u8 = ptr::null_mut();
            let mut num_frames_available = 0u32;
            let mut flags = AUDCLNT_BUFFERFLAGS_SILENT;

            unsafe {
                let result = capture_client.GetBuffer(
                    &mut data_ptr,
                    &mut num_frames_available,
                    &mut flags,
                    ptr::null_mut(),
                    ptr::null_mut(),
                );

                match result {
                    Ok(_) => {
                        if flags != AUDCLNT_BUFFERFLAGS_SILENT && num_frames_available > 0 {
                            let buffer_size = (num_frames_available * 2 * 4) as usize; // frames * channels * bytes_per_sample
                            let buffer = std::slice::from_raw_parts(data_ptr, buffer_size);

                            Self::process_audio_buffer(
                                buffer,
                                self.config.frame_sender.as_ref(),
                                self.config.level_sender.as_ref(),
                                self.config.gain.as_ref(),
                            )?;
                        }

                        capture_client
                            .ReleaseBuffer(num_frames_available)
                            .map_err(|e| {
                                SpeakerRecorderError::WasapiError(format!(
                                    "Failed to release buffer: {e}"
                                ))
                            })?;
                    }
                    Err(e) => {
                        log::warn!("Failed to get audio buffer: {}", e);
                    }
                }
            }
        }

        unsafe {
            audio_client.Stop();
            CoUninitialize();
        }

        Ok(())
    }

    fn find_default_output(&self) -> Result<Option<(u32, String)>, SpeakerRecorderError> {
        log::info!("Searching for output audio devices...");

        let device_enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };

        // Get default output device
        let device = unsafe {
            device_enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| {
                    SpeakerRecorderError::DeviceError(format!(
                        "Failed to get default audio endpoint: {e}"
                    ))
                })?
        };

        let device_id = unsafe {
            device.GetId().map_err(|e| {
                SpeakerRecorderError::DeviceError(format!("Failed to get device ID: {e}"))
            })?
        };

        let device_property_store: IPropertyStore = unsafe {
            device.OpenPropertyStore(STGM_READ).map_err(|e| {
                SpeakerRecorderError::DeviceError(format!("Failed to open property store: {e}"))
            })?
        };

        // Get device friendly name
        let friendly_name_prop = unsafe {
            device_property_store
                .GetValue(&PKEY_Device_FriendlyName)
                .map_err(|e| {
                    SpeakerRecorderError::DeviceError(format!("Failed to get friendly name: {e}"))
                })?
        };

        let friendly_name = unsafe {
            friendly_name_prop.to_string().map_err(|e| {
                SpeakerRecorderError::DeviceError(format!(
                    "Failed to convert friendly name to string: {e}"
                ))
            })?
        };

        log::info!(
            "Found default device: {} (ID: {})",
            friendly_name,
            device_id
        );

        Ok(Some((1, friendly_name))) // Use ID 1 as placeholder since Windows uses string IDs
    }
}
