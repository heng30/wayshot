//! turn-rs: A Rust library for generating page flip effects.
//!
//! Inspired by [turn.js](https://github.com/blasten/turn.js), this library
//! renders realistic page flip animations.
//!
//! # Core API
//!
//! ```rust
//! use image::{Rgba, RgbaImage};
//! use turn_rs::{Corner, FlipAxis, FlipConfig, FlipDirection, render_flip};
//!
//! let w = 400u32;
//! let h = 600u32;
//! let front = RgbaImage::from_pixel(w, h, Rgba([70, 130, 220, 255]));
//! let back  = RgbaImage::from_pixel(w, h, Rgba([80, 180, 80, 255]));
//!
//! let config = FlipConfig {
//!     corner: Corner::BottomRight,
//!     duration_ms: 800,
//!     time_ms: 400,      // halfway through the animation
//!     shadow: true,
//!     direction: FlipDirection::Forward,
//!     axis: FlipAxis::Horizontal,
//!     flip_extent: 1.0,  // full flip (0.0–1.0, default 1.0)
//! };
//!
//! let frame = render_flip(&front, &back, &config);
//! ```
//!
//! # Feature: `animation`
//!
//! Enable the `animation` feature to generate animated WebP files:
//!
//! ```toml
//! turn-rs = { version = "0.1", features = ["animation"] }
//! ```

pub mod flip;
pub mod render;

#[cfg(feature = "animation")]
pub mod animation;

pub use flip::Corner;
pub use render::{FlipAxis, FlipConfig, FlipDirection, render_flip};

#[cfg(feature = "animation")]
pub use animation::{FlipError, generate_flip, generate_flip_to_pngs, generate_flip_to_webp};
