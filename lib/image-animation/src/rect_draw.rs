use crate::AnimationInit;
use background_animation::{
    Animation, AnimationRecordConfig, impl_animation, scale_pixel_for_height,
};
use draw_utils::{blend_pixel, smoothstep};
use image::{Rgba, RgbaImage};
use std::{path::PathBuf, time::Duration};

/// Dash pattern for dashed lines
#[derive(Debug, Clone, PartialEq)]
pub enum DashStyle {
    /// Solid line (no dash)
    Solid,
    /// Standard dash: equal dash and gap lengths
    /// Parameter is the dash length in pixels
    Dash(f32),
    /// Custom dash pattern: alternating (dash, gap, dash, gap, ...)
    Custom(Vec<f32>),
}

impl Default for DashStyle {
    fn default() -> Self {
        DashStyle::Solid
    }
}

impl std::fmt::Display for DashStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DashStyle::Solid => write!(f, "solid"),
            DashStyle::Dash(_) => write!(f, "dash"),
            DashStyle::Custom(_) => write!(f, "custom"),
        }
    }
}

impl DashStyle {
    /// Returns true if the given offset (along the path) falls in a "draw" region
    fn is_draw_at(&self, offset: f32) -> bool {
        match self {
            DashStyle::Solid => true,
            DashStyle::Dash(len) => {
                let total = len * 2.0;
                if total < 0.1 {
                    return true;
                }
                let pos = offset % total;
                pos < *len
            }
            DashStyle::Custom(pattern) => {
                let total: f32 = pattern.iter().sum();
                if total < 0.1 || pattern.is_empty() {
                    return true;
                }
                let pos = offset % total;
                let mut accum = 0.0f32;
                for (i, &len) in pattern.iter().enumerate() {
                    accum += len;
                    if pos < accum {
                        return i % 2 == 0;
                    }
                }
                true
            }
        }
    }
}

/// Line style configuration for the rectangle
#[derive(Debug, Clone, derivative::Derivative)]
#[derivative(Default)]
pub struct LineStyle {
    /// Line color (RGBA). Default: white
    #[derivative(Default(value = "(255, 255, 255, 255)"))]
    pub color: (u8, u8, u8, u8),

    /// Line width in pixels. Default: 4.0
    #[derivative(Default(value = "4.0"))]
    pub width: f32,

    /// Dash style. Default: Solid
    pub dash: DashStyle,
}

/// Rectangle style configuration
#[derive(Debug, Clone, derivative::Derivative)]
#[derivative(Default)]
pub struct RectStyle {
    /// Rectangle width in pixels. Default: 300.0
    #[derivative(Default(value = "300.0"))]
    pub width: f32,

    /// Rectangle height in pixels. Default: 200.0
    #[derivative(Default(value = "200.0"))]
    pub height: f32,

    /// Corner radius in pixels. Default: 0.0 (sharp corners)
    #[derivative(Default(value = "0.0"))]
    pub corner_radius: f32,
}

/// A segment of the path, either a straight line or an arc
#[derive(Debug, Clone)]
enum PathSegment {
    /// Straight line from start to end
    Line {
        start: (f32, f32),
        end: (f32, f32),
        length: f32,
        cumulative_start: f32,
    },
    /// Arc: center, radius, start_angle, total_angle (radians), length, cumulative_start
    Arc {
        center: (f32, f32),
        radius: f32,
        start_angle: f32,
        total_angle: f32,
        length: f32,
        cumulative_start: f32,
    },
}

impl PathSegment {
    fn length(&self) -> f32 {
        match self {
            PathSegment::Line { length, .. } => *length,
            PathSegment::Arc { length, .. } => *length,
        }
    }

    fn cumulative_start(&self) -> f32 {
        match self {
            PathSegment::Line {
                cumulative_start, ..
            } => *cumulative_start,
            PathSegment::Arc {
                cumulative_start, ..
            } => *cumulative_start,
        }
    }

    fn is_corner(&self) -> bool {
        matches!(self, PathSegment::Arc { .. })
    }

