use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use rayon::prelude::*;

/// Luminance calculation standard (used when preserve_luminance is enabled)
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub enum LuminanceStandard {
    /// HDTV standard (most common): R*0.2126 + G*0.7152 + B*0.0722
    #[default]
    BT709,
    /// SDTV standard: R*0.299 + G*0.587 + B*0.114
    BT601,
    /// HDR standard: R*0.2627 + G*0.6780 + B*0.0593
    BT2020,
}

impl LuminanceStandard {
    /// Calculate luminance from linear RGB values (0.0-1.0 range)
    pub fn calculate_luminance(&self, r: f32, g: f32, b: f32) -> f32 {
        match self {
            LuminanceStandard::BT709 => r * 0.2126 + g * 0.7152 + b * 0.0722,
            LuminanceStandard::BT601 => r * 0.299 + g * 0.587 + b * 0.114,
            LuminanceStandard::BT2020 => r * 0.2627 + g * 0.6780 + b * 0.0593,
        }
    }
}

/// HSL (Hue, Saturation, Lightness) adjustment filter
///
/// This filter allows adjusting the hue, saturation, and lightness of an image.
/// All three parameters can be animated using keyframes.
///
/// # Parameters
/// - `hue_shift`: Hue rotation in degrees (-180 to 180)
/// - `saturation`: Saturation adjustment (-1 to 1, where 0 = no change)
/// - `lightness`: Lightness adjustment (-1 to 1, where 0 = no change)
/// - `preserve_luminance`: When true, preserves perceived luminance after adjustments
/// - `luminance_standard`: The standard used for luminance calculation
#[derive(Debug, Clone, derive_setters::Setters, serde::Serialize, serde::Deserialize)]
#[setters(prefix = "with_")]
pub struct HSLAdjustFilter {
    /// Hue rotation in degrees (-180 to 180)
    pub hue_shift: f32,
    /// Saturation adjustment (-1 to 1, 0 = no change, -1 = grayscale, 1 = double saturation)
    pub saturation: f32,
    /// Lightness adjustment (-1 to 1, 0 = no change, -1 = black, 1 = white)
    pub lightness: f32,
    /// When true, preserves the original perceived luminance after HSL adjustments
    #[serde(default)]
    #[setters(skip)]
    pub preserve_luminance: bool,
    /// The standard used for luminance calculation (only effective when preserve_luminance is true)
    #[serde(default)]
    #[setters(skip)]
    pub luminance_standard: LuminanceStandard,

    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl Default for HSLAdjustFilter {
    fn default() -> Self {
        Self {
            hue_shift: 0.0,
            saturation: 0.0,
            lightness: 0.0,
            preserve_luminance: false,
            luminance_standard: LuminanceStandard::default(),
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl HSLAdjustFilter {
    pub const NAME: &'static str = "hsl adjust";

    pub fn new(hue_shift: f32, saturation: f32, lightness: f32) -> Self {
        Self {
            hue_shift: hue_shift.clamp(-180.0, 180.0),
            saturation: saturation.clamp(-1.0, 1.0),
            lightness: lightness.clamp(-1.0, 1.0),
            preserve_luminance: false,
            luminance_standard: LuminanceStandard::default(),
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    pub fn with_preserve_luminance(mut self, standard: LuminanceStandard) -> Self {
        self.preserve_luminance = true;
        self.luminance_standard = standard;
        self
    }

    pub fn with_preserve_luminance_option(
        mut self,
        preserve: bool,
        standard: LuminanceStandard,
    ) -> Self {
        self.preserve_luminance = preserve;
        self.luminance_standard = standard;
        self
    }

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("hue_shift", "Hue Shift", -180.0, 180.0, 0.0),
            AnimatableProperty::float("saturation", "Saturation", -1.0, 1.0, 0.0),
            AnimatableProperty::float("lightness", "Lightness", -1.0, 1.0, 0.0),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    /// Convert RGB (0-255) to HSL
    /// Returns (hue: 0-360, saturation: 0-1, lightness: 0-1)
    fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
        let r = r as f32 / 255.0;
        let g = g as f32 / 255.0;
        let b = b as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;

        if max == min {
            // Achromatic (gray)
            return (0.0, 0.0, l);
        }

        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };

        let h = match max {
            x if x == r => (g - b) / d + if g < b { 6.0 } else { 0.0 },
            x if x == g => (b - r) / d + 2.0,
            _ => (r - g) / d + 4.0,
        };

        (h * 60.0, s, l)
    }

    /// Convert HSL to RGB (0-255)
    /// Input: hue 0-360, saturation 0-1, lightness 0-1
    fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
        if s == 0.0 {
            // Achromatic
            let v = (l * 255.0).clamp(0.0, 255.0) as u8;
            return (v, v, v);
        }

        let h = h / 360.0;
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;

        let r = Self::hue_to_rgb(p, q, h + 1.0 / 3.0);
        let g = Self::hue_to_rgb(p, q, h);
        let b = Self::hue_to_rgb(p, q, h - 1.0 / 3.0);

        (
            (r * 255.0).clamp(0.0, 255.0) as u8,
            (g * 255.0).clamp(0.0, 255.0) as u8,
            (b * 255.0).clamp(0.0, 255.0) as u8,
        )
    }

    fn hue_to_rgb(p: f32, q: f32, t: f32) -> f32 {
        let mut t = t;
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }

        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 1.0 / 2.0 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    }

    #[inline]
    fn adjust_pixel(
        pixel: &mut image::Rgba<u8>,
        hue_shift: f32,
        saturation: f32,
        lightness: f32,
        preserve_luminance: bool,
        luminance_standard: LuminanceStandard,
    ) {
        let r = pixel.0[0];
        let g = pixel.0[1];
        let b = pixel.0[2];
        let a = pixel.0[3];

        // Store original luminance if preservation is enabled
        let original_luminance = if preserve_luminance {
            luminance_standard.calculate_luminance(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
            )
        } else {
            0.0
        };

        let (mut h, mut s, mut l) = Self::rgb_to_hsl(r, g, b);

        // Apply hue shift (normalize to 0-360)
        h = (h + hue_shift + 360.0) % 360.0;

        // Apply saturation adjustment
        // saturation: -1 to 1
        // -1 = desaturate completely (grayscale)
        // 0 = no change
        // 1 = double saturation
        s = if saturation < 0.0 {
            s * (1.0 + saturation) // Reduce saturation
        } else {
            s + (1.0 - s) * saturation // Increase saturation
        };
        s = s.clamp(0.0, 1.0);

        // Apply lightness adjustment
        // lightness: -1 to 1
        // -1 = black
        // 0 = no change
        // 1 = white
        l = l + lightness * (1.0 - (2.0 * l - 1.0).abs());
        l = l.clamp(0.0, 1.0);

        // Convert back to RGB
        let (new_r, new_g, new_b) = Self::hsl_to_rgb(h, s, l);

        // Preserve luminance if enabled
        if preserve_luminance && original_luminance > 0.0 {
            let new_r_f = new_r as f32 / 255.0;
            let new_g_f = new_g as f32 / 255.0;
            let new_b_f = new_b as f32 / 255.0;

            let current_luminance =
                luminance_standard.calculate_luminance(new_r_f, new_g_f, new_b_f);

            if current_luminance > 0.001 {
                let scale = original_luminance / current_luminance;
                pixel.0[0] = ((new_r_f * scale) * 255.0).clamp(0.0, 255.0) as u8;
                pixel.0[1] = ((new_g_f * scale) * 255.0).clamp(0.0, 255.0) as u8;
                pixel.0[2] = ((new_b_f * scale) * 255.0).clamp(0.0, 255.0) as u8;
            } else {
                pixel.0[0] = new_r;
                pixel.0[1] = new_g;
                pixel.0[2] = new_b;
            }
        } else {
            pixel.0[0] = new_r;
            pixel.0[1] = new_g;
            pixel.0[2] = new_b;
        }

        // Preserve alpha
        pixel.0[3] = a;
    }
}

impl VideoFilter for HSLAdjustFilter {
    crate::impl_default_video_filter!(HSLAdjustFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Get interpolated values at current time
        let hue_shift = self
            .get_value_at_time(time_ms, "hue_shift", self.hue_shift)
            .clamp(-180.0, 180.0);
        let saturation = self
            .get_value_at_time(time_ms, "saturation", self.saturation)
            .clamp(-1.0, 1.0);
        let lightness = self
            .get_value_at_time(time_ms, "lightness", self.lightness)
            .clamp(-1.0, 1.0);

        // If no adjustments needed, skip processing
        if hue_shift.abs() < 0.01 && saturation.abs() < 0.001 && lightness.abs() < 0.001 {
            return Ok(());
        }

        let preserve_luminance = self.preserve_luminance;
        let luminance_standard = self.luminance_standard;

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                buffer.par_pixels_mut().for_each(|pixel| {
                    Self::adjust_pixel(
                        pixel,
                        hue_shift,
                        saturation,
                        lightness,
                        preserve_luminance,
                        luminance_standard,
                    );
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

        for (property, value) in [
            ("hue_shift", self.hue_shift),
            ("saturation", self.saturation),
            ("lightness", self.lightness),
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
