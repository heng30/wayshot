use screen_capture::MonitorCursorPositionConfig;
use screen_capture_wayland_wlr::{available_screens, monitor_cursor_position};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Get available screens and use the first one for demo
    let screens = available_screens()?;
    if screens.is_empty() {
        log::warn!("No screens available");
        return Ok(());
    }

    let target_screen = screens[0].clone();
    log::info!(
        "Using screen: {} ({}x{} at ({}, {}))",
        target_screen.name,
        target_screen.logical_size.width,
        target_screen.logical_size.height,
        target_screen.position.x,
        target_screen.position.y
    );

    let stop_sig = Arc::new(AtomicBool::new(false));
    let stop_sig_clone = stop_sig.clone();

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60));
        log::info!("60 seconds elapsed, stopping recording...");
        stop_sig.store(true, Ordering::Relaxed);
    });

    let config = MonitorCursorPositionConfig::new(target_screen, stop_sig_clone)
        .with_use_transparent_layer_surface(false)
        .with_hole_radius(50);

    if let Err(e) = monitor_cursor_position(config, move |position| {
        log::info!(
            "dimensions: {}x{} at ({}, {}). (x, y) = ({}, {})",
            position.output_width,
            position.output_height,
            position.output_x,
            position.output_y,
            position.x,
            position.y
        );
    }) {
        log::error!("{e}");
    }

    Ok(())
}
