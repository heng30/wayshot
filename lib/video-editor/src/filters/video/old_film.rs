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

/// Old Film / Damaged Film Effect Filter
///
/// Simulates vintage celluloid film damage effects including:
/// - Scratches: Random vertical scratch lines
/// - Dust: Random small dust particles
/// - Flicker: Brightness variation over time
/// - Vertical Lines: Static artifact lines
/// - Jitter: Frame position displacement
/// - Sepia: Vintage color tone shift
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
pub struct OldFilmFilter {
    /// Seed for reproducible randomness (per segment) - NOT keyframeable
    #[derivative(Default(value = "0"))]
    pub seed: u64,

    // === Scratches ===
    /// Intensity of scratches (0.0-1.0) - KEYFRAMEABLE
    #[derivative(Default(value = "0.3"))]
    pub scratch_intensity: f32,
    /// Average number of scratches per frame - NOT keyframeable
    #[derivative(Default(value = "3"))]
    pub scratch_count: u32,
    /// Width of scratches in pixels (base 1080p) - NOT keyframeable
    #[derivative(Default(value = "2"))]
    pub scratch_width: u32,

    // === Dust ===
    /// Intensity of dust particles (0.0-1.0) - KEYFRAMEABLE
    #[derivative(Default(value = "0.2"))]
    pub dust_intensity: f32,
    /// Average number of dust particles per frame - NOT keyframeable
    #[derivative(Default(value = "50"))]
    pub dust_count: u32,
    /// Maximum dust particle size in pixels - NOT keyframeable
    #[derivative(Default(value = "4"))]
    pub dust_size_max: u32,

    // === Flicker ===
    /// Brightness variation amount (0.0-0.3) - KEYFRAMEABLE
    #[derivative(Default(value = "0.1"))]
    pub flicker_intensity: f32,
    /// How fast flicker changes (1.0-10.0) - KEYFRAMEABLE
    #[derivative(Default(value = "3.0"))]
    pub flicker_speed: f32,

    // === Vertical Lines ===
    /// Intensity of vertical artifact lines (0.0-1.0) - KEYFRAMEABLE
    #[derivative(Default(value = "0.2"))]
    pub vertical_lines_intensity: f32,
    /// Number of vertical lines - NOT keyframeable
    #[derivative(Default(value = "5"))]
    pub vertical_lines_count: u32,

    // === Jitter ===
    /// Pixel displacement amount (0.0-10.0) - KEYFRAMEABLE
    #[derivative(Default(value = "2.0"))]
    pub jitter_intensity: f32,

    // === Sepia ===
    /// Sepia tone amount (0.0-1.0) - KEYFRAMEABLE
    #[derivative(Default(value = "0.3"))]
    pub sepia_intensity: f32,

    /// Keyframe tracks for animatable properties
    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl OldFilmFilter {
    pub const NAME: &'static str = "old film";

    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![
            AnimatableProperty::float("scratch_intensity", "Scratch Intensity", 0.0, 1.0, 0.3),
            AnimatableProperty::float("dust_intensity", "Dust Intensity", 0.0, 1.0, 0.2),
            AnimatableProperty::float("flicker_intensity", "Flicker Intensity", 0.0, 0.3, 0.1),
            AnimatableProperty::float("flicker_speed", "Flicker Speed", 1.0, 10.0, 3.0),
            AnimatableProperty::float("vertical_lines_intensity", "Vertical Lines", 0.0, 1.0, 0.2),
            AnimatableProperty::float("jitter_intensity", "Jitter", 0.0, 10.0, 2.0),
            AnimatableProperty::float("sepia_intensity", "Sepia", 0.0, 1.0, 0.3),
        ]
    }

    fn get_value_at_time(&self, time_ms: i64, property: &str, default: f32) -> f32 {
        self.keyframe_tracks
            .get_track(property)
            .map(|track| get_float_at_time(track, time_ms, default))
            .unwrap_or(default)
    }

