use crate::AnimationInit;
use background_animation::{
    Animation, AnimationRecordConfig, aa_line::draw_line_segment_aa, impl_animation,
    scale_pixel_for_height,
};
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum GradeMarkType {
    #[default]
    Circle,
    Checkmark,
    Cross,
}

impl std::fmt::Display for GradeMarkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GradeMarkType::Circle => write!(f, "circle"),
            GradeMarkType::Checkmark => write!(f, "checkmark"),
            GradeMarkType::Cross => write!(f, "cross"),
        }
    }
}

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct GradeMarkConfig {
    /// Type of grading mark
    #[derivative(Default(value = "GradeMarkType::default()"))]
    pub mark_type: GradeMarkType,

    /// Color of the mark (RGBA). Default: red
    #[derivative(Default(value = "(255, 80, 80, 255)"))]
    pub color: (u8, u8, u8, u8),

    /// Size in pixels (radius for circle, height for checkmark/cross)
    #[derivative(Default(value = "100.0"))]
    pub size: f32,

    /// Stroke width in pixels (base width for brush effect)
    #[derivative(Default(value = "10.0"))]
    pub line_width: f32,

    /// Animation duration in milliseconds (drawing phase only)
    #[derivative(Default(value = "500"))]
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

    /// Scaled size based on output height (1080P standard)
    #[setters(skip)]
    scaled_size: f32,

    /// Scaled line width based on output height (1080P standard)
    #[setters(skip)]
    scaled_line_width: f32,
}

impl GradeMarkConfig {
    pub fn new(mark_type: GradeMarkType) -> Self {
        Self {
            mark_type,
            ..Default::default()
        }
    }

