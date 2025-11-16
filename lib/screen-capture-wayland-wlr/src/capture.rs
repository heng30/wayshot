use crate::{
    Error, available_screens,
    backend::{self, State},
};
use screen_capture::{
    Capture, CaptureStatus, CaptureStreamCallbackData, CaptureStreamConfig, LogicalSize, Position,
};
use spin_sleep::SpinSleeper;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

pub fn capture_all_outputs(include_cursor: bool) -> Result<Capture, Error> {
    let (mut state, mut event_queue) = backend::connect_and_get_output_info()?;
    inner_capture_all_outputs(&mut state, &mut event_queue, include_cursor)
}

fn inner_capture_all_outputs(
    state: &mut State,
    event_queue: &mut wayland_client::EventQueue<State>,
    include_cursor: bool,
) -> Result<Capture, Error> {
    if state.output_infos.is_empty() {
        return Err(crate::Error::NoCaptures);
    }

    let output_index = 0;

    // Set the capture region to match the full output size
    state.output_infos[output_index].image_logical_position =
        state.output_infos[output_index].output_logical_position;
    state.output_infos[output_index].image_logical_size =
        state.output_infos[output_index].output_logical_size;

    // Request screen capture from the compositor
    state
        .wlr_screencopy_manager
        .as_ref()
        .unwrap()
        .capture_output(
            include_cursor as i32,
            &state.output_infos[output_index].wl_output,
            &event_queue.handle(),
            output_index,
        );

    // Wait for the capture to complete by processing events
    while !state.output_infos[output_index].image_ready {
        event_queue.blocking_dispatch(state)?;
    }

    // Reset the ready flag for next capture
    state.output_infos[output_index].image_ready = false;

    // Convert the captured data to a buffer
    captures_to_buffer(&state.output_infos)
}

pub fn capture_output(name: &str, include_cursor: bool) -> Result<Capture, Error> {
    let (mut state, mut event_queue) = backend::connect_and_get_output_info()?;
    inner_capture_output(&mut state, &mut event_queue, name, include_cursor)
}

fn inner_capture_output(
    state: &mut State,
    event_queue: &mut wayland_client::EventQueue<State>,
    name: &str,
    include_cursor: bool,
) -> Result<Capture, Error> {
    // Filter outputs to keep only the one with the specified name
    state.output_infos.retain_mut(|output_info| {
        if output_info.name.clone().unwrap_or_default() == name {
            true
        } else {
            // Release Wayland resources for unused outputs
            output_info.wl_output.release();
            false
        }
    });

    // Check if the requested output was found
    if state.output_infos.is_empty() {
        return Err(crate::Error::NoOutput(name.to_owned()));
    }

    // Set capture region to the full output size at position (0, 0)
    state.output_infos[0].image_logical_position = Some(Position { x: 0, y: 0 });
    state.output_infos[0].image_logical_size = state.output_infos[0].output_logical_size;

    // Request screen capture
    state
        .wlr_screencopy_manager
        .as_ref()
        .ok_or(crate::Error::Unimplemented(
            "Unsupported Window Manager which doesn't implement `wlroots` protocol.".to_string(),
        ))?
        .capture_output(
            include_cursor as i32,
            &state.output_infos[0].wl_output,
            &event_queue.handle(),
            0,
        );

    // Wait for capture completion
    while !state.output_infos[0].image_ready {
        event_queue.blocking_dispatch(state)?;
    }

    // Reset ready flag
    state.output_infos[0].image_ready = false;

    // Convert to buffer
    captures_to_buffer(&state.output_infos)
}

pub fn capture_output_stream(
    config: CaptureStreamConfig,
    mut cb: impl FnMut(CaptureStreamCallbackData),
) -> Result<CaptureStatus, Error> {
    let (mut state, mut event_queue) = backend::connect_and_get_output_info()?;

    let mut index = 0;
    // Calculate frame interval if FPS is specified
    let interval_ms = config.fps.map(|v| 1000.0 / v);
    let sleeper = SpinSleeper::default();
    let start_time = std::time::Instant::now();
    let mut last_cleanup = std::time::Instant::now();
    const CLEANUP_INTERVAL: Duration = Duration::from_secs(5); // Clean every 5 seconds

    // Main capture loop
    loop {
        // Check for cancellation signal
        if config.cancel_sig.load(Ordering::Relaxed) {
            log::info!("Exit capture iter process after Stopped");

            let attempts = dispatch_pending(&mut state, &mut event_queue);
            if attempts > 0 {
                log::info!("Exit capture iter process, pending envent counts: {attempts}");
            }

            drop(state);
            return Ok(CaptureStatus::Stopped);
        }

        // Periodically clean up event queue
        if last_cleanup.elapsed() > CLEANUP_INTERVAL {
            let attempts = dispatch_pending(&mut state, &mut event_queue);
            if attempts > 0 {
                log::info!("After 5 seconds, pending envent counts: {attempts}");
            }

            last_cleanup = std::time::Instant::now();
        }

        // Perform the actual capture
        let start = Instant::now();
        let ouput = inner_capture_output(
            &mut state,
            &mut event_queue,
            &config.name,
            config.include_cursor,
        )?;

        // Call the user-provided callback with capture data
        cb(CaptureStreamCallbackData {
            frame_index: index,
            capture_time: start.elapsed(),
            elapse: start_time.elapsed(),
            data: ouput,
        });

        // Maintain target frame rate if specified
        if let Some(interval) = interval_ms {
            let target_time =
                start_time + Duration::from_millis((interval * (index + 1) as f64) as u64);
            sleeper.sleep_until(target_time);
        }

        index += 1;
    }
}

