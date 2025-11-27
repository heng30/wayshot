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
use winapi::{
    Interface as WinApiInterface,
    shared::{mmreg::WAVE_FORMAT_IEEE_FLOAT, winerror::FAILED},
    um::{
        audioclient::IAudioClient as IAudioClientWinApi, mmdeviceapi::IMMDevice as IMMDeviceWinApi,
    },
};
use windows::Win32::{Media::Audio::*, System::Com::*};

pub struct SpeakerRecorderWindows {
    config: SpeakerRecorderConfig,
    device_info: Option<(u32, String)>,
    com_initialized: bool,
    device_format: Option<WAVEFORMATEX>,
    cached_spec: Option<WavSpec>,
}

impl SpeakerRecorderWindows {
    pub fn new(config: SpeakerRecorderConfig) -> Result<Self, SpeakerRecorderError> {
        log::debug!(
            "Initializing SpeakerRecorderWindows in thread: {:?}",
            std::thread::current().id()
        );

        unsafe {
            let hr = CoInitializeEx(Some(ptr::null()), COINIT_MULTITHREADED);
            let com_initialized = match hr.0 {
                0 => {
                    // S_OK - COM initialized successfully
                    log::debug!("COM initialized successfully in this thread");
                    true
                }
                1 => {
                    // S_FALSE - COM was already initialized
                    log::debug!("COM was already initialized in this thread");
                    false
                }
                -2147417850i32 => {
                    // RPC_E_CHANGED_MODE = 0x80010106
                    log::debug!(
                        "COM was already initialized with different mode, trying to continue"
                    );
                    false
                }
                _ => {
                    return Err(SpeakerRecorderError::WasapiError(format!(
                        "Failed to initialize COM: HRESULT=0x{:08X}",
                        hr.0
                    )));
                }
            };

            log::debug!(
                "COM initialization result: HRESULT=0x{:08X}, com_initialized={}",
                hr.0,
                com_initialized
            );

            let mut recorder = Self {
                config,
                device_info: None,
                com_initialized,
                device_format: None,
                cached_spec: None,
            };

            // Directly call the inner method to find and store device info and format
            recorder.device_info = recorder.find_default_output_inner()?;
            Ok(recorder)
        }
    }

    fn create_audio_client(
        &self,
    ) -> std::result::Result<(IAudioClient, IAudioCaptureClient), SpeakerRecorderError> {
        log::info!("Creating WASAPI audio client...");

        let device_enumerator: IMMDeviceEnumerator = unsafe {
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER).map_err(|e| {
                SpeakerRecorderError::WasapiError(format!(
                    "Failed to create device enumerator: {e}"
                ))
            })?
        };

        let device = unsafe {
            device_enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| {
                    SpeakerRecorderError::WasapiError(format!(
                        "Failed to get default audio endpoint: {e}"
                    ))
                })?
        };

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

        let audio_client: IAudioClient = unsafe { std::mem::transmute_copy(&audio_client_winapi) };
        log::info!("✅ Successfully activated IAudioClient interface!");

        let wave_format = self.device_format.ok_or_else(|| {
            SpeakerRecorderError::DeviceError("Device format not available".to_string())
        })?;

        let samples_per_sec = wave_format.nSamplesPerSec;
        let channels = wave_format.nChannels;
        let bits_per_sample = wave_format.wBitsPerSample;
        let format_tag = wave_format.wFormatTag;
        log::info!(
            "Using device supported format: {}Hz, {} channels, {}-bit, tag={}",
            samples_per_sec,
            channels,
            bits_per_sample,
            format_tag
        );

        let p_format = &wave_format as *const _ as *mut WAVEFORMATEX;

        // Initialize audio client for loopback recording
        // 1 second = 10,000,000 REFTIMES (100ns units)
        let hns_requested_duration = 1_000_000; // 100ms buffer

        unsafe {
            audio_client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    hns_requested_duration,
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

        let capture_client: IAudioCaptureClient = unsafe {
            audio_client.GetService().map_err(|e| {
                SpeakerRecorderError::WasapiError(format!("Failed to get capture client: {e}"))
            })?
        };

        log::info!("✅ Capture client obtained successfully!");
        log::info!("🎉 WASAPI setup completed successfully!");

        Ok((audio_client, capture_client))
    }

    fn process_audio_buffer(
        buffer: &[u8],
        frame_sender: Option<&Sender<Vec<f32>>>,
        level_sender: Option<&Sender<f32>>,
        gain: Option<&Arc<AtomicI32>>,
    ) -> std::result::Result<(), SpeakerRecorderError> {
        // For Windows speaker recording, we're always working with 32-bit float format
        // since we specifically requested WAVE_FORMAT_IEEE_FLOAT in get_device_supported_format
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

        if let Some(ref tx) = frame_sender
            && let Err(e) = tx.try_send(processed_samples.to_vec())
        {
            log::warn!("try send speaker audio frame failed: {e}");
        }

        if let Some(ref tx) = level_sender
            && let Some(db) = calc_rms_level(processed_samples)
            && let Err(e) = tx.try_send(db)
        {
            log::warn!("try send speaker audio db level data failed: {e}");
        }

        Ok(())
    }
}

