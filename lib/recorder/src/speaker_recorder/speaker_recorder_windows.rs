use crate::{
    audio_level::{apply_gain, calc_rms_level},
    speaker_recorder::{SpeakerRecorder, SpeakerRecorderConfig, SpeakerRecorderError},
};
use crossbeam::channel::Sender;
use hound::WavSpec;
use spin_sleep::SpinSleeper;
use std::{
    ptr,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};
use winapi::Interface as WinApiInterface;
use winapi::shared::mmreg::WAVE_FORMAT_IEEE_FLOAT;
use winapi::shared::winerror::FAILED;
use winapi::um::audioclient::IAudioClient as IAudioClientWinApi;
use winapi::um::mmdeviceapi::IMMDevice as IMMDeviceWinApi;
use windows::Win32::{Media::Audio::*, System::Com::*};

pub struct SpeakerRecorderWindows {
    config: SpeakerRecorderConfig,
    device_info: Option<(u32, String)>,
    com_initialized: bool, // Track COM initialization state
}

impl SpeakerRecorderWindows {
    pub fn new(config: SpeakerRecorderConfig) -> std::result::Result<Self, SpeakerRecorderError> {
        // Initialize COM for WinAPI WASAPI
        unsafe {
            let hr = CoInitializeEx(Some(ptr::null()), COINIT_MULTITHREADED);
            if FAILED(hr.0) {
                return Err(SpeakerRecorderError::WasapiError(format!(
                    "Failed to initialize COM: HRESULT=0x{:08X}",
                    hr.0
                )));
            }
        }

        let mut recorder = Self {
            config,
            device_info: None,
            com_initialized: false, // Will be set to true if COM initialization succeeds
        };

        let output_device = recorder.find_default_output()?;
        recorder.device_info = output_device.clone();
        recorder.com_initialized = true; // COM was initialized successfully
        Ok(recorder)
    }

    fn create_audio_client(
        &self,
    ) -> std::result::Result<(IAudioClient, IAudioCaptureClient), SpeakerRecorderError> {
        log::info!("Creating WASAPI audio client with Windows crate...");

        let device_enumerator: IMMDeviceEnumerator = unsafe {
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER).map_err(|e| {
                SpeakerRecorderError::WasapiError(format!(
                    "Failed to create device enumerator: {e}"
                ))
            })?
        };

