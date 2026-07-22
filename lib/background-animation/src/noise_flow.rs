use crate::{AnimationInit, FlowDirection};
use image::RgbaImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ColorPalette {
    pub colors: Vec<(u8, u8, u8, u8)>,
}

impl ColorPalette {
    pub fn new(colors: Vec<(u8, u8, u8, u8)>) -> Self {
        Self { colors }
    }

    fn get_color(&self, t: f32) -> (u8, u8, u8, u8) {
        if self.colors.is_empty() {
            return (128, 128, 128, 255);
        }
        if self.colors.len() == 1 {
            return self.colors[0];
        }

        let t = t.clamp(0.0, 1.0);
        let idx = t * (self.colors.len() - 1) as f32;
        let lower_idx = idx.floor() as usize;
        let upper_idx = (lower_idx + 1).min(self.colors.len() - 1);
        let blend = idx - lower_idx as f32;

        let c1 = self.colors[lower_idx];
        let c2 = self.colors[upper_idx];

        (
            (c1.0 as f32 + (c2.0 as f32 - c1.0 as f32) * blend) as u8,
            (c1.1 as f32 + (c2.1 as f32 - c1.1 as f32) * blend) as u8,
            (c1.2 as f32 + (c2.2 as f32 - c1.2 as f32) * blend) as u8,
            (c1.3 as f32 + (c2.3 as f32 - c1.3 as f32) * blend) as u8,
        )
    }
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct NoiseFlowConfig {
    // Scale of noise features (higher = larger patterns)
    #[derivative(Default(value = "0.005"))]
    pub noise_scale: f32,

    // Flow speed (time increment per frame for 3D noise)
    #[derivative(Default(value = "0.5"))]
    pub animation_speed: f32,

    #[derivative(Default(
        value = "ColorPalette::new(vec![(20, 20, 60, 255), (40, 80, 120, 255), (80, 150, 200, 255), (150, 200, 250, 255)])"
    ))]
    pub color_palette: ColorPalette,

    #[derivative(Default(value = "(0, 0, 30)"))]
    pub bg_color: (u8, u8, u8),

    #[derivative(Default)]
    pub flow_direction: FlowDirection,

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
}

// 3D Perlin noise implementation for flowing animation
mod noise {
    // Permutation table for noise
    const PERM: [u8; 256] = [
        151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30,
        69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94,
        252, 219, 203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 97, 85, 144,
        171, 62, 113, 138, 102, 158, 99, 186, 212, 127, 80, 116, 123, 6, 147, 93, 191, 140, 113,
        128, 116, 111, 115, 158, 125, 191, 126, 96, 130, 144, 141, 135, 151, 46, 30, 136, 161, 79,
        141, 142, 137, 123, 113, 104, 95, 120, 169, 45, 127, 165, 77, 181, 107, 90, 170, 215, 125,
        93, 83, 144, 231, 166, 81, 54, 147, 190, 119, 168, 220, 162, 144, 138, 154, 145, 157, 155,
        118, 181, 127, 98, 93, 168, 156, 169, 162, 156, 156, 157, 153, 166, 187, 171, 177, 168,
        154, 173, 186, 158, 165, 162, 166, 146, 170, 161, 166, 175, 191, 188, 194, 171, 169, 173,
        188, 189, 191, 186, 189, 178, 194, 192, 195, 189, 187, 190, 199, 195, 199, 195, 200, 194,
        195, 194, 195, 200, 197, 199, 198, 198, 203, 197, 200, 202, 207, 203, 200, 206, 207, 207,
        209, 207, 211, 204, 207, 210, 209, 211, 209, 210, 213, 211, 210, 211, 211, 214, 212, 214,
        213, 214, 216, 214, 215, 216, 218, 217, 218, 220, 219, 221, 220, 222, 224, 222, 223, 225,
        225, 227, 226, 227, 228, 229, 230, 232, 231, 233, 232,
    ];

