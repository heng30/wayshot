//! fork: https://GitHub.com/QmDeve/AndroidLiquidGlassView/

use crate::{
    Result,
    filters::{
        interpolation::{get_float_at_time, get_float2_at_time},
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        subtitle::style::scale_pixel_f32_for_height,
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use glam::Vec2;
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
pub struct LiquidGlassFilter {
    /// Pane left position, normalized (0.0-1.0) relative to frame width.
    #[derivative(Default(value = "0.25"))]
    pub x: f32,
    /// Pane top position, normalized (0.0-1.0) relative to frame height.
    #[derivative(Default(value = "0.25"))]
    pub y: f32,
    /// Pane width, normalized (0.0-1.0) relative to frame width.
    #[derivative(Default(value = "0.5"))]
    pub width: f32,
    /// Pane height, normalized (0.0-1.0) relative to frame height.
    #[derivative(Default(value = "0.5"))]
    pub height: f32,
    /// Corner radius in pixels at a 1080p reference (scaled to the frame at runtime).
    #[derivative(Default(value = "40.0"))]
    pub corner_radius: f32,
    /// Refraction edge height in pixels at a 1080p reference.
    #[derivative(Default(value = "20.0"))]
    pub refraction_height: f32,
    /// Refraction displacement amount in pixels at a 1080p reference.
    #[derivative(Default(value = "70.0"))]
    pub refraction_amount: f32,
    /// Gaussian blur radius in pixels at a 1080p reference (0 = off).
    #[derivative(Default(value = "0.0"))]
    pub blur_radius: f32,
    /// Chromatic aberration / dispersion intensity.
    #[derivative(Default(value = "0.5"))]
    pub chromatic_aberration: f32,
    /// Depth gradient influence on the refraction direction.
    #[derivative(Default(value = "0.3"))]
    pub depth_effect: f32,
    /// Contrast adjustment (0.0 = neutral).
    #[derivative(Default(value = "0.0"))]
    pub contrast: f32,
    /// White point shift. Positive toward white, negative toward black.
    #[derivative(Default(value = "0.0"))]
    pub white_point: f32,
    /// Saturation multiplier (1.0 = neutral, 0.0 = grayscale).
    #[derivative(Default(value = "1.0"))]
    pub chroma_multiplier: f32,
    /// Tint color as [R, G, B] in 0.0-1.0 range.
    #[derivative(Default(value = "[1.0, 1.0, 1.0]"))]
    pub tint_color: [f32; 3],
    /// Tint blend amount (0.0-1.0).
    #[derivative(Default(value = "0.0"))]
    pub tint_alpha: f32,

    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl LiquidGlassFilter {
    pub const NAME: &'static str = "liquid glass";

    pub fn new() -> Self {
        Self::default()
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float2("position", "Position", 0.0, 1.0, 0.25, 0.25),
            AnimatableProperty::float2("size", "Size", 0.0, 1.0, 0.5, 0.5),
            AnimatableProperty::float("corner_radius", "Corner Radius", 0.0, 1080.0, 40.0),
            AnimatableProperty::float("refraction_height", "Refraction Height", 0.0, 200.0, 20.0),
            AnimatableProperty::float("refraction_amount", "Refraction Amount", 0.0, 200.0, 70.0),
            AnimatableProperty::float("blur_radius", "Blur Radius", 0.0, 100.0, 0.0),
            AnimatableProperty::float(
                "chromatic_aberration",
                "Chromatic Aberration",
                0.0,
                2.0,
                0.5,
            ),
            AnimatableProperty::float("depth_effect", "Depth Effect", -2.0, 2.0, 0.3),
            AnimatableProperty::float("contrast", "Contrast", -1.0, 1.0, 0.0),
            AnimatableProperty::float("white_point", "White Point", -1.0, 1.0, 0.0),
            AnimatableProperty::float("chroma_multiplier", "Saturation", 0.0, 2.0, 1.0),
            AnimatableProperty::float("tint_alpha", "Tint Alpha", 0.0, 1.0, 0.0),
        ]
    }

    fn get_values_at_time(&self, time_ms: i64) -> LiquidGlassValues {
        let float_at = |name: &str, default: f32| {
            self.keyframe_tracks
                .get_track(name)
                .map(|track| get_float_at_time(track, time_ms, default))
                .unwrap_or(default)
        };

        let (x, y) = self
            .keyframe_tracks
            .get_track("position")
            .map(|track| get_float2_at_time(track, time_ms, self.x, self.y))
            .unwrap_or((self.x, self.y));

        let (width, height) = self
            .keyframe_tracks
            .get_track("size")
            .map(|track| get_float2_at_time(track, time_ms, self.width, self.height))
            .unwrap_or((self.width, self.height));

        LiquidGlassValues {
            x,
            y,
            width,
            height,
            corner_radius: float_at("corner_radius", self.corner_radius),
            refraction_height: float_at("refraction_height", self.refraction_height),
            refraction_amount: float_at("refraction_amount", self.refraction_amount),
            blur_radius: float_at("blur_radius", self.blur_radius),
            chromatic_aberration: float_at("chromatic_aberration", self.chromatic_aberration),
            depth_effect: float_at("depth_effect", self.depth_effect),
            contrast: float_at("contrast", self.contrast),
            white_point: float_at("white_point", self.white_point),
            chroma_multiplier: float_at("chroma_multiplier", self.chroma_multiplier),
            tint_alpha: float_at("tint_alpha", self.tint_alpha),
        }
    }

    fn apply_to_buffer(buffer: &mut RgbaImage, values: &LiquidGlassValues, tint_color: [f32; 3]) {
        let (width, height) = buffer.dimensions();
        if width == 0 || height == 0 {
            return;
        }

        let pane_left = (values.x.clamp(0.0, 1.0) * width as f32).floor().max(0.0) as u32;
        let pane_top = (values.y.clamp(0.0, 1.0) * height as f32).floor().max(0.0) as u32;
        let pane_right = (pane_left as f32 + values.width.clamp(0.0, 1.0) * width as f32)
            .ceil()
            .min(width as f32) as u32;
        let pane_bottom = (pane_top as f32 + values.height.clamp(0.0, 1.0) * height as f32)
            .ceil()
            .min(height as f32) as u32;

        if pane_right <= pane_left || pane_bottom <= pane_top {
            return;
        }

        let half_size = Vec2::new(
            (pane_right - pane_left) as f32 * 0.5,
            (pane_bottom - pane_top) as f32 * 0.5,
        );

        let corner_radius = scale_pixel_f32_for_height(values.corner_radius, height);
        let refraction_height = scale_pixel_f32_for_height(values.refraction_height, height);
        let refraction_amount = scale_pixel_f32_for_height(values.refraction_amount, height);
        let min_smooth_radius = scale_pixel_f32_for_height(30.0, height);
        let blur_radius = scale_pixel_f32_for_height(values.blur_radius, height);

        let resolved = ResolvedGlass {
            pane_left,
            pane_top,
            half_size,
            radius: corner_radius.clamp(0.0, half_size.x.min(half_size.y)),
            refraction_height,
            refraction_amount,
            min_smooth_radius,
            chromatic_aberration: values.chromatic_aberration,
            depth_effect: values.depth_effect,
            contrast: values.contrast,
            white_point: values.white_point,
            chroma_multiplier: values.chroma_multiplier,
            tint_alpha: values.tint_alpha,
            tint_color,
        };
        let content = if blur_radius > 0.0 {
            Self::gaussian_blur(buffer, blur_radius)
        } else {
            buffer.clone()
        };

        let src: &RgbaImage = buffer;
        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let in_pane =
                            x >= pane_left && x < pane_right && y >= pane_top && y < pane_bottom;
                        if !in_pane {
                            return src.get_pixel(x, y).0;
                        }
                        let lx = (x - pane_left) as f32;
                        let ly = (y - pane_top) as f32;
                        match Self::glass_pixel(&content, lx, ly, &resolved) {
                            Some(pixel) => pixel,
                            None => src.get_pixel(x, y).0,
                        }
                    })
                    .collect()
            })
            .collect();

        let pixels: Vec<u8> = rows.into_iter().flatten().flatten().collect();
        *buffer = RgbaImage::from_raw(width, height, pixels)
            .expect("Buffer size matches image dimensions");
    }

    fn glass_pixel(content: &RgbaImage, lx: f32, ly: f32, g: &ResolvedGlass) -> Option<[u8; 4]> {
        let centered = Vec2::new(lx - g.half_size.x, ly - g.half_size.y);
        let sd = sd_rounded_rect(centered, g.half_size, g.radius);
        if sd > 0.0 {
            return None;
        }

        let base_x = g.pane_left as f32 + lx;
        let base_y = g.pane_top as f32 + ly;

        if -sd >= g.refraction_height {
            let base = sample_bilinear(content, base_x, base_y);
            return Some(grade(base, g));
        }

        let t = 1.0 - (-sd / g.refraction_height.max(f32::EPSILON));
        let d = circle_map(t.clamp(0.0, 1.0)) * g.refraction_amount;

        let smooth_radius = (g.radius * 1.5).max(g.min_smooth_radius);
        let grad_radius = smooth_radius.min(g.half_size.x.min(g.half_size.y));

        let shape_grad = grad_sd_rounded_rect(centered, g.half_size, grad_radius);
        let depth_grad = safe_normalize(centered, shape_grad);
        let grad = safe_normalize(shape_grad + g.depth_effect * depth_grad, shape_grad);

        let base_disp = d * grad;
        let dispersion = g.chromatic_aberration * (centered.x * centered.y)
            / (g.half_size.x * g.half_size.y).max(f32::EPSILON);
        let dispersed = base_disp * dispersion;

        let sample = |offset: Vec2| -> [f32; 4] {
            sample_bilinear(
                content,
                base_x + base_disp.x + offset.x,
                base_y + base_disp.y + offset.y,
            )
        };

        let red = sample(dispersed);
        let orange = sample(dispersed * (2.0 / 3.0));
        let yellow = sample(dispersed * (1.0 / 3.0));
        let green = sample(Vec2::ZERO);
        let cyan = sample(dispersed * (-1.0 / 3.0));
        let blue = sample(dispersed * (-2.0 / 3.0));
        let purple = sample(-dispersed);

        let r = red[0] / 3.5 + orange[0] / 3.5 + yellow[0] / 3.5 + purple[0] / 7.0;
        let gcol = orange[1] / 7.0 + yellow[1] / 3.5 + green[1] / 3.5 + cyan[1] / 3.5;
        let b = cyan[2] / 3.0 + blue[2] / 3.0 + purple[2] / 3.0;
        let a = (red[3] + orange[3] + yellow[3] + green[3] + cyan[3] + blue[3] + purple[3]) / 7.0;

        Some(grade([r, gcol, b, a], g))
    }

    fn gaussian_blur(img: &RgbaImage, sigma: f32) -> RgbaImage {
        if sigma <= 0.0 {
            return img.clone();
        }

        let radius = (sigma * 3.0).ceil().max(1.0) as usize;
        let size = radius * 2 + 1;
        let two_sigma_sq = 2.0 * sigma * sigma;

        let mut kernel = Vec::with_capacity(size);
        let mut sum = 0.0;
        for i in 0..size {
            let x = i as f32 - radius as f32;
            let weight = (-x * x / two_sigma_sq).exp();
            kernel.push(weight);
            sum += weight;
        }
        for weight in &mut kernel {
            *weight /= sum;
        }

        let horizontal = convolve_separable(img, &kernel, true);
        convolve_separable(&horizontal, &kernel, false)
    }
}

