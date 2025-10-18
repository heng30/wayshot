use crate::{
    Capture, CaptureIterCallbackData, CaptureIterConfig, CaptureIterStatus, Error,
    available_screens,
    backend::{self, State},
    screen_info::{LogicalSize, Position},
};
use spin_sleep::SpinSleeper;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

/// Captures all connected outputs as a single image.
///
/// This function captures all available outputs and composites them into a single image.
/// The outputs are positioned according to their logical positions in the Wayland compositor.
///
/// # Arguments
///
/// * `include_cursor` - Whether to include the mouse cursor in the capture
///
/// # Returns
///
/// Returns a [`Capture`] containing the composited image of all outputs.
///
/// # Errors
///
/// Returns an error if:
/// - No outputs are available
/// - Connection to Wayland server fails
/// - Event dispatch fails
pub fn capture_all_outputs(include_cursor: bool) -> Result<Capture, Error> {
    let (mut state, mut event_queue) = backend::connect_and_get_output_info()?;
    inner_capture_all_outputs(&mut state, &mut event_queue, include_cursor)
}

/// Internal function to capture all outputs.
///
/// This function handles the actual capture process for all outputs.
fn inner_capture_all_outputs(
    state: &mut State,
    event_queue: &mut wayland_client::EventQueue<State>,
    include_cursor: bool,
) -> Result<Capture, Error> {
    // Check if there are any outputs available
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

/// Captures a specific output by name.
///
/// This function captures a single output identified by its name.
/// Output names can be obtained by querying the Wayland compositor.
///
/// # Arguments
///
/// * `name` - The name of the output to capture
/// * `include_cursor` - Whether to include the mouse cursor in the capture
///
/// # Returns
///
/// Returns a [`Capture`] containing the image of the specified output.
///
/// # Errors
///
/// Returns an error if:
/// - The specified output is not found
/// - Connection to Wayland server fails
/// - Event dispatch fails
pub fn capture_output(name: &str, include_cursor: bool) -> Result<Capture, Error> {
    let (mut state, mut event_queue) = backend::connect_and_get_output_info()?;
    inner_capture_output(&mut state, &mut event_queue, name, include_cursor)
}

/// Internal function to capture a specific output by name.
///
/// This function filters outputs by name and performs the capture.
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

/// Captures a specific output repeatedly at a specified frame rate.
///
/// This function continuously captures the specified output and calls the provided
/// callback with each captured frame. The capture loop can be cancelled using the
/// `cancel_sig` in the configuration.
///
/// # Arguments
///
/// * `config` - Configuration for the iterative capture
/// * `cb` - Callback function that receives frame index, capture data, and capture duration
///
/// # Returns
///
/// Returns a [`CaptureIterStatus`] indicating whether the iteration finished normally
/// or was cancelled.
///
/// # Errors
///
/// Returns an error if:
/// - The specified output is not found
/// - Connection to Wayland server fails
/// - Event dispatch fails
///
/// # Example
///
/// ```no_run
/// use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
/// use lib::capture::{CaptureIterConfig, CaptureIterStatus};
///
/// let cancel_sig = Arc::new(AtomicBool::new(false));
/// let config = CaptureIterConfig {
///     name: "eDP-1".to_string(),
///     include_cursor: true,
///     fps: Some(30.0),
///     cancel_sig: cancel_sig.clone(),
/// };
///
/// let result = capture_output_iter(config, |data| {
///     println!("Frame {} captured in {:?}", data.frame_index, data.capture_time);
///     // Process the capture...
/// });
///
/// match result {
///     Ok(CaptureIterStatus::Stopped) => println!("Capture Stopped"),
///     Ok(CaptureIterStatus::Finished) => println!("Capture finished"),
///     Err(e) => eprintln!("Capture error: {}", e),
/// }
/// ```
pub fn capture_output_iter(
    config: CaptureIterConfig,
    mut cb: impl FnMut(CaptureIterCallbackData),
) -> Result<CaptureIterStatus, Error> {
    let (mut state, mut event_queue) = backend::connect_and_get_output_info()?;

    let mut index = 0;
    // Calculate frame interval if FPS is specified
    let interval_ms = config.fps.map(|v| 1000.0 / v);
    let sleeper = SpinSleeper::default();
    let start_time = std::time::Instant::now();

    // Main capture loop
    loop {
        // Check for cancellation signal
        if config.cancel_sig.load(Ordering::Relaxed) {
            log::info!("Exit capture_iter process after Stopped");
            drop(state);
            return Ok(CaptureIterStatus::Stopped);
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
        cb(CaptureIterCallbackData {
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

/// Captures a specific region of an output.
///
/// This function captures a rectangular region of a specific output.
/// The region coordinates and size are specified in logical pixels.
///
/// # Arguments
///
/// * `name` - The name of the output to capture from
/// * `region_position` - The top-left position of the capture region in logical pixels
/// * `region_size` - The size of the capture region in logical pixels
/// * `include_cursor` - Whether to include the mouse cursor in the capture
///
/// # Returns
///
/// Returns a [`Capture`] containing the image of the specified region.
///
/// # Errors
///
/// Returns an error if:
/// - The specified output is not found
/// - The scale factor is zero
/// - Connection to Wayland server fails
/// - Event dispatch fails
///
/// # Example
///
/// ```no_run
/// use lib::capture::{capture_region, Position, LogicalSize};
///
/// let position = Position::new(100, 100);
/// let size = LogicalSize::new(800, 600);
/// let capture = capture_region("eDP-1", position, size, false).unwrap();
/// println!("Captured region: {}x{}", capture.width, capture.height);
/// ```
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

    // Capture each intersecting output
    for (i, output_info) in state.output_infos.iter_mut().enumerate() {
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

        break;
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

/// Converts captured output information into a buffer.
///
/// This internal function takes the captured output information and converts it
/// into a [`Capture`] structure containing the pixel data.
///
/// # Arguments
///
/// * `output_infos` - Vector of output information from the capture
///
/// # Returns
///
/// Returns a [`Capture`] containing the pixel data from the first output.
///
/// # Errors
///
/// Returns an error if no output information is provided.
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

/// Measure the average capture time for a specific output.
///
/// This function performs multiple captures of the first available output and
/// calculates the average time taken per capture. Useful for performance testing.
///
/// # Arguments
///
/// * `counts` - Number of captures to perform for timing measurement
///
/// # Returns
///
/// Returns `Some(Duration)` with the average capture time, or `None` if:
/// - No outputs are available
/// - `counts` is zero
/// - Connection to Wayland server fails
///
/// # Example
///
/// ```no_run
/// use lib::capture::capture_output_mean_time;
///
/// if let Some(avg_time) = capture_mean_time("eDP-1", 10) {
///     println!("Average capture time: {:?}", avg_time);
/// } else {
///     println!("Failed to measure capture time");
/// }
/// ```
pub fn capture_mean_time(screen_name: &str, counts: u32) -> Result<Duration, Error> {
    assert!(counts > 0);

    let screen_infos = available_screens()?;
    if screen_infos.is_empty() {
        return Err(crate::Error::NoOutput(
            "available screen no found".to_string(),
        ));
    }

    if screen_infos
        .iter()
        .find(|item| item.name == screen_name)
        .is_none()
    {
        return Err(crate::Error::NoOutput(format!(
            "{screen_name} is not in available screen list"
        )));
    }

    let start = Instant::now();
    for _ in 0..counts {
        _ = capture_output(screen_name, true);
    }
    Ok(start.elapsed() / counts)
}
