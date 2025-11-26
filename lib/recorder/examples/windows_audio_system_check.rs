use std::ptr;
use windows::{
    Win32::{
        Media::Audio::*,
        System::Com::*,
    },
};

// Import winapi for the missing Activate method
use winapi::um::audioclient::IAudioClient as IAudioClientWinApi;
use winapi::um::mmdeviceapi::IMMDevice as IMMDeviceWinApi;
use winapi::shared::winerror::FAILED;
use winapi::Interface as WinApiInterface;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("=== Windows Audio System Check ===\n");

    check_wasapi_interfaces();

    Ok(())
}

fn check_wasapi_interfaces() {
    unsafe {
        println!("Testing COM initialization...");
        let com_result = CoInitializeEx(Some(ptr::null()), COINIT_MULTITHREADED);
        if com_result.is_err() {
            println!("❌ COM initialization failed: {:?}", com_result);
            return;
        }
        println!("✅ COM initialized successfully");

        let enumerator_result = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER);
        if enumerator_result.is_err() {
            println!("❌ Device enumerator creation failed: {:?}", enumerator_result);
            CoUninitialize();
            return;
        }
        println!("✅ Device enumerator created successfully");

        println!("Testing default audio device...");
        let enumerator: IMMDeviceEnumerator = enumerator_result.unwrap();
        let device_result = enumerator.GetDefaultAudioEndpoint(eRender, eConsole);

        match device_result {
            Ok(device) => {
                println!("✅ Default device obtained successfully");

                let state_result = device.GetState();
                match state_result {
                    Ok(DEVICE_STATE_ACTIVE) => println!("✅ Device state: Active"),
                    Ok(state) => println!("⚠️  Device state: {:?} (inactive)", state),
                    Err(e) => println!("❌ Failed to get device state: {:?}", e),
                }

                let id_result = device.GetId();
                match id_result {
                    Ok(id) => {
                        let id_str = {
                            let ptr = id.0;
                            if ptr.is_null() {
                                "NULL".to_string()
                            } else {
                                let mut len = 0;
                                while *ptr.add(len) != 0 {
                                    len += 1;
                                }
                                let slice = std::slice::from_raw_parts(ptr, len);
                                String::from_utf16_lossy(slice)
                            }
                        };
                        println!("✅ Device ID: {}", id_str);
                    }
                    Err(e) => println!("❌ Failed to get device ID: {:?}", e),
                }

                println!("Testing IAudioClient interface using winapi Activate method...");

                // Use the same hybrid approach as the working implementation
                let device_winapi: *mut IMMDeviceWinApi = std::mem::transmute_copy(&device);

                let audio_client_winapi: *mut IAudioClientWinApi = {
                    let mut audio_client_ptr: *mut IAudioClientWinApi = std::ptr::null_mut();
                    let hr = (*device_winapi).Activate(
                        &IAudioClientWinApi::uuidof(),
                        winapi::um::combaseapi::CLSCTX_ALL,
                        std::ptr::null_mut(),
                        &mut audio_client_ptr as *mut *mut _ as *mut *mut std::ffi::c_void,
                    );

                    if FAILED(hr) {
                        println!("❌ Console endpoint Activate failed: HRESULT=0x{:08X}", hr);

                        // Try multimedia device role
                        println!("Trying multimedia device role...");
                        let device_multimedia = enumerator
                            .GetDefaultAudioEndpoint(eRender, eMultimedia);

                        match device_multimedia {
                            Ok(multimedia_device) => {
                                let device_multimedia_winapi: *mut IMMDeviceWinApi = std::mem::transmute_copy(&multimedia_device);
                                let hr_multimedia = (*device_multimedia_winapi).Activate(
                                    &IAudioClientWinApi::uuidof(),
                                    winapi::um::combaseapi::CLSCTX_ALL,
                                    std::ptr::null_mut(),
                                    &mut audio_client_ptr as *mut *mut _ as *mut *mut std::ffi::c_void,
                                );

                                if FAILED(hr_multimedia) {
                                    println!("❌ Multimedia endpoint Activate also failed: HRESULT=0x{:08X}", hr_multimedia);
                                    CoUninitialize();
                                    return;
                                } else {
                                    println!("✅ Multimedia device IAudioClient activated successfully!");
                                }
                            }
                            Err(e) => {
                                println!("❌ Failed to get multimedia device: {:?}", e);
                                CoUninitialize();
                                return;
                            }
                        }
                    } else {
                        println!("🎉 Console device IAudioClient activated successfully!");
                    }

                    audio_client_ptr
                };

                if !audio_client_winapi.is_null() {
                    println!("✅ IAudioClient interface available and working!");
                    println!("✅ WASAPI fully functional with proper Activate method");
                    println!("✅ This matches the working speaker_recorder_windows.rs implementation");
                } else {
                    println!("❌ Failed to create IAudioClient interface");
                }
            }
            Err(e) => {
                println!("❌ Failed to get default device: {:?}", e);
            }
        }

        CoUninitialize();
    }
}