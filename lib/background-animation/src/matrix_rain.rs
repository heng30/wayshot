use crate::AnimationInit;
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};
use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
pub struct MatrixColumn {
    /// Current y position of the column head
    y: f32,
    /// Fall speed for this column (cells per frame)
    speed: f32,
    /// Length of the trailing characters
    trail_length: usize,
    /// Phase offset for variation
    phase: f32,
    /// Current brightness (for flicker)
    brightness: f32,
    /// Character change timer
    char_change_timer: u32,
}

/// Random floating particle character
#[derive(Debug, Clone)]
pub struct MatrixParticle {
    x: u32,
    y: f32,
    speed: f32,
    brightness: f32,
    ch: char,
    lifetime: u32,
}

/// Pre-rendered glyph image with placement info
#[derive(Debug, Clone)]
struct CachedGlyph {
    data: Vec<u8>,
    width: u32,
    height: u32,
    left: i32,
    top: i32,
}

#[derive(Debug, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct MatrixRainConfig {
    /// Number of columns (default: auto-calculated based on width)
    #[derivative(Default(value = "0"))]
    pub columns: usize,

    /// Character cell size in pixels (default: 16)
    #[derivative(Default(value = "16"))]
    pub cell_size: u32,

    /// Minimum fall speed in cells per frame (default: 0.5)
    #[derivative(Default(value = "0.5"))]
    pub min_speed: f32,

    /// Maximum fall speed in cells per frame (default: 1.5)
    #[derivative(Default(value = "1.5"))]
    pub max_speed: f32,

    /// Trail length in characters (default: 20)
    #[derivative(Default(value = "20"))]
    pub trail_length: usize,

    /// Fade speed for trailing characters (default: 0.04)
    #[derivative(Default(value = "0.04"))]
    pub fade_speed: f32,

    /// Primary color (default: Matrix green)
    #[derivative(Default(value = "(0, 255, 70)"))]
    pub color: (u8, u8, u8),

    /// Background color (default: black)
    #[derivative(Default(value = "(0, 0, 0)"))]
    pub bg_color: (u8, u8, u8),

    /// Glow intensity for head character (default: 2.0)
    #[derivative(Default(value = "2.0"))]
    pub glow_intensity: f32,

    /// Character change probability (default: 0.1)
    #[derivative(Default(value = "0.1"))]
    pub char_change_prob: f32,

    /// Flicker probability for trail characters (default: 0.05)
    #[derivative(Default(value = "0.05"))]
    pub flicker_prob: f32,

    /// Particle density (particles per 1000 cells, default: 5)
    #[derivative(Default(value = "5"))]
    pub particle_density: u32,

    /// Character set to use
    #[setters(skip)]
    #[serde(skip)]
    chars: Vec<char>,

    #[setters(skip)]
    #[serde(skip)]
    width: u32,

    #[setters(skip)]
    #[serde(skip)]
    height: u32,

    #[setters(skip)]
    #[serde(skip)]
    fps: u32,

    #[setters(skip)]
    #[serde(skip)]
    total_frames: usize,

    #[setters(skip)]
    #[serde(skip)]
    current_frame: usize,

    #[setters(skip)]
    #[serde(skip)]
    matrix_columns: Vec<MatrixColumn>,

    /// Floating particles
    #[setters(skip)]
    #[serde(skip)]
    particles: Vec<MatrixParticle>,

    /// Font system for glyph rendering
    #[setters(skip)]
    #[serde(skip)]
    font_system: Option<FontSystem>,

    /// Swash cache for glyph images
    #[setters(skip)]
    #[serde(skip)]
    swash_cache: Option<SwashCache>,

    /// Pre-rendered glyph cache: char -> glyph image
    #[setters(skip)]
    #[serde(skip)]
    glyph_cache: HashMap<char, CachedGlyph>,
}