    fn fade(t: f32) -> f32 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + t * (b - a)
    }

    fn perm_idx(x: i32) -> u8 {
        PERM[(x & 255) as usize]
    }

    fn grad3(hash: u8, x: f32, y: f32, z: f32) -> f32 {
        let h = hash & 15;
        let u = if h < 8 { x } else { y };
        let v = if h < 4 {
            y
        } else if h == 12 || h == 14 {
            x
        } else {
            z
        };
        (if (h & 1) != 0 { -u } else { u }) + (if (h & 2) != 0 { -v } else { v })
    }

    /// 3D Perlin noise for time-based animation
    pub fn noise3d(x: f32, y: f32, z: f32) -> f32 {
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let zi = z.floor() as i32;
        let xf = x - xi as f32;
        let yf = y - yi as f32;
        let zf = z - zi as f32;

        let u = fade(xf);
        let v = fade(yf);
        let w = fade(zf);

        // Hash coordinates for 8 corners of the cube
        let p_xi = perm_idx(xi) as i32;
        let p_xi1 = perm_idx(xi + 1) as i32;

        let p0 = perm_idx(p_xi + yi) as i32;
        let p1 = perm_idx(p_xi + yi + 1) as i32;
        let p2 = perm_idx(p_xi1 + yi) as i32;
        let p3 = perm_idx(p_xi1 + yi + 1) as i32;

        let a = perm_idx(p0 + zi);
        let b = perm_idx(p1 + zi);
        let c = perm_idx(p2 + zi);
        let d = perm_idx(p3 + zi);
        let e = perm_idx(p0 + zi + 1);
        let f = perm_idx(p1 + zi + 1);
        let g = perm_idx(p2 + zi + 1);
        let h = perm_idx(p3 + zi + 1);

        // Interpolate along x
        let x1 = lerp(grad3(a, xf, yf, zf), grad3(c, xf - 1.0, yf, zf), u);
        let x2 = lerp(
            grad3(b, xf, yf - 1.0, zf),
            grad3(d, xf - 1.0, yf - 1.0, zf),
            u,
        );
        let x3 = lerp(
            grad3(e, xf, yf, zf - 1.0),
            grad3(g, xf - 1.0, yf, zf - 1.0),
            u,
        );
        let x4 = lerp(
            grad3(f, xf, yf - 1.0, zf - 1.0),
            grad3(h, xf - 1.0, yf - 1.0, zf - 1.0),
            u,
        );

        // Interpolate along y
        let y1 = lerp(x1, x2, v);
        let y2 = lerp(x3, x4, v);

        // Interpolate along z
        lerp(y1, y2, w)
    }

    /// Fractal Brownian Motion (FBM) with time dimension for flowing animation
    pub fn fbm_time(x: f32, y: f32, time: f32, octaves: u32) -> f32 {
        let mut value = 0.0;
        let mut amplitude = 0.5;
        let mut frequency = 1.0;
        let mut max_value = 0.0;

        for _ in 0..octaves {
            value += amplitude * noise3d(x * frequency, y * frequency, time * frequency);
            max_value += amplitude;
            amplitude *= 0.5;
            frequency *= 2.0;
        }

        value / max_value
    }
}

impl NoiseFlowConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn generate_frame(&self, frame: usize) -> RgbaImage {
        let mut img = RgbaImage::new(self.width, self.height);

        // Time-based animation using 3D noise
        let time = frame as f32 * self.animation_speed;
        let noise_scale = self.noise_scale;

        // Parallel pixel processing - each pixel is 4 bytes (RGBA)
        img.as_mut()
            .par_chunks_mut(4)
            .enumerate()
            .for_each(|(pixel_idx, pixel_bytes)| {
                let x = pixel_idx % self.width as usize;
                let y = pixel_idx / self.width as usize;

                let nx = x as f32 * noise_scale;
                let ny = y as f32 * noise_scale;

                // Use 3D FBM for flowing animation (time as z dimension)
                let noise_val = noise::fbm_time(nx, ny, time, 4);

                // Map noise to color (noise is in -1 to 1 range, normalize to 0-1)
                let t = (noise_val + 1.0) / 2.0;

                // Get color from palette
                let color = self.color_palette.get_color(t);

                pixel_bytes[0] = color.0;
                pixel_bytes[1] = color.1;
                pixel_bytes[2] = color.2;
                pixel_bytes[3] = color.3;
            });

        img
    }
}

impl Iterator for NoiseFlowConfig {
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

impl AnimationInit for NoiseFlowConfig {
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

crate::impl_animation!(NoiseFlowConfig);

