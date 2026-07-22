use rayon::prelude::*;

use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};

/// Edge detection filter using Sobel operator.
/// Detects edges in the image and can output them as black lines on white background
/// (sketch mode) or white lines on black background (inverted mode).
#[derive(
    Debug,
    Clone,
    derivative::Derivative,
    derive_setters::Setters,
    serde::Serialize,
    serde::Deserialize,
)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[serde(default)]
#[non_exhaustive]
pub struct EdgeDetectFilter {
    /// Edge threshold (0-255). Higher values show fewer edges.
    #[derivative(Default(value = "30.0"))]
    pub threshold: f32,
    /// Edge intensity/strength (0.0-2.0). Controls how strong edges appear.
    #[derivative(Default(value = "1.0"))]
    pub strength: f32,
    /// Invert edge colors: true = white edges on black, false = black edges on white.
    #[derivative(Default(value = "false"))]
    pub invert: bool,
    /// Edge color (RGBA) for the detected edges.
    #[derivative(Default(value = "[0, 0, 0, 255] "))]
    pub edge_color: [u8; 4],
    /// Background color (RGBA) for non-edge areas.
    #[derivative(Default(value = "[255, 255, 255, 255]"))]
    pub background_color: [u8; 4],
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl EdgeDetectFilter {
    pub const NAME: &'static str = "edge detect";

    pub fn new(threshold: f32, strength: f32) -> Self {
        Self {
            threshold: threshold.clamp(0.0, 255.0),
            strength: strength.clamp(0.0, 2.0),
            invert: false,
            edge_color: [0, 0, 0, 255],
            background_color: [255, 255, 255, 255],
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("threshold", "Threshold", 0.0, 255.0, 30.0),
            AnimatableProperty::float("strength", "Strength", 0.0, 2.0, 1.0),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    /// Sobel kernels for gradient calculation
    const SOBEL_GX: [[f32; 3]; 3] = [[-1.0, 0.0, 1.0], [-2.0, 0.0, 2.0], [-1.0, 0.0, 1.0]];

    const SOBEL_GY: [[f32; 3]; 3] = [[-1.0, -2.0, -1.0], [0.0, 0.0, 0.0], [1.0, 2.0, 1.0]];

    /// Calculate edge magnitude for a pixel using Sobel operator.
    /// Returns the gradient magnitude normalized to 0-255 range.
    #[inline]
    fn calculate_edge_magnitude(
        image: &image::RgbaImage,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> f32 {
        let mut gx = 0.0;
        let mut gy = 0.0;

        // Apply Sobel kernels
        for ky in 0..3 {
            for kx in 0..3 {
                let px = (x as i32 + kx as i32 - 1).clamp(0, width as i32 - 1) as u32;
                let py = (y as i32 + ky as i32 - 1).clamp(0, height as i32 - 1) as u32;

                let pixel = image.get_pixel(px, py);
                // Convert to grayscale luminance
                let luminance = pixel.0[0] as f32 * 0.2126
                    + pixel.0[1] as f32 * 0.7152
                    + pixel.0[2] as f32 * 0.0722;

                gx += luminance * Self::SOBEL_GX[ky][kx];
                gy += luminance * Self::SOBEL_GY[ky][kx];
            }
        }

        // Calculate gradient magnitude
        (gx * gx + gy * gy).sqrt()
    }

    /// Apply edge detection to an image buffer (parallelized).
    fn apply_edge_detection(
        buffer: &mut image::RgbaImage,
        threshold: f32,
        strength: f32,
        invert: bool,
        edge_color: [u8; 4],
        background_color: [u8; 4],
    ) {
        let width = buffer.width();
        let height = buffer.height();

        // First, calculate edge magnitudes for all pixels (parallelized by row)
        let edge_magnitudes_rows: Vec<Vec<f32>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| Self::calculate_edge_magnitude(buffer, x, y, width, height))
                    .collect()
            })
            .collect();
        let edge_magnitudes: Vec<f32> = edge_magnitudes_rows.into_iter().flatten().collect();

        // Then apply edge detection using the calculated magnitudes (parallelized by row)
        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .enumerate()
            .map(|(y_idx, y)| {
                (0..width)
                    .enumerate()
                    .map(|(x_idx, x)| {
                        let idx = y_idx * width as usize + x_idx;
                        let magnitude = edge_magnitudes[idx];

                        // Apply strength to edge magnitude
                        let adjusted_magnitude = magnitude * strength;

                        // Determine if this pixel is an edge
                        let is_edge = adjusted_magnitude > threshold;

                        // Choose colors based on invert setting
                        let (fg_color, bg_color) = if invert {
                            (background_color, edge_color) // White edges on black
                        } else {
                            (edge_color, background_color) // Black edges on white
                        };

                        if is_edge {
                            // Calculate edge intensity for smooth blending
                            let edge_intensity =
                                ((adjusted_magnitude - threshold) / 100.0).min(1.0);

                            // Blend edge color with background for softer edges
                            let r = ((bg_color[0] as f32 * (1.0 - edge_intensity)
                                + fg_color[0] as f32 * edge_intensity)
                                .clamp(0.0, 255.0)) as u8;
                            let g = ((bg_color[1] as f32 * (1.0 - edge_intensity)
                                + fg_color[1] as f32 * edge_intensity)
                                .clamp(0.0, 255.0)) as u8;
                            let b = ((bg_color[2] as f32 * (1.0 - edge_intensity)
                                + fg_color[2] as f32 * edge_intensity)
                                .clamp(0.0, 255.0)) as u8;

                            // Get original alpha from source image
                            let a = buffer.get_pixel(x, y).0[3];

                            [r, g, b, a]
                        } else {
                            // Non-edge area: use background color, preserve alpha
                            let a = buffer.get_pixel(x, y).0[3];
                            [bg_color[0], bg_color[1], bg_color[2], a]
                        }
                    })
                    .collect()
            })
            .collect();

        // Flatten and rebuild image
        let pixels: Vec<u8> = rows.into_iter().flatten().flatten().collect();
        *buffer = image::RgbaImage::from_raw(width, height, pixels)
            .expect("Buffer size matches image dimensions");
    }
}

impl VideoFilter for EdgeDetectFilter {
    crate::impl_default_video_filter!(EdgeDetectFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        let threshold = self
            .get_value_at_time(time_ms, "threshold", self.threshold)
            .clamp(0.0, 255.0);
        let strength = self
            .get_value_at_time(time_ms, "strength", self.strength)
            .clamp(0.0, 2.0);

        let invert = self.invert;
        let edge_color = self.edge_color;
        let background_color = self.background_color;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_edge_detection(
                    buffer,
                    threshold,
                    strength,
                    invert,
                    edge_color,
                    background_color,
                );
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

        for (property, value) in [("threshold", self.threshold), ("strength", self.strength)] {
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
