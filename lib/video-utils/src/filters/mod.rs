//! Video and audio filters
//!
//! This module provides various filter operations for video and audio processing.

pub mod scale;
pub mod transform;
pub mod fade;
pub mod crop;
pub mod color;
pub mod crossfade;
pub mod text_overlay;

pub use scale::{scale_video, ScaleConfig, ScaleQuality, scale_to_fit, scale_to_exact};
pub use transform::{rotate_video, flip_video, RotateAngle, FlipDirection,
    rotate_90, rotate_180, flip_horizontal, flip_vertical};
pub use fade::{fade_video, FadeConfig, FadeType, fade_in, fade_out};
pub use crop::{crop_video, CropConfig, CropMode, crop_center, crop_to_aspect};
pub use color::{adjust_color, ColorAdjustConfig, adjust_brightness, adjust_contrast, adjust_saturation};
pub use crossfade::{crossfade_videos, CrossfadeConfig};
pub use text_overlay::{text_overlay, TextOverlayConfig, TextPosition, TextAlignment, add_watermark, add_title};