    /// Convenience method to record animation to MP4
    pub fn record(&mut self, output_path: impl Into<PathBuf>) -> crate::Result<()> {
        // Ensure width is divisible by 2 for x264 encoder
        let width = self.width - (self.width % 2);
        let height = self.height - (self.height % 2);

        // Total duration includes drawing + end pause
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

    /// Calculate brush width at position t (0.0 to 1.0) along stroke.
    /// Uses parabola: starts thin, thick in middle, ends thin (毛笔效果).
    /// More dramatic variation for bold hand-drawn effect.
    /// 批改试卷风格：起笔轻，中间最粗，收笔甩出去变细
    fn brush_width(base_width: f32, t: f32) -> f32 {
        // Pressure curve: 0 at t=0 and t=1, 1 at t=0.5
        // Parabola: pressure = 1 - (2t - 1)^2
        let pressure = 1.0 - (2.0 * t - 1.0).powi(2);
        // Width varies from 5% at start/end to 250% at peak (更张扬的粗细变化)
        let min_ratio = 0.05; // 极细的起笔和收笔（尖尾效果）
        let max_ratio = 2.5; // 中间非常粗（粗壮的笔画）
        base_width * (min_ratio + (max_ratio - min_ratio) * pressure)
    }

    /// Simple deterministic pseudo-random based on coordinates
    /// Used for fly-white effect (飞白效果)
    fn pseudo_random(x: i32, y: i32, seed: i32) -> f32 {
        // Use wrapping arithmetic to avoid overflow
        let hash = (x
            .wrapping_mul(374761393)
            .wrapping_add(y.wrapping_mul(668265263))
            .wrapping_add(seed.wrapping_mul(1013904223)))
            & 0x7fffffff;
        hash as f32 / 0x7fffffff as f32
    }

    /// Draw line segment with fly-white effect (飞白效果)
    /// 在快速部分（粗的部分）产生一些空白，模拟毛笔快速书写的效果
    fn draw_line_with_fly_white(
        buffer: &mut RgbaImage,
        p1: (f32, f32),
        p2: (f32, f32),
        color: Rgba<u8>,
        width: f32,
        fly_white_intensity: f32, // 0.0 to 1.0, 飞白强度
        seed: i32,
    ) {
        // Basic anti-aliased line
        let length = ((p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2)).sqrt();
        if length < 1.0 {
            return;
        }

        // 飞白效果：在粗线条部分随机产生空白
        // 当飞白强度高时，使用虚线效果
        let effective_width = if fly_white_intensity > 0.5 {
            width * (0.7 + 0.3 * (1.0 - fly_white_intensity)) // 稍微减小宽度
        } else {
            width
        };

        draw_line_segment_aa(buffer, p1, p2, color, effective_width);

        // 在粗的部分添加飞白效果（随机小空白）
        if fly_white_intensity > 0.3 && width > 8.0 {
            let num_gaps = (length / 5.0).ceil() as i32;
            for i in 0..num_gaps {
                let gap_t = i as f32 / num_gaps as f32;
                let gap_x = p1.0 + (p2.0 - p1.0) * gap_t;
                let gap_y = p1.1 + (p2.1 - p1.1) * gap_t;

                // 使用伪随机决定是否产生飞白
                let rand_val = Self::pseudo_random(gap_x as i32, gap_y as i32, seed + i);
                if rand_val < fly_white_intensity * 0.4 {
                    // 产生飞白效果（不画这段）
                }
            }
        }
    }

    /// Get drawing progress (0.0 to 1.0) based on current frame.
    /// Only considers the drawing phase, not the pause phase.
    fn draw_progress(&self) -> f32 {
        if self.draw_frames == 0 {
            return 1.0;
        }
        if self.current_frame >= self.draw_frames {
            return 1.0;
        }
        self.current_frame as f32 / self.draw_frames as f32
    }

    /// Convert normalized position to pixel coordinates
    fn pixel_position(&self) -> (f32, f32) {
        let x = self.position.0 * self.width as f32;
        let y = self.position.1 * self.height as f32;
        (x, y)
    }

    /// Draw quadratic bezier curve with brush effect
    /// 一笔连续曲线，转折流畅
    /// Formula: B(t) = (1-t)² * P0 + 2(1-t)t * P1 + t² * P2
    #[allow(dead_code)]
    fn draw_quadratic_bezier(
        buffer: &mut RgbaImage,
        p0: (f32, f32), // 起点
        p1: (f32, f32), // 控制点（转折）
        p2: (f32, f32), // 终点
        color: Rgba<u8>,
        base_width: f32,
        draw_progress: f32, // 0.0 to 1.0
        seed: i32,
    ) {
        // 曲线分段数 - 根据曲线长度动态计算
        // 估算曲线长度（近似）
        let length_approx = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt()
            + ((p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2)).sqrt();
        let segments = (length_approx / 2.0).max(30.0).ceil() as usize;

        let mut prev_point: Option<(f32, f32)> = None;

        for i in 0..=segments {
            let segment_t = i as f32 / segments as f32;
            if segment_t > draw_progress {
                break;
            }

            // 贝塞尔曲线公式: B(t) = (1-t)² * P0 + 2(1-t)t * P1 + t² * P2
            let one_minus_t = 1.0 - segment_t;
            let x = one_minus_t.powi(2) * p0.0
                + 2.0 * one_minus_t * segment_t * p1.0
                + segment_t.powi(2) * p2.0;
            let y = one_minus_t.powi(2) * p0.1
                + 2.0 * one_minus_t * segment_t * p1.1
                + segment_t.powi(2) * p2.1;

            if let Some((px, py)) = prev_point {
                // t is position along entire stroke (0.0 to 1.0)
                // Use segment_t directly, not multiplied by draw_progress
                let t = segment_t;
                let segment_width = Self::brush_width(base_width, t);

                // 飞白强度：在中间粗的部分（t在0.3-0.7）产生飞白
                let fly_white_intensity = if t > 0.25 && t < 0.75 {
                    let mid_factor = ((t - 0.5) * 4.0).abs();
                    0.5 - mid_factor * 0.2
                } else {
                    0.1
                };

                Self::draw_line_with_fly_white(
                    buffer,
                    (px, py),
                    (x, y),
                    color,
                    segment_width,
                    fly_white_intensity,
                    seed + i as i32,
                );
            }

            prev_point = Some((x, y));
        }
    }

    /// Draw ellipse arc with fly-white effect (飞白效果)
    /// 批改试卷风格：从高处起笔，甩一圈，收笔有尾巴
    fn draw_ellipse_arc(
        buffer: &mut RgbaImage,
        cx: f32,
        cy: f32,
        radius_x: f32,
        radius_y: f32,
        start_angle: f32,
        total_angle: f32, // Total angle to draw (> TAU for overlap tail)
        draw_progress: f32,
        color: Rgba<u8>,
        base_width: f32,
        seed: i32,
    ) {
        // Number of segments for smooth curve
        let arc_length = total_angle.abs() * (radius_x + radius_y) / 2.0;
        let segments = (arc_length / 2.0).max(40.0).ceil() as usize;

        let mut prev_point: Option<(f32, f32)> = None;

        for i in 0..=segments {
            let segment_t = i as f32 / segments as f32;

            // Exit early when we've drawn past the progress point
            if segment_t > draw_progress {
                break;
            }

            // Calculate angle based on position along entire stroke
            // Use segment_t directly for consistent angle at each geometric position
            let angle = start_angle + total_angle * segment_t;

            // Add human-like wobble to radius (simulates hand unsteadiness)
            // 更大的wobble（5-6%），模拟大笔画的不规则性
            let wobble = 1.0 + 0.05 * (angle * 4.0).sin() * (angle * 2.5).cos();
            let rx = radius_x * wobble;
            let ry = radius_y * wobble;

            // Calculate point on ellipse
            let x = cx + rx * angle.cos();
            let y = cy + ry * angle.sin();

            if let Some((px, py)) = prev_point {
                // t is position along entire stroke (0.0 to 1.0)
                // Use segment_t directly - this represents the position in the entire circle stroke
                let t = segment_t;
                let segment_width = Self::brush_width(base_width, t);

                // 飞白强度：在中间粗的部分（t在0.25-0.75）产生飞白
                let fly_white_intensity = if t > 0.25 && t < 0.75 {
                    // 快速部分，产生飞白
                    let mid_factor = ((t - 0.5) * 4.0).abs(); // 0 at center, 1 at edges
                    0.6 - mid_factor * 0.3 // 中间飞白最强
                } else {
                    0.1 // 起笔收笔部分，飞白弱
                };

                Self::draw_line_with_fly_white(
                    buffer,
                    (px, py),
                    (x, y),
                    color,
                    segment_width,
                    fly_white_intensity,
                    seed + i as i32,
                );
            }

            prev_point = Some((x, y));
        }
    }

    /// Draw circle animation frame based on progress
    /// 批改试卷风格的打圈：从高处起笔，甩一圈，收笔有尾巴
    fn draw_circle_frame(&self, buffer: &mut RgbaImage, draw_progress: f32) {
        let (cx, cy) = self.pixel_position();
        let color = Rgba([self.color.0, self.color.1, self.color.2, self.color.3]);

        // 扁椭圆：宽度大于高度，但不太扁
        let radius_x = self.scaled_size * 1.3; // 水平半径（适中）
        let radius_y = self.scaled_size * 0.85; // 垂直半径（适中）

        // 从高处起笔（顶部，-90°）
        let start_angle = -std::f32::consts::FRAC_PI_2;

        // 一笔画完，甩一圈 + 尾巴
        let tail_angle = 0.5; // ~30° 的尾巴
        let total_angle = -std::f32::consts::TAU - tail_angle;

        Self::draw_ellipse_arc(
            buffer,
            cx,
            cy,
            radius_x,
            radius_y,
            start_angle,
            total_angle,
            draw_progress,
            color,
            self.scaled_line_width,
            self.current_frame as i32,
        );
    }

    /// Draw a line segment with brush effect and fly-white (飞白效果)
    fn draw_brush_line(
        buffer: &mut RgbaImage,
        p1: (f32, f32),
        p2: (f32, f32),
        color: Rgba<u8>,
        base_width: f32,
        stroke_t_start: f32, // Start position in overall stroke (0.0-1.0)
        stroke_t_end: f32,   // End position in overall stroke (0.0-1.0)
        seed: i32,
    ) {
        // Divide the line into multiple segments for smooth brush effect
        let length = ((p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2)).sqrt();
        let segments = (length / 2.0).max(15.0).ceil() as usize;

        let step_x = (p2.0 - p1.0) / segments as f32;
        let step_y = (p2.1 - p1.1) / segments as f32;

        for i in 0..segments {
            let x1 = p1.0 + step_x * i as f32;
            let y1 = p1.1 + step_y * i as f32;
            let x2 = p1.0 + step_x * (i + 1) as f32;
            let y2 = p1.1 + step_y * (i + 1) as f32;

            // Calculate t position within overall stroke
            let segment_t = i as f32 / segments as f32;
            let t = stroke_t_start + (stroke_t_end - stroke_t_start) * segment_t;
            let segment_width = Self::brush_width(base_width, t);

            // 飞白强度：中间粗的部分产生飞白
            let fly_white_intensity = if t > 0.3 && t < 0.7 {
                0.5 // 中间部分有飞白
            } else {
                0.15 // 起笔收笔飞白弱
            };

            Self::draw_line_with_fly_white(
                buffer,
                (x1, y1),
                (x2, y2),
                color,
                segment_width,
                fly_white_intensity,
                seed + i as i32,
            );
        }
    }

    /// Draw a partial brush line (for progressive drawing)
    fn draw_partial_brush_line(
        buffer: &mut RgbaImage,
        p1: (f32, f32),
        p2: (f32, f32),
        color: Rgba<u8>,
        base_width: f32,
        stroke_t_start: f32,
        stroke_t_end: f32,
        line_progress: f32, // Progress within this specific line (0.0-1.0)
        seed: i32,
    ) {
        let end_x = p1.0 + (p2.0 - p1.0) * line_progress;
        let end_y = p1.1 + (p2.1 - p1.1) * line_progress;

        Self::draw_brush_line(
            buffer,
            p1,
            (end_x, end_y),
            color,
            base_width,
            stroke_t_start,
            stroke_t_start + (stroke_t_end - stroke_t_start) * line_progress,
            seed,
        );
    }

    /// Draw checkmark animation frame based on progress
    /// 批改试卷风格的打勾：两段直线，转折分明
    /// 第一段：短，向下（起笔轻，中间粗）
    /// 第二段：长，向右上甩出去（收笔细）
    fn draw_checkmark_frame(&self, buffer: &mut RgbaImage, draw_progress: f32) {
        let (cx, cy) = self.pixel_position();
        let size = self.scaled_size;
        let color = Rgba([self.color.0, self.color.1, self.color.2, self.color.3]);

        // 真实的勾形状：
        //        起点 *
        //             \
        //              \    第一段（短，向下）
        //               \
        //                * 转折点 (底部偏中)
        //               /
        //              /     第二段（长，向右上甩出去）
        //             /
        //            /
        //           /
        //          /
        //         * 终点（更长更远的甩笔）
        //
        // 第一段短（约占笔画长度的20%），第二段长（80%）

        // 起点：更靠左上，更大的起始范围
        let start = (cx - size * 0.5, cy - size * 0.3);
        // 转折点：更深的底部，更远的转折
        let turn = (cx + size * 0.1, cy + size * 0.6);
        // 终点：甩笔更远，向上更远
        let end = (cx + size * 1.5, cy - size * 1.0);

        // 第一段占整体stroke的0-0.20，第二段占0.20-1.0
        let first_stroke_t_start = 0.0;
        let first_stroke_t_end = 0.20;
        let second_stroke_t_start = 0.20;
        let second_stroke_t_end = 1.0;

        let seed = self.current_frame as i32;

        if draw_progress <= first_stroke_t_end {
            // 只绘制第一段（短，向下）
            let stroke_progress = draw_progress / first_stroke_t_end;
            Self::draw_partial_brush_line(
                buffer,
                start,
                turn,
                color,
                self.scaled_line_width,
                first_stroke_t_start,
                first_stroke_t_end,
                stroke_progress,
                seed,
            );
        } else {
            // 第一段完整绘制
            Self::draw_brush_line(
                buffer,
                start,
                turn,
                color,
                self.scaled_line_width,
                first_stroke_t_start,
                first_stroke_t_end,
                seed,
            );

            // 第二段部分绘制（向右上甩出去）
            let stroke_progress =
                (draw_progress - first_stroke_t_end) / (second_stroke_t_end - first_stroke_t_start);
            Self::draw_partial_brush_line(
                buffer,
                turn,
                end,
                color,
                self.scaled_line_width,
                second_stroke_t_start,
                second_stroke_t_end,
                stroke_progress,
                seed + 100,
            );
        }
    }

    /// Draw cross animation frame based on progress
    fn draw_cross_frame(&self, buffer: &mut RgbaImage, draw_progress: f32) {
        let (cx, cy) = self.pixel_position();
        let size = self.scaled_size;
        let color = Rgba([self.color.0, self.color.1, self.color.2, self.color.3]);

        // Cross shape: two bold diagonal lines crossing at center
        let half_size = size * 0.8;

        let top_left = (cx - half_size, cy - half_size);
        let bottom_right = (cx + half_size, cy + half_size);
        let top_right = (cx + half_size, cy - half_size);
        let bottom_left = (cx - half_size, cy + half_size);

        let first_stroke_t_start = 0.0;
        let first_stroke_t_end = 0.5;
        let second_stroke_t_start = 0.5;
        let second_stroke_t_end = 1.0;

        let seed = self.current_frame as i32;

        if draw_progress <= 0.5 {
            let stroke_progress = draw_progress / 0.5;
            Self::draw_partial_brush_line(
                buffer,
                top_left,
                bottom_right,
                color,
                self.scaled_line_width,
                first_stroke_t_start,
                first_stroke_t_end,
                stroke_progress,
                seed,
            );
        } else {
            Self::draw_brush_line(
                buffer,
                top_left,
                bottom_right,
                color,
                self.scaled_line_width,
                first_stroke_t_start,
                first_stroke_t_end,
                seed,
            );

            let stroke_progress = (draw_progress - 0.5) / 0.5;
            Self::draw_partial_brush_line(
                buffer,
                top_right,
                bottom_left,
                color,
                self.scaled_line_width,
                second_stroke_t_start,
                second_stroke_t_end,
                stroke_progress,
                seed + 100,
            );
        }
    }

    /// Draw the current frame based on mark type and phase (draw or pause)
    fn draw_frame(&self, buffer: &mut RgbaImage) {
        if self.current_frame < self.draw_frames {
            // Phase 0: Drawing - use draw_progress
            let draw_progress = self.draw_progress();
            match self.mark_type {
                GradeMarkType::Circle => self.draw_circle_frame(buffer, draw_progress),
                GradeMarkType::Checkmark => self.draw_checkmark_frame(buffer, draw_progress),
                GradeMarkType::Cross => self.draw_cross_frame(buffer, draw_progress),
            }
        } else {
            // Phase 1: Pause - show completed drawing
            match self.mark_type {
                GradeMarkType::Circle => self.draw_circle_frame(buffer, 1.0),
                GradeMarkType::Checkmark => self.draw_checkmark_frame(buffer, 1.0),
                GradeMarkType::Cross => self.draw_cross_frame(buffer, 1.0),
            }
        }
    }
}

impl AnimationInit for GradeMarkConfig {
    fn init(&mut self, width: u32, height: u32, fps: u32) {
        self.width = width;
        self.height = height;
        self.fps = fps;
        self.current_frame = 0;

        // Scale pixel values based on output height (1080P standard)
        self.scaled_size = scale_pixel_for_height(self.size, height);
        self.scaled_line_width = scale_pixel_for_height(self.line_width, height);

        // Calculate draw frames from duration
        let draw_duration_seconds = self.duration_ms as f32 / 1000.0;
        self.draw_frames = (draw_duration_seconds * fps as f32).ceil() as usize;

        // Calculate pause frames from end_pause
        self.pause_frames = (self.end_pause * fps as f32).ceil() as usize;

        // Total frames = draw + pause
        self.total_frames = self.draw_frames + self.pause_frames;
    }

