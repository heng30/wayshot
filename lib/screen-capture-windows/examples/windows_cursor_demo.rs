use screen_capture::{MonitorCursorPositionConfig, ScreenCapture};
use screen_capture_windows::ScreenCaptureWindows;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Windows Cursor Position Tracking Demo");
    println!("====================================");

    let mut screen_capture = ScreenCaptureWindows::default();

    let screens = screen_capture.available_screens()?;
    println!("Found {} screen(s)", screens.len());

    if screens.is_empty() {
        println!("No screens found!");
        return Ok(());
    }

    let screen = screens[0].clone();
    println!("Using screen: {}", screen.name);
    println!(
        "Screen position: ({}, {})",
        screen.position.x, screen.position.y
    );
    println!(
        "Screen size: {}x{}",
        screen.logical_size.width, screen.logical_size.height
    );

    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_signal_clone = stop_signal.clone();

    ctrlc::set_handler(move || {
        println!("\nStopping cursor tracking...");
        stop_signal_clone.store(true, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    let cursor_config = MonitorCursorPositionConfig::new(screen.clone(), stop_signal.clone());

    println!("\nStarting cursor position tracking...");
    println!("Move your mouse around. Press Ctrl+C to stop.\n");

    screen_capture.monitor_cursor_position(cursor_config, |cursor_pos| {
        println!(
            "Cursor: (x, y)=({:4}, {:4}) | ScreenOffset=({:4}, {:4}) | Screen={}x{}",
            cursor_pos.x,
            cursor_pos.y,
            cursor_pos.output_x,
            cursor_pos.output_y,
            cursor_pos.output_width,
            cursor_pos.output_height
        );
    })?;

    println!("Demo completed!");
    Ok(())
}