    /// Get the point at parameter t (0.0 to 1.0) along this segment
    fn point_at(&self, t: f32) -> (f32, f32) {
        match self {
            PathSegment::Line { start, end, .. } => (
                start.0 + (end.0 - start.0) * t,
                start.1 + (end.1 - start.1) * t,
            ),
            PathSegment::Arc {
                center,
                radius,
                start_angle,
                total_angle,
                ..
            } => {
                let angle = start_angle + total_angle * t;
                (
                    center.0 + radius * angle.cos(),
                    center.1 + radius * angle.sin(),
                )
            }
        }
    }

    /// Compute the perpendicular distance from pixel (px, py) to this segment,
    /// and the arc-length offset where the closest point lies.
    /// Returns (distance, arc_length_offset, t_param).
    fn project_pixel(&self, px: f32, py: f32) -> (f32, f32, f32) {
        match self {
            PathSegment::Line {
                start,
                end,
                length,
                cumulative_start,
                ..
            } => {
                let dx = end.0 - start.0;
                let dy = end.1 - start.1;
                let len_sq = dx * dx + dy * dy;
                if len_sq < 0.01 {
                    let ddx = px - start.0;
                    let ddy = py - start.1;
                    return ((ddx * ddx + ddy * ddy).sqrt(), *cumulative_start, 0.0);
                }
                // Project pixel onto the line segment: t = dot(P-A, B-A) / |B-A|²
                let t = ((px - start.0) * dx + (py - start.1) * dy) / len_sq;
                let t = t.clamp(0.0, 1.0);
                let closest_x = start.0 + dx * t;
                let closest_y = start.1 + dy * t;
                let ddx = px - closest_x;
                let ddy = py - closest_y;
                let dist = (ddx * ddx + ddy * ddy).sqrt();
                let arc_offset = cumulative_start + length * t;
                (dist, arc_offset, t)
            }
            PathSegment::Arc {
                center,
                radius,
                start_angle,
                total_angle,
                length,
                cumulative_start,
                ..
            } => {
                let ddx = px - center.0;
                let ddy = py - center.1;
                let dist_to_center = (ddx * ddx + ddy * ddy).sqrt();

                let pixel_angle = ddy.atan2(ddx);
                let total = *total_angle;
                let arc_len = *length;
                let cum_start = *cumulative_start;
                let start = *start_angle;

                let two_pi = std::f32::consts::TAU;
                let angle_diff = pixel_angle - start;
                let t = if total.abs() > 0.01 {
                    let diff = angle_diff % two_pi;
                    let diff = if total > 0.0 {
                        if diff < 0.0 { diff + two_pi } else { diff }
                    } else {
                        if diff > 0.0 { diff - two_pi } else { diff }
                    };
                    diff / total
                } else {
                    0.0
                };

                // If t is within [0, 1], the pixel projects onto the arc interior —
                // use radial distance to the circle.
                // If t is outside [0, 1], the closest point is one of the arc endpoints —
                // compute Euclidean distance to that endpoint instead.
                let (dist, t_clamped) = if t < 0.0 {
                    let ep = self.point_at(0.0);
                    let ex = px - ep.0;
                    let ey = py - ep.1;
                    ((ex * ex + ey * ey).sqrt(), 0.0)
                } else if t > 1.0 {
                    let ep = self.point_at(1.0);
                    let ex = px - ep.0;
                    let ey = py - ep.1;
                    ((ex * ex + ey * ey).sqrt(), 1.0)
                } else {
                    ((dist_to_center - radius).abs(), t)
                };

                let arc_offset = cum_start + arc_len * t_clamped;
                (dist, arc_offset, t_clamped)
            }
        }
    }
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct RectDrawConfig {
    /// Line style configuration
    #[derivative(Default(value = "LineStyle::default()"))]
    pub line_style: LineStyle,

    /// Rectangle style configuration
    #[derivative(Default(value = "RectStyle::default()"))]
    pub rect_style: RectStyle,

    /// Animation duration in milliseconds (drawing phase only)
    #[derivative(Default(value = "800"))]
    pub duration_ms: u32,

    /// Pause duration after drawing completes (in seconds)
    #[derivative(Default(value = "1.0"))]
    pub end_pause: f32,

    /// Center position as normalized coordinates (0.0 to 1.0)
    #[derivative(Default(value = "(0.5, 0.5)"))]
    pub position: (f32, f32),

    /// Output width in pixels
    #[derivative(Default(value = "400"))]
    pub width: u32,

    /// Output height in pixels
    #[derivative(Default(value = "400"))]
    pub height: u32,

    /// Frames per second
    #[derivative(Default(value = "25"))]
    #[setters(skip)]
    fps: u32,

    /// Frames for drawing phase
    #[setters(skip)]
    draw_frames: usize,

    /// Frames for pause phase
    #[setters(skip)]
    pause_frames: usize,

    /// Total number of frames
    #[setters(skip)]
    total_frames: usize,

    /// Current frame index
    #[setters(skip)]
    current_frame: usize,

    /// Scaled line width based on output height (1080P standard)
    #[setters(skip)]
    scaled_line_width: f32,

    /// Scaled rect width based on output height (1080P standard)
    #[setters(skip)]
    scaled_rect_width: f32,

    /// Scaled rect height based on output height (1080P standard)
    #[setters(skip)]
    scaled_rect_height: f32,

    /// Scaled corner radius based on output height (1080P standard)
    #[setters(skip)]
    scaled_corner_radius: f32,

    /// Precomputed path segments
    #[setters(skip)]
    path_segments: Vec<PathSegment>,

    /// Arc-length ranges where corners occur — no dash gaps allowed here.
    /// Each entry is (start_length, end_length) along the path.
    #[setters(skip)]
    corner_ranges: Vec<(f32, f32)>,

    /// Total path length
    #[setters(skip)]
    path_length: f32,
}

impl RectDrawConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Convenience method to record animation to MP4
    pub fn record(&mut self, output_path: impl Into<PathBuf>) -> crate::Result<()> {
        let width = self.width - (self.width % 2);
        let height = self.height - (self.height % 2);

        let total_duration_ms = self.duration_ms + (self.end_pause * 1000.0) as u32;
        let duration = Duration::from_millis(total_duration_ms as u64);
        let config =
            AnimationRecordConfig::new(width, height, self.fps, duration, output_path.into());
        self.animate_record(config)
    }

