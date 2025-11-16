use crate::{
    PortalCapturer, available_screens,
    error::{Error, Result},
};
use crossbeam::channel::bounded;
use screen_capture::{Capture, CaptureStatus, CaptureStreamCallbackData, CaptureStreamConfig};
use std::{
    os::fd::IntoRawFd,
    time::{Duration, Instant},
};

pub fn capture_output_stream(
    config: CaptureStreamConfig,
    mut cb: impl FnMut(CaptureStreamCallbackData),
) -> Result<CaptureStatus> {
    let screens = available_screens().map_err(|e| Error::ScreenInfoError(e.to_string()))?;
    if screens.is_empty() {
        return Err(Error::ScreenInfoError(
            "No found available screens".to_string(),
        ));
    }

    let screen_info = match screens.iter().find(|item| item.name == config.name) {
        Some(screen) => screen.clone(),
        None => {
            return Err(Error::ScreenInfoError(format!(
                "No found available screen: {}",
                config.name
            )));
        }
    };

    let (sender, receiver) = bounded(128);
    let screen_size = screen_info.logical_size.clone();
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| Error::Other(format!("New tokio runtime for screencast failed: {e}")))?;

    rt.spawn(async move {
        let mut backend = PortalCapturer::new(screen_info)
            .with_include_cursor(config.include_cursor)
            .with_fps(config.fps.unwrap_or(25.0) as u32)
            .with_stop_sig(config.cancel_sig)
            .with_sender(Some(sender));

        let (stream, fd) = backend.open_portal().await.expect("failed to open portal");
        let pipewire_node_id = stream.pipe_wire_node_id();

        log::info!(
            "node id {}, fd {}",
            pipewire_node_id,
            &fd.try_clone().unwrap().into_raw_fd()
        );

        if let Err(e) = backend.start_streaming(pipewire_node_id, fd).await {
            log::warn!("Error: {e}");
        };
    });

    let mut index = 0;
    let start_time = std::time::Instant::now();

    while let Ok(frame) = receiver.recv() {
        let start = Instant::now();
        let capture = Capture {
            width: screen_size.width as u32,
            height: screen_size.height as u32,
            pixel_data: frame,
        };

        cb(CaptureStreamCallbackData {
            frame_index: index,
            capture_time: start.elapsed(),
            elapse: start_time.elapsed(),
            data: capture,
        });

        index += 1;
    }

    log::info!("exit capture receiver thread...");

    Ok(CaptureStatus::Finished)
}

pub fn capture_mean_time(_screen_name: &str, _counts: u32) -> Result<Option<Duration>> {
    Ok(None)
}
