use crate::AnimationInit;
use image::RgbaImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GlitchConfig {
    /// Overall intensity (0.0-1.0), affects all sub-effects by default
    #[derivative(Default(value = "0.3"))]
    pub intensity: f32,

    /// Scan line effect (default: enabled)
    #[derivative(Default(value = "true"))]
    pub scan_lines_enabled: bool,

    /// Scan line intensity (default: follows overall intensity)
    #[setters(skip)]
    #[serde(skip)]
    pub scan_line_intensity: f32,

    /// Scan line spacing in pixels (default: 3)
    #[derivative(Default(value = "3"))]
    pub scan_line_spacing: u32,

    /// RGB channel split effect (default: enabled)
    #[derivative(Default(value = "true"))]
    pub rgb_split_enabled: bool,

    /// RGB split offset in pixels (default: 3)
    #[derivative(Default(value = "3"))]
    pub rgb_split_offset: i32,

    /// Block shift effect (default: enabled)
    #[derivative(Default(value = "true"))]
    pub block_shift_enabled: bool,

    /// Block shift intensity (default: follows overall intensity)
    #[setters(skip)]
    #[serde(skip)]
    pub block_shift_intensity: f32,

    /// Maximum block shift offset in pixels (default: 8)
    #[derivative(Default(value = "8"))]
    pub block_shift_max_offset: i32,

    /// Noise interference effect (default: enabled)
    #[derivative(Default(value = "true"))]
    pub noise_enabled: bool,

    /// Noise intensity (default: follows overall intensity)
    #[setters(skip)]
    #[serde(skip)]
    pub noise_intensity: f32,

    /// Animation speed (how fast effects change, default: 0.5)
    #[derivative(Default(value = "0.5"))]
    pub animation_speed: f32,

    #[derivative(Default(value = "(0, 0, 0)"))]
    pub bg_color: (u8, u8, u8),

    #[setters(skip)]
    #[serde(skip)]
    width: u32,

    #[setters(skip)]
    #[serde(skip)]
    height: u32,

    #[setters(skip)]
    #[serde(skip)]
    total_frames: usize,

    #[setters(skip)]
    #[serde(skip)]
    current_frame: usize,

    #[setters(skip)]
    #[serde(skip)]
    fps: u32,

    /// Pre-computed random values for block shifts
    #[setters(skip)]
    #[serde(skip)]
    block_seeds: Vec<u32>,
}

