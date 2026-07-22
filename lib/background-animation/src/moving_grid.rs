use crate::{AnimationInit, FlowDirection};
use image::{Rgba, RgbaImage, imageops::FilterType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct MovingGridConfig {
    /// Number of horizontal rows.
    #[derivative(Default(value = "10"))]
    pub rows: usize,

    /// Number of vertical columns.
    #[derivative(Default(value = "24"))]
    pub cols: usize,

    /// Scroll speed (pixels per second at 1080P standard).
    #[derivative(Default(value = "60.0"))]
    pub speed: f32,

    /// Movement direction of the grid (Up or Down).
    #[derivative(Default(value = "FlowDirection::Up"))]
    pub direction: FlowDirection,

    #[derivative(Default(value = "(80, 80, 80, 255)"))]
    pub line_color: (u8, u8, u8, u8),

    #[derivative(Default(value = "1.0"))]
    pub line_width: f32,

    #[derivative(Default(value = "(5, 5, 15)"))]
    pub bg_color: (u8, u8, u8),

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
    fps: u32,

    #[setters(skip)]
    #[serde(skip)]
    total_frames: usize,

    #[setters(skip)]
    #[serde(skip)]
    current_frame: usize,

    #[setters(skip)]
    #[serde(skip)]
    offset: f32,
}

/// Draw a perfectly centered vertical line with sub-pixel AA.
///
/// Each pixel's opacity is determined by its distance from the line center,
/// producing uniform width regardless of whether `cx` falls on an integer
/// pixel boundary.
fn draw_vertical_segment(
    img: &mut RgbaImage,
    cx: f32,
    y1: f32,
    y2: f32,
    color: Rgba<u8>,
    line_width: f32,
) {
    let half_w = line_width / 2.0;
    let iy_min = y1.min(y2).max(0.0) as u32;
    let iy_max = y2.max(y1).min(img.height() as f32) as u32;
    let ix_min = (cx - half_w - 1.0).max(0.0) as u32;
    let ix_max = (cx + half_w + 2.0).min(img.width() as f32) as u32;

    for iy in iy_min..iy_max {
        for ix in ix_min..ix_max {
            let px = ix as f32 + 0.5;
            let dist = (px - cx).abs();
            let dist_from_center = dist - half_w;

            // Smooth AA transition over 1-pixel ramp
            let opacity = if dist_from_center <= -0.5 {
                1.0
            } else if dist_from_center >= 0.5 {
                continue;
            } else {
                0.5 - dist_from_center
            };

            let src_alpha = opacity * (color[3] as f32 / 255.0);
            if src_alpha < 0.02 {
                continue;
            }

            let pixel = img.get_pixel_mut(ix, iy);
            let dst_alpha = pixel[3] as f32 / 255.0;
            let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
            if out_alpha > 0.0 {
                pixel[0] = ((color[0] as f32 * src_alpha
                    + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
                    / out_alpha) as u8;
                pixel[1] = ((color[1] as f32 * src_alpha
                    + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
                    / out_alpha) as u8;
                pixel[2] = ((color[2] as f32 * src_alpha
                    + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
                    / out_alpha) as u8;
                pixel[3] = (out_alpha * 255.0) as u8;
            }
        }
    }
}

/// Draw a horizontal line with proper width and sub-pixel AA.
///
/// Unlike Wu's algorithm which only covers 2 pixel rows, this function
/// correctly handles arbitrary line widths by computing coverage based
/// on distance from the line center.
fn draw_horizontal_line(
    img: &mut RgbaImage,
    y_center: f32,
    x1: f32,
    x2: f32,
    color: Rgba<u8>,
    line_width: f32,
) {
    let half_w = line_width / 2.0;
    let ix_min = x1.min(x2).max(0.0) as u32;
    let ix_max = x2.max(x1).min(img.width() as f32) as u32;
    let iy_min = (y_center - half_w - 1.0).max(0.0) as u32;
    let iy_max = (y_center + half_w + 2.0).min(img.height() as f32) as u32;

    for iy in iy_min..iy_max {
        for ix in ix_min..ix_max {
            let py = iy as f32 + 0.5;
            let dist = (py - y_center).abs();
            let dist_from_center = dist - half_w;

            // Smooth AA transition over 1-pixel ramp
            let opacity = if dist_from_center <= -0.5 {
                1.0
            } else if dist_from_center >= 0.5 {
                continue;
            } else {
                0.5 - dist_from_center
            };

            let src_alpha = opacity * (color[3] as f32 / 255.0);
            if src_alpha < 0.02 {
                continue;
            }

            let pixel = img.get_pixel_mut(ix, iy);
            let dst_alpha = pixel[3] as f32 / 255.0;
            let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
            if out_alpha > 0.0 {
                pixel[0] = ((color[0] as f32 * src_alpha
                    + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
                    / out_alpha) as u8;
                pixel[1] = ((color[1] as f32 * src_alpha
                    + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
                    / out_alpha) as u8;
                pixel[2] = ((color[2] as f32 * src_alpha
                    + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
                    / out_alpha) as u8;
                pixel[3] = (out_alpha * 255.0) as u8;
            }
        }
    }
}

impl MovingGridConfig {
    pub fn new() -> Self {
        Self::default()
    }

    fn generate_frame(&self) -> RgbaImage {
        let scale = self.supersample;
        let sw = self.width * scale;
        let sh = self.height * scale;
        let swf = sw as f32;
        let shf = sh as f32;
        let scale_f = scale as f32;

        let row_h = shf / self.rows as f32;
        // Line width: scale for display resolution, then for supersampling.
        // The supersample factor ensures lines are crisp after Lanczos downscale.
        let lw = crate::scale_pixel_for_height(self.line_width, self.height) * scale_f;
        let lc = Rgba([
            self.line_color.0,
            self.line_color.1,
            self.line_color.2,
            self.line_color.3,
        ]);
        let n_cols = self.cols;

        let y_off = self.offset % row_h;

        let mut img = RgbaImage::from_pixel(
            sw,
            sh,
            Rgba([self.bg_color.0, self.bg_color.1, self.bg_color.2, 255]),
        );

        // --- Draw horizontal lines ---
        let col_spacing = swf / (n_cols + 1) as f32;
        // Extra rows above/below viewport to prevent edge flicker:
        // lines near the edge have their AA pixels clipped, causing visible
        // thinning.  Extending well past the boundary ensures the AA fade
        // happens entirely off-screen.
        let extra = (lw.ceil() as isize) + 3;
        for i in -(extra)..(self.rows as isize + extra + 1) {
            let y = shf - (i as f32 * row_h - y_off);

            draw_horizontal_line(&mut img, y, 0.0, swf, lc, lw);
        }

        // --- Draw vertical lines ---
        for j in 0..n_cols {
            let x = col_spacing * (j as f32 + 1.0);
            draw_vertical_segment(&mut img, x, 0.0, shf, lc, lw);
        }

        if scale > 1 {
            image::imageops::resize(&img, self.width, self.height, FilterType::Lanczos3)
        } else {
            img
        }
    }
}

impl Iterator for MovingGridConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }
        let frame = self.generate_frame();
        let scroll_speed = crate::scale_pixel_for_height(self.speed, self.height);
        let sign = if self.direction == FlowDirection::Down {
            1.0
        } else {
            -1.0
        };
        self.offset += sign * scroll_speed / self.fps as f32;
        self.current_frame += 1;
        Some(frame)
    }
}

impl AnimationInit for MovingGridConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;
        self.offset = 0.0;
        self.current_frame = 0;
    }

    fn reset(&mut self) {
        self.current_frame = 0;
        self.offset = 0.0;
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames;
    }
}

crate::impl_animation!(MovingGridConfig);