struct LiquidGlassValues {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    corner_radius: f32,
    refraction_height: f32,
    refraction_amount: f32,
    blur_radius: f32,
    chromatic_aberration: f32,
    depth_effect: f32,
    contrast: f32,
    white_point: f32,
    chroma_multiplier: f32,
    tint_alpha: f32,
}

struct ResolvedGlass {
    pane_left: u32,
    pane_top: u32,
    half_size: Vec2,
    radius: f32,
    refraction_height: f32,
    refraction_amount: f32,
    min_smooth_radius: f32,
    chromatic_aberration: f32,
    depth_effect: f32,
    contrast: f32,
    white_point: f32,
    chroma_multiplier: f32,
    tint_alpha: f32,
    tint_color: [f32; 3],
}

fn convolve_separable(img: &RgbaImage, kernel: &[f32], horizontal: bool) -> RgbaImage {
    let (width, height) = img.dimensions();
    let radius = kernel.len() / 2;

    let rows: Vec<Vec<[u8; 4]>> = (0..height)
        .into_par_iter()
        .map(|y| {
            (0..width)
                .map(|x| {
                    let mut acc = [0.0f32; 4];
                    for (i, &weight) in kernel.iter().enumerate() {
                        let offset = i as isize - radius as isize;
                        let (sx, sy) = if horizontal {
                            ((x as isize + offset).clamp(0, width as isize - 1) as u32, y)
                        } else {
                            (
                                x,
                                (y as isize + offset).clamp(0, height as isize - 1) as u32,
                            )
                        };
                        let pixel = img.get_pixel(sx, sy).0;
                        for c in 0..4 {
                            acc[c] += pixel[c] as f32 * weight;
                        }
                    }
                    [
                        acc[0].clamp(0.0, 255.0) as u8,
                        acc[1].clamp(0.0, 255.0) as u8,
                        acc[2].clamp(0.0, 255.0) as u8,
                        acc[3].clamp(0.0, 255.0) as u8,
                    ]
                })
                .collect()
        })
        .collect();

    let pixels: Vec<u8> = rows.into_iter().flatten().flatten().collect();
    RgbaImage::from_raw(width, height, pixels).expect("Buffer size matches image dimensions")
}

