use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::RgbaImage;
use rayon::prelude::*;

/// Sketch/line art filter that converts video to pencil sketch style.
/// Uses a color dodge blend technique for realistic pencil sketch effect.
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
pub struct SketchFilter {
    /// Line intensity/darkness (0.0-1.0). Higher values create darker lines.
    #[derivative(Default(value = "0.8"))]
    pub line_intensity: f32,
    /// Line width/thickness via blur radius (1.0-10.0). Larger values create thicker lines.
    #[derivative(Default(value = "3.0"))]
    pub line_width: f32,
    /// Paper/background color (RGBA) - default white.
    #[derivative(Default(value = "[255, 255, 255, 255]"))]
    pub paper_color: [u8; 4],
    /// Pencil/line color (RGBA) - default graphite gray.
    #[derivative(Default(value = "[60, 60, 60, 255]"))]
    pub pencil_color: [u8; 4],
    /// Detail level/sensitivity (0.0-1.0). Higher values capture more fine details.
    #[derivative(Default(value = "0.5"))]
    pub detail_level: f32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl SketchFilter {
    pub const NAME: &'static str = "sketch";

    pub fn new(line_intensity: f32, line_width: f32) -> Self {
        Self {
            line_intensity: line_intensity.clamp(0.0, 1.0),
            line_width: line_width.clamp(1.0, 10.0),
            paper_color: [255, 255, 255, 255],
            pencil_color: [60, 60, 60, 255],
            detail_level: 0.5,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("line_intensity", "Line Intensity", 0.0, 1.0, 0.8),
            AnimatableProperty::float("line_width", "Line Width", 1.0, 10.0, 3.0),
            AnimatableProperty::float("detail_level", "Detail Level", 0.0, 1.0, 0.5),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    /// Convert image to grayscale.
    fn to_grayscale(image: &RgbaImage) -> Vec<f32> {
        image
            .pixels()
            .map(|p| p.0[0] as f32 * 0.2126 + p.0[1] as f32 * 0.7152 + p.0[2] as f32 * 0.0722)
            .collect()
    }

    /// Invert grayscale values.
    fn invert_grayscale(grayscale: &[f32]) -> Vec<f32> {
        grayscale.iter().map(|v| 255.0 - v).collect()
    }

    /// Generate a 1D Gaussian kernel for blur.
    fn generate_gaussian_kernel(radius: f32) -> Vec<f32> {
        let kernel_radius = radius.ceil() as usize;
        let kernel_size = 2 * kernel_radius + 1;

        let sigma = radius / 2.0;
        let sigma_sq = sigma * sigma;
        let two_sigma_sq = 2.0 * sigma_sq;

        let mut kernel = Vec::with_capacity(kernel_size);
        let mut sum = 0.0;

        for i in 0..kernel_size {
            let x = (i as isize - kernel_radius as isize) as f32;
            let weight = (-x * x / two_sigma_sq).exp();
            kernel.push(weight);
            sum += weight;
        }

        if sum > 0.0 {
            for weight in &mut kernel {
                *weight /= sum;
            }
        }

        kernel
    }

    /// Apply horizontal blur to grayscale data (parallelized by row).
    fn blur_horizontal(data: &[f32], width: u32, height: u32, kernel: &[f32]) -> Vec<f32> {
        let kernel_radius = kernel.len() / 2;
        let width_usize = width as usize;

        // Process rows in parallel
        let rows: Vec<Vec<f32>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let mut value = 0.0;

                        for (i, weight) in kernel.iter().enumerate() {
                            let px = (x as isize + i as isize - kernel_radius as isize)
                                .clamp(0, width as isize - 1)
                                as usize;
                            let idx = (y as usize) * width_usize + px;
                            value += data[idx] * weight;
                        }

                        value
                    })
                    .collect()
            })
            .collect();