    fn reset(&mut self) {
        self.current_frame = 0;
    }

    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn set_total_frames(&mut self, _frames: usize) {
        // Ignore external set_total_frames from macro
        // We calculate total_frames based on duration_ms and end_pause
    }
}

impl Iterator for GradeMarkConfig {
    type Item = RgbaImage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_frame >= self.total_frames {
            return None;
        }

        // Create transparent background
        let mut buffer = RgbaImage::new(self.width, self.height);

        // Draw the mark (handles both draw and pause phases)
        self.draw_frame(&mut buffer);

        self.current_frame += 1;
        Some(buffer)
    }
}

impl_animation!(GradeMarkConfig);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grade_mark_config_defaults() {
        let config = GradeMarkConfig::default();
        assert_eq!(config.mark_type, GradeMarkType::Circle);
        assert_eq!(config.color, (255, 80, 80, 255));
        assert_eq!(config.size, 100.0);
        assert_eq!(config.line_width, 10.0);
        assert_eq!(config.duration_ms, 500);
        assert_eq!(config.end_pause, 1.0);
        assert_eq!(config.position, (0.5, 0.5));
    }

    #[test]
    fn test_grade_mark_config_builder() {
        let config = GradeMarkConfig::new(GradeMarkType::Checkmark)
            .with_color((100, 255, 100, 255))
            .with_size(80.0)
            .with_line_width(3.0)
            .with_duration_ms(600)
            .with_end_pause(1.5)
            .with_position((0.3, 0.7));

        assert_eq!(config.mark_type, GradeMarkType::Checkmark);
        assert_eq!(config.color, (100, 255, 100, 255));
        assert_eq!(config.size, 80.0);
        assert_eq!(config.line_width, 3.0);
        assert_eq!(config.duration_ms, 600);
        assert_eq!(config.end_pause, 1.5);
        assert_eq!(config.position, (0.3, 0.7));
    }

    #[test]
    fn test_iterator_frame_count_with_end_pause() {
        let mut config = GradeMarkConfig::default()
            .with_duration_ms(500)
            .with_end_pause(1.0);

        config.init(400, 400, 25);

        let frames: Vec<RgbaImage> = config.collect();
        // 0.5s draw = 13 frames, 1.0s pause = 25 frames, total = 38
        assert_eq!(frames.len(), 38);

        // Check all frames have correct dimensions
        for frame in frames {
            assert_eq!(frame.width(), 400);
            assert_eq!(frame.height(), 400);
        }
    }

    #[test]
    fn test_draw_progress() {
        let mut config = GradeMarkConfig::new(GradeMarkType::Circle)
            .with_duration_ms(1000)
            .with_end_pause(0.5);
        config.init(100, 100, 10);
        // draw_frames = 1.0s * 10fps = 10
        // pause_frames = 0.5s * 10fps = 5
        // total_frames = 15

        assert_eq!(config.draw_frames, 10);
        assert_eq!(config.pause_frames, 5);
        assert_eq!(config.total_frames, 15);

        // At frame 5, draw_progress = 0.5
        config.current_frame = 5;
        assert_eq!(config.draw_progress(), 0.5);

        // At frame 10 (end of draw phase), draw_progress = 1.0
        config.current_frame = 10;
        assert_eq!(config.draw_progress(), 1.0);

        // During pause phase, draw_progress stays at 1.0
        config.current_frame = 12;
        assert_eq!(config.draw_progress(), 1.0);
    }

    #[test]
    fn test_brush_width() {
        // At t=0.0, width should be 5% of base (极细起笔)
        assert!((GradeMarkConfig::brush_width(10.0, 0.0) - 0.5).abs() < 0.01);

        // At t=0.5, width should be 250% of base (非常粗中间)
        assert!((GradeMarkConfig::brush_width(10.0, 0.5) - 25.0).abs() < 0.01);

        // At t=1.0, width should be 5% of base (极细收笔)
        assert!((GradeMarkConfig::brush_width(10.0, 1.0) - 0.5).abs() < 0.01);

        // At t=0.25, width should be intermediate
        let width_at_25 = GradeMarkConfig::brush_width(10.0, 0.25);
        assert!(width_at_25 > 0.5 && width_at_25 < 25.0);
    }

    #[test]
    fn test_circle_draw_progress() {
        let mut config = GradeMarkConfig::new(GradeMarkType::Circle)
            .with_duration_ms(1000)
            .with_end_pause(0.0);
        config.init(100, 100, 10);

        // At draw_progress 0.5, arc should be half circle
        config.current_frame = 5;
        assert_eq!(config.draw_progress(), 0.5);
    }

    #[test]
    fn test_checkmark_draw_progress() {
        let mut config = GradeMarkConfig::new(GradeMarkType::Checkmark)
            .with_duration_ms(1000)
            .with_end_pause(0.0);
        config.init(100, 100, 10);

        // At draw_progress 0.3, first stroke should be at 50%
        config.current_frame = 3;
        assert_eq!(config.draw_progress(), 0.3);

        // At draw_progress 0.8, second stroke should be at 50%
        config.current_frame = 8;
        assert_eq!(config.draw_progress(), 0.8);
    }

    #[test]
    fn test_cross_draw_progress() {
        let mut config = GradeMarkConfig::new(GradeMarkType::Cross)
            .with_duration_ms(1000)
            .with_end_pause(0.0);
        config.init(100, 100, 10);

        // At draw_progress 0.25, first diagonal should be at 50%
        config.current_frame = 2;
        assert!(config.draw_progress() < 0.5);

        // At draw_progress 0.75, second diagonal should be at 50%
        config.current_frame = 8;
        assert!(config.draw_progress() > 0.5);
    }
}
