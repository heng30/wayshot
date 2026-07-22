use crate::AnimationInit;
use background_animation::{
    Animation, AnimationRecordConfig, impl_animation, scale_pixel_for_height,
};
use draw_utils::blend_pixel;
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
                        // Even index = dash (draw), odd index = gap (skip)
                        return i % 2 == 0;
                    }
                }
                true
            }
        }
    }
}

/// Line style configuration for the arrow
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

/// Arrow style configuration
#[derive(Debug, Clone, derivative::Derivative)]
#[derivative(Default)]
pub struct ArrowStyle {
    /// Arrow shaft length in pixels. Default: 200.0
    #[derivative(Default(value = "200.0"))]
    pub length: f32,

    /// Arrow head length in pixels (distance from tip to base). Default: 40.0
    #[derivative(Default(value = "40.0"))]
    pub head_length: f32,

    /// Arrow head width in pixels (total width at base). Default: 30.0
    #[derivative(Default(value = "30.0"))]
    pub head_width: f32,

    /// Arrow direction in degrees (0 = right, 90 = down, 180 = left, 270 = up). Default: 0
    #[derivative(Default(value = "0.0"))]
    pub direction: f32,
}

/// A segment of the arrow path
#[derive(Debug, Clone)]
enum PathSegment {
    /// Shaft line (dash applies)
    Shaft {
        start: (f32, f32),
        end: (f32, f32),
        length: f32,
        cumulative_start: f32,
    },
    /// Head line (always solid, dash does not apply)
    Head {
        start: (f32, f32),
        end: (f32, f32),
        length: f32,
        cumulative_start: f32,
    },
}

impl PathSegment {
    fn length(&self) -> f32 {
        match self {
            PathSegment::Shaft { length, .. } | PathSegment::Head { length, .. } => *length,
        }
    }

    fn cumulative_start(&self) -> f32 {
        match self {
            PathSegment::Shaft {
                cumulative_start, ..
            }
            | PathSegment::Head {
                cumulative_start, ..
            } => *cumulative_start,
        }
    }

    /// Whether dash style should apply to this segment
    fn is_shaft(&self) -> bool {
        matches!(self, PathSegment::Shaft { .. })
    }

    /// Get the start and end points of this segment
    fn endpoints(&self) -> ((f32, f32), (f32, f32)) {
        match self {
            PathSegment::Shaft { start, end, .. } | PathSegment::Head { start, end, .. } => {
                (*start, *end)
            }
        }
    }

    /// Get the point at parameter t (0.0 to 1.0) along this segment
    fn point_at(&self, t: f32) -> (f32, f32) {
        match self {
            PathSegment::Shaft { start, end, .. } | PathSegment::Head { start, end, .. } => {
                let x = start.0 + (end.0 - start.0) * t;
                let y = start.1 + (end.1 - start.1) * t;
                (x, y)
            }
        }
    }
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ArrowDrawConfig {
    /// Line style configuration
    #[derivative(Default(value = "LineStyle::default()"))]
    pub line_style: LineStyle,

    /// Arrow style configuration
    #[derivative(Default(value = "ArrowStyle::default()"))]
    pub arrow_style: ArrowStyle,

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

    /// Scaled shaft length based on output height (1080P standard)
    #[setters(skip)]
    scaled_length: f32,

    /// Scaled head length based on output height (1080P standard)
    #[setters(skip)]
    scaled_head_length: f32,

    /// Scaled head width based on output height (1080P standard)
    #[setters(skip)]
    scaled_head_width: f32,

    /// Precomputed path segments
    #[setters(skip)]
    path_segments: Vec<PathSegment>,

    /// Total path length
    #[setters(skip)]
    path_length: f32,
}

impl ArrowDrawConfig {
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

    /// Convert normalized position to pixel coordinates (center of arrow)
    fn pixel_position(&self) -> (f32, f32) {
        let x = self.position.0 * self.width as f32;
        let y = self.position.1 * self.height as f32;
        (x, y)
    }

