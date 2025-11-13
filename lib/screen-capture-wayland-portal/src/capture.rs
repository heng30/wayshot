use crate::{
    backend::{request_screen_access, PipeWireCapturer, StreamHandle},
    error::{Error, Result},
};
use screen_capture::{
    Capture, CaptureStatus, CaptureStreamCallbackData, CaptureStreamConfig, LogicalSize, Position,
};
use spin_sleep::SpinSleeper;
use std::sync::atomic::Ordering;
use tokio::runtime::Runtime;
use std::time::{Duration, Instant};

pub async fn capture_screenshot_async(_screen_name: &str) -> Result<Capture> {
    capture_all_outputs().await
}

pub fn capture_output(name: &str, include_cursor: bool) -> Result<Capture> {
    let rt = Runtime::new().map_err(|e| Error::Other(format!("Failed to create runtime: {}", e)))?;

    rt.block_on(async {
        if include_cursor {
            let streams = request_screen_access(
                ashpd::desktop::screencast::CursorMode::Embedded,
                ashpd::desktop::screencast::SourceType::Monitor,
            )
            .await
            .map_err(|e| Error::Other(format!("Failed to request screen access: {}", e)))?;

            let stream = streams
                .iter()
                .find(|s| s.screen_info.name == name)
                .ok_or_else(|| Error::Other(format!("Screen '{}' not found", name)))?;

            capture_single_frame(stream).await
        } else {
            capture_output_by_name(name).await
        }
    })
}

pub fn capture_mean_time(screen_name: &str, counts: u32) -> crate::error::Result<Duration> {
    assert!(counts > 0);

    let start = Instant::now();
    for _ in 0..counts {
        _ = capture_output(screen_name, true)?;
    }
    Ok(start.elapsed() / counts)
}

pub fn capture_output_stream(
    config: CaptureStreamConfig,
    mut cb: impl FnMut(CaptureStreamCallbackData),
) -> crate::error::Result<CaptureStatus> {
    let rt = Runtime::new().map_err(|e| Error::Other(format!("Failed to create runtime: {}", e)))?;

    rt.block_on(async {
        streaming_capture(config, cb).await
    })
}

pub async fn capture_all_outputs() -> Result<Capture> {
    let streams = request_screen_access(
        ashpd::desktop::screencast::CursorMode::Hidden,
        ashpd::desktop::screencast::SourceType::Monitor,
    )
    .await
    .map_err(|e| Error::Other(format!("Failed to request screen access: {}", e)))?;

    if streams.is_empty() {
        return Err(Error::Other("No screens available".to_string()));
    }

    // Capture from the first screen
    let stream = &streams[0];
    capture_single_frame(stream).await
}

pub async fn capture_output_by_name(name: &str) -> Result<Capture> {
    let streams = request_screen_access(
        ashpd::desktop::screencast::CursorMode::Hidden,
        ashpd::desktop::screencast::SourceType::Monitor,
    )
    .await
    .map_err(|e| Error::Other(format!("Failed to request screen access: {}", e)))?;

    let stream = streams
        .iter()
        .find(|s| s.screen_info.name == name)
        .ok_or_else(|| Error::Other(format!("Screen '{}' not found", name)))?;

    capture_single_frame(stream).await
}

pub async fn capture_region(
    name: &str,
    _region_position: Position,
    _region_size: LogicalSize,
) -> Result<Capture> {
    // XDG portal typically doesn't support region capture directly
    // We'll capture the full screen and let the caller crop if needed
    capture_output_by_name(name).await
}

async fn capture_single_frame(stream: &StreamHandle) -> Result<Capture> {
    log::info!("Capturing frame from screen: {} (node_id: {}, size: {}x{})",
               stream.screen_info.name, stream.pipewire_node_id, stream.width, stream.height);

    // Create PipeWire capturer
    let mut capturer = PipeWireCapturer::new()
        .map_err(|e| Error::Other(format!("Failed to create PipeWire capturer: {}", e)))?;

    // Create PipeWire stream
    capturer.create_stream(stream.pipewire_node_id, stream.width, stream.height)
        .await
        .map_err(|e| Error::Other(format!("Failed to create PipeWire stream: {}", e)))?;

    // Capture frame data (already in RGBA format)
    let frame_data = capturer.capture_frame(stream.pipewire_node_id)
        .await
        .map_err(|e| Error::Other(format!("Failed to capture frame: {}", e)))?;

    log::info!("Successfully captured frame: {}x{}, {} bytes",
               stream.width, stream.height, frame_data.len());

    Ok(Capture {
        width: stream.width,
        height: stream.height,
        pixel_data: frame_data,
    })
}

async fn streaming_capture(
    config: CaptureStreamConfig,
    mut cb: impl FnMut(CaptureStreamCallbackData),
) -> Result<CaptureStatus> {
    let cursor_mode = if config.include_cursor {
        ashpd::desktop::screencast::CursorMode::Embedded
    } else {
        ashpd::desktop::screencast::CursorMode::Hidden
    };

    let streams = request_screen_access(
        cursor_mode,
        ashpd::desktop::screencast::SourceType::Monitor,
    )
    .await
    .map_err(|e| Error::Other(format!("Failed to request screen access: {}", e)))?;

    let stream = streams
        .iter()
        .find(|s| s.screen_info.name == config.name)
        .ok_or_else(|| Error::Other(format!("Screen '{}' not found", config.name)))?;

    // Create PipeWire capturer for streaming
    let mut capturer = PipeWireCapturer::new()
        .map_err(|e| Error::Other(format!("Failed to create PipeWire capturer: {}", e)))?;

    capturer.create_stream(stream.pipewire_node_id, stream.width, stream.height)
        .await
        .map_err(|e| Error::Other(format!("Failed to create PipeWire stream: {}", e)))?;

    let mut index = 0;
    let interval_ms = config.fps.map(|v| 1000.0 / v);
    let sleeper = SpinSleeper::default();
    let start_time = std::time::Instant::now();

    log::info!("Starting PipeWire screen capture stream for screen: {} at {} FPS",
               config.name,
               config.fps.unwrap_or(30.0));

    // Main capture loop using PipeWire
    loop {
        // Check for cancellation signal
        if config.cancel_sig.load(Ordering::Relaxed) {
            log::info!("Stream capture stopped by user after {} frames", index);
            return Ok(CaptureStatus::Stopped);
        }

        // Capture frame from PipeWire
        match capturer.capture_frame(stream.pipewire_node_id).await {
            Ok(frame_data) => {
                let start = Instant::now();

                let capture = Capture {
                    width: stream.width,
                    height: stream.height,
                    pixel_data: frame_data,
                };

                cb(CaptureStreamCallbackData {
                    frame_index: index,
                    capture_time: start.elapsed(),
                    elapse: start_time.elapsed(),
                    data: capture,
                });
            }
            Err(e) => {
                log::error!("Failed to capture frame: {}", e);
                return Err(Error::Other(format!("Frame capture failed: {}", e)));
            }
        }

        // Maintain target frame rate
        if let Some(interval) = interval_ms {
            let target_time =
                start_time + Duration::from_millis((interval * (index + 1) as f64) as u64);
            sleeper.sleep_until(target_time);
        }

        index += 1;

        // Log progress every 100 frames
        if index % 100 == 0 {
            log::info!("PipeWire stream capture progress: {} frames captured", index);
        }

        // For demo purposes, stop after a few frames
        if index >= 3 {
            log::info!("Demo: stopping after {} frames", index);
            return Ok(CaptureStatus::Stopped);
        }
    }
}