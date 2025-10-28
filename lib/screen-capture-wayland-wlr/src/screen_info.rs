use crate::backend::{self, OutputInfo};
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
            protocol_get()
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

fn protocol_get() -> Result<Vec<ScreenInfo>, ScreenInfoError> {
    let (state, _) = backend::connect_and_get_output_info()
        .map_err(|e| ScreenInfoError::Other(e.to_string()))?;
    let mut infos = vec![];

    for output_info in state.output_infos.iter() {
        if let OutputInfo {
            name: Some(name),
            output_logical_position: Some(output_logical_position),
            output_logical_size: Some(output_logical_size),
            transform: Some(transform),
            scale_factor,
            ..
        } = output_info
        {
            infos.push(ScreenInfo {
                name: name.clone(),
                position: Position {
                    x: output_logical_position.x,
                    y: output_logical_position.y,
                },
                logical_size: LogicalSize {
                    width: output_logical_size.width,
                    height: output_logical_size.height,
                },
                physical_size: None,
                scale_factor: *scale_factor as f32,
                transform: (*transform).into(),
            });
        }
    }

    Ok(infos)
}