    /// Hash-based pseudo-random number generator for deterministic effects.
    /// Returns a value in range [0, 1).
    fn hash_to_float(mut h: u64) -> f32 {
        // scramble the hash
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^= h >> 33;
        // Use the upper 32 bits to avoid overflow issues
        ((h >> 32) as f32) / (u32::MAX as f32)
    }

    /// Generate a random value based on seed and time.
    fn random_at_time(seed: u64, time_ms: i64, index: u64) -> f32 {
        let combined = seed
            .wrapping_add(time_ms as u64)
            .wrapping_add(index * 12345);
        Self::hash_to_float(combined)
    }

    /// Apply sepia tone effect to a pixel (inline for performance).
    #[inline]
    fn apply_sepia_pixel(r: u8, g: u8, b: u8, intensity: f32) -> [u8; 3] {
        let rf = r as f32;
        let gf = g as f32;
        let bf = b as f32;

        // Sepia tone formula
        let sepia_r = (rf * 0.393 + gf * 0.769 + bf * 0.189).min(255.0);
        let sepia_g = (rf * 0.349 + gf * 0.686 + bf * 0.168).min(255.0);
        let sepia_b = (rf * 0.272 + gf * 0.534 + bf * 0.131).min(255.0);

        // Blend with original based on intensity
        [
            (rf + (sepia_r - rf) * intensity) as u8,
            (gf + (sepia_g - gf) * intensity) as u8,
            (bf + (sepia_b - bf) * intensity) as u8,
        ]
    }

    /// Apply brightness adjustment (flicker) to a pixel (inline for performance).
    #[inline]
    fn apply_flicker_pixel(r: u8, g: u8, b: u8, flicker_amount: f32) -> [u8; 3] {
        let adjustment = flicker_amount * 255.0;
        [
            ((r as f32 + adjustment).clamp(0.0, 255.0)) as u8,
            ((g as f32 + adjustment).clamp(0.0, 255.0)) as u8,
            ((b as f32 + adjustment).clamp(0.0, 255.0)) as u8,
        ]
    }