    /// Build the arrow path as segments.
    /// Arrow shape: shaft (line) + head (two lines forming a V)
    /// Drawing order: shaft first, then left head line, then right head line
    fn build_path(&mut self) {
        let (cx, cy) = self.pixel_position();
        let direction_rad = self.arrow_style.direction.to_radians();

        // Calculate arrow geometry
        let shaft_length = self.scaled_length;
        let head_length = self.scaled_head_length;
        let head_width = self.scaled_head_width;

        // Direction unit vector
        let dir_x = direction_rad.cos();
        let dir_y = direction_rad.sin();

        // Perpendicular unit vector (for head width)
        let perp_x = -dir_y;
        let perp_y = dir_x;

        // Arrow tip position (center + half shaft in direction)
        let tip_x = cx + dir_x * shaft_length * 0.5;
        let tip_y = cy + dir_y * shaft_length * 0.5;

        // Arrow tail position (center - half shaft in direction)
        let tail_x = cx - dir_x * shaft_length * 0.5;
        let tail_y = cy - dir_y * shaft_length * 0.5;

        // Head base center (back from tip by head_length)
        let head_base_x = tip_x - dir_x * head_length;
        let head_base_y = tip_y - dir_y * head_length;

        // Head left and right points
        let head_left_x = head_base_x + perp_x * head_width * 0.5;
        let head_left_y = head_base_y + perp_y * head_width * 0.5;
        let head_right_x = head_base_x - perp_x * head_width * 0.5;
        let head_right_y = head_base_y - perp_y * head_width * 0.5;

        self.path_segments.clear();
        let mut cumulative = 0.0f32;

        // Segment 1: Shaft (tail to tip) — dash applies
        let shaft_dx = tip_x - tail_x;
        let shaft_dy = tip_y - tail_y;
        let shaft_len = (shaft_dx * shaft_dx + shaft_dy * shaft_dy).sqrt();
        self.path_segments.push(PathSegment::Shaft {
            start: (tail_x, tail_y),
            end: (tip_x, tip_y),
            length: shaft_len,
            cumulative_start: cumulative,
        });
        cumulative += shaft_len;

        // Segment 2: Left head line (tip to head_left) — always solid
        let left_dx = head_left_x - tip_x;
        let left_dy = head_left_y - tip_y;
        let left_len = (left_dx * left_dx + left_dy * left_dy).sqrt();
        self.path_segments.push(PathSegment::Head {
            start: (tip_x, tip_y),
            end: (head_left_x, head_left_y),
            length: left_len,
            cumulative_start: cumulative,
        });
        cumulative += left_len;

        // Segment 3: Right head line (tip to head_right) — always solid
        let right_dx = head_right_x - tip_x;
        let right_dy = head_right_y - tip_y;
        let right_len = (right_dx * right_dx + right_dy * right_dy).sqrt();
        self.path_segments.push(PathSegment::Head {
            start: (tip_x, tip_y),
            end: (head_right_x, head_right_y),
            length: right_len,
            cumulative_start: cumulative,
        });
        cumulative += right_len;

        self.path_length = cumulative;
    }

    /// Check if a given arc_length offset should be drawn.
    /// Dash only applies to shaft; head is always solid.
    fn should_draw(&self, offset: f32, is_shaft: bool) -> bool {
        if !is_shaft {
            return true;
        }
        self.line_style.dash.is_draw_at(offset)
    }

