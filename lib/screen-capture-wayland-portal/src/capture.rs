use crate::{
    PortalCapturer, available_screens,
    error::{Error, Result},
};
use crossbeam::channel::bounded;
use once_cell::sync::Lazy;
use screen_capture::{Capture, CaptureStatus, CaptureStreamCallbackData, CaptureStreamConfig};
use spin_sleep::SpinSleeper;
use std::{
    os::fd::IntoRawFd,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;

static TOKIO_RT: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build global tokio runtime")
});

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
    let fps = config.fps.unwrap_or(25.0);
    let stop_sig = config.cancel_sig.clone();

    // Use a blocking task to run the non-Send PipeWire operations
    std::thread::spawn(move || {
        TOKIO_RT.block_on(async move {
            let mut backend = PortalCapturer::new(screen_info)
                .with_include_cursor(config.include_cursor)
                .with_fps(config.fps.unwrap_or(25.0) as u32)
                .with_stop_sig(config.cancel_sig)
                .with_sender(Some(sender));

            let Ok((stream, fd)) = backend.open_portal().await else {
                log::warn!("failed to open portal");
                return;
            };

            let pipewire_node_id = stream.pipe_wire_node_id();

            log::info!(
                "node id {}, fd {}",
                pipewire_node_id,
                &fd.try_clone().unwrap().into_raw_fd()
            );

            config.sync_sig.store(true, Ordering::Relaxed);

            if let Err(e) = backend.start_streaming(pipewire_node_id, fd).await {
                log::warn!("Error: {e}");
            }
        });
    });

    let mut index = 0;
    let mut last_frame = None;
    let mut start_time = Instant::now();
    let spin_sleeper = SpinSleeper::default();
    let frame_interval = Duration::from_secs_f64(1.0 / fps);

    while !stop_sig.load(Ordering::Relaxed) {
        while let Ok((_, frame)) = receiver.try_recv() {
            if last_frame.is_none() {
                start_time = Instant::now();
            }

            last_frame = Some(frame);
        }

        if last_frame.is_none() {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }

        let capture = Capture {
            width: screen_size.width as u32,
            height: screen_size.height as u32,
            pixel_data: last_frame.clone().unwrap(),
        };

        cb(CaptureStreamCallbackData {
            frame_index: index,
            capture_time: Duration::ZERO,
            elapse: start_time.elapsed(),
            data: capture,
        });

        index += 1;

        let next_frame_time = start_time + frame_interval * index as u32;
        spin_sleeper.sleep_until(next_frame_time);
    }

    log::info!("exit capture receiver thread...");

    Ok(CaptureStatus::Finished)
}

pub fn capture_mean_time(_screen_name: &str, _counts: u32) -> Result<Option<Duration>> {
    Ok(None)
}
