use serde::Deserialize;

#[derive(thiserror::Error, Debug)]
pub enum ScreenInfoError {
    #[error("{0}")]
    Command(String),

    #[error("{0}")]
    Unimplemented(String),

    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Default)]
pub struct ScreenInfo {
    /// Name of the output (e.g., "eDP-1", "HDMI-A-1")
    pub name: String,

    /// Logical position of the output in compositor space
    pub position: Position,

    /// Logical size of the output in pixels
    pub logical_size: LogicalSize,

    /// Physical size of the output in millimeters, if available
    pub physical_size: Option<PhysicalSize>,

    /// Output transformation (rotation, flipping)
    pub transform: Transform,

    /// Scale factor of the output
    pub scale_factor: f32,
}

#[derive(Debug, Clone, Default, Deserialize, Copy, PartialEq, PartialOrd)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Copy, PartialEq, PartialOrd)]
pub struct LogicalSize {
    pub width: i32,
    pub height: i32,
}

impl LogicalSize {
    pub fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Copy, PartialEq, PartialOrd)]
pub struct PhysicalSize {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Deserialize, Copy, PartialEq, PartialOrd)]
pub enum Transform {
    Normal,
    _90,
    _180,
    _270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

impl Default for Transform {
    fn default() -> Self {
        Self::Normal
    }
}

#[cfg(all(target_os = "linux", feature = "wayland"))]
impl From<wayland_client::protocol::wl_output::Transform> for Transform {
    fn from(value: wayland_client::protocol::wl_output::Transform) -> Self {
        use wayland_client::protocol::wl_output::Transform as WlTransform;

        match value {
            WlTransform::Normal => Self::Normal,
            WlTransform::_90 => Self::_90,
            WlTransform::_180 => Self::_180,
            WlTransform::_270 => Self::_270,
            WlTransform::Flipped => Self::Flipped,
            WlTransform::Flipped90 => Self::Flipped90,
            WlTransform::Flipped180 => Self::Flipped180,
            WlTransform::Flipped270 => Self::Flipped270,
            _ => Self::Normal,
        }
    }
}