impl Clone for MatrixRainConfig {
    fn clone(&self) -> Self {
        Self {
            columns: self.columns,
            cell_size: self.cell_size,
            min_speed: self.min_speed,
            max_speed: self.max_speed,
            trail_length: self.trail_length,
            fade_speed: self.fade_speed,
            color: self.color,
            bg_color: self.bg_color,
            glow_intensity: self.glow_intensity,
            char_change_prob: self.char_change_prob,
            flicker_prob: self.flicker_prob,
            particle_density: self.particle_density,
            chars: self.chars.clone(),
            width: self.width,
            height: self.height,
            fps: self.fps,
            total_frames: self.total_frames,
            current_frame: self.current_frame,
            matrix_columns: self.matrix_columns.clone(),
            particles: self.particles.clone(),
            font_system: None,
            swash_cache: None,
            glyph_cache: HashMap::new(),
        }
    }
}

impl MatrixRainConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn init_chars() -> Vec<char> {
        let matrix_chars = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz@#$%^&*";
        matrix_chars.chars().collect()
    }

    fn prerender_glyphs(&mut self) {
        if self.font_system.is_none() || self.swash_cache.is_none() {
            return;
        }

        let font_system = self.font_system.as_mut().unwrap();
        let swash_cache = self.swash_cache.as_mut().unwrap();
        let font_size = self.cell_size as f32 * 1.2;

        let metrics = Metrics {
            font_size,
            line_height: font_size,
        };

        let attrs = Attrs::new();

        for ch in &self.chars {
            let mut buffer = Buffer::new(font_system, metrics);
            buffer.set_text(&ch.to_string(), &attrs, Shaping::Basic, None);
            buffer.shape_until_scroll(font_system, false);

            for run in buffer.layout_runs() {
                for glyph in run.glyphs {
                    let physical = glyph.physical((0.0, 0.0), 1.0);

                    if let Some(glyph_img) = swash_cache.get_image(font_system, physical.cache_key) {
                        let placement = &glyph_img.placement;
                        let cached = CachedGlyph {
                            data: glyph_img.data.clone(),
                            width: placement.width,
                            height: placement.height,
                            left: placement.left,
                            top: placement.top,
                        };
                        self.glyph_cache.insert(*ch, cached);
                    }
                }
            }
        }

        log::info!(
            "Pre-rendered {} glyphs for Matrix rain",
            self.glyph_cache.len()
        );
    }

    /// Get a random character based on position and frame
    fn get_char(&self, col_idx: usize, trail_idx: usize, frame: usize) -> char {
        if self.chars.is_empty() {
            return '0';
        }
        let base_idx = (col_idx * 7 + trail_idx * 3 + frame) % self.chars.len();
        self.chars[base_idx]
    }

    /// Check if character should change this frame
    fn should_change_char(&self, col_idx: usize, trail_idx: usize, frame: usize) -> bool {
        let change_seed = crate::pseudo_phase(col_idx + trail_idx * 13 + frame * 17);
        change_seed < self.char_change_prob
    }

    /// Check if character should flicker this frame
    fn should_flicker(&self, col_idx: usize, trail_idx: usize, frame: usize) -> bool {
        let flicker_seed = crate::pseudo_phase(col_idx * 11 + trail_idx * 5 + frame * 23);
        flicker_seed < self.flicker_prob
    }

    fn update_columns(&mut self) {
        let height_cells = self.height / self.cell_size;

        self.matrix_columns.par_iter_mut().for_each(|col| {
            // Move column down slowly
            col.y += col.speed;

            // Increment character change timer
            col.char_change_timer += 1;

            // Reset when past bottom
            if col.y > height_cells as f32 + col.trail_length as f32 {
                col.y = -(col.trail_length as f32) * crate::pseudo_phase(col.phase as usize) * 3.0;
                col.speed = self.min_speed
                    + (self.max_speed - self.min_speed) * crate::pseudo_phase(col.phase as usize + 100);
                col.trail_length = (self.trail_length as f32
                    * (0.5 + 0.5 * crate::pseudo_phase(col.phase as usize + 200)))
                    as usize;
                col.char_change_timer = 0;
            }

            // Head character brightness with flicker
            let head_flicker = crate::pseudo_phase(self.current_frame + col.phase as usize);
            col.brightness = 0.8 + 0.2 * head_flicker;
        });

        // Update particles
        self.particles.iter_mut().for_each(|p| {
            p.y += p.speed;
            p.lifetime -= 1;

            // Fade out as lifetime decreases
            p.brightness = p.brightness * 0.95;
        });

        // Remove dead particles
        self.particles.retain(|p| p.lifetime > 0 && p.y < self.height as f32 / self.cell_size as f32);

        // Spawn new particles
        let num_cells = (self.width / self.cell_size) * (self.height / self.cell_size);
        let target_particles = (num_cells / 1000 * self.particle_density) as usize;
        let spawn_prob = if self.particles.len() < target_particles {
            0.02
        } else {
            0.005
        };

        let spawn_seed = crate::pseudo_phase(self.current_frame);
        if spawn_seed < spawn_prob && self.particles.len() < target_particles * 2 && !self.chars.is_empty() {
            let col = (crate::pseudo_phase(self.current_frame + 1) * (self.width / self.cell_size) as f32) as u32;
            let y = crate::pseudo_phase(self.current_frame + 2) * (self.height as f32 / self.cell_size as f32);
            let ch_idx = (crate::pseudo_phase(self.current_frame + 3) * self.chars.len() as f32) as usize;
            let ch = self.chars[ch_idx.min(self.chars.len() - 1)];

            self.particles.push(MatrixParticle {
                x: col * self.cell_size,
                y,
                speed: crate::pseudo_phase(self.current_frame + 4) * 0.5 + 0.2,
                brightness: 0.3 + crate::pseudo_phase(self.current_frame + 5) * 0.4,
                ch,
                lifetime: 30 + (crate::pseudo_phase(self.current_frame + 6) * 30.0) as u32,
            });
        }
    }

    fn generate_frame(&self) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Fill background
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        let cell_size = self.cell_size;
        let chars = &self.chars;
        let base_color = self.color;
        let glow_intensity = self.glow_intensity;
        let fade_speed = self.fade_speed;
        let glyph_cache = &self.glyph_cache;

        // Draw particles first (background layer)
        for particle in &self.particles {
            let y = (particle.y as u32 * cell_size).min(self.height - cell_size);
            let brightness = particle.brightness;

            let r = (base_color.0 as f32 * brightness * 0.3).min(255.0) as u8;
            let g = (base_color.1 as f32 * brightness * 0.3).min(255.0) as u8;
            let b = (base_color.2 as f32 * brightness * 0.3).min(255.0) as u8;

            Self::draw_cached_glyph(
                &mut img,
                particle.x,
                y,
                particle.ch,
                r,
                g,
                b,
                cell_size,
                glyph_cache,
                false,
            );
        }

        // Draw each column
        for (col_idx, column) in self.matrix_columns.iter().enumerate() {
            let x = col_idx as u32 * cell_size;

            // Draw trailing characters
            for trail_idx in 0..column.trail_length {
                let char_y = column.y - trail_idx as f32;

                // Skip if outside visible area
                if char_y < 0.0 || char_y >= self.height as f32 / cell_size as f32 {
                    continue;
                }

                let y = (char_y as u32 * cell_size).min(self.height - cell_size);

                // Check for flicker (character briefly disappears)
                if trail_idx > 0 && self.should_flicker(col_idx, trail_idx, self.current_frame) {
                    continue;
                }

                // Calculate fade: head is brightest, trail fades
                let fade_factor = if trail_idx == 0 {
                    // Head character - bright white-ish green with glow
                    column.brightness * glow_intensity
                } else {
                    // Trail characters - fading with slight brightness variation
                    let base_fade = (1.0 - trail_idx as f32 * fade_speed).max(0.0);
                    let brightness_var = crate::pseudo_phase(col_idx + trail_idx + self.current_frame) * 0.1;
                    (base_fade + brightness_var) * column.brightness
                };

                // Get character - may change each frame for particle effect
                let ch = if self.should_change_char(col_idx, trail_idx, self.current_frame) && !chars.is_empty() {
                    // Randomly change character
                    let new_idx = (crate::pseudo_phase(col_idx + trail_idx + self.current_frame * 3) * chars.len() as f32) as usize;
                    chars[new_idx.min(chars.len() - 1)]
                } else {
                    self.get_char(col_idx, trail_idx, self.current_frame)
                };

                // Calculate color with fade
                let r = (base_color.0 as f32 * fade_factor).min(255.0) as u8;
                let g = (base_color.1 as f32 * fade_factor).min(255.0) as u8;
                let b = (base_color.2 as f32 * fade_factor).min(255.0) as u8;

                // Draw character
                Self::draw_cached_glyph(
                    &mut img,
                    x,
                    y,
                    ch,
                    r,
                    g,
                    b,
                    cell_size,
                    glyph_cache,
                    trail_idx == 0,
                );
            }
        }

        img
    }

    fn draw_cached_glyph(
        img: &mut RgbaImage,
        x: u32,
        y: u32,
        ch: char,
        r: u8,
        g: u8,
        b: u8,
        cell_size: u32,
        glyph_cache: &HashMap<char, CachedGlyph>,
        is_head: bool,
    ) {
        let glyph = glyph_cache.get(&ch);

        if let Some(cached) = glyph {
            let glyph_x = x as i32 + (cell_size as i32 - cached.width as i32) / 2;
            let baseline_y = y as i32 + cell_size as i32;
            let glyph_y = baseline_y - cached.top;

            let mut idx = 0;
            for dy in 0..cached.height as i32 {
                for dx in 0..cached.width as i32 {
                    let alpha = cached.data[idx] as f32 / 255.0;
                    idx += 1;

                    if alpha > 0.01 {
                        let img_x = (glyph_x + cached.left + dx) as u32;
                        let img_y = (glyph_y + dy) as u32;

                        if img_x < img.width() && img_y < img.height() {
                            let pixel = img.get_pixel_mut(img_x, img_y);

                            let (final_r, final_g, final_b) = if is_head {
                                // Head: brighter, slight white tint
                                let glow = 1.5;
                                (
                                    ((r as f32 * glow + 50.0) * alpha).min(255.0) as u8,
                                    ((g as f32 * glow) * alpha).min(255.0) as u8,
                                    ((b as f32 * glow) * alpha).min(255.0) as u8,
                                )
                            } else {
                                (r, g, b)
                            };

                            let src_alpha = alpha;
                            let dst_alpha = pixel[3] as f32 / 255.0;
                            let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

                            if out_alpha > 0.0 {
                                pixel[0] = ((final_r as f32 * src_alpha
                                    + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[1] = ((final_g as f32 * src_alpha
                                    + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[2] = ((final_b as f32 * src_alpha
                                    + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[3] = (out_alpha * 255.0) as u8;
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Iterator for MatrixRainConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        self.update_columns();
        let frame = self.generate_frame();
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for MatrixRainConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;

        if self.chars.is_empty() {
            self.chars = Self::init_chars();
        }

        self.font_system = Some(FontSystem::new());
        self.swash_cache = Some(SwashCache::new());
        self.prerender_glyphs();

        let num_columns = if self.columns == 0 {
            width / self.cell_size
        } else {
            self.columns as u32
        };

        self.matrix_columns = Vec::with_capacity(num_columns as usize);
        for i in 0..num_columns as usize {
            let speed = self.min_speed
                + (self.max_speed - self.min_speed) * crate::pseudo_phase(i);
            let trail_length = (self.trail_length as f32
                * (0.5 + 0.5 * crate::pseudo_phase(i + 100)))
                as usize;
            let y = -(trail_length as f32) * crate::pseudo_phase(i + 200) * 3.0;

            self.matrix_columns.push(MatrixColumn {
                y,
                speed,
                trail_length,
                phase: crate::pseudo_phase(i + 300) * 10.0,
                brightness: 1.0,
                char_change_timer: 0,
            });
        }

        self.particles = Vec::new();
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.matrix_columns.clear();
        self.particles.clear();
        self.glyph_cache.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(MatrixRainConfig);