        // Get default audio endpoint
        let device = unsafe {
            device_enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| {
                    SpeakerRecorderError::WasapiError(format!(
                        "Failed to get default audio endpoint: {e}"
                    ))
                })?
        };

        // Log device state
        let device_state = unsafe { device.GetState().unwrap_or(DEVICE_STATE_DISABLED) };
        log::debug!("Device state: {:?}", device_state);

        if device_state != DEVICE_STATE_ACTIVE {
            return Err(SpeakerRecorderError::DeviceError(
                "Audio device is not active".to_string(),
            ));
        }

        // Since windows crate 0.62 doesn't have Activate method, we need to use winapi for Activate
        log::debug!("Activating IAudioClient using winapi's Activate method...");

        // Convert windows crate IMMDevice to winapi IMMDevice
        // Both are COM interfaces, so we can cast them
        let device_winapi: *mut IMMDeviceWinApi = unsafe { std::mem::transmute_copy(&device) };

        let audio_client_winapi: *mut IAudioClientWinApi = unsafe {
            let mut audio_client_ptr: *mut IAudioClientWinApi = std::ptr::null_mut();
            let hr = (*device_winapi).Activate(
                &IAudioClientWinApi::uuidof(),
                winapi::um::combaseapi::CLSCTX_ALL,
                std::ptr::null_mut(),
                &mut audio_client_ptr as *mut *mut _ as *mut *mut std::ffi::c_void,
            );

            if FAILED(hr) {
                log::debug!("Console endpoint failed, trying multimedia...");
                let device_multimedia = device_enumerator
                    .GetDefaultAudioEndpoint(eRender, eMultimedia)
                    .map_err(|e| {
                        SpeakerRecorderError::WasapiError(format!(
                            "Failed to get multimedia endpoint: {e}"
                        ))
                    })?;

                let device_multimedia_winapi: *mut IMMDeviceWinApi =
                    std::mem::transmute_copy(&device_multimedia);
                let hr_multimedia = (*device_multimedia_winapi).Activate(
                    &IAudioClientWinApi::uuidof(),
                    winapi::um::combaseapi::CLSCTX_ALL,
                    std::ptr::null_mut(),
                    &mut audio_client_ptr as *mut *mut _ as *mut *mut std::ffi::c_void,
                );

                if FAILED(hr_multimedia) {
                    return Err(SpeakerRecorderError::WasapiError(format!(
                        "Both console and multimedia endpoints failed. Console: HRESULT=0x{:08X}, Multimedia: HRESULT=0x{:08X}",
                        hr, hr_multimedia
                    )));
                }
            }

            audio_client_ptr
        };

        // Convert back to windows crate IAudioClient
        let audio_client: IAudioClient = unsafe { std::mem::transmute_copy(&audio_client_winapi) };
        log::info!("✅ Successfully activated IAudioClient interface!");

        // Configure audio format - use 32-bit float to match our WAV writer spec
        let wave_format = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_IEEE_FLOAT as u16,
            nChannels: 2,
            nSamplesPerSec: 44100,
            nAvgBytesPerSec: 44100 * 2 * 4, // sample_rate * channels * bytes_per_sample (32-bit float)
            nBlockAlign: 2 * 4,             // channels * bytes_per_sample
            wBitsPerSample: 32,             // 32-bit float
            cbSize: 0,                      // No extra data for IEEE float
        };

        let p_format = &wave_format as *const _ as *mut WAVEFORMATEX;

        // Initialize audio client for loopback recording
        // 1秒 = 10,000,000 REFTIMES (100ns units)
        // 请求 100ms 的缓冲区，给 Loopback 足够的缓冲空间
        let hns_requested_duration = 1_000_000; // 100ms buffer

        unsafe {
            audio_client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    hns_requested_duration, // 请求 100ms 缓冲
                    0,
                    p_format,
                    None,
                )
                .map_err(|e| {
                    SpeakerRecorderError::WasapiError(format!(
                        "Audio client initialization failed: {e}"
                    ))
                })?;
        }

        log::info!("✅ Audio client initialized successfully!");

        // Get capture client
        let capture_client: IAudioCaptureClient = unsafe {
            audio_client.GetService().map_err(|e| {
                SpeakerRecorderError::WasapiError(format!("Failed to get capture client: {e}"))
            })?
        };

        log::info!("✅ Capture client obtained successfully!");

        // Get the actual device format to ensure compatibility
        let device_format_ptr = unsafe {
            audio_client.GetMixFormat().map_err(|e| {
                SpeakerRecorderError::WasapiError(format!("Failed to get device mix format: {e}"))
            })?
        };

        let device_format = unsafe { *device_format_ptr };
        let format_tag = device_format.wFormatTag;
        let channels = device_format.nChannels;
        let sample_rate = device_format.nSamplesPerSec;
        let bits_per_sample = device_format.wBitsPerSample;
        log::info!(
            "Device format: wFormatTag={}, nChannels={}, nSamplesPerSec={}, wBitsPerSample={}",
            format_tag,
            channels,
            sample_rate,
            bits_per_sample
        );

        // Note: We're using our configured format (32-bit float) for better compatibility
        // The device shows WAVE_FORMAT_EXTENSIBLE (65534) which should be compatible
        log::info!("🎉 WASAPI setup completed successfully!");

        Ok((audio_client, capture_client))
    }

    fn process_audio_buffer(
        buffer: &[u8],
        frame_sender: Option<&Sender<Vec<f32>>>,
        level_sender: Option<&Sender<f32>>,
        gain: Option<&Arc<AtomicI32>>,
    ) -> std::result::Result<(), SpeakerRecorderError> {
        // Directly interpret buffer as 32-bit float samples
        let f32_samples: &[f32] = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr() as *const f32,
                buffer.len() / std::mem::size_of::<f32>(),
            )
        };

        let mut samples = f32_samples.to_vec();

        let processed_samples = if let Some(ref gain) = gain {
            apply_gain(&mut samples, gain.load(Ordering::Relaxed) as f32);
            &samples[..]
        } else {
            &samples
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
            sample_rate: 44100,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        }
    }

    fn get_device_info(&self) -> Option<(u32, String)> {
        self.device_info.clone()
    }

    fn find_default_output(
        &self,
    ) -> std::result::Result<Option<(u32, String)>, SpeakerRecorderError> {
        self.find_default_output()
    }

    fn start_recording(self) -> std::result::Result<(), SpeakerRecorderError> {
        let Some((_, ref node_name)) = self.device_info else {
            return Err(SpeakerRecorderError::DeviceError(
                "No output speaker device found".to_string(),
            ));
        };

        log::info!("Start recording speaker. device: {}", node_name);

        let (audio_client, capture_client) = self.create_audio_client()?;

        unsafe {
            audio_client.Start().map_err(|e| {
                SpeakerRecorderError::WasapiError(format!("Failed to start audio client: {e}"))
            })?;
        }

        log::info!("Successfully connected to audio device");

        // 建议：休眠时间设为 5ms (缓冲区 100ms，每次取一点，保持低延迟但高稳定性)
        let loop_delay = Duration::from_millis(5);
        let sleeper = SpinSleeper::new(1_000_000); // 1ms accuracy is enough

        while !self.config.stop_sig.load(Ordering::Relaxed) {
            // 1. 先睡一小会儿
            sleeper.sleep(loop_delay);

            // 2. 循环读取缓冲区直到读空
            // WASAPI Capture Client 可能积压了多个包，我们需要一次性读完
            loop {
                let mut data_ptr: *mut u8 = ptr::null_mut();
                let mut num_frames_available: u32 = 0;
                let mut flags: u32 = 0;

                let result = unsafe {
                    capture_client.GetBuffer(
                        &mut data_ptr,
                        &mut num_frames_available,
                        &mut flags,
                        None,
                        None,
                    )
                };

                match result {
                    Ok(_) => {
                        if num_frames_available == 0 {
                            // 缓冲区空了，释放并跳出内部循环，继续 sleep
                            unsafe {
                                capture_client.ReleaseBuffer(0).ok();
                            }
                            break;
                        }

                        // 检查静音标志 (当系统没有声音播放时，WASAPI 会返回静音包)
                        let is_silent = (flags & 1) != 0; // AUDCLNT_BUFFERFLAGS_SILENT

                        if is_silent {
                            // 系统静音：发送全 0 数据
                            let silent_len = (num_frames_available * 2) as usize; // Stereo
                            let silent_buffer = vec![0.0f32; silent_len];

                            if let Some(ref tx) = self.config.frame_sender {
                                let _ = tx.try_send(silent_buffer);
                            }
                            // Level 发送极小值
                            if let Some(ref tx) = self.config.level_sender {
                                let _ = tx.try_send(-96.0);
                            }
                        } else {
                            // 有真实音频数据
                            let buffer_len = (num_frames_available * 2 * 4) as usize; // frames * channels * bytes_per_sample
                            let buffer =
                                unsafe { std::slice::from_raw_parts(data_ptr, buffer_len) };

                            log::debug!("Processing {} bytes of audio data", buffer_len);
                            Self::process_audio_buffer(
                                buffer,
                                self.config.frame_sender.as_ref(),
                                self.config.level_sender.as_ref(),
                                self.config.gain.as_ref(),
                            )?;
                        }

                        // 释放缓冲区
                        unsafe {
                            if let Err(e) = capture_client.ReleaseBuffer(num_frames_available) {
                                log::error!("Failed to release buffer: {}", e);
                            }
                        }

                        // 注意：这里不要 sleep，继续 loop 检查是否还有积压的数据
                    }
                    Err(e) => {
                        // 如果发生错误（比如设备被拔出），通常应该退出或重试
                        // AUDCLNT_E_DEVICE_INVALIDATED
                        if e.code().0 == -2004287484 {
                            // 0x88890004
                            log::error!("Device invalidated!");
                            return Err(SpeakerRecorderError::DeviceError(
                                "Device disconnected".into(),
                            ));
                        }
                        // 其他临时错误，稍作休眠
                        break;
                    }
                }
            }
        }

        unsafe {
            audio_client.Stop().ok();
        }

        log::info!("Speaker recording stopped");
        Ok(())
    }
}