    /// Convenience method to record animation to animated WebP (preserves transparency)
    pub fn record_webp(&mut self, output_path: impl Into<PathBuf>) -> crate::Result<()> {
        let width = self.width - (self.width % 2);
        let height = self.height - (self.height % 2);

        let total_duration_ms = self.duration_ms + (self.end_pause * 1000.0) as u32;
        let duration = Duration::from_millis(total_duration_ms as u64);
        let config =
            AnimationRecordConfig::new(width, height, self.fps, duration, output_path.into());
        self.animate_record_webp(config)
    }

    /// Get drawing progress (0.0 to 1.0) based on current frame
    fn draw_progress(&self) -> f32 {
        if self.draw_frames == 0 {
            return 1.0;
        }
        if self.current_frame >= self.draw_frames {
            return 1.0;
        }
        self.current_frame as f32 / self.draw_frames as f32
    }

    /// Convert normalized position to pixel coordinates (center of rectangle)
    fn pixel_position(&self) -> (f32, f32) {
        (
            self.position.0 * self.width as f32,
            self.position.1 * self.height as f32,
        )
    }

    /// Build the rectangle path as segments (lines and arcs).
    /// Clockwise: top-left → top-right → bottom-right → bottom-left → close.
    fn build_path(&mut self) {
        let (cx, cy) = self.pixel_position();
        let rw = self.scaled_rect_width / 2.0;
        let rh = self.scaled_rect_height / 2.0;
        let cr = self.scaled_corner_radius.min(rw.min(rh));

        let left = cx - rw;
        let right = cx + rw;
        let top = cy - rh;
        let bottom = cy + rh;

        self.path_segments.clear();
        self.corner_ranges.clear();
        let mut cumulative = 0.0f32;

        let add_line = |start: (f32, f32), end: (f32, f32), cum: &mut f32| {
            let dx = end.0 - start.0;
            let dy = end.1 - start.1;
            let length = (dx * dx + dy * dy).sqrt();
            let seg = PathSegment::Line {
                start,
                end,
                length,
                cumulative_start: *cum,
            };
            *cum += length;
            seg
        };

        let add_arc =
            |center: (f32, f32), radius: f32, start_angle: f32, total_angle: f32, cum: &mut f32| {
                let length = radius * total_angle.abs();
                let seg = PathSegment::Arc {
                    center,
                    radius,
                    start_angle,
                    total_angle,
                    length,
                    cumulative_start: *cum,
                };
                *cum += length;
                seg
            };

        if cr < 1.0 {
            self.path_segments
                .push(add_line((left, top), (right, top), &mut cumulative));
            self.path_segments
                .push(add_line((right, top), (right, bottom), &mut cumulative));
            self.path_segments
                .push(add_line((right, bottom), (left, bottom), &mut cumulative));
            self.path_segments
                .push(add_line((left, bottom), (left, top), &mut cumulative));

            // Sharp corners: mark small ranges around each corner point
            let corner_margin = self.scaled_line_width;
            let offsets: [f32; 4] = [
                0.0,
                self.path_segments[0].length(),
                self.path_segments[0].length() + self.path_segments[1].length(),
                self.path_segments[0].length()
                    + self.path_segments[1].length()
                    + self.path_segments[2].length(),
            ];
            for &off in &offsets {
                self.corner_ranges
                    .push((off - corner_margin, off + corner_margin));
            }
            // Wrap-around corner at (left, top)
            self.corner_ranges.push((
                self.path_length - corner_margin,
                self.path_length + corner_margin,
            ));
        } else {
            // Top-left corner arc
            self.path_segments.push(add_arc(
                (left + cr, top + cr),
                cr,
                std::f32::consts::PI,
                std::f32::consts::FRAC_PI_2,
                &mut cumulative,
            ));
            // Top edge
            self.path_segments.push(add_line(
                (left + cr, top),
                (right - cr, top),
                &mut cumulative,
            ));
            // Top-right corner arc
            self.path_segments.push(add_arc(
                (right - cr, top + cr),
                cr,
                std::f32::consts::FRAC_PI_2 * 3.0,
                std::f32::consts::FRAC_PI_2,
                &mut cumulative,
            ));
            // Right edge
            self.path_segments.push(add_line(
                (right, top + cr),
                (right, bottom - cr),
                &mut cumulative,
            ));
            // Bottom-right corner arc
            self.path_segments.push(add_arc(
                (right - cr, bottom - cr),
                cr,
                0.0,
                std::f32::consts::FRAC_PI_2,
                &mut cumulative,
            ));
            // Bottom edge
            self.path_segments.push(add_line(
                (right - cr, bottom),
                (left + cr, bottom),
                &mut cumulative,
            ));
            // Bottom-left corner arc
            self.path_segments.push(add_arc(
                (left + cr, bottom - cr),
                cr,
                std::f32::consts::FRAC_PI_2,
                std::f32::consts::FRAC_PI_2,
                &mut cumulative,
            ));
            // Left edge
            self.path_segments.push(add_line(
                (left, bottom - cr),
                (left, top + cr),
                &mut cumulative,
            ));

            // Rounded corners: entire arc is a corner range
            for seg in &self.path_segments {
                if seg.is_corner() {
                    self.corner_ranges.push((
                        seg.cumulative_start(),
                        seg.cumulative_start() + seg.length(),
                    ));
                }
            }
        }

        self.path_length = cumulative;
    }

