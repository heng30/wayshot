use crate::{
    Error,
    backend::{self, OutputInfo},
};
use serde::Deserialize;
use std::process::Command;
use wayland_client::protocol::wl_output::Transform;

/// Information about an available screen output.
///
/// This struct contains metadata about a connected display output,
/// including its name, position, size, and transformation properties.
///
/// # Example
///
/// ```no_run
/// use lib::capture::available_screens;
///
/// let screens = available_screens().unwrap();
/// for screen in screens {
///     println!("Screen: {}", screen.name);
///     println!("  Position: ({}, {})", screen.position.x, screen.position.y);
///     println!("  Size: {}x{}", screen.logical_size.width, screen.logical_size.height);
///     println!("  Scale: {}", screen.scale_factor);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ScreenInfo {
    /// Name of the output (e.g., "eDP-1", "HDMI-A-1")
    pub name: String,

    /// Logical position of the output in compositor space
    pub position: Position,

    /// Logical size of the output in pixels
    ///
    /// This represents the size of the output in logical pixels,
    /// which may differ from the physical size due to scaling.
    pub logical_size: LogicalSize,

    /// Physical size of the output in millimeters, if available
    pub physical_size: Option<PhysicalSize>,

    /// Output transformation (rotation, flipping)
    pub transform: Transform,

    /// Scale factor of the output
    ///
    /// This value represents how many physical pixels correspond to
    /// one logical pixel. For example, a scale factor of 2.0 means
    /// that each logical pixel is represented by 2x2 physical pixels.
    pub scale_factor: f32,
}

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

/// A position in 2D space, typically representing screen coordinates.
///
/// This struct represents a point in a 2D coordinate system with integer coordinates.
/// It's commonly used to represent the position of screens or capture regions.
///
/// # Example
///
/// ```no_run
/// use lib::capture::Position;
///
/// let pos = Position::new(100, 200);
/// println!("Position: ({}, {})", pos.x, pos.y);
/// ```
#[derive(Debug, Clone, Deserialize, Copy, PartialEq, PartialOrd)]
pub struct Position {
    /// X coordinate
    pub x: i32,
    /// Y coordinate
    pub y: i32,
}

impl Position {
    /// Creates a new position with the given coordinates.
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// A size in logical pixels.
///
/// This struct represents dimensions in logical pixels, which account for
/// display scaling. Logical pixels may differ from physical pixels based
/// on the display's scale factor.
///
/// # Example
///
/// ```no_run
/// use lib::capture::LogicalSize;
///
/// let size = LogicalSize::new(1920, 1080);
/// println!("Size: {}x{}", size.width, size.height);
/// ```
#[derive(Debug, Clone, Deserialize, Copy, PartialEq, PartialOrd)]
pub struct LogicalSize {
    /// Width in logical pixels
    pub width: i32,
    /// Height in logical pixels
    pub height: i32,
}

impl LogicalSize {
    /// Creates a new logical size with the given dimensions.
    pub fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }
}

/// A physical size in millimeters.
///
/// This struct represents the physical dimensions of a display output
/// in millimeters. This information is useful for calculating DPI/PPI.
///
/// # Example
///
/// ```no_run
/// use lib::capture::{ScreenInfo, available_screens};
///
/// let screens = available_screens().unwrap();
/// for screen in screens {
///     if let Some(physical_size) = screen.physical_size {
///         println!("Physical size: {}x{} mm", physical_size.width, physical_size.height);
///     }
/// }
/// ```
#[derive(Debug, Clone, Deserialize, Copy, PartialEq, PartialOrd)]
pub struct PhysicalSize {
    /// Width in millimeters
    pub width: i32,
    /// Height in millimeters
    pub height: i32,
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

/// Gets information about all available screen outputs.
///
/// This function queries the system for information about all connected
/// and enabled display outputs. It first attempts to use the `wlr-randr`
/// command-line tool for more accurate information, and falls back to
/// the Wayland protocol if `wlr-randr` is not available.
///
/// # Returns
///
/// Returns a vector of [`ScreenInfo`] structs containing information about
/// each available screen output.
///
/// # Errors
///
/// Returns an error if:
/// - No screens are found
/// - Connection to Wayland server fails
/// - Event dispatch fails
/// - JSON parsing fails when using `wlr-randr`
///
/// # Example
///
/// ```no_run
/// use lib::capture::available_screens;
///
/// let screens = available_screens().unwrap();
/// for screen in screens {
///     println!("Screen: {}", screen.name);
///     println!("  Position: ({}, {})", screen.position.x, screen.position.y);
///     println!("  Size: {}x{}", screen.logical_size.width, screen.logical_size.height);
///     println!("  Scale: {}", screen.scale_factor);
/// }
/// ```
pub fn available_screens() -> Result<Vec<ScreenInfo>, Error> {
    match cmd_get() {
        Ok(info) => Ok(info),
        Err(e) => {
            log::warn!("{e}");
            protocol_get()
        }
    }
}

fn cmd_get() -> Result<Vec<ScreenInfo>, Error> {
    let exe = "wlr-randr";
    if which::which(exe).is_err() {
        return Err(Error::Command("wlr-randr no found".to_string()));
    }

    let output = Command::new(exe)
        .arg("--json")
        .output()
        .map_err(|e| Error::Command(format!("run wlr-randr failed: {}", e)))?;

    if !output.status.success() {
        return Err(Error::Command("run wlr-randr failed".to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let screen_list: Vec<ScreenJson> = serde_json::from_str(&stdout)
        .map_err(|e| Error::Other(format!("parsr json failed: {}", e)))?;

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
                        return Err(Error::Other(format!(
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
        return Err(Error::Other("can not find screen".to_string()));
    }

    Ok(screens)
}

fn protocol_get() -> Result<Vec<ScreenInfo>, Error> {
    let (state, _) = backend::connect_and_get_output_info()?;
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
                transform: *transform,
            });
        }
    }

    Ok(infos)
}

#[cfg(test)]
mod test {
    use super::*;

    // cargo test test_avaiable_screens -- --no-capture
    #[test]
    fn test_avaiable_screens() -> Result<(), Error> {
        let infos = available_screens()?;
        println!("{:?}", infos);

        Ok(())
    }
}
