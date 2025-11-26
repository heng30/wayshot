use screen_capture::{CursorError, CursorPosition, MonitorCursorPositionConfig, Position};
use std::{sync::atomic::Ordering, thread, time::Duration};
use winapi::{shared::windef::POINT, um::winuser::GetCursorPos};

pub fn monitor_cursor_position(
    config: MonitorCursorPositionConfig,
    mut callback: impl FnMut(CursorPosition) + Send + 'static,
) -> Result<(), CursorError> {
    let mut last_position = Position { x: -1, y: -1 };

    loop {
        if config.stop_sig.load(Ordering::Relaxed) {
            log::info!("exit monitor_cursor_position thread...");
            break;
        }

        let mut point = POINT { x: -1, y: -1 };
        unsafe {
            if GetCursorPos(&mut point as *mut POINT) == 0 {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
        }

        let current_position = Position {
            x: point.x as i32,
            y: point.y as i32,
            // x: (point.x as f32 * config.screen_info.scale_factor) as i32,
            // y: (point.y as f32 * config.screen_info.scale_factor) as i32,
        };

        if current_position != last_position {
            last_position = current_position;

            let cursor_pos = CursorPosition {
                x: current_position.x,
                y: current_position.y,
                output_x: config.screen_info.position.x,
                output_y: config.screen_info.position.y,
                output_width: config.screen_info.logical_size.width,
                output_height: config.screen_info.logical_size.height,
            };

            callback(cursor_pos);
        }

        thread::sleep(Duration::from_millis(5));
    }

    Ok(())
}