    /// Check if a given arc_length offset falls within a corner range.
    fn is_in_corner_range(&self, offset: f32) -> bool {
        for &(start, end) in &self.corner_ranges {
            let s = if start < 0.0 {
                start + self.path_length
            } else {
                start
            };
            let e = end.min(self.path_length);
            if offset >= s && offset <= e {
                return true;
            }
            // Wrap-around
            if start < 0.0 && (offset <= e || offset >= self.path_length + start) {
                return true;
            }
        }
        false
    }

    /// Check if a given arc_length offset should be drawn (considering dash and corner rules).
    fn should_draw(&self, offset: f32) -> bool {
        if self.is_in_corner_range(offset) {
            return true;
        }
        self.line_style.dash.is_draw_at(offset)
    }

    /// Render the path using SDF (Signed Distance Field) anti-aliasing.
    ///
    /// For each pixel we analytically compute the distance to the closest point
    /// on the path (`project_pixel`). This distance is the SDF value — the
    /// perpendicular distance from the pixel to the line's centre-line.
    ///
    /// Anti-aliasing uses smoothstep coverage:
    /// - **Edge AA**: `smoothstep(half_width - aa, half_width + aa, dist)`
    ///   produces a smooth transition from full colour to transparent across
    ///   the line boundary.
    /// - **Tip AA**: at the drawing front a second smoothstep fades the line
    ///   out so it doesn't end with a hard cut.
    fn render_path(&self, buffer: &mut RgbaImage, draw_progress: f32) {
        // Extend target slightly past path_length to ensure the path closes
        // without a visible gap. The overlap matches the tip AA fade band so
        // the drawing front overlaps the starting point when progress = 1.0.
        let tip_overlap = (self.scaled_line_width * 0.5).max(2.0);
        let target_length = self.path_length * draw_progress
            + if draw_progress >= 1.0 {
                tip_overlap
            } else {
                0.0
            };
        if target_length < 0.01 {
            return;
        }

        let half_width = self.scaled_line_width / 2.0;
        // Edge AA transition band — 1.5 px on each side of the line edge
        let edge_aa = 1.5f32;
        let search_margin = half_width + edge_aa + 1.0;

        // Tip AA transition band at the drawing front
        let tip_aa = (self.scaled_line_width * 0.5).max(2.0);

        let color = Rgba([
            self.line_style.color.0,
            self.line_style.color.1,
            self.line_style.color.2,
            self.line_style.color.3,
        ]);

        // Determine the bounding box of the drawn portion of the path
        let mut x_min = f32::MAX;
        let mut x_max = f32::MIN;
        let mut y_min = f32::MAX;
        let mut y_max = f32::MIN;
        for seg in &self.path_segments {
            let seg_start = seg.cumulative_start();
            if seg_start > target_length + tip_aa {
                break;
            }
            let seg_end = seg_start + seg.length();
            let steps = if seg.length() < 1.0 { 2 } else { 8 };
            let max_t = if seg_end > target_length + tip_aa {
                ((target_length + tip_aa - seg_start) / seg.length()).min(1.0)
            } else {
                1.0
            };
            for i in 0..=steps {
                let t = (i as f32 / steps as f32) * max_t;
                let pt = seg.point_at(t);
                x_min = x_min.min(pt.0);
                x_max = x_max.max(pt.0);
                y_min = y_min.min(pt.1);
                y_max = y_max.max(pt.1);
            }
        }

        let bx_min = (x_min - search_margin).max(0.0) as u32;
        let bx_max = (x_max + search_margin).min(buffer.width() as f32 - 1.0) as u32;
        let by_min = (y_min - search_margin).max(0.0) as u32;
        let by_max = (y_max + search_margin).min(buffer.height() as f32 - 1.0) as u32;

        // Pre-filter segments: only those that overlap the drawn region
        let active_segments: Vec<&PathSegment> = self
            .path_segments
            .iter()
            .filter(|seg| seg.cumulative_start() <= target_length + tip_aa)
            .collect();

        for y in by_min..=by_max {
            for x in bx_min..=bx_max {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;

                // Find the closest segment and its arc-length offset
                let mut best_dist = f32::MAX;
                let mut best_offset = 0.0f32;

                for seg in &active_segments {
                    let (dist, offset, _) = seg.project_pixel(px, py);
                    if offset > target_length + tip_aa {
                        continue;
                    }
                    if dist < best_dist {
                        best_dist = dist;
                        best_offset = offset;
                    }
                }

                if best_dist >= f32::MAX {
                    continue;
                }

                // --- SDF Edge anti-aliasing ---
                // `best_dist - half_width` is the signed distance from the pixel
                // to the line edge (negative = inside, positive = outside).
                // smoothstep gives a soft transition across the edge.
                let dist_from_edge = best_dist - half_width;
                let edge_coverage = 1.0 - smoothstep(-edge_aa, edge_aa, dist_from_edge);

                // --- Tip anti-aliasing ---
                // Smooth fade at the drawing front so the endpoint isn't a hard cut.
                let tip_coverage = if best_offset <= target_length - tip_aa {
                    1.0
                } else if best_offset >= target_length + tip_aa {
                    0.0
                } else if best_offset > target_length {
                    // Past the target: fade out
                    1.0 - smoothstep(0.0, tip_aa, best_offset - target_length)
                } else {
                    // Just before the target: slight fade in at the very front
                    smoothstep(0.0, tip_aa, target_length - best_offset)
                };

                let coverage = edge_coverage * tip_coverage;

                if coverage > 0.01 && self.should_draw(best_offset.min(target_length)) {
                    blend_pixel(buffer, x, y, &color, coverage);
                }
            }
        }
    }