    /// Apply base effects (sepia + flicker) to entire image in parallel.
    fn apply_base_effects(
        buffer: &mut image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
        sepia_intensity: f32,
        flicker_amount: f32,
    ) {
        let width = buffer.width();
        let height = buffer.height();

        // Process rows in parallel for sepia and flicker
        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let pixel = buffer.get_pixel(x, y);
                        let r = pixel.0[0];
                        let g = pixel.0[1];
                        let b = pixel.0[2];
                        let a = pixel.0[3];

                        // Apply sepia
                        let [r, g, b] = if sepia_intensity > 0.0 {
                            Self::apply_sepia_pixel(r, g, b, sepia_intensity)
                        } else {
                            [r, g, b]
                        };

                        // Apply flicker
                        let [r, g, b] = Self::apply_flicker_pixel(r, g, b, flicker_amount);

                        [r, g, b, a]
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

    /// Apply jitter effect by copying pixels with offset.
    fn apply_jitter(
        buffer: &mut image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
        jitter_x: i32,
        jitter_y: i32,
    ) {
        let width = buffer.width();
        let height = buffer.height();

        // Create a copy of the original buffer
        let original = buffer.clone();

        // Process rows in parallel
        let rows: Vec<Vec<[u8; 4]>> = (0..height)
            .into_par_iter()
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let src_x = (x as i32 - jitter_x).clamp(0, width as i32 - 1) as u32;
                        let src_y = (y as i32 - jitter_y).clamp(0, height as i32 - 1) as u32;
                        original.get_pixel(src_x, src_y).0
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

    /// Draw a vertical scratch line.
    fn draw_scratch(
        buffer: &mut image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
        x: u32,
        width: u32,
        intensity: f32,
    ) {
        let height = buffer.height();
        let opacity = intensity * 0.8; // Max opacity for scratches

        for y in 0..height {
            for dx in 0..width {
                let px = x + dx;
                if px < buffer.width() {
                    let pixel = buffer.get_pixel_mut(px, y);
                    // Light scratches (bright lines)
                    let brighten = opacity * 100.0;
                    pixel.0[0] = ((pixel.0[0] as f32 + brighten).min(255.0)) as u8;
                    pixel.0[1] = ((pixel.0[1] as f32 + brighten).min(255.0)) as u8;
                    pixel.0[2] = ((pixel.0[2] as f32 + brighten).min(255.0)) as u8;
                }
            }
        }
    }

    /// Draw a dust particle (small dark or light spot).
    fn draw_dust(
        buffer: &mut image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
        x: u32,
        y: u32,
        size: u32,
        intensity: f32,
        is_dark: bool,
    ) {
        let width = buffer.width();
        let height = buffer.height();

        for dx in 0..size {
            for dy in 0..size {
                let px = x + dx;
                let py = y + dy;
                if px < width && py < height {
                    let pixel = buffer.get_pixel_mut(px, py);
                    if is_dark {
                        // Dark dust particle
                        let darken = intensity * 80.0;
                        pixel.0[0] = ((pixel.0[0] as f32 - darken).max(0.0)) as u8;
                        pixel.0[1] = ((pixel.0[1] as f32 - darken).max(0.0)) as u8;
                        pixel.0[2] = ((pixel.0[2] as f32 - darken).max(0.0)) as u8;
                    } else {
                        // Light dust particle (white speck)
                        let brighten = intensity * 60.0;
                        pixel.0[0] = ((pixel.0[0] as f32 + brighten).min(255.0)) as u8;
                        pixel.0[1] = ((pixel.0[1] as f32 + brighten).min(255.0)) as u8;
                        pixel.0[2] = ((pixel.0[2] as f32 + brighten).min(255.0)) as u8;
                    }
                }
            }
        }
    }

    /// Draw a vertical artifact line (static damage line).
    fn draw_vertical_line(
        buffer: &mut image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
        x: u32,
        intensity: f32,
    ) {
        let height = buffer.height();
        let width = buffer.width();

        if x >= width {
            return;
        }

        for y in 0..height {
            let pixel = buffer.get_pixel_mut(x, y);
            // Semi-transparent vertical artifact
            let darken = intensity * 30.0;
            pixel.0[0] = ((pixel.0[0] as f32 - darken).max(0.0)) as u8;
            pixel.0[1] = ((pixel.0[1] as f32 - darken).max(0.0)) as u8;
            pixel.0[2] = ((pixel.0[2] as f32 - darken).max(0.0)) as u8;
        }
    }
}

impl VideoFilter for OldFilmFilter {
    crate::impl_default_video_filter!(OldFilmFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let time_ms = data.relative_timeline_offset.as_millis() as i64;

        // Get interpolated values at current time
        let scratch_intensity = self
            .get_value_at_time(time_ms, "scratch_intensity", self.scratch_intensity)
            .clamp(0.0, 1.0);
        let dust_intensity = self
            .get_value_at_time(time_ms, "dust_intensity", self.dust_intensity)
            .clamp(0.0, 1.0);
        let flicker_intensity = self
            .get_value_at_time(time_ms, "flicker_intensity", self.flicker_intensity)
            .clamp(0.0, 0.3);
        let flicker_speed = self
            .get_value_at_time(time_ms, "flicker_speed", self.flicker_speed)
            .clamp(1.0, 10.0);
        let vertical_lines_intensity = self
            .get_value_at_time(
                time_ms,
                "vertical_lines_intensity",
                self.vertical_lines_intensity,
            )
            .clamp(0.0, 1.0);
        let jitter_intensity = self
            .get_value_at_time(time_ms, "jitter_intensity", self.jitter_intensity)
            .clamp(0.0, 10.0);
        let sepia_intensity = self
            .get_value_at_time(time_ms, "sepia_intensity", self.sepia_intensity)
            .clamp(0.0, 1.0);

        // Calculate flicker amount using sine wave + noise
        let flicker_base = (time_ms as f32 / 1000.0 * flicker_speed).sin();
        let flicker_noise = Self::random_at_time(self.seed, time_ms, 1000) - 0.5;
        let flicker_amount = flicker_intensity * (flicker_base * 0.5 + flicker_noise * 0.5);

        // Calculate jitter offsets
        let jitter_x = if jitter_intensity > 0.0 {
            let rand_x = Self::random_at_time(self.seed, time_ms, 2000) * 2.0 - 1.0;
            (rand_x * jitter_intensity).round() as i32
        } else {
            0
        };
        let jitter_y = if jitter_intensity > 0.0 {
            let rand_y = Self::random_at_time(self.seed, time_ms, 2001) * 2.0 - 1.0;
            (rand_y * jitter_intensity * 0.5).round() as i32 // Less vertical jitter
        } else {
            0
        };

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                let width = buffer.width();
                let height = buffer.height();

                // Scale factors for resolution (base is 1080p)
                let scale = height as f32 / 1080.0;
                let scaled_scratch_width = (self.scratch_width as f32 * scale).max(1.0) as u32;
                let scaled_dust_size_max = (self.dust_size_max as f32 * scale).max(1.0) as u32;

                // 1. Apply sepia tone + flicker (parallelized)
                Self::apply_base_effects(buffer, sepia_intensity, flicker_amount);

                // 2. Apply jitter (parallelized)
                if jitter_intensity > 0.0 && (jitter_x != 0 || jitter_y != 0) {
                    Self::apply_jitter(buffer, jitter_x, jitter_y);
                }

                // 3. Draw scratches (must be sequential - additive effects)
                if scratch_intensity > 0.0 {
                    for i in 0..self.scratch_count {
                        let rand_x = Self::random_at_time(self.seed, time_ms, i as u64);
                        let x = (rand_x * width as f32) as u32;
                        Self::draw_scratch(buffer, x, scaled_scratch_width, scratch_intensity);
                    }
                }

                // 4. Draw dust particles (must be sequential - additive effects)
                if dust_intensity > 0.0 {
                    for i in 0..self.dust_count {
                        let i64 = i as u64;
                        let rand_x = Self::random_at_time(self.seed, time_ms, i64 + 100);
                        let rand_y = Self::random_at_time(self.seed, time_ms, i64 + 101);
                        let rand_size = Self::random_at_time(self.seed, time_ms, i64 + 102);
                        let rand_dark = Self::random_at_time(self.seed, time_ms, i64 + 103);

                        let x = (rand_x * width as f32) as u32;
                        let y = (rand_y * height as f32) as u32;
                        let size = (rand_size * scaled_dust_size_max as f32).max(1.0) as u32;
                        let is_dark = rand_dark > 0.5;

                        Self::draw_dust(buffer, x, y, size, dust_intensity, is_dark);
                    }
                }

                // 5. Draw vertical artifact lines (must be sequential - additive effects)
                if vertical_lines_intensity > 0.0 {
                    for i in 0..self.vertical_lines_count {
                        let rand_x = Self::random_at_time(self.seed, time_ms, i as u64 + 500);
                        let x = (rand_x * width as f32) as u32;
                        Self::draw_vertical_line(buffer, x, vertical_lines_intensity);
                    }
                }
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
            ("scratch_intensity", self.scratch_intensity),
            ("dust_intensity", self.dust_intensity),
            ("flicker_intensity", self.flicker_intensity),
            ("flicker_speed", self.flicker_speed),
            ("vertical_lines_intensity", self.vertical_lines_intensity),
            ("jitter_intensity", self.jitter_intensity),
            ("sepia_intensity", self.sepia_intensity),
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