    /// Render the path using distance-field anti-aliasing.
    /// Uses exact point-to-segment distance for precise line joins.
    fn render_path(&self, buffer: &mut RgbaImage, draw_progress: f32) {
        let target_length = self.path_length * draw_progress;
        if target_length < 0.01 {
            return;
        }

        let half_width = self.scaled_line_width / 2.0;
        // Anti-aliasing range: smooth transition zone at line edges
        let aa_range = (self.scaled_line_width * 0.25).max(0.8);

        let color = Rgba([
            self.line_style.color.0,
            self.line_style.color.1,
            self.line_style.color.2,
            self.line_style.color.3,
        ]);

        // Build the list of active segments (up to target_length)
        let mut active_segments: Vec<(&PathSegment, f32)> = Vec::new(); // (segment, usable_length)
        let mut x_min = f32::MAX;
        let mut x_max = f32::MIN;
        let mut y_min = f32::MAX;
        let mut y_max = f32::MIN;

        for seg in &self.path_segments {
            let seg_start = seg.cumulative_start();
            let seg_len = seg.length();

            if seg_start >= target_length {
                break;
            }

            let usable_len = (target_length - seg_start).min(seg_len);
            active_segments.push((seg, usable_len));

            // Expand bounding box from segment endpoints
            let (s, e) = seg.endpoints();
            // If partially drawn, clip the end point
            if usable_len < seg_len {
                let t = usable_len / seg_len;
                let clipped_end = seg.point_at(t);
                x_min = x_min.min(s.0).min(clipped_end.0);
                x_max = x_max.max(s.0).max(clipped_end.0);
                y_min = y_min.min(s.1).min(clipped_end.1);
                y_max = y_max.max(s.1).max(clipped_end.1);
            } else {
                x_min = x_min.min(s.0).min(e.0);
                x_max = x_max.max(s.0).max(e.0);
                y_min = y_min.min(s.1).min(e.1);
                y_max = y_max.max(s.1).max(e.1);
            }
        }

        if active_segments.is_empty() {
            return;
        }

        let margin = (half_width + aa_range).ceil() as i32 + 2;
        let bx_min = (x_min - margin as f32).max(0.0) as u32;
        let bx_max = (x_max + margin as f32).min(buffer.width() as f32 - 1.0) as u32;
        let by_min = (y_min - margin as f32).max(0.0) as u32;
        let by_max = (y_max + margin as f32).min(buffer.height() as f32 - 1.0) as u32;

        // Distance-field rendering: for each pixel, compute exact distance to each segment
        for y in by_min..=by_max {
            for x in bx_min..=bx_max {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;

                let mut min_dist = f32::MAX;
                let mut best_offset = 0.0f32;
                let mut best_is_shaft = true;

                for &(seg, usable_len) in &active_segments {
                    let (start, end) = seg.endpoints();
                    let seg_len = seg.length();
                    let seg_cum = seg.cumulative_start();
                    let is_shaft = seg.is_shaft();

                    // If segment is partially drawn, clip the end
                    let actual_end = if usable_len < seg_len {
                        let t = usable_len / seg_len;
                        seg.point_at(t)
                    } else {
                        end
                    };

                    let (dist, offset) =
                        dist_point_to_segment(px, py, start, actual_end, seg_cum, usable_len);

                    if dist < min_dist {
                        min_dist = dist;
                        best_offset = offset;
                        best_is_shaft = is_shaft;
                    }
                }

                let dist_from_edge = min_dist - half_width;

                if dist_from_edge <= aa_range {
                    let coverage = if dist_from_edge <= 0.0 {
                        1.0
                    } else {
                        1.0 - dist_from_edge / aa_range
                    };

                    if coverage > 0.02 && self.should_draw(best_offset, best_is_shaft) {
                        blend_pixel(buffer, x, y, &color, coverage);
                    }
                }
            }
        }
    }

    /// Draw the current frame
    fn draw_frame(&self, buffer: &mut RgbaImage) {
        if self.current_frame < self.draw_frames {
            let draw_progress = self.draw_progress();
            self.render_path(buffer, draw_progress);
        } else {
            self.render_path(buffer, 1.0);
        }
    }
}

/// Compute the distance from a point to a line segment,
/// and the closest point's arc-length offset along the segment.
fn dist_point_to_segment(
    px: f32,
    py: f32,
    start: (f32, f32),
    end: (f32, f32),
    seg_cumulative_start: f32,
    seg_length: f32,
) -> (f32, f32) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.001 {
        let dist = ((px - start.0).powi(2) + (py - start.1).powi(2)).sqrt();
        return (dist, seg_cumulative_start);
    }
    let t = (((px - start.0) * dx + (py - start.1) * dy) / len_sq).clamp(0.0, 1.0);
    let closest_x = start.0 + t * dx;
    let closest_y = start.1 + t * dy;
    let dist = ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt();
    let offset = seg_cumulative_start + t * seg_length;
    (dist, offset)
}

impl AnimationInit for ArrowDrawConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;
        self.current_frame = 0;

        self.scaled_line_width = scale_pixel_for_height(self.line_style.width, height);
        self.scaled_length = scale_pixel_for_height(self.arrow_style.length, height);
        self.scaled_head_length = scale_pixel_for_height(self.arrow_style.head_length, height);
        self.scaled_head_width = scale_pixel_for_height(self.arrow_style.head_width, height);

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

impl Iterator for ArrowDrawConfig {
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

impl_animation!(ArrowDrawConfig);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arrow_draw_config_defaults() {
        let config = ArrowDrawConfig::default();
        assert_eq!(config.line_style.color, (255, 255, 255, 255));
        assert_eq!(config.line_style.width, 4.0);
        assert!(matches!(config.line_style.dash, DashStyle::Solid));
        assert_eq!(config.arrow_style.length, 200.0);
        assert_eq!(config.arrow_style.head_length, 40.0);
        assert_eq!(config.arrow_style.head_width, 30.0);
        assert_eq!(config.arrow_style.direction, 0.0);
        assert_eq!(config.duration_ms, 800);
        assert_eq!(config.end_pause, 1.0);
        assert_eq!(config.position, (0.5, 0.5));
    }

