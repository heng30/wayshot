use crate::{
    Result,
    filters::{
        interpolation::{get_float2_at_time, get_float_at_time},
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::RgbaImage;
use rayon::prelude::*;

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
pub struct GaussianBlurFilter {
    /// Blur radius/kernel size (0.0-50.0). Larger values create stronger blur.
    #[derivative(Default(value = "5.0"))]
    pub radius: f32,
    /// Gaussian distribution standard deviation (0.1-20.0). Controls blur smoothness.
    #[derivative(Default(value = "2.0"))]
    pub sigma: f32,
    /// Left edge of blur region (0.0-1.0, normalized).
    #[serde(default)]
    pub left: f32,
    /// Top edge of blur region (0.0-1.0, normalized).
    #[serde(default)]
    pub top: f32,
    /// Width of blur region (0.0-1.0, normalized, default 1.0 = full width).
    #[derivative(Default(value = "1.0"))]
    #[serde(default = "default_one")]
    pub width: f32,
    /// Height of blur region (0.0-1.0, normalized, default 1.0 = full height).
    #[derivative(Default(value = "1.0"))]
    #[serde(default = "default_one")]
    pub height: f32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

fn default_one() -> f32 {
    1.0
}

impl GaussianBlurFilter {
    pub const NAME: &'static str = "gaussian blur";

    pub fn new(radius: f32) -> Self {
        Self {
            radius: radius.clamp(0.0, 50.0),
            sigma: 2.0,
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("radius", "Radius", 0.0, 50.0, 5.0),
            AnimatableProperty::float("sigma", "Sigma", 0.1, 20.0, 2.0),
            AnimatableProperty::float2("region", "Position", 0.0, 1.0, 0.0, 0.0), // left, top
            AnimatableProperty::float2("size", "Size", 0.0, 1.0, 1.0, 1.0),       // width, height
        ]
    }

    /// Get interpolated region values at a specific time
    fn get_values_at_time(&self, time_ms: i64) -> GaussianBlurValues {
        let radius = self
            .keyframe_tracks
            .get_track("radius")
            .map(|t| get_float_at_time(t, time_ms, self.radius))
            .unwrap_or(self.radius);

        let sigma = self
            .keyframe_tracks
            .get_track("sigma")
            .map(|t| get_float_at_time(t, time_ms, self.sigma))
            .unwrap_or(self.sigma);

        let (left, top) = self
            .keyframe_tracks
            .get_track("region")
            .map(|track| get_float2_at_time(track, time_ms, self.left, self.top))
            .unwrap_or((self.left, self.top));

        let (width, height) = self
            .keyframe_tracks
            .get_track("size")
            .map(|track| get_float2_at_time(track, time_ms, self.width, self.height))
            .unwrap_or((self.width, self.height));

        GaussianBlurValues {
            radius: radius.clamp(0.0, 50.0),
            sigma: sigma.clamp(0.1, 20.0),
            left: left.clamp(0.0, 1.0),
            top: top.clamp(0.0, 1.0),
            width: width.clamp(0.0, 1.0),
            height: height.clamp(0.0, 1.0),
        }
    }

    /// Generate a 1D Gaussian kernel
    fn generate_gaussian_kernel(radius: f32, sigma: f32) -> Vec<f32> {
        // Convert radius to integer kernel size (odd number)
        let kernel_radius = radius.ceil() as usize;
        let kernel_size = 2 * kernel_radius + 1;

        let mut kernel = Vec::with_capacity(kernel_size);
        let sigma_sq = sigma * sigma;
        let two_sigma_sq = 2.0 * sigma_sq;

        // Calculate weights
        let mut sum = 0.0;
        for i in 0..kernel_size {
            let x = (i as isize - kernel_radius as isize) as f32;
            let weight = (-x * x / two_sigma_sq).exp();
            kernel.push(weight);
            sum += weight;
        }

        // Normalize kernel
        if sum > 0.0 {
            for weight in &mut kernel {
                *weight /= sum;
            }
        }

        kernel
    }

    /// Apply horizontal convolution (parallelized)
    fn convolve_horizontal(image: &RgbaImage, kernel: &[f32]) -> RgbaImage {
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

    /// Apply vertical convolution (parallelized)
    fn convolve_vertical(image: &RgbaImage, kernel: &[f32]) -> RgbaImage {
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

    /// Apply Gaussian blur using separable convolution (horizontal + vertical)
    fn apply_blur(image: &mut RgbaImage, radius: f32, sigma: f32) {
        if radius <= 0.0 || sigma <= 0.0 {
            return;
        }

        let kernel = Self::generate_gaussian_kernel(radius, sigma);
        let horizontal = Self::convolve_horizontal(image, &kernel);
        let vertical = Self::convolve_vertical(&horizontal, &kernel);
        *image = vertical;
    }

    /// Apply Gaussian blur to a specific region of the image
    fn apply_blur_region(image: &mut RgbaImage, values: &GaussianBlurValues) {
        if values.radius <= 0.0 || values.sigma <= 0.0 {
            return;
        }

        let img_width = image.width();
        let img_height = image.height();

        // Convert normalized values to pixel values
        let px_left = (values.left * img_width as f32).round() as u32;
        let px_top = (values.top * img_height as f32).round() as u32;
        let px_width = (values.width * img_width as f32).round() as u32;
        let px_height = (values.height * img_height as f32).round() as u32;

        // Clamp values
        let px_left = px_left.min(img_width);
        let px_top = px_top.min(img_height);
        let px_right = (px_left + px_width).min(img_width);
        let px_bottom = (px_top + px_height).min(img_height);

        if px_right <= px_left || px_bottom <= px_top {
            return;
        }

        // Full image blur (default region covers entire image)
        if px_left == 0 && px_top == 0 && px_right == img_width && px_bottom == img_height {
            Self::apply_blur(image, values.radius, values.sigma);
            return;
        }

        // Extract the region, blur it, and composite back
        let region_width = px_right - px_left;
        let region_height = px_bottom - px_top;

        let mut region = image::SubImage::new(&mut *image, px_left, px_top, region_width, region_height)
            .to_image();

        Self::apply_blur(&mut region, values.radius, values.sigma);

        // Copy blurred region back into the original image
        image::imageops::overlay(image, &region, px_left as i64, px_top as i64);
    }
}

/// Interpolated gaussian blur values at a specific time
struct GaussianBlurValues {
    radius: f32,
    sigma: f32,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

impl VideoFilter for GaussianBlurFilter {
    crate::impl_default_video_filter!(GaussianBlurFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;
        let values = self.get_values_at_time(time_ms);

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_blur_region(buffer, &values);
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

        for (property, value) in [("radius", self.radius), ("sigma", self.sigma)] {
            if let Some(track) = tracks.get_track(property)
                && track.keyframes.iter().any(|k| k.time_ms == time_ms)
            {
                tracks.update_keyframe_value(property, time_ms, KeyframeValue::Float(value));
                updated = true;
            }
        }

        if let Some(track) = tracks.get_track("region")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "region",
                time_ms,
                KeyframeValue::Float2(self.left, self.top),
            );
            updated = true;
        }

        if let Some(track) = tracks.get_track("size")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "size",
                time_ms,
                KeyframeValue::Float2(self.width, self.height),
            );
            updated = true;
        }

        updated
    }
}