fn dispatch_pending(state: &mut State, event_queue: &mut wayland_client::EventQueue<State>) -> u32 {
    let mut attempts = 0;
    let cleanup_start = std::time::Instant::now();

    // Send buffered requests to Wayland server
    _ = event_queue.flush();

    // Thoroughly clean up event queue before exit
    while attempts < 10 && cleanup_start.elapsed() < Duration::from_millis(100) {
        match event_queue.dispatch_pending(state) {
            Ok(0) => break, // Queue is empty
            Ok(_) => {}     // Processed events, continue
            Err(e) => {
                log::warn!("Error dispatching pending events during cleanup: {}", e);
                break;
            }
        }
        attempts += 1;
    }

    attempts
}

pub fn capture_region(
    name: &str,
    region_position: Position,
    region_size: LogicalSize,
    include_cursor: bool,
) -> Result<Capture, Error> {
    let infos = available_screens()?;
    let info = infos.iter().find(|info| info.name == name);

    if info.is_none() {
        return Err(Error::Other(format!("no found screen `{}`", name)));
    }

    let scale_factor = info.unwrap().scale_factor;
    if scale_factor == 0.0 {
        return Err(Error::Other("scale factor is 0".to_string()));
    }

    let x = (region_position.x as f32 / scale_factor) as i32;
    let y = (region_position.y as f32 / scale_factor) as i32;
    let w = (region_size.width as f32 / scale_factor) as i32;
    let h = (region_size.height as f32 / scale_factor) as i32;

    let (mut state, mut event_queue) = backend::connect_and_get_output_info()?;

    state.output_infos.retain_mut(|output_info| {
        if output_info.name.clone().unwrap_or_default() == name {
            true
        } else {
            output_info.wl_output.release();
            false
        }
    });

    // Capture the first intersecting output
    if let Some((i, output_info)) = state.output_infos.iter_mut().enumerate().next() {
        state
            .wlr_screencopy_manager
            .as_ref()
            .unwrap()
            .capture_output_region(
                include_cursor as i32,
                &output_info.wl_output,
                x,
                y,
                w,
                h,
                &event_queue.handle(),
                i,
            );
    }

    // Wait for all captures to complete
    while state
        .output_infos
        .iter()
        .any(|output_info| !output_info.image_ready)
    {
        event_queue.blocking_dispatch(&mut state)?;
    }

    // Convert captured data to buffer (currently only supports single output)
    captures_to_buffer(&state.output_infos)
}

fn captures_to_buffer(output_infos: &[backend::OutputInfo]) -> Result<Capture, Error> {
    // Ensure we have at least one output with captured data
    if output_infos.is_empty() {
        return Err(crate::Error::NoCaptures);
    }

    // Get the first output (only one output is captured in current implementation)
    let first_output = output_infos.iter().next().unwrap();
    let image_mmap = first_output.image_mmap.as_ref().unwrap();
    let mmap_size = first_output.image_mmap_size.unwrap();

    // Copy the memory-mapped pixel data to a Vec for safe ownership
    let pixel_data = image_mmap.to_vec();

    Ok(Capture {
        width: mmap_size.width as u32,
        height: mmap_size.height as u32,
        pixel_data,
    })
}

pub fn capture_mean_time(screen_name: &str, counts: u32) -> Result<Option<Duration>, Error> {
    assert!(counts > 0);

    let screen_infos = available_screens()?;
    if screen_infos.is_empty() {
        return Err(crate::Error::NoOutput(
            "available screen no found".to_string(),
        ));
    }

    if !screen_infos.iter().any(|item| item.name == screen_name) {
        return Err(crate::Error::NoOutput(format!(
            "{screen_name} is not in available screen list"
        )));
    }

    let start = Instant::now();
    for _ in 0..counts {
        _ = capture_output(screen_name, true);
    }
    Ok(Some(start.elapsed() / counts))
}