impl SpeakerRecorder for SpeakerRecorderWindows {
    fn spec(&self) -> WavSpec {
        if let Some(ref spec) = self.cached_spec {
            spec.clone()
        } else if let Some(ref format) = self.device_format {
            WavSpec {
                channels: format.nChannels as u16,
                sample_rate: format.nSamplesPerSec,
                bits_per_sample: format.wBitsPerSample as u16,
                sample_format: hound::SampleFormat::Float,
            }
        } else {
            log::warn!("No device format or cached spec available, using default format");
            WavSpec {
                channels: 2,
                sample_rate: 44100,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            }
        }
    }

    fn get_device_info(&self) -> Option<(u32, String)> {
        self.device_info.clone()
    }

    fn find_default_output(
        &self,
    ) -> std::result::Result<Option<(u32, String)>, SpeakerRecorderError> {
        Ok(self.device_info.clone())
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

        let loop_delay = Duration::from_millis(5);
        let sleeper = SpinSleeper::new(1_000_000); // 1ms

        let sample_rate = self
            .device_format
            .as_ref()
            .map(|f| f.nSamplesPerSec as u32)
            .unwrap_or(44100);

        log::info!("Using sample rate: {}Hz", sample_rate);

        let start_time = std::time::Instant::now();
        let mut total_frames_written: u64 = 0;
        let latency_threshold_frames = (sample_rate as f64 * 0.020) as u64; // 20ms

        while !self.config.stop_sig.load(Ordering::Relaxed) {
            sleeper.sleep(loop_delay);

            let elapsed = start_time.elapsed();
            let expected_frames = (elapsed.as_secs_f64() * sample_rate as f64) as u64;

            let channels = self
                .device_format
                .as_ref()
                .map(|f| f.nChannels as usize)
                .unwrap_or(2); // Fallback to stereo

            // If the number of lagging frames exceeds the threshold,
            // it indicates that the system has stopped sending packets,
            // and silence needs to be filled in.
            if expected_frames > total_frames_written + latency_threshold_frames {
                let missing_frames = expected_frames - total_frames_written;
                let samples_to_fill = (missing_frames as usize * channels) as usize; // Use actual channel count
                let silent_buffer = vec![0.0f32; samples_to_fill];

                if let Some(ref tx) = self.config.frame_sender {
                    _ = tx.try_send(silent_buffer);
                }

                if let Some(ref tx) = self.config.level_sender {
                    let _ = tx.try_send(-200.0);
                }

                log::trace!("Filled silence gap: {} frames", missing_frames);
                total_frames_written += missing_frames;
            }

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
                            unsafe {
                                capture_client.ReleaseBuffer(0).ok();
                            }
                            break;
                        }

                        total_frames_written += num_frames_available as u64;

                        let is_silent = (flags & 1) != 0;
                        if is_silent {
                            let silent_len = (num_frames_available as usize * channels) as usize;
                            let silent_buffer = vec![0.0f32; silent_len];

                            if let Some(ref tx) = self.config.frame_sender {
                                _ = tx.try_send(silent_buffer);
                            }

                            if let Some(ref tx) = self.config.level_sender {
                                _ = tx.try_send(-200.0);
                            }
                        } else {
                            // Calculate buffer length based on actual format
                            let bytes_per_sample = self
                                .device_format
                                .as_ref()
                                .map(|f| f.wBitsPerSample as u32 / 8)
                                .unwrap_or(4); // Default to 32-bit (4 bytes)
                            let buffer_len =
                                (num_frames_available * channels as u32 * bytes_per_sample)
                                    as usize;
                            let buffer =
                                unsafe { std::slice::from_raw_parts(data_ptr, buffer_len) };

                            log::trace!("Processing {} bytes of audio data", buffer_len);

                            Self::process_audio_buffer(
                                buffer,
                                self.config.frame_sender.as_ref(),
                                self.config.level_sender.as_ref(),
                                self.config.gain.as_ref(),
                            )?;
                        }

                        unsafe {
                            if let Err(e) = capture_client.ReleaseBuffer(num_frames_available) {
                                log::error!("Failed to release buffer: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        if e.code().0 == -2004287484 {
                            log::warn!("Device invalidated!");
                            return Err(SpeakerRecorderError::DeviceError(
                                "Device disconnected".into(),
                            ));
                        }
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
    fn find_default_output_inner(&mut self) -> Result<Option<(u32, String)>, SpeakerRecorderError> {
        log::info!("Searching for output audio devices...");

        let device_enumerator: IMMDeviceEnumerator = unsafe {
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
                SpeakerRecorderError::WasapiError(format!(
                    "Failed to create device enumerator: {e}"
                ))
            })?
        };

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

        // Get device supported format by creating a temporary audio client
        self.get_device_supported_format(&device)?;
        self.cached_spec = Some(self.spec());

        Ok(Some((1, friendly_name))) // Use ID 1 as placeholder since Windows uses string IDs
    }

    fn get_device_supported_format(
        &mut self,
        device: &IMMDevice,
    ) -> std::result::Result<(), SpeakerRecorderError> {
        log::info!("Getting device supported format...");

        // Since windows crate 0.62 doesn't have Activate method, we need to use winapi for Activate
        let device_winapi: *mut IMMDeviceWinApi = unsafe { std::mem::transmute_copy(device) };

        let audio_client_winapi: *mut IAudioClientWinApi = unsafe {
            let mut audio_client_ptr: *mut IAudioClientWinApi = std::ptr::null_mut();
            let hr = (*device_winapi).Activate(
                &IAudioClientWinApi::uuidof(),
                winapi::um::combaseapi::CLSCTX_ALL,
                std::ptr::null_mut(),
                &mut audio_client_ptr as *mut *mut _ as *mut *mut std::ffi::c_void,
            );

            if FAILED(hr) {
                // Try multimedia endpoint if console fails
                let device_enumerator: IMMDeviceEnumerator =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
                        SpeakerRecorderError::WasapiError(format!(
                            "Failed to create device enumerator: {e}"
                        ))
                    })?;

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
                        "Both console and multimedia endpoints failed for format check. Console: HRESULT=0x{:08X}, Multimedia: HRESULT=0x{:08X}",
                        hr, hr_multimedia
                    )));
                }
            }

            audio_client_ptr
        };

        // Convert back to windows crate IAudioClient
        let audio_client: IAudioClient = unsafe { std::mem::transmute_copy(&audio_client_winapi) };

        // Try different formats to find one that's supported
        let formats_to_try = vec![
            // High quality format
            WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_IEEE_FLOAT as u16,
                nChannels: 2,
                nSamplesPerSec: 48000,
                nAvgBytesPerSec: 48000 * 2 * 4,
                nBlockAlign: 2 * 4,
                wBitsPerSample: 32,
                cbSize: 0,
            },
            // Standard CD quality format
            WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_IEEE_FLOAT as u16,
                nChannels: 2,
                nSamplesPerSec: 44100,
                nAvgBytesPerSec: 44100 * 2 * 4,
                nBlockAlign: 2 * 4,
                wBitsPerSample: 32,
                cbSize: 0,
            },
            // Lower quality format
            WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_IEEE_FLOAT as u16,
                nChannels: 2,
                nSamplesPerSec: 22050,
                nAvgBytesPerSec: 22050 * 2 * 4,
                nBlockAlign: 2 * 4,
                wBitsPerSample: 32,
                cbSize: 0,
            },
        ];

        for (i, format) in formats_to_try.iter().enumerate() {
            let samples_per_sec = format.nSamplesPerSec;
            let channels = format.nChannels;
            let bits_per_sample = format.wBitsPerSample;

            log::debug!(
                "Trying format {}: {}Hz, {} channels, {}-bit float",
                i + 1,
                samples_per_sec,
                channels,
                bits_per_sample,
            );

            let mut closest_format_ptr: *mut WAVEFORMATEX = std::ptr::null_mut();
            let hr = unsafe {
                audio_client.IsFormatSupported(
                    AUDCLNT_SHAREMODE_SHARED,
                    format,
                    Some(&mut closest_format_ptr),
                )
            };

            if hr.0 == 0 {
                if !closest_format_ptr.is_null() {
                    // Device returned a different format that it supports
                    let supported_format = unsafe { *closest_format_ptr };
                    let samples_per_sec = supported_format.nSamplesPerSec;
                    let channels = supported_format.nChannels;
                    let bits_per_sample = supported_format.wBitsPerSample;

                    log::info!(
                        "Device returned closest supported format: {}Hz, {} channels, {}-bit",
                        samples_per_sec,
                        channels,
                        bits_per_sample,
                    );
                    self.device_format = Some(supported_format);
                } else {
                    // Device supports our requested format exactly
                    log::info!(
                        "Device supports requested format exactly: {}Hz, {} channels, {}-bit float",
                        samples_per_sec,
                        channels,
                        bits_per_sample,
                    );
                    self.device_format = Some(*format);
                }
                break;
            } else {
                log::debug!("Format {} not supported: HRESULT=0x{:08X}", i + 1, hr.0);
                if i == formats_to_try.len() - 1 {
                    return Err(SpeakerRecorderError::WasapiError(format!(
                        "No supported audio format found for device. Last HRESULT=0x{:08X}",
                        hr.0
                    )));
                }
            }
        }

        if let Some(format) = self.device_format {
            let samples_per_sec = format.nSamplesPerSec;
            let channels = format.nChannels;
            let bits_per_sample = format.wBitsPerSample;
            let format_tag = format.wFormatTag;

            log::info!(
                "✅ Selected device format: {}Hz, {} channels, {}-bit, tag={}",
                samples_per_sec,
                channels,
                bits_per_sample,
                format_tag,
            );
        } else {
            return Err(SpeakerRecorderError::WasapiError(
                "Failed to determine supported audio format".to_string(),
            ));
        }

        Ok(())
    }
}
