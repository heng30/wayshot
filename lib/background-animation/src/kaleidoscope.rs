use crate::AnimationInit;
use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct KaleidoscopeConfig {
    /// Number of symmetric segments (default: 8)
    #[derivative(Default(value = "8"))]
    pub segments: usize,

    /// Rotation speed in radians per second (default: 0.5)
    #[derivative(Default(value = "0.5"))]
    pub rotation_speed: f32,

    /// Center point as percentage of width/height (default: center)
    #[derivative(Default(value = "(0.5, 0.5)"))]
    pub center: (f32, f32),

    /// Color palette for the pattern (default: rainbow gradient)
    #[derivative(Default(value = "vec![
        (255, 50, 50),   // Red
        (255, 150, 50),  // Orange
        (255, 255, 50),  // Yellow
        (50, 255, 50),   // Green
        (50, 255, 255),  // Cyan
        (50, 150, 255),  // Blue
        (150, 50, 255),  // Purple
        (255, 50, 150),  // Pink
    ]"))]
    pub colors: Vec<(u8, u8, u8)>,

    /// Background color (default: dark)
    #[derivative(Default(value = "(10, 10, 20)"))]
    pub bg_color: (u8, u8, u8),

    /// Pattern scale factor (default: 1.0)
    #[derivative(Default(value = "1.0"))]
    pub scale: f32,

    /// Pattern complexity (number of radial bands) (default: 6)
    #[derivative(Default(value = "6"))]
    pub complexity: usize,

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
}

impl KaleidoscopeConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn generate_frame(&self, frame: usize) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        let time = frame as f32 / self.fps as f32;
        let rotation = time * self.rotation_speed;

        let cx = self.center.0 * self.width as f32;
        let cy = self.center.1 * self.height as f32;

        let width = self.width;
        let segments = self.segments;
        let colors = &self.colors;
        let complexity = self.complexity;
        let scale = self.scale;

        // Fill with background first
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        // Process each pixel
        img.as_mut()
            .par_chunks_mut(4)
            .enumerate()
            .for_each(|(pixel_idx, pixel_bytes)| {
                let x = (pixel_idx % width as usize) as f32;
                let y = (pixel_idx / width as usize) as f32;

                // Convert to polar coordinates centered at center point
                let dx = x - cx;
                let dy = y - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let mut angle = dy.atan2(dx);

                // Apply rotation
                angle += rotation;

                // Normalize angle to [0, segment_angle)
                let segment_angle = std::f32::consts::TAU / segments as f32;
                let mut normalized_angle = angle % segment_angle;

                // Ensure positive angle
                if normalized_angle < 0.0 {
                    normalized_angle += segment_angle;
                }

                // Mirror within segment: if past half-way, reflect back
                let half_segment = segment_angle / 2.0;
                if normalized_angle > half_segment {
                    normalized_angle = segment_angle - normalized_angle;
                }

                // Scale the distance
                let scaled_dist = dist * scale;

                // Generate pattern based on distance and angle
                let color = Self::compute_pattern_color(
                    scaled_dist,
                    normalized_angle,
                    time,
                    colors,
                    complexity,
                    segments,
                );

                pixel_bytes[0] = color.0;
                pixel_bytes[1] = color.1;
                pixel_bytes[2] = color.2;
                pixel_bytes[3] = 255;
            });

        img
    }

    fn compute_pattern_color(
        dist: f32,
        angle: f32,
        time: f32,
        colors: &[(u8, u8, u8)],
        complexity: usize,
        segments: usize,
    ) -> (u8, u8, u8) {
        if colors.is_empty() {
            return (128, 128, 128);
        }

        // Create radial bands
        let radial_freq = complexity as f32 * 0.5;
        let radial_pattern = (dist / 40.0 * radial_freq + time * 2.0).sin();

        // Create angular variations
        let angular_pattern = (angle * segments as f32 * 3.0 + time).sin();

        // Combine patterns
        let pattern_value = (radial_pattern + angular_pattern * 0.5) * 0.5 + 0.5;
        let pattern_value = pattern_value.clamp(0.0, 1.0);

        // Map to color palette
        let color_idx = (pattern_value * (colors.len() as f32 - 1.0)).round() as usize;
        let color_idx = color_idx.min(colors.len() - 1);

        // Add subtle time-based brightness variation
        let brightness = 0.8 + 0.2 * (time * 3.0 + dist * 0.01).sin();

        let base_color = colors[color_idx];
        (
            (base_color.0 as f32 * brightness).min(255.0) as u8,
            (base_color.1 as f32 * brightness).min(255.0) as u8,
            (base_color.2 as f32 * brightness).min(255.0) as u8,
        )
    }
}

impl Iterator for KaleidoscopeConfig {
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

impl AnimationInit for KaleidoscopeConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;
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

crate::impl_animation!(KaleidoscopeConfig);