    /// Draw the current frame
    fn draw_frame(&self, buffer: &mut RgbaImage) {
        if self.current_frame < self.draw_frames {
            self.render_path(buffer, self.draw_progress());
        } else {
            self.render_path(buffer, 1.0);
        }
    }
}

impl AnimationInit for RectDrawConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;
        self.current_frame = 0;

        self.scaled_line_width = scale_pixel_for_height(self.line_style.width, height);
        self.scaled_rect_width = scale_pixel_for_height(self.rect_style.width, height);
        self.scaled_rect_height = scale_pixel_for_height(self.rect_style.height, height);
        self.scaled_corner_radius = scale_pixel_for_height(self.rect_style.corner_radius, height);

        self.build_path();

        let draw_duration_seconds = self.duration_ms as f32 / 1000.0;
        self.draw_frames = (draw_duration_seconds * fps as f32).ceil() as usize;
        self.pause_frames = (self.end_pause * fps as f32).ceil() as usize;
        self.total_frames = self.draw_frames + self.pause_frames;
    }

    fn reset(&mut self) {
        self.current_frame = 0;
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, _frames: usize) {}
}

impl Iterator for RectDrawConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }
        let mut buffer = RgbaImage::new(self.width, self.height);
        self.draw_frame(&mut buffer);
        self.current_frame += 1;
        Some(buffer)
    }
}

