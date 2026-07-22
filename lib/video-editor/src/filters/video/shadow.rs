use crate::{
    Result,
    filters::{
        interpolation::{get_color_at_time, get_float_at_time},
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        subtitle::style::scale_pixel_f32_for_height,
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use image::{Rgba, RgbaImage};
use rayon::prelude::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, derivative::Derivative)]
#[derivative(Default)]
pub struct ShadowFilter {
    /// Shadow color (RGBA)
    #[derivative(Default(value = "[0, 0, 0, 255]"))]
    pub color: [u8; 4],
    /// Shadow opacity (0.0-1.0)
    #[derivative(Default(value = "0.8"))]
    pub opacity: f32,
    /// Shadow size/spread ratio (0.0-1.0)
    #[derivative(Default(value = "0.4"))]
    pub size: f32,
    /// Shadow blur radius in pixels (0.0-100.0)
    #[derivative(Default(value = "10.0"))]
    pub blur: f32,
    /// Shadow offset angle in degrees (0-360)
    #[derivative(Default(value = "135.0"))]
    pub angle: f32,
    /// Shadow offset distance in pixels (0.0-200.0)
    #[derivative(Default(value = "10.0"))]
    pub distance: f32,
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl ShadowFilter {
    pub const NAME: &'static str = "shadow";

    pub fn new(color: [u8; 4], opacity: f32, blur: f32, angle: f32, distance: f32) -> Self {
        Self {
            color,
            opacity: opacity.clamp(0.0, 1.0),
            size: 0.4,
            blur: blur.clamp(0.0, 100.0),
            angle: angle % 360.0,
            distance: distance.clamp(0.0, 200.0),
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size.clamp(0.0, 1.0);
        self
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::color("color", "Color", 0, 0, 0, 255),
            AnimatableProperty::float("opacity", "Opacity", 0.0, 1.0, 0.8),
            AnimatableProperty::float("size", "Size", 0.0, 1.0, 0.4),
            AnimatableProperty::float("blur", "Blur", 0.0, 100.0, 10.0),
            AnimatableProperty::float("angle", "Angle", 0.0, 360.0, 135.0),
            AnimatableProperty::float("distance", "Distance", 0.0, 200.0, 10.0),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    /// Generate a 1D Gaussian kernel
    fn generate_gaussian_kernel(radius: f32, sigma: f32) -> Vec<f32> {
        let kernel_radius = radius.ceil() as usize;
        let kernel_size = 2 * kernel_radius + 1;

        let mut kernel = Vec::with_capacity(kernel_size);
        let sigma = if sigma <= 0.0 { radius / 3.0 } else { sigma };
        let sigma_sq = sigma * sigma;
        let two_sigma_sq = 2.0 * sigma_sq;

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

    fn convolve_horizontal(image: &RgbaImage, kernel: &[f32]) -> RgbaImage {
        let (width, height) = image.dimensions();
        let kernel_radius = kernel.len() / 2;

        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let mut r = 0.0f32;
                        let mut g = 0.0f32;
                        let mut b = 0.0f32;
                        let mut a = 0.0f32;

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

        let pixels: Vec<u8> = rows.into_iter().flatten().flatten().collect();
        RgbaImage::from_raw(width, height, pixels).expect("Buffer size matches image dimensions")
    }

    fn convolve_vertical(image: &RgbaImage, kernel: &[f32]) -> RgbaImage {
        let (width, height) = image.dimensions();
        let kernel_radius = kernel.len() / 2;

        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let mut r = 0.0f32;
                        let mut g = 0.0f32;
                        let mut b = 0.0f32;
                        let mut a = 0.0f32;

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

        let pixels: Vec<u8> = rows.into_iter().flatten().flatten().collect();
        RgbaImage::from_raw(width, height, pixels).expect("Buffer size matches image dimensions")
    }

    fn apply_gaussian_blur(image: &mut RgbaImage, radius: f32) {
        if radius <= 0.0 {
            return;
        }
        let sigma = radius / 3.0;
        let kernel = Self::generate_gaussian_kernel(radius, sigma);
        let horizontal = Self::convolve_horizontal(image, &kernel);
        let vertical = Self::convolve_vertical(&horizontal, &kernel);
        *image = vertical;
    }

    /// Create a shadow layer from the source image's alpha channel.
    /// The shadow is colorized, scaled by (1 + size), blurred, and offset.
    /// Returns (shadow_canvas, expand_x, expand_y) where expand values indicate
    /// how much the canvas was expanded on each side beyond the source dimensions.
    fn create_shadow_layer(
        source: &RgbaImage,
        color: [u8; 4],
        opacity: f32,
        size: f32,
        blur: f32,
        offset_x: i32,
        offset_y: i32,
    ) -> (RgbaImage, i32, i32) {
        let (width, height) = source.dimensions();

        // Scale factor: size=0 → 1.0x, size=1 → 2.0x
        let scale = 1.0 + size;

        // How much the scaled shadow extends beyond the source on each side
        let size_expand_x = (width as f32 * size / 2.0).ceil() as i32;
        let size_expand_y = (height as f32 * size / 2.0).ceil() as i32;

        // Canvas expansion for blur + size + offset
        let blur_expand = blur.ceil() as i32;
        let expand_x = size_expand_x + blur_expand + offset_x.unsigned_abs() as i32;
        let expand_y = size_expand_y + blur_expand + offset_y.unsigned_abs() as i32;

        let canvas_width = width + 2 * expand_x as u32;
        let canvas_height = height + 2 * expand_y as u32;

        // Create shadow canvas
        let mut shadow = RgbaImage::new(canvas_width, canvas_height);

        // Draw scaled shadow: center of source maps to center of canvas
        let half_src_w = width as f32 / 2.0;
        let half_src_h = height as f32 / 2.0;

        for y in 0..canvas_height {
            for x in 0..canvas_width {
                // Map canvas coords to source coords (center-scaled)
                let src_x = ((x as f32 - expand_x as f32) / scale + half_src_w * (1.0 - 1.0 / scale))
                    .clamp(0.0, width as f32 - 1.0);
                let src_y = ((y as f32 - expand_y as f32) / scale + half_src_h * (1.0 - 1.0 / scale))
                    .clamp(0.0, height as f32 - 1.0);

                let src_xi = src_x as u32;
                let src_yi = src_y as u32;
                let pixel = source.get_pixel(src_xi, src_yi);
                let src_alpha = pixel.0[3] as f32 / 255.0;
                if src_alpha > 0.0 {
                    let out_alpha = (color[3] as f32 * opacity * src_alpha).clamp(0.0, 255.0) as u8;
                    shadow.put_pixel(x, y, Rgba([color[0], color[1], color[2], out_alpha]));
                }
            }
        }

        // Apply Gaussian blur
        Self::apply_gaussian_blur(&mut shadow, blur);

        (shadow, expand_x, expand_y)
    }
}

impl VideoFilter for ShadowFilter {
    crate::impl_default_video_filter!(ShadowFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let output_height = data.config.output_height;
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        let color = self
            .keyframe_tracks
            .get_track("color")
            .map(|track| {
                get_color_at_time(
                    track,
                    time_ms,
                    (self.color[0], self.color[1], self.color[2], self.color[3]),
                )
            })
            .map(|c| [c.0, c.1, c.2, c.3])
            .unwrap_or(self.color);

        let opacity = self
            .get_value_at_time(time_ms, "opacity", self.opacity)
            .clamp(0.0, 1.0);
        let size = self
            .get_value_at_time(time_ms, "size", self.size)
            .clamp(0.0, 1.0);
        let blur = self
            .get_value_at_time(time_ms, "blur", self.blur)
            .clamp(0.0, 100.0);
        let angle = self.get_value_at_time(time_ms, "angle", self.angle) % 360.0;
        let distance = self
            .get_value_at_time(time_ms, "distance", self.distance)
            .clamp(0.0, 200.0);

        // Scale pixel values based on frame height (size is ratio, blur/distance are pixels)
        let scaled_blur = scale_pixel_f32_for_height(blur, output_height);
        let scaled_distance = scale_pixel_f32_for_height(distance, output_height);

        // Calculate offset from angle and distance
        let rad = angle.to_radians();
        let offset_x = (scaled_distance * rad.cos()).round() as i32;
        let offset_y = (scaled_distance * rad.sin()).round() as i32;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                let (shadow_canvas, expand_x, expand_y) = Self::create_shadow_layer(
                    buffer,
                    color,
                    opacity,
                    size,
                    scaled_blur,
                    offset_x,
                    offset_y,
                );

                // Composite: shadow behind, original on top (parallel by rows)
                let (w, h) = buffer.dimensions();
                let canvas_w = shadow_canvas.width();
                let canvas_h = shadow_canvas.height();
                let buf_rows: Vec<Vec<[u8; 4]>> = (0..h)
                    .into_par_iter()
                    .map(|y| {
                        let mut row = vec![[0u8; 4]; w as usize];
                        for x in 0..w {
                            let pixel = buffer.get_pixel(x, y);
                            let src_alpha = pixel.0[3] as f32 / 255.0;

                            // Map buffer coords to shadow canvas coords
                            let sx = x as i32 + expand_x - offset_x;
                            let sy = y as i32 + expand_y - offset_y;

                            if sx >= 0
                                && sy >= 0
                                && (sx as u32) < canvas_w
                                && (sy as u32) < canvas_h
                            {
                                let shadow_px = shadow_canvas.get_pixel(sx as u32, sy as u32);
                                let shadow_alpha = shadow_px.0[3] as f32 / 255.0;

                                if shadow_alpha > 0.0 {
                                    // Shadow behind, original on top
                                    let out_alpha = src_alpha + shadow_alpha * (1.0 - src_alpha);
                                    if out_alpha > 0.0 {
                                        row[x as usize] = [
                                            ((pixel.0[0] as f32 * src_alpha
                                                + shadow_px.0[0] as f32 * shadow_alpha
                                                    * (1.0 - src_alpha))
                                                / out_alpha) as u8,
                                            ((pixel.0[1] as f32 * src_alpha
                                                + shadow_px.0[1] as f32 * shadow_alpha
                                                    * (1.0 - src_alpha))
                                                / out_alpha) as u8,
                                            ((pixel.0[2] as f32 * src_alpha
                                                + shadow_px.0[2] as f32 * shadow_alpha
                                                    * (1.0 - src_alpha))
                                                / out_alpha) as u8,
                                            (out_alpha * 255.0) as u8,
                                        ];
                                    } else {
                                        row[x as usize] = pixel.0;
                                    }
                                } else {
                                    row[x as usize] = pixel.0;
                                }
                            } else {
                                row[x as usize] = pixel.0;
                            }
                        }
                        row
                    })
                    .collect();

                let pixels: Vec<u8> = buf_rows.into_iter().flatten().flatten().collect();
                *buffer = RgbaImage::from_raw(w, h, pixels).expect("Buffer size matches");
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

        if let Some(track) = tracks.get_track("color")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "color",
                time_ms,
                KeyframeValue::Color(self.color[0], self.color[1], self.color[2], self.color[3]),
            );
            updated = true;
        }

        for (property, value) in [
            ("opacity", self.opacity),
            ("size", self.size),
            ("blur", self.blur),
            ("angle", self.angle),
            ("distance", self.distance),
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
