//! Camera and Screen Recording Demo
//!
//! This example demonstrates how to record your screen with a camera overlay mixed in.
//! The camera image will be overlaid on the screen recording using the specified shape.
//!
//! Features demonstrated:
//! - Screen recording with camera overlay
//! - Camera image composition (circle shape)
//! - Multi-threaded image processing (4 workers when camera is enabled)
//! - Audio recording from microphone and desktop speaker
//! - 10 seconds recording duration
//!
//! Usage:
//! ```bash
//! cargo run --example camera_and_recording_demo --features wayland-wlr
//! ```

use camera::{Rgba, Shape, ShapeCircle, image_composition::ShapeBase};
use recorder::{
    AudioRecorder, CameraMixConfig, FPS, RecorderConfig, RecordingSession, platform_screen_capture,
};
use screen_capture::ScreenCapture;
use std::{sync::atomic::Ordering, thread, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Camera and Screen Recording Demo");
    log::info!("====================================");

    let cameras = camera::query_available_cameras();
    if cameras.is_empty() {
        log::warn!("No cameras found! Please connect a camera and try again.");
        log::info!("The recording will continue without camera overlay.");
    } else {
        log::info!("Found {} camera(s):", cameras.len());
        for (i, cam) in cameras.iter().enumerate() {
            log::info!("  {}. {} - {}", i, cam.name, cam.description);
        }
        log::info!("Using first camera: {}", cameras[0].name);
    }

    let audio_recorder = AudioRecorder::new();
    let default_input = audio_recorder.get_default_input_device()?;

    if let Some(device) = &default_input {
        log::info!(
            "Audio input device: {}. Config: {:?}",
            device.name,
            device.default_config
        );
    } else {
        log::warn!("No default audio input device found!");
    }

    let mut screen_capturer = platform_screen_capture();
    let screen_infos = screen_capturer.available_screens()?;

    if screen_infos.is_empty() {
        panic!("No screens found!");
    }

    log::info!(
        "Screen: {} ({}x{})",
        screen_infos[0].name,
        screen_infos[0].logical_size.width,
        screen_infos[0].logical_size.height
    );

    let camera_mix_config = if !cameras.is_empty() {
        let circle = ShapeCircle::default().with_radius(150).with_base(
            ShapeBase::default()
                .with_pos((0.85, 0.15)) // Position at top-right corner (85%, 15%)
                .with_border_width(8) // 8 pixel white border
                .with_border_color(Rgba([255, 255, 255, 255])),
        );

        CameraMixConfig::default()
            .with_enable(true)
            .with_camera_name(Some(cameras[0].name.clone()))
            .with_width(640)
            .with_height(480) // 720x1280 or 480x640
            .with_fps(25)
            .with_shape(Shape::Circle(circle))
            .with_mirror_horizontal(true)
    } else {
        CameraMixConfig::default()
    };

    // Create recorder configuration
    let config = RecorderConfig::new(
        screen_infos[0].name.clone(),
        screen_infos[0].logical_size.clone(),
        #[cfg(not(target_os = "windows"))]
        RecorderConfig::make_filename("/tmp"),
        #[cfg(target_os = "windows")]
        RecorderConfig::make_filename("C:/Users/blue/Desktop"),
    )
    .with_process_mode(recorder::ProcessMode::RecordScreen)
    .with_audio_device_name(default_input.map(|d| d.name))
    .with_enable_recording_speaker(true) // Record desktop/system audio
    .with_enable_denoise(true) // Enable noise reduction
    .with_convert_to_mono(true) // Convert audio to mono
    .with_resolution(recorder::Resolution::P1080)
    .with_fps(FPS::Fps25)
    .with_camera_mix_config(camera_mix_config);

    log::info!("Recording configuration:");
    log::info!("  Resolution: 1080p");
    log::info!("  FPS: 30");
    log::info!("  Camera overlay: {}", config.camera_mix_config.enable);
    log::info!("  Duration: 10 seconds");

    log::info!("Starting recording...");

    let rt_handle = tokio::runtime::Handle::current();
    let mut session = RecordingSession::new(config);
    session.start(rt_handle, screen_capturer)?;

    let stop_sig = session.get_stop_sig().clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(10));
        log::info!("10 seconds elapsed, stopping recording...");
        stop_sig.store(true, Ordering::Relaxed);
    });

    session.wait()?;

    log::info!("Recording completed successfully!");
    log::info!("Check the output directory for the recorded video file.");

    Ok(())
}
