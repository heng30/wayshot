use crate::ScreenInfo;
use std::sync::{Arc, atomic::AtomicBool};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CursorError {
    #[error("Failed to connect to Wayland display: {0}")]
    ConnectionFailed(String),

    #[error("Required Wayland protocol not available: {0}")]
    ProtocolNotAvailable(String),

    #[error("Failed to get pointer: {0}")]
    PointerFailed(String),

    #[error("Failed to get configurations from Wayland server: {0}")]
    ConfigurationFailed(String),

    #[cfg(feature = "wayland")]
    #[error("Dispatch Failed: {0}")]
    DispatchFailed(#[from] wayland_client::DispatchError),
}

#[derive(Debug, Clone, derive_setters::Setters)]
#[setters(prefix = "with_")]
pub struct MonitorCursorPositionConfig {
    #[setters(skip)]
    pub screen_info: ScreenInfo,
    pub stop_sig: Arc<AtomicBool>,

    pub use_transparent_layer_surface: bool,
    pub hole_radius: i32,
}

impl MonitorCursorPositionConfig {
    pub fn new(screen_info: ScreenInfo, stop_sig: Arc<AtomicBool>) -> Self {
        Self {
            screen_info,
            use_transparent_layer_surface: true,
            hole_radius: 50,
            stop_sig,
        }
    }
}