fn sample_bilinear(img: &RgbaImage, fx: f32, fy: f32) -> [f32; 4] {
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return [0.0, 0.0, 0.0, 1.0];
    }

    let x = fx.clamp(0.0, width as f32 - 1.0);
    let y = fy.clamp(0.0, height as f32 - 1.0);

    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);

    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let p00 = img.get_pixel(x0, y0).0;
    let p10 = img.get_pixel(x1, y0).0;
    let p01 = img.get_pixel(x0, y1).0;
    let p11 = img.get_pixel(x1, y1).0;

    let mut out = [0.0f32; 4];
    for c in 0..4 {
        let top = lerp(p00[c] as f32 / 255.0, p10[c] as f32 / 255.0, tx);
        let bottom = lerp(p01[c] as f32 / 255.0, p11[c] as f32 / 255.0, tx);
        out[c] = lerp(top, bottom, ty);
    }
    out
}

fn grade(color: [f32; 4], g: &ResolvedGlass) -> [u8; 4] {
    let (r, gg, b) = saturate_color((color[0], color[1], color[2]), g.chroma_multiplier);

    let target = if g.white_point > 0.0 { 1.0 } else { 0.0 };
    let wp = g.white_point.abs();
    let r = lerp(r, target, wp);
    let gg = lerp(gg, target, wp);
    let b = lerp(b, target, wp);

    let r = (r - 0.5) * (1.0 + g.contrast) + 0.5;
    let gg = (gg - 0.5) * (1.0 + g.contrast) + 0.5;
    let b = (b - 0.5) * (1.0 + g.contrast) + 0.5;

    let r = lerp(r, g.tint_color[0], g.tint_alpha);
    let gg = lerp(gg, g.tint_color[1], g.tint_alpha);
    let b = lerp(b, g.tint_color[2], g.tint_alpha);

    [
        (r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (gg.clamp(0.0, 1.0) * 255.0).round() as u8,
        (b.clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn saturate_color(rgb: (f32, f32, f32), amount: f32) -> (f32, f32, f32) {
    let lr = srgb_to_linear(rgb.0);
    let lg = srgb_to_linear(rgb.1);
    let lb = srgb_to_linear(rgb.2);

    let y = 0.2126 * lr + 0.7152 * lg + 0.0722 * lb;

    (
        linear_to_srgb(lerp(y, lr, amount)),
        linear_to_srgb(lerp(y, lg, amount)),
        linear_to_srgb(lerp(y, lb, amount)),
    )
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn sd_rounded_rect(coord: Vec2, half_size: Vec2, radius: f32) -> f32 {
    let corner = coord.abs() - (half_size - Vec2::splat(radius));
    let outside = corner.max(Vec2::ZERO).length() - radius;
    let inside = corner.x.max(corner.y).min(0.0);
    outside + inside
}

fn grad_sd_rounded_rect(coord: Vec2, half_size: Vec2, radius: f32) -> Vec2 {
    let corner = coord.abs() - (half_size - Vec2::splat(radius));
    if corner.x >= 0.0 || corner.y >= 0.0 {
        let outside = corner.max(Vec2::ZERO);
        let outside_length = outside.length();
        if outside_length > 0.001 {
            return coord.signum() * (outside / outside_length);
        }
        let use_x = if corner.y <= corner.x { 1.0 } else { 0.0 };
        return Vec2::new(
            use_x * safe_sign(coord.x),
            (1.0 - use_x) * safe_sign(coord.y),
        );
    }

    let grad_x = if corner.y <= corner.x { 1.0 } else { 0.0 };
    coord.signum() * Vec2::new(grad_x, 1.0 - grad_x)
}

fn circle_map(x: f32) -> f32 {
    1.0 - (1.0 - x * x).sqrt()
}

fn safe_sign(value: f32) -> f32 {
    if value < 0.0 { -1.0 } else { 1.0 }
}

fn safe_normalize(value: Vec2, fallback: Vec2) -> Vec2 {
    let len = value.length();
    if len > 0.001 { value / len } else { fallback }
}

impl VideoFilter for LiquidGlassFilter {
    crate::impl_default_video_filter!(LiquidGlassFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;
        let values = self.get_values_at_time(time_ms);

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                Self::apply_to_buffer(buffer, &values, self.tint_color);
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
            ("corner_radius", self.corner_radius),
            ("refraction_height", self.refraction_height),
            ("refraction_amount", self.refraction_amount),
            ("blur_radius", self.blur_radius),
            ("chromatic_aberration", self.chromatic_aberration),
            ("depth_effect", self.depth_effect),
            ("contrast", self.contrast),
            ("white_point", self.white_point),
            ("chroma_multiplier", self.chroma_multiplier),
            ("tint_alpha", self.tint_alpha),
        ] {
            if let Some(track) = tracks.get_track(property)
                && track.keyframes.iter().any(|k| k.time_ms == time_ms)
            {
                tracks.update_keyframe_value(property, time_ms, KeyframeValue::Float(value));
                updated = true;
            }
        }

        if let Some(track) = tracks.get_track("position")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "position",
                time_ms,
                KeyframeValue::Float2(self.x, self.y),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_name() {
        assert_eq!(LiquidGlassFilter::NAME, "liquid glass");
    }

    #[test]
    fn test_default_values() {
        let filter = LiquidGlassFilter::default();
        assert!((filter.x - 0.25).abs() < f32::EPSILON);
        assert!((filter.width - 0.5).abs() < f32::EPSILON);
        assert!((filter.corner_radius - 40.0).abs() < f32::EPSILON);
        assert!((filter.refraction_amount - 70.0).abs() < f32::EPSILON);
        assert!((filter.chroma_multiplier - 1.0).abs() < f32::EPSILON);
        assert_eq!(filter.tint_color, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_supports_keyframes() {
        let filter = LiquidGlassFilter::default();
        assert!(filter.supports_keyframes());
        assert_eq!(filter.get_animatable_properties().len(), 12);
    }

    #[test]
    fn test_srgb_roundtrip() {
        for &v in &[0.0, 0.001, 0.04045, 0.2, 0.5, 0.8, 1.0] {
            let roundtrip = linear_to_srgb(srgb_to_linear(v));
            assert!((roundtrip - v).abs() < 1e-4, "roundtrip failed for {v}");
        }
    }
}
