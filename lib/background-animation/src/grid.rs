use crate::{AnimationInit, aa_line::draw_line_segment_aa, pseudo_phase};
use image::{Rgba, RgbaImage, imageops::FilterType};
use imageproc::drawing::draw_filled_circle_mut;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Blend a source pixel onto a destination pixel using "over" alpha compositing.
/// This is the correct way to merge semi-transparent pixels.
fn blend_pixel(dst: &mut Rgba<u8>, src: &Rgba<u8>) {
    let src_alpha = src[3] as f32 / 255.0;
    let dst_alpha = dst[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

    if out_alpha > 0.0 {
        dst[0] = ((src[0] as f32 * src_alpha + dst[0] as f32 * dst_alpha * (1.0 - src_alpha)) / out_alpha) as u8;
        dst[1] = ((src[1] as f32 * src_alpha + dst[1] as f32 * dst_alpha * (1.0 - src_alpha)) / out_alpha) as u8;
        dst[2] = ((src[2] as f32 * src_alpha + dst[2] as f32 * dst_alpha * (1.0 - src_alpha)) / out_alpha) as u8;
        dst[3] = (out_alpha * 255.0) as u8;
    }
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GridConfig {
    #[derivative(Default(value = "20"))]
    pub rows: usize,

    #[derivative(Default(value = "20"))]
    pub cols: usize,

    // Edge oscillation amplitude in pixels
    #[derivative(Default(value = "10.0"))]
    pub amplitude: f32,

    // Internal node oscillation amplitude in pixels
    #[derivative(Default(value = "5.0"))]
    pub node_amplitude: f32,

    // Oscillation speed (angle increment per frame)
    #[derivative(Default(value = "0.1"))]
    pub frequency: f32,

    #[derivative(Default(value = "1"))]
    pub node_radius: i32,

    // Line color (R, G, B, A)
    #[derivative(Default(value = "(76, 76, 76, 255)"))]
    pub line_color: (u8, u8, u8, u8),

    // Line width in pixels
    #[derivative(Default(value = "1.0"))]
    pub line_width: f32,

    // Background color (R, G, B)
    #[derivative(Default(value = "(0, 0, 0)"))]
    pub bg_color: (u8, u8, u8),

    #[derivative(Default(value = "(255, 255, 255, 255)"))]
    pub node_color: (u8, u8, u8, u8),

    // Number of segments per edge (higher = smoother curves)
    #[derivative(Default(value = "20"))]
    pub segments_per_edge: usize,

    // Supersampling factor for anti-aliasing (2 = 2x resolution)
    #[derivative(Default(value = "2"))]
    pub supersample: u32,

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
    grid: Vec<(f32, f32)>,

    // Horizontal edge phase offsets
    #[setters(skip)]
    #[serde(skip)]
    h_offsets: Vec<f32>,

    // Vertical edge phase offsets
    #[setters(skip)]
    #[serde(skip)]
    v_offsets: Vec<f32>,

    // Node X-axis phase offsets
    #[setters(skip)]
    #[serde(skip)]
    node_dx_phase: Vec<f32>,

    // Node Y-axis phase offsets
    #[setters(skip)]
    #[serde(skip)]
    node_dy_phase: Vec<f32>,
}

impl GridConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn is_border(&self, i: usize, j: usize) -> bool {
        i == 0 || i == self.rows - 1 || j == 0 || j == self.cols - 1
    }

    fn generate_frame(&self, frame: usize) -> RgbaImage {
        let scale = self.supersample;
        let super_width = self.width * scale;
        let super_height = self.height * scale;

        let phase = frame as f32 * self.frequency;
        let total_nodes = self.rows * self.cols;

        // Compute node offsets for this frame in parallel (border nodes have 0 offset)
        let node_dx: Vec<f32> = (0..total_nodes)
            .into_par_iter()
            .map(|idx| {
                let i = idx / self.cols;
                let j = idx % self.cols;
                if i == 0 || i == self.rows - 1 || j == 0 || j == self.cols - 1 {
                    0.0
                } else {
                    self.node_amplitude * (phase + self.node_dx_phase[idx]).sin()
                }
            })
            .collect();

        let node_dy: Vec<f32> = (0..total_nodes)
            .into_par_iter()
            .map(|idx| {
                let i = idx / self.cols;
                let j = idx % self.cols;
                if i == 0 || i == self.rows - 1 || j == 0 || j == self.cols - 1 {
                    0.0
                } else {
                    self.node_amplitude * (phase + self.node_dy_phase[idx]).sin()
                }
            })
            .collect();

        let line_color = Rgba([
            self.line_color.0,
            self.line_color.1,
            self.line_color.2,
            self.line_color.3,
        ]);
        // Scale line width for supersampling
        let line_width = self.line_width * scale as f32;
        let segments_per_edge = self.segments_per_edge;
        let amplitude = self.amplitude * scale as f32;
        let cols = self.cols;
        let rows = self.rows;
        let scale_f = scale as f32;
        let grid = &self.grid;
        let h_offsets = &self.h_offsets;
        let v_offsets = &self.v_offsets;

        // Generate horizontal edges layer in parallel (at supersampled resolution)
        let h_layer: RgbaImage = (0..rows)
            .into_par_iter()
            .fold(
                || RgbaImage::new(super_width, super_height),
                |mut layer, i| {
                    for j in 0..(cols - 1) {
                        let edge_idx = i * (cols - 1) + j;
                        let left_idx = i * cols + j;
                        let right_idx = i * cols + j + 1;
                        let (x1, y_base) = grid[left_idx];
                        let (x2, _) = grid[right_idx];
                        let dx_left = node_dx[left_idx] * scale_f;
                        let dx_right = node_dx[right_idx] * scale_f;
                        let dy_left = node_dy[left_idx] * scale_f;
                        let dy_right = node_dy[right_idx] * scale_f;
                        let edge_dy = amplitude * (phase + h_offsets[edge_idx]).sin();

                        // Scale base coordinates
                        let x1d = x1 * scale_f + dx_left;
                        let x2d = x2 * scale_f + dx_right;
                        let y_base_scaled = y_base * scale_f;
                        let mut prev = (x1d, y_base_scaled + dy_left);
                        for k in 1..=segments_per_edge {
                            let t = k as f32 / segments_per_edge as f32;
                            let x = x1d + (x2d - x1d) * t;
                            let interp_dy = dy_left * (1.0 - t) + dy_right * t;
                            let envelope = (std::f32::consts::PI * t).sin();
                            let curr = (x, y_base_scaled + interp_dy + edge_dy * envelope);
                            draw_line_segment_aa(&mut layer, prev, curr, line_color, line_width);
                            prev = curr;
                        }
                    }
                    layer
                },
            )
            .reduce(
                || RgbaImage::new(super_width, super_height),
                |mut acc, layer| {
                    for (x, y, pixel) in layer.enumerate_pixels() {
                        if pixel.0[3] > 0 {
                            let acc_pixel = acc.get_pixel_mut(x, y);
                            blend_pixel(acc_pixel, pixel);
                        }
                    }
                    acc
                },
            );

        // Generate vertical edges layer in parallel (at supersampled resolution)
        let v_layer: RgbaImage = (0..cols)
            .into_par_iter()
            .fold(
                || RgbaImage::new(super_width, super_height),
                |mut layer, j| {
                    for i in 0..(rows - 1) {
                        let edge_idx = j * (rows - 1) + i;
                        let top_idx = i * cols + j;
                        let bottom_idx = (i + 1) * cols + j;
                        let (x_base, y1) = grid[top_idx];
                        let (_, y2) = grid[bottom_idx];
                        let dx_top = node_dx[top_idx] * scale_f;
                        let dx_bottom = node_dx[bottom_idx] * scale_f;
                        let dy_top = node_dy[top_idx] * scale_f;
                        let dy_bottom = node_dy[bottom_idx] * scale_f;
                        let edge_dx = amplitude * (phase + v_offsets[edge_idx]).sin();

                        // Scale base coordinates
                        let x_base_scaled = x_base * scale_f;
                        let y1d = y1 * scale_f + dy_top;
                        let y2d = y2 * scale_f + dy_bottom;
                        let mut prev = (x_base_scaled + dx_top, y1d);
                        for k in 1..=segments_per_edge {
                            let t = k as f32 / segments_per_edge as f32;
                            let y = y1d + (y2d - y1d) * t;
                            let interp_dx = dx_top * (1.0 - t) + dx_bottom * t;
                            let envelope = (std::f32::consts::PI * t).sin();
                            let curr = (x_base_scaled + interp_dx + edge_dx * envelope, y);
                            draw_line_segment_aa(&mut layer, prev, curr, line_color, line_width);
                            prev = curr;
                        }
                    }
                    layer
                },
            )
            .reduce(
                || RgbaImage::new(super_width, super_height),
                |mut acc, layer| {
                    for (x, y, pixel) in layer.enumerate_pixels() {
                        if pixel.0[3] > 0 {
                            let acc_pixel = acc.get_pixel_mut(x, y);
                            blend_pixel(acc_pixel, pixel);
                        }
                    }
                    acc
                },
            );

        // Create final image with background (at supersampled resolution)
        let mut img = RgbaImage::new(super_width, super_height);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]);
        }

        // Merge layers with proper alpha blending
        for (x, y, pixel) in h_layer.enumerate_pixels() {
            if pixel.0[3] > 0 {
                let img_pixel = img.get_pixel_mut(x, y);
                blend_pixel(img_pixel, pixel);
            }
        }

        for (x, y, pixel) in v_layer.enumerate_pixels() {
            if pixel.0[3] > 0 {
                let img_pixel = img.get_pixel_mut(x, y);
                blend_pixel(img_pixel, pixel);
            }
        }

        // Draw nodes (internal nodes oscillate, border nodes fixed) - at supersampled resolution
        let node_color = Rgba([self.node_color.0, self.node_color.1, self.node_color.2, self.node_color.3]);
        let node_radius = (self.node_radius as f32 * scale as f32).round() as i32;
        for i in 0..self.rows {
            for j in 0..self.cols {
                let idx = i * self.cols + j;
                let (ox, oy) = self.grid[idx];
                // node_dx and node_dy are in original pixel units, scale them for supersampled resolution
                let px = ((ox + node_dx[idx]) * scale_f) as i32;
                let py = ((oy + node_dy[idx]) * scale_f) as i32;
                draw_filled_circle_mut(&mut img, (px, py), node_radius, node_color);
            }
        }

        // Downscale with Lanczos3 for high-quality anti-aliasing
        if scale > 1 {
            image::imageops::resize(&img, self.width, self.height, FilterType::Lanczos3)
        } else {
            img
        }
    }
}

