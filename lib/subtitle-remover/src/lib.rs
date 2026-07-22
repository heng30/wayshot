//! # subtitle-remover
//!
//! A Rust library for removing subtitles from images.
//!
//! Provides core pipeline: create mask from bounding boxes → inpaint → composite result.
//! Uses LaMa ONNX-based inpainting backend.

pub mod config;
pub mod inpaint;
pub mod mask;

// Re-export primary types
pub use config::{Config, LAMA_DOWNLOAD_URL};
pub use inpaint::{InpaintError, Inpainter, LamaInpainter, Mask};
pub use mask::{BBox, InpaintArea, batch_generator, create_mask, expand_frame_ranges, get_inpaint_area_by_mask,
    is_frame_in_ab_sections};
