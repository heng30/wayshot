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
    let mut capture_start = Instant::now();

    while let Ok((elapse, frame)) = receiver.recv() {
        let capture = Capture {
            width: screen_size.width as u32,
            height: screen_size.height as u32,
            pixel_data: frame,
        };

        cb(CaptureStreamCallbackData {
            frame_index: index,
            capture_time: capture_start.elapsed(),
            elapse: elapse,
            data: capture,
        });

        capture_start = Instant::now();
        index += 1;
    }

    log::info!("exit capture receiver thread...");

    Ok(CaptureStatus::Finished)
}

pub fn capture_mean_time(_screen_name: &str, _counts: u32) -> Result<Option<Duration>> {
    Ok(None)

    // use std::sync::{
    //     Arc, atomic::{AtomicBool, Ordering},
    // },
    // let capturer = crate::ScreenCaptureWaylandPortal::default();
    // let try_seconds = counts.clamp(1, 5) as u64;
    //
    // let config = CaptureStreamConfig {
    //     name: screen_name.to_string(),
    //     include_cursor: true,
    //     fps: Some(60.0),
    //     cancel_sig: Arc::new(AtomicBool::new(false)),
    // };
    //
    // let mut total_frames = 0;
    // let total_frames_mut = &mut total_frames;
    // let stop_sig = config.cancel_sig.clone();
    //
    // capturer
    //     .capture_output_stream(config, move |data| {
    //         *total_frames_mut = data.frame_index;
    //
    //         log::debug!("yyyyyyyyyyyyy {:?}", data.elapse);
    //         if data.elapse > Duration::from_secs(try_seconds) {
    //             stop_sig.store(true, Ordering::Relaxed);
    //         }
    //     })
    //     .map_err(|e| Error::Other(e.to_string()))?;
    //
    // log::info!("average fps: {}", total_frames / try_seconds);
    //
    // if total_frames == 0 {
    //     return Err(Error::NoOutput("Can not capture frame".to_string()));
    // }
    //
    // Ok(Some(Duration::from_millis(
    //     1000 * try_seconds / total_frames,
    // )))
    //
}