    #[test]
    fn test_arrow_draw_config_builder() {
        let config = ArrowDrawConfig::new()
            .with_line_style(LineStyle {
                color: (255, 0, 0, 255),
                width: 6.0,
                dash: DashStyle::Dash(10.0),
            })
            .with_arrow_style(ArrowStyle {
                length: 300.0,
                head_length: 50.0,
                head_width: 40.0,
                direction: 45.0,
            })
            .with_duration_ms(1000)
            .with_end_pause(2.0)
            .with_position((0.3, 0.7));

        assert_eq!(config.line_style.color, (255, 0, 0, 255));
        assert_eq!(config.line_style.width, 6.0);
        assert!(matches!(config.line_style.dash, DashStyle::Dash(10.0)));
        assert_eq!(config.arrow_style.length, 300.0);
        assert_eq!(config.arrow_style.head_length, 50.0);
        assert_eq!(config.arrow_style.head_width, 40.0);
        assert_eq!(config.arrow_style.direction, 45.0);
        assert_eq!(config.duration_ms, 1000);
        assert_eq!(config.end_pause, 2.0);
        assert_eq!(config.position, (0.3, 0.7));
    }

    #[test]
    fn test_draw_progress() {
        let mut config = ArrowDrawConfig::new()
            .with_duration_ms(1000)
            .with_end_pause(0.5);
        config.init(100, 100, 10);

        assert_eq!(config.draw_frames, 10);
        assert_eq!(config.pause_frames, 5);
        assert_eq!(config.total_frames, 15);

        config.current_frame = 5;
        assert_eq!(config.draw_progress(), 0.5);

        config.current_frame = 10;
        assert_eq!(config.draw_progress(), 1.0);

        config.current_frame = 12;
        assert_eq!(config.draw_progress(), 1.0);
    }

    #[test]
    fn test_path_building() {
        let mut config = ArrowDrawConfig::new()
            .with_arrow_style(ArrowStyle {
                length: 200.0,
                head_length: 40.0,
                head_width: 30.0,
                direction: 0.0, // pointing right
            })
            .with_position((0.5, 0.5));
        config.width = 400;
        config.height = 400;
        config.scaled_length = 200.0;
        config.scaled_head_length = 40.0;
        config.scaled_head_width = 30.0;
        config.build_path();

        // 3 line segments: shaft + 2 head lines
        assert_eq!(config.path_segments.len(), 3);
        assert!(config.path_length > 0.0);
    }

    #[test]
    fn test_dash_style_is_draw_at() {
        // Solid always draws
        let solid = DashStyle::Solid;
        assert!(solid.is_draw_at(0.0));
        assert!(solid.is_draw_at(100.0));

        // Dash(10): draw 0-10, gap 10-20, draw 20-30, ...
        let dash = DashStyle::Dash(10.0);
        assert!(dash.is_draw_at(0.0));
        assert!(dash.is_draw_at(5.0));
        assert!(!dash.is_draw_at(10.0));
        assert!(!dash.is_draw_at(15.0));
        assert!(dash.is_draw_at(20.0));

        // Custom: [20, 10, 5, 10] — draw 0-20, gap 20-30, draw 30-35, gap 35-45
        let custom = DashStyle::Custom(vec![20.0, 10.0, 5.0, 10.0]);
        assert!(custom.is_draw_at(0.0));
        assert!(custom.is_draw_at(19.0));
        assert!(!custom.is_draw_at(20.0));
        assert!(!custom.is_draw_at(25.0));
        assert!(custom.is_draw_at(30.0));
        assert!(custom.is_draw_at(34.0));
        assert!(!custom.is_draw_at(35.0));
    }

    #[test]
    fn test_should_draw_head_always_true() {
        let config = ArrowDrawConfig::new().with_line_style(LineStyle {
            dash: DashStyle::Dash(10.0),
            ..Default::default()
        });

        // Head segments always draw, regardless of dash offset
        assert!(config.should_draw(15.0, false)); // head at gap offset
        assert!(config.should_draw(5.0, false)); // head at draw offset

        // Shaft segments respect dash
        assert!(!config.should_draw(15.0, true)); // shaft at gap offset
        assert!(config.should_draw(5.0, true)); // shaft at draw offset
    }

    #[test]
    fn test_iterator_frame_count() {
        let mut config = ArrowDrawConfig::new()
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