impl Drop for SpeakerRecorderWindows {
    fn drop(&mut self) {
        if self.com_initialized {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

impl SpeakerRecorderWindows {
    fn find_default_output(
        &self,
    ) -> std::result::Result<Option<(u32, String)>, SpeakerRecorderError> {
        log::info!("Searching for output audio devices...");

        let device_enumerator: IMMDeviceEnumerator = unsafe {
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
                SpeakerRecorderError::WasapiError(format!(
                    "Failed to create device enumerator: {e}"
                ))
            })?
        };

        // Get default output device
        let device = unsafe {
            device_enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| {
                    SpeakerRecorderError::WasapiError(format!(
                        "Failed to get default audio endpoint: {e}"
                    ))
                })?
        };

        let device_id_pwstr = unsafe {
            device.GetId().map_err(|e| {
                SpeakerRecorderError::WasapiError(format!("Failed to get device ID: {e}"))
            })?
        };

        // Convert PWSTR to String - use a simple approach
        let device_id = unsafe {
            let ptr = device_id_pwstr.0;
            if ptr.is_null() {
                String::new()
            } else {
                let mut len = 0;
                while *ptr.add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(ptr, len);
                String::from_utf16_lossy(slice)
            }
        };

        // For Windows, we'll use the device ID as the name since getting the friendly name
        // requires additional COM interfaces that might not be available
        let friendly_name = device_id.clone();

        log::info!(
            "Found default device: {} (ID: {})",
            friendly_name,
            friendly_name
        );

        Ok(Some((1, friendly_name))) // Use ID 1 as placeholder since Windows uses string IDs
    }
}
