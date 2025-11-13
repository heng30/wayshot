use display_info::DisplayInfo;
use screen_capture::{LogicalSize, PhysicalSize, Position, ScreenInfo, ScreenInfoError, Transform};
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct ScreenJson {
    name: String,
    physical_size: PhysicalSize,
    modes: Vec<Model>,
    scale: f32,
    enabled: bool,
    transform: String,
    position: Position,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Model {
    width: i32,
    height: i32,
    refresh: f32,
    preferred: bool,
    current: bool,
}

pub fn available_screens() -> Result<Vec<ScreenInfo>, ScreenInfoError> {
    match cmd_get() {
        Ok(info) => Ok(info),
        Err(e) => {
            log::warn!("{e}");
            display_get()
        }
    }
}

fn cmd_get() -> Result<Vec<ScreenInfo>, ScreenInfoError> {
    let exe = "wlr-randr";
    if which::which(exe).is_err() {
        return Err(ScreenInfoError::Command("wlr-randr no found".to_string()));
    }

    let output = Command::new(exe)
        .arg("--json")
        .output()
        .map_err(|e| ScreenInfoError::Command(format!("run wlr-randr failed: {}", e)))?;

    if !output.status.success() {
        return Err(ScreenInfoError::Command("run wlr-randr failed".to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let screen_list: Vec<ScreenJson> = serde_json::from_str(&stdout)
        .map_err(|e| ScreenInfoError::Other(format!("parsr json failed: {}", e)))?;

    let mut screens = vec![];
    for screen in screen_list {
        if !screen.enabled {
            continue;
        }
        for model in screen.modes {
            if !model.current {
                continue;
            }

            screens.push(ScreenInfo {
                name: screen.name.clone(),
                logical_size: LogicalSize {
                    width: model.width,
                    height: model.height,
                },
                physical_size: Some(screen.physical_size),
                scale_factor: screen.scale,
                position: screen.position.clone(),

                transform: match screen.transform.to_lowercase().as_str() {
                    "normal" => Transform::Normal,
                    "90" => Transform::_90,
                    "180" => Transform::_180,
                    "270" => Transform::_270,
                    "flipped" => Transform::Flipped,
                    "flipped-90" => Transform::Flipped90,
                    "flipped-180" => Transform::Flipped180,
                    "flipped-270" => Transform::Flipped270,
                    _ => {
                        return Err(ScreenInfoError::Other(format!(
                            "Unknow screent transform: {}",
                            screen.transform
                        )));
                    }
                },
            });

            break;
        }
    }

    if screens.is_empty() {
        return Err(ScreenInfoError::Other("can not find screen".to_string()));
    }

    Ok(screens)
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
