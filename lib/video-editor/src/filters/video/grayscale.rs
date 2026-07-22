use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::Rgba;
use rayon::prelude::*;

use super::LuminanceStandard;

/// Grayscale filter with keyframe support
///
/// This filter converts an image to grayscale with adjustable intensity,
/// luminance standard, and contrast enhancement.
///
/// # Parameters
/// - `intensity`: Grayscale intensity (0.0-1.0), 0 = original color, 1 = full grayscale
/// - `luminance_standard`: Standard for calculating luminance (BT709, BT601, BT2020)
/// - `contrast`: Contrast enhancement (-1.0 to 1.0), 0 = no change
#[derive(
    Debug,
    Clone,
    derivative::Derivative,
    derive_setters::Setters,
    serde::Serialize,
    serde::Deserialize,
)]
#[derivative(Default)]
#[serde(default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GrayscaleFilter {
    /// Grayscale intensity (0.0-1.0). 0.0 = original color, 1.0 = full grayscale.
    #[derivative(Default(value = "1.0"))]
    pub intensity: f32,
    /// Luminance calculation standard.
    #[derivative(Default)]
    pub luminance_standard: LuminanceStandard,
    /// Contrast enhancement (-1.0 to 1.0). 0.0 = no change.
    #[derivative(Default(value = "0.0"))]
    pub contrast: f32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl GrayscaleFilter {
    pub const NAME: &'static str = "grayscale";

    pub fn new(intensity: f32) -> Self {
        Self {
            intensity: intensity.clamp(0.0, 1.0),
            luminance_standard: LuminanceStandard::default(),
            contrast: 0.0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("intensity", "Intensity", 0.0, 1.0, 1.0),
            AnimatableProperty::float("contrast", "Contrast", -1.0, 1.0, 0.0),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    /// Apply contrast adjustment to a luminance value.
    /// contrast: -1.0 to 1.0
    /// -1.0 = minimum contrast (all gray)
    /// 0.0 = no change
    /// 1.0 = maximum contrast (black or white)
    #[inline]
    fn apply_contrast(luminance: f32, contrast: f32) -> f32 {
        // Apply contrast using a sigmoid-like curve
        // At contrast = 0, returns luminance unchanged
        // At contrast = 1, pushes values toward 0 or 1
        // At contrast = -1, pushes values toward 0.5 (gray)
        if contrast.abs() < 0.001 {
            return luminance;
        }

        if contrast > 0.0 {
            // Increase contrast: push away from 0.5
            let factor = 1.0 + contrast;
            ((luminance - 0.5) * factor + 0.5).clamp(0.0, 1.0)
        } else {
            // Decrease contrast: push toward 0.5
            let factor = 1.0 + contrast.abs();
            0.5 + (luminance - 0.5) / factor
        }
    }

    #[inline]
    fn apply_pixel(
        pixel: &mut Rgba<u8>,
        intensity: f32,
        contrast: f32,
        luminance_standard: LuminanceStandard,
    ) {
        let r = pixel.0[0] as f32 / 255.0;
        let g = pixel.0[1] as f32 / 255.0;
        let b = pixel.0[2] as f32 / 255.0;

        // Calculate luminance based on standard
        let luminance = luminance_standard.calculate_luminance(r, g, b);

        // Apply contrast to the grayscale value
        let gray = Self::apply_contrast(luminance, contrast);

        // Blend original color with grayscale based on intensity
        pixel.0[0] = ((r * (1.0 - intensity) + gray * intensity) * 255.0).clamp(0.0, 255.0) as u8;
        pixel.0[1] = ((g * (1.0 - intensity) + gray * intensity) * 255.0).clamp(0.0, 255.0) as u8;
        pixel.0[2] = ((b * (1.0 - intensity) + gray * intensity) * 255.0).clamp(0.0, 255.0) as u8;
    }
}

impl VideoFilter for GrayscaleFilter {
    crate::impl_default_video_filter!(GrayscaleFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Get interpolated values at current time
        let intensity = self
            .get_value_at_time(time_ms, "intensity", self.intensity)
            .clamp(0.0, 1.0);
        let contrast = self
            .get_value_at_time(time_ms, "contrast", self.contrast)
            .clamp(-1.0, 1.0);

        // If intensity is near zero, skip processing (original color)
        if intensity < 0.001 {
            return Ok(());
        }

        let luminance_standard = self.luminance_standard;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                buffer.par_pixels_mut().for_each(|pixel| {
                    Self::apply_pixel(pixel, intensity, contrast, luminance_standard);
                });
            }
        }

        Ok(())
    }

    fn get_animatable_properties(&self) -> Vec<AnimatableProperty> {
        Self::animatable_properties()
    }

    fn get_keyframe_tracks(&self) -> KeyframeTracks {
        self.keyframe_tracks.clone()
    }

    fn set_keyframe_tracks(&mut self, tracks: KeyframeTracks) {
        self.keyframe_tracks = tracks;
    }

    fn supports_keyframes(&self) -> bool {
        true
    }

    fn update_keyframes_at_time(&self, tracks: &mut KeyframeTracks, time_ms: i64) -> bool {
        let mut updated = false;

        for (property, value) in [("intensity", self.intensity), ("contrast", self.contrast)] {
            if let Some(track) = tracks.get_track(property)
                && track.keyframes.iter().any(|k| k.time_ms == time_ms)
            {
                tracks.update_keyframe_value(property, time_ms, KeyframeValue::Float(value));
                updated = true;
            }
        }

        updated
    }
}

