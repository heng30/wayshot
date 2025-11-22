use display_info::DisplayInfo;
use screen_capture::{LogicalSize, Position, ScreenInfo, ScreenInfoError, Transform};

pub fn available_screens() -> Result<Vec<ScreenInfo>, ScreenInfoError> {
    display_get()
}

fn display_get() -> Result<Vec<ScreenInfo>, ScreenInfoError> {
    let mut screens = Vec::new();
    let displays = DisplayInfo::all()
        .map_err(|e| ScreenInfoError::Other(format!("Failed to get display info: {e}")))?;

    for display in displays {
        let screen_info = ScreenInfo {
            name: display.name,
            position: Position {
                x: display.x,
                y: display.y,
            },
            logical_size: LogicalSize {
                width: display.width as i32,
                height: display.height as i32,
            },
            physical_size: None,
            transform: match display.rotation as i32 {
                0 => Transform::Normal,
                90 => Transform::_90,
                180 => Transform::_180,
                270 => Transform::_270,
                _ => Transform::Normal,
            },
            scale_factor: display.scale_factor as f32,
        };

        screens.push(screen_info);
    }

    if screens.is_empty() {
        return Err(ScreenInfoError::Other("No displays found".to_string()));
    }

    Ok(screens)
}