        rows.into_iter().flatten().collect()
    }

    /// Apply vertical blur to grayscale data (parallelized by row).
    fn blur_vertical(data: &[f32], width: u32, height: u32, kernel: &[f32]) -> Vec<f32> {
        let kernel_radius = kernel.len() / 2;
        let width_usize = width as usize;

        // Process rows in parallel
        let rows: Vec<Vec<f32>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let mut value = 0.0;

                        for (i, weight) in kernel.iter().enumerate() {
                            let py = (y as isize + i as isize - kernel_radius as isize)
                                .clamp(0, height as isize - 1)
                                as usize;
                            let idx = py * width_usize + (x as usize);
                            value += data[idx] * weight;
                        }

                        value
                    })
                    .collect()
            })
            .collect();

        rows.into_iter().flatten().collect()
    }

    /// Apply Gaussian blur to grayscale data.
    fn apply_blur(data: &[f32], width: u32, height: u32, radius: f32) -> Vec<f32> {
        if radius <= 0.0 {
            return data.to_vec();
        }

        let kernel = Self::generate_gaussian_kernel(radius);
        let horizontal = Self::blur_horizontal(data, width, height, &kernel);
        Self::blur_vertical(&horizontal, width, height, &kernel)
    }

    /// Color dodge blend operation: grayscale / (256 - inverted_blur) * 256
    /// Creates the sketch effect where bright areas stay bright and dark areas become darker.
    fn color_dodge(grayscale: f32, inverted_blur: f32) -> f32 {
        // Avoid division by zero
        let divisor = 256.0 - inverted_blur;
        if divisor < 1.0 {
            return 255.0; // Very bright area
        }

        (grayscale / divisor) * 256.0
    }

    /// Apply sketch effect to an image buffer (parallelized by row).
    fn apply_sketch(
        buffer: &mut RgbaImage,
        line_intensity: f32,
        line_width: f32,
        paper_color: [u8; 4],
        pencil_color: [u8; 4],
        detail_level: f32,
    ) {
        let width = buffer.width();
        let height = buffer.height();

        // Step 1: Convert to grayscale
        let grayscale = Self::to_grayscale(buffer);

        // Step 2: Invert the grayscale
        let inverted = Self::invert_grayscale(&grayscale);

        // Step 3: Apply Gaussian blur to inverted image
        // Adjust blur radius based on line_width and detail_level
        let adjusted_radius = line_width * (1.0 + detail_level);
        let blurred_inverted = Self::apply_blur(&inverted, width, height, adjusted_radius);

        // Step 4: Blend using color dodge (grayscale / inverted_blur) - parallelized
        let sketch_values: Vec<f32> = grayscale
            .par_iter()
            .zip(blurred_inverted.par_iter())
            .map(|(g, b)| Self::color_dodge(*g, *b))
            .collect();

        // Step 5: Apply line intensity and blend with paper/pencil colors (parallelized by row)
        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let idx = (y as usize) * (width as usize) + (x as usize);
                        let sketch_value = sketch_values[idx].clamp(0.0, 255.0);

                        // Normalize sketch value to 0-1 range
                        let normalized = sketch_value / 255.0;

                        // Blend between pencil color (dark) and paper color (light)
                        let blend_factor = normalized;
                        let pencil_blend = 1.0 - blend_factor;
                        let adjusted_pencil_blend = pencil_blend * line_intensity;

                        // Final color: blend paper and pencil
                        let r = paper_color[0] as f32 * blend_factor
                            + pencil_color[0] as f32 * adjusted_pencil_blend;
                        let g = paper_color[1] as f32 * blend_factor
                            + pencil_color[1] as f32 * adjusted_pencil_blend;
                        let b = paper_color[2] as f32 * blend_factor
                            + pencil_color[2] as f32 * adjusted_pencil_blend;
                        // Blend alpha based on paper and pencil alpha
                        let a = paper_color[3] as f32 * blend_factor
                            + pencil_color[3] as f32 * adjusted_pencil_blend;

                        // Preserve original alpha if both colors have full alpha
                        let original = buffer.get_pixel(x, y);
                        let final_alpha = if paper_color[3] == 255 && pencil_color[3] == 255 {
                            original.0[3]
                        } else {
                            a.clamp(0.0, 255.0) as u8
                        };

                        [
                            r.clamp(0.0, 255.0) as u8,
                            g.clamp(0.0, 255.0) as u8,
                            b.clamp(0.0, 255.0) as u8,
                            final_alpha,
                        ]
                    })
                    .collect()
            })
            .collect();

        // Apply results back to buffer
        for (y_idx, row) in rows.iter().enumerate() {
            for (x_idx, pixel_data) in row.iter().enumerate() {
                let pixel = buffer.get_pixel_mut(x_idx as u32, y_idx as u32);
                pixel.0 = *pixel_data;
            }
        }
    }
}

impl VideoFilter for SketchFilter {
    crate::impl_default_video_filter!(SketchFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        let line_intensity = self
            .get_value_at_time(time_ms, "line_intensity", self.line_intensity)
            .clamp(0.0, 1.0);
        let line_width = self
            .get_value_at_time(time_ms, "line_width", self.line_width)
            .clamp(1.0, 10.0);
        let detail_level = self
            .get_value_at_time(time_ms, "detail_level", self.detail_level)
            .clamp(0.0, 1.0);

        let paper_color = self.paper_color;
        let pencil_color = self.pencil_color;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_sketch(
                    buffer,
                    line_intensity,
                    line_width,
                    paper_color,
                    pencil_color,
                    detail_level,
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

        for (property, value) in [
            ("line_intensity", self.line_intensity),
            ("line_width", self.line_width),
            ("detail_level", self.detail_level),
        ] {
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