impl_animation!(RectDrawConfig);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_draw_config_defaults() {
        let config = RectDrawConfig::default();
        assert_eq!(config.line_style.color, (255, 255, 255, 255));
        assert_eq!(config.line_style.width, 4.0);
        assert!(matches!(config.line_style.dash, DashStyle::Solid));
        assert_eq!(config.rect_style.width, 300.0);
        assert_eq!(config.rect_style.height, 200.0);
        assert_eq!(config.rect_style.corner_radius, 0.0);
        assert_eq!(config.duration_ms, 800);
        assert_eq!(config.end_pause, 1.0);
        assert_eq!(config.position, (0.5, 0.5));
    }

    #[test]
    fn test_rect_draw_config_builder() {
        let config = RectDrawConfig::new()
            .with_line_style(LineStyle {
                color: (255, 0, 0, 255),
                width: 6.0,
                dash: DashStyle::Dash(10.0),
            })
            .with_rect_style(RectStyle {
                width: 400.0,
                height: 300.0,
                corner_radius: 20.0,
            })
            .with_duration_ms(1000)
            .with_end_pause(2.0)
            .with_position((0.3, 0.7));
        assert_eq!(config.line_style.color, (255, 0, 0, 255));
        assert_eq!(config.line_style.width, 6.0);
        assert!(matches!(config.line_style.dash, DashStyle::Dash(10.0)));
        assert_eq!(config.rect_style.width, 400.0);
        assert_eq!(config.rect_style.corner_radius, 20.0);
    }

    #[test]
    fn test_draw_progress() {
        let mut config = RectDrawConfig::new()
            .with_duration_ms(1000)
            .with_end_pause(0.5);
        config.init(100, 100, 10);
        assert_eq!(config.draw_frames, 10);
        assert_eq!(config.pause_frames, 5);
        assert_eq!(config.total_frames, 15);
        config.current_frame = 5;
        assert_eq!(config.draw_progress(), 0.5);
        config.current_frame = 12;
        assert_eq!(config.draw_progress(), 1.0);
    }

    #[test]
    fn test_path_building_sharp_corners() {
        let mut config = RectDrawConfig::new()
            .with_rect_style(RectStyle {
                width: 200.0,
                height: 100.0,
                corner_radius: 0.0,
            })
            .with_position((0.5, 0.5));
        config.width = 400;
        config.height = 400;
        config.scaled_rect_width = 200.0;
        config.scaled_rect_height = 100.0;
        config.scaled_corner_radius = 0.0;
        config.build_path();
        assert_eq!(config.path_segments.len(), 4);
        assert!(config.path_length > 0.0);
        assert!(!config.corner_ranges.is_empty());
    }

    #[test]
    fn test_path_building_rounded_corners() {
        let mut config = RectDrawConfig::new()
            .with_rect_style(RectStyle {
                width: 200.0,
                height: 100.0,
                corner_radius: 20.0,
            })
            .with_position((0.5, 0.5));
        config.width = 400;
        config.height = 400;
        config.scaled_rect_width = 200.0;
        config.scaled_rect_height = 100.0;
        config.scaled_corner_radius = 20.0;
        config.build_path();
        assert_eq!(config.path_segments.len(), 8);
        let arc_count = config
            .path_segments
            .iter()
            .filter(|s| s.is_corner())
            .count();
        assert_eq!(arc_count, 4);
        assert_eq!(config.corner_ranges.len(), 4);
    }

    #[test]
    fn test_dash_style_is_draw_at() {
        assert!(DashStyle::Solid.is_draw_at(0.0));
        assert!(DashStyle::Solid.is_draw_at(100.0));
        let dash = DashStyle::Dash(10.0);
        assert!(dash.is_draw_at(5.0));
        assert!(!dash.is_draw_at(10.0));
        assert!(!dash.is_draw_at(15.0));
        assert!(dash.is_draw_at(20.0));
    }

    #[test]
    fn test_project_pixel_line() {
        let seg = PathSegment::Line {
            start: (0.0, 0.0),
            end: (100.0, 0.0),
            length: 100.0,
            cumulative_start: 0.0,
        };
        // Point directly on the line
        let (dist, offset, _) = seg.project_pixel(50.0, 0.0);
        assert!((dist - 0.0).abs() < 0.1);
        assert!((offset - 50.0).abs() < 0.1);
        // Point above the line
        let (dist2, offset2, _) = seg.project_pixel(50.0, 10.0);
        assert!((dist2 - 10.0).abs() < 0.1);
        assert!((offset2 - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_project_pixel_arc() {
        let seg = PathSegment::Arc {
            center: (100.0, 100.0),
            radius: 50.0,
            start_angle: 0.0,
            total_angle: std::f32::consts::FRAC_PI_2,
            length: 50.0 * std::f32::consts::FRAC_PI_2,
            cumulative_start: 0.0,
        };
        // Point on the arc at angle 0 (right side of circle)
        let (dist, _, _) = seg.project_pixel(150.0, 100.0);
        assert!((dist - 0.0).abs() < 1.0);
        // Point outside the arc
        let (dist2, _, _) = seg.project_pixel(160.0, 100.0);
        assert!((dist2 - 10.0).abs() < 1.0);
    }

    #[test]
    fn test_should_draw_corner_always_true() {
        let mut config = RectDrawConfig::new().with_line_style(LineStyle {
            dash: DashStyle::Dash(10.0),
            ..Default::default()
        });
        config.scaled_rect_width = 200.0;
        config.scaled_rect_height = 200.0;
        config.scaled_corner_radius = 30.0;
        config.scaled_line_width = 4.0;
        config.width = 400;
        config.height = 400;
        config.position = (0.5, 0.5);
        config.build_path();
        assert!(!config.corner_ranges.is_empty());
        let (corner_start, corner_end) = config.corner_ranges[0];
        let corner_mid = (corner_start + corner_end) / 2.0;
        assert!(config.should_draw(corner_mid));
    }

    #[test]
    fn test_should_draw_sharp_corner_ranges() {
        let mut config = RectDrawConfig::new().with_line_style(LineStyle {
            dash: DashStyle::Dash(10.0),
            ..Default::default()
        });
        config.scaled_rect_width = 200.0;
        config.scaled_rect_height = 100.0;
        config.scaled_corner_radius = 0.0;
        config.scaled_line_width = 4.0;
        config.width = 400;
        config.height = 400;
        config.position = (0.5, 0.5);
        config.build_path();
        assert!(!config.corner_ranges.is_empty());
        assert!(config.is_in_corner_range(0.0));
        assert!(config.is_in_corner_range(1.0));
    }

    #[test]
    fn test_iterator_frame_count() {
        let mut config = RectDrawConfig::new()
            .with_duration_ms(500)
            .with_end_pause(1.0);
        config.init(400, 400, 25);
        let frames: Vec<RgbaImage> = config.collect();
        assert_eq!(frames.len(), 38);
        for frame in frames {
            assert_eq!(frame.width(), 400);
            assert_eq!(frame.height(), 400);
        }
    }
}
