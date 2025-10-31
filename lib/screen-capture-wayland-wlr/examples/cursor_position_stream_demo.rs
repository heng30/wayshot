use screen_capture_wayland_wlr::monitor_cursor_position;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let stop_sig = Arc::new(AtomicBool::new(false));
    let stop_sig_clone = stop_sig.clone();

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60));
        log::info!("60 seconds elapsed, stopping recording...");
        stop_sig.store(true, Ordering::Relaxed);
    });

    monitor_cursor_position(stop_sig_clone, move |position| {
        log::info!("Current mouse position: x={}, y={}", position.x, position.y);
        log::info!(
            "Output dimensions: {}x{} at ({}, {})",
            position.output_width,
            position.output_height,
            position.output_x,
            position.output_y
        );
    });

    Ok(())
}
