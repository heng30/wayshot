//! A screen capture library for Wayland compositors.
//!
//! This library provides functionality to capture screens, specific outputs, or regions
//! from Wayland compositors using the `wlr-screencopy-unstable-v1` protocol.
//!
//! # Overview
//!
//! The library supports three main capture modes:
//! - **Full screen capture**: Capture all connected outputs as a single composited image
//! - **Output-specific capture**: Capture a specific output by name
//! - **Region capture**: Capture a specific region of an output
//! - **Iterative capture**: Continuously capture frames at a specified frame rate
//!
//! # Dependencies
//!
//! - `wayland-client` - Wayland client library
//! - `wayland-protocols-wlr` - WLR protocols for screen capture
//! - `image` - Image processing and saving
//! - `serde` - Serialization for screen information
//!
//! # Examples
//!
//! ## Capture all outputs
//! ```no_run
//! use lib::capture::capture_all_outputs;
//!
//! let capture = capture_all_outputs(true).unwrap();
//! println!("Captured image: {}x{}", capture.width, capture.height);
//! ```
//!
//! ## Capture specific output
//! ```no_run
//! use lib::capture::capture_output;
//!
//! let capture = capture_output("eDP-1", false).unwrap();
//! println!("Captured output: {}x{}", capture.width, capture.height);
//! ```
//!
//! ## Get available screens
//! ```no_run
//! use lib::capture::available_screens;
//!
//! let screens = available_screens().unwrap();
//! for screen in screens {
//!     println!("Screen: {} at ({}, {})", screen.name, screen.position.x, screen.position.y);
//! }
//! ```

use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

mod backend;
mod capture;
mod capture_error;
mod screen_info;

pub use capture::*;
pub use capture_error::*;
pub use screen_info::*;

/// Represents a captured screen image.
///
/// This struct contains the pixel data and dimensions of a captured screen area.
/// The pixel data is stored in RGBA format with 8 bits per channel.
///
/// # Example
///
/// ```no_run
/// use lib::capture::{Capture, capture_output};
///
/// let capture = capture_output("eDP-1", false).unwrap();
/// println!("Image size: {}x{}", capture.width, capture.height);
/// println!("Pixel data length: {} bytes", capture.pixel_data.len());
/// ```
#[derive(Debug, Clone)]
pub struct Capture {
    /// Width of the captured image in pixels
    pub width: u32,
    /// Height of the captured image in pixels
    pub height: u32,
    /// Raw pixel data in RGBA format
    ///
    /// The pixel data is stored as a contiguous array of bytes in RGBA format.
    /// Each pixel occupies 4 bytes: red, green, blue, and alpha channels.
    /// The data is stored row-major, starting from the top-left corner.
    pub pixel_data: Vec<u8>,
}

/// Status of an iterative capture operation.
///
/// This enum indicates whether an iterative capture operation completed normally
/// or was stopped by user request.
#[derive(Debug)]
pub enum CaptureIterStatus {
    /// The capture iteration completed normally
    Finished,
    /// The capture iteration was stopped by user request
    Stopped,
}

/// Configuration for iterative screen capture operations.
///
/// This struct contains all the parameters needed to configure a continuous
/// screen capture operation that runs at a specified frame rate.
///
/// # Example
///
/// ```no_run
/// use std::sync::{Arc, atomic::AtomicBool};
/// use lib::capture::CaptureIterConfig;
///
/// let cancel_sig = Arc::new(AtomicBool::new(false));
/// let config = CaptureIterConfig {
///     name: "eDP-1".to_string(),
///     include_cursor: true,
///     fps: Some(30.0),
///     cancel_sig: cancel_sig.clone(),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct CaptureIterConfig {
    /// Name of the output to capture
    pub name: String,
    /// Whether to include the mouse cursor in the capture
    pub include_cursor: bool,
    /// Target frames per second for capture (None for maximum speed)
    pub fps: Option<f64>,
    /// Cancellation signal - when set to true, the capture loop will exit
    pub cancel_sig: Arc<AtomicBool>,
}

/// Callback data for iterative screen capture operations.
///
/// This struct contains information about each captured frame during
/// an iterative capture operation.
///
/// # Example
///
/// ```no_run
/// use lib::capture::{CaptureIterCallbackData, CaptureIterConfig};
///
/// let config = CaptureIterConfig { /* ... */ };
/// capture_output_iter(config, |data: CaptureIterCallbackData| {
///     println!("Frame {} captured in {:?}", data.frame_index, data.capture_time);
/// });
/// ```
#[derive(Debug, Clone)]
pub struct CaptureIterCallbackData {
    /// Index of the captured frame (starts from 0)
    pub frame_index: u64,
    /// Time taken to capture this frame
    pub capture_time: Duration,
    /// Total elapsed time since the capture started
    pub elapse: Duration,
    /// The captured image data
    pub data: Capture,
}
