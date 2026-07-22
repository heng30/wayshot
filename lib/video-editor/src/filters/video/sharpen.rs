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

/// Sharpen filter using Unsharp Mask technique.
/// Enhances edges and details by subtracting a blurred version from the original.
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
#[non_exhaustive]
pub struct SharpenFilter {
    /// Sharpening strength/intensity (0.0-5.0). Higher values create more pronounced sharpening.
    #[derivative(Default(value = "1.0"))]
    pub strength: f32,
    /// Sharpening radius (0.0-10.0). Controls the size of details enhanced.
    #[derivative(Default(value = "1.0"))]
    pub radius: f32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl SharpenFilter {
    pub const NAME: &'static str = "sharpen";

    pub fn new(strength: f32) -> Self {
        Self {
            strength: strength.clamp(0.0, 5.0),
            radius: 1.0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("strength", "Strength", 0.0, 5.0, 1.0),
            AnimatableProperty::float("radius", "Radius", 0.0, 10.0, 1.0),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    /// Generate a simple blur kernel for unsharp mask
    fn generate_blur_kernel(radius: f32) -> Vec<f32> {
        let kernel_radius = radius.ceil() as usize;
        let kernel_size = 2 * kernel_radius + 1;

        // Simple box blur kernel (uniform weights)
        let weight = 1.0 / kernel_size as f32;
        vec![weight; kernel_size]
    }

    /// Apply horizontal blur for unsharp mask (parallelized)
    fn blur_horizontal(image: &RgbaImage, kernel: &[f32]) -> RgbaImage {
        let (width, height) = image.dimensions();
        let kernel_radius = kernel.len() / 2;

        // Pre-compute all pixel values in parallel, row by row
        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let mut r = 0.0;
                        let mut g = 0.0;
                        let mut b = 0.0;
                        let mut a = 0.0;

                        for (i, weight) in kernel.iter().enumerate() {
                            let px = (x as isize + i as isize - kernel_radius as isize)
                                .clamp(0, width as isize - 1)
                                as u32;
                            let pixel = image.get_pixel(px, y);
                            r += pixel.0[0] as f32 * weight;
                            g += pixel.0[1] as f32 * weight;
                            b += pixel.0[2] as f32 * weight;
                            a += pixel.0[3] as f32 * weight;
                        }

                        [
                            r.clamp(0.0, 255.0) as u8,
                            g.clamp(0.0, 255.0) as u8,
                            b.clamp(0.0, 255.0) as u8,
                            a.clamp(0.0, 255.0) as u8,
                        ]
                    })
                    .collect()
            })
            .collect();

        // Flatten and build result image
        let pixels: Vec<u8> = rows.into_iter().flatten().flatten().collect();
        RgbaImage::from_raw(width, height, pixels).expect("Buffer size matches image dimensions")
    }

    /// Apply vertical blur for unsharp mask (parallelized)
    fn blur_vertical(image: &RgbaImage, kernel: &[f32]) -> RgbaImage {
        let (width, height) = image.dimensions();
        let kernel_radius = kernel.len() / 2;

        // Pre-compute all pixel values in parallel, row by row
        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let mut r = 0.0;
                        let mut g = 0.0;
                        let mut b = 0.0;
                        let mut a = 0.0;

                        for (i, weight) in kernel.iter().enumerate() {
                            let py = (y as isize + i as isize - kernel_radius as isize)
                                .clamp(0, height as isize - 1)
                                as u32;
                            let pixel = image.get_pixel(x, py);
                            r += pixel.0[0] as f32 * weight;
                            g += pixel.0[1] as f32 * weight;
                            b += pixel.0[2] as f32 * weight;
                            a += pixel.0[3] as f32 * weight;
                        }

                        [
                            r.clamp(0.0, 255.0) as u8,
                            g.clamp(0.0, 255.0) as u8,
                            b.clamp(0.0, 255.0) as u8,
                            a.clamp(0.0, 255.0) as u8,
                        ]
                    })
                    .collect()
            })
            .collect();

        // Flatten and build result image
        let pixels: Vec<u8> = rows.into_iter().flatten().flatten().collect();
        RgbaImage::from_raw(width, height, pixels).expect("Buffer size matches image dimensions")
    }

    /// Apply sharpening using Unsharp Mask (parallelized):
    /// sharpened = original + strength * (original - blurred)
    fn apply_sharpen(image: &mut RgbaImage, strength: f32, radius: f32) {
        if strength <= 0.0 || radius <= 0.0 {
            return;
        }

        let kernel = Self::generate_blur_kernel(radius);
        let blurred_h = Self::blur_horizontal(image, &kernel);
        let blurred = Self::blur_vertical(&blurred_h, &kernel);

        let (width, height) = image.dimensions();

        // Pre-compute all pixel values in parallel, row by row
        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let original = image.get_pixel(x, y);
                        let blurred_pixel = blurred.get_pixel(x, y);

                        // Unsharp mask formula: original + strength * (original - blurred)
                        let r = original.0[0] as f32
                            + strength * (original.0[0] as f32 - blurred_pixel.0[0] as f32);
                        let g = original.0[1] as f32
                            + strength * (original.0[1] as f32 - blurred_pixel.0[1] as f32);
                        let b = original.0[2] as f32
                            + strength * (original.0[2] as f32 - blurred_pixel.0[2] as f32);

                        [
                            r.clamp(0.0, 255.0) as u8,
                            g.clamp(0.0, 255.0) as u8,
                            b.clamp(0.0, 255.0) as u8,
                            original.0[3], // Keep original alpha
                        ]
                    })
                    .collect()
            })
            .collect();

        // Flatten and build result image
        let pixels: Vec<u8> = rows.into_iter().flatten().flatten().collect();
        *image = RgbaImage::from_raw(width, height, pixels)
            .expect("Buffer size matches image dimensions");
    }
}

impl VideoFilter for SharpenFilter {
    crate::impl_default_video_filter!(SharpenFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        let strength = self
            .get_value_at_time(time_ms, "strength", self.strength)
            .clamp(0.0, 5.0);
        let radius = self
            .get_value_at_time(time_ms, "radius", self.radius)
            .clamp(0.0, 10.0);

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_sharpen(buffer, strength, radius);
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

        for (property, value) in [("strength", self.strength), ("radius", self.radius)] {
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