impl GlitchConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Simple hash function for pseudo-random values
    fn simple_hash(seed: u32) -> u32 {
        let mut val = seed.wrapping_mul(2654435761);
        val ^= val >> 17;
        val = val.wrapping_mul(2654435761);
        val ^= val >> 13;
        val
    }

    /// Generate pseudo-random value from frame and seed
    fn pseudo_random(frame: usize, seed: u32, min: f32, max: f32) -> f32 {
        let combined = (frame as u32).wrapping_add(seed);
        let hash = Self::simple_hash(combined);
        let normalized = (hash as f32) / (u32::MAX as f32);
        min + normalized * (max - min)
    }

    fn generate_frame(&self, frame: usize) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Time in seconds
        let time = frame as f32 / self.fps as f32;
        let time_phase = time * self.animation_speed;

        // Fill with background color first
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        let width = self.width;
        let height = self.height;

        // Process in parallel
        img.as_mut()
            .par_chunks_mut(4)
            .enumerate()
            .for_each(|(pixel_idx, pixel_bytes)| {
                let x = (pixel_idx % width as usize) as i32;
                let y = (pixel_idx / width as usize) as i32;

                let mut r = self.bg_color.0 as f32;
                let mut g = self.bg_color.1 as f32;
                let mut b = self.bg_color.2 as f32;

                // Apply scan lines
                if self.scan_lines_enabled {
                    let scan_y = y as u32 % (self.scan_line_spacing + 1);
                    if scan_y == 0 {
                        // Slight darkening on scan line rows
                        let scan_factor = (time_phase * 2.0).sin() * 0.5 + 0.5;
                        let darken = 1.0 - self.scan_line_intensity * 0.15 * scan_factor;
                        r *= darken;
                        g *= darken;
                        b *= darken;
                    }
                }

                // Apply RGB channel split
                if self.rgb_split_enabled {
                    // Offset varies slightly over time
                    let offset_base = self.rgb_split_offset as f32;
                    let offset_var = (time_phase * 1.3).sin() * 0.3;
                    let _current_offset = ((offset_base + offset_var).abs() as i32).max(1);

                    // For display, we'll add a subtle color tint at edges
                    // This creates a subtle chromatic aberration feel
                    let edge_factor = (x as f32 / width as f32 - 0.5).abs();
                    if edge_factor > 0.4 {
                        let tint = self.intensity * 0.1 * (edge_factor - 0.4) / 0.1;
                        // Slight red tint on left, blue tint on right
                        if x < width as i32 / 2 {
                            r += tint * 15.0;
                        } else {
                            b += tint * 15.0;
                        }
                    }
                }

                // Apply block shift (horizontal strip displacement)
                if self.block_shift_enabled {
                    let block_height = 4u32;
                    let block_idx = y as u32 / block_height;
                    // Use height to normalize the seed index
                    let seed_idx = (block_idx as usize)
                        .min(self.block_seeds.len() - 1)
                        .min(height as usize / 4 - 1);
                    let seed = self.block_seeds[seed_idx];

                    // Only shift some blocks, based on frame time
                    let shift_chance = Self::pseudo_random(frame, seed, 0.0, 1.0);
                    if shift_chance > 0.7 {
                        // Occasional shift
                        let shift_amount = Self::pseudo_random(
                            frame ^ seed as usize,
                            seed,
                            -(self.block_shift_max_offset as f32),
                            self.block_shift_max_offset as f32,
                        ) * self.block_shift_intensity;

                        // Visual indicator: slight brightness variation
                        let brightness = 1.0 + shift_amount * 0.01;
                        r *= brightness;
                        g *= brightness;
                        b *= brightness;
                    }
                }

                // Apply noise
                if self.noise_enabled {
                    let noise_seed =
                        Self::simple_hash((x as u32).wrapping_add((y as u32).wrapping_mul(width)));
                    let noise_phase =
                        (time_phase * 3.0 + noise_seed as f32 / u32::MAX as f32).sin();
                    if noise_phase > 0.95 {
                        // Occasional noise pixel
                        let noise_val = Self::pseudo_random(frame, noise_seed, -1.0, 1.0)
                            * self.noise_intensity
                            * 20.0;
                        r = (r + noise_val).clamp(0.0, 255.0);
                        g = (g + noise_val).clamp(0.0, 255.0);
                        b = (b + noise_val).clamp(0.0, 255.0);
                    }
                }

                // Additional glitch: occasional horizontal tear
                let tear_seed = Self::simple_hash((y as u32).wrapping_mul(7));
                let tear_chance = Self::pseudo_random(frame, tear_seed, 0.0, 1.0);
                if tear_chance > 0.98 && self.intensity > 0.2 {
                    // Rare full-row brightness shift
                    let shift = (time_phase * 7.0).sin() * 10.0 * self.intensity;
                    r = (r + shift).clamp(0.0, 255.0);
                    g = (g + shift).clamp(0.0, 255.0);
                    b = (b + shift).clamp(0.0, 255.0);
                }

                pixel_bytes[0] = r.clamp(0.0, 255.0) as u8;
                pixel_bytes[1] = g.clamp(0.0, 255.0) as u8;
                pixel_bytes[2] = b.clamp(0.0, 255.0) as u8;
                pixel_bytes[3] = 255;
            });

        img
    }
}

impl Iterator for GlitchConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        let frame = self.generate_frame(self.current_frame);
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for GlitchConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        // Set sub-intensities from main intensity if not explicitly set
        if self.scan_line_intensity == 0.0 {
            self.scan_line_intensity = self.intensity;
        }
        if self.block_shift_intensity == 0.0 {
            self.block_shift_intensity = self.intensity;
        }
        if self.noise_intensity == 0.0 {
            self.noise_intensity = self.intensity;
        }

        // Pre-compute block seeds for consistent randomness
        // One seed per potential block (height / 4 = max blocks)
        let num_blocks = (height / 4) as usize;
        self.block_seeds = (0..num_blocks)
            .map(|i| Self::simple_hash(i as u32))
            .collect();
    }

    fn reset(&mut self) {
        self.current_frame = 0;
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(GlitchConfig);
