use screen_capture::MonitorCursorPositionConfig;
use screen_capture_wayland_portal::{available_screens, monitor_cursor_position};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let screens = available_screens()?;
    if screens.is_empty() {
        log::warn!("No screens found!");
        return Ok(());
    }

    log::debug!("Available screens:");
    for (i, screen) in screens.iter().enumerate() {
        log::debug!(
            "{}: {} ({}x{} at {},{})",
            i,
            screen.name,
            screen.logical_size.width,
            screen.logical_size.height,
            screen.position.x,
            screen.position.y
        );
    }

    let screen_info = screens[0].clone();
    log::debug!("\nUsing screen: {}", screen_info.name);

    let stop_signal = std::sync::Arc::new(AtomicBool::new(false));
    let stop_signal_clone = stop_signal.clone();

    ctrlc::set_handler(move || {
        log::debug!("\nReceived Ctrl+C, stopping...");
        stop_signal_clone.store(true, Ordering::Relaxed);
    })?;

    log::debug!("Starting cursor position tracking... Move your mouse around!");
    log::debug!("Press Ctrl+C to stop.");

    let config = MonitorCursorPositionConfig::new(screen_info, stop_signal.clone());

    let position_count = std::sync::Arc::new(AtomicU64::new(0));
    let position_count_clone = position_count.clone();

    if let Err(e) = monitor_cursor_position(config, move |position| {
        let count = position_count_clone.fetch_add(1, Ordering::Relaxed) + 1;

        log::debug!(
            "Cursor #{}: x={}, y={} (output: {},{} size: {}x{})",
            count,
            position.x,
            position.y,
            position.output_x,
            position.output_y,
            position.output_width,
            position.output_height
        );
    }) {
        log::warn!("Cursor monitoring error: {}", e);
    }

    let total_positions = position_count.load(Ordering::Relaxed);
    log::debug!("Received {} cursor position updates.", total_positions);

    Ok(())
}
