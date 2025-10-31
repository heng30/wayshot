use screen_capture_wayland_wlr::get_cursor_position;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    match get_cursor_position() {
        Ok(Some(position)) => {
            log::info!("Current mouse position: x={}, y={}", position.x, position.y);
            log::info!(
                "Output dimensions: {}x{} at ({}, {})",
                position.output_width,
                position.output_height,
                position.output_x,
                position.output_y
            );
        }
        Err(e) => log::warn!("{e}"),
        _ => log::warn!("No mouse position"),
    }

    Ok(())
}