impl Iterator for GridConfig {
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

impl AnimationInit for GridConfig {
    fn init(&mut self, width: u32, height: u32, _fps: u32) {
        self.width = width;
        self.height = height;
        let cell_width = self.width as f32 / (self.cols - 1) as f32;
        let cell_height = self.height as f32 / (self.rows - 1) as f32;

        self.grid = Vec::with_capacity(self.rows * self.cols);
        for i in 0..self.rows {
            for j in 0..self.cols {
                let x = j as f32 * cell_width;
                let y = i as f32 * cell_height;
                self.grid.push((x, y));
            }
        }

        let total_h_edges = self.rows * (self.cols - 1);
        self.h_offsets = (0..total_h_edges).map(pseudo_phase).collect();

        let total_v_edges = (self.rows - 1) * self.cols;
        self.v_offsets = (0..total_v_edges).map(|s| pseudo_phase(s + 1000)).collect();

        let total_nodes = self.rows * self.cols;
        self.node_dx_phase = vec![0.0f32; total_nodes];
        self.node_dy_phase = vec![0.0f32; total_nodes];
        let mut node_seed = 2000;
        for i in 0..self.rows {
            for j in 0..self.cols {
                let idx = i * self.cols + j;
                if !self.is_border(i, j) {
                    self.node_dx_phase[idx] = pseudo_phase(node_seed);
                    node_seed += 1;
                    self.node_dy_phase[idx] = pseudo_phase(node_seed);
                    node_seed += 1;
                }
            }
        }
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.grid.clear();
        self.h_offsets.clear();
        self.v_offsets.clear();
        self.node_dx_phase.clear();
        self.node_dy_phase.clear();
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(GridConfig);
