use std::{fmt, ops::Range, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FramePosition {
    pub frame_index: usize, // 源视频中的帧索引（从 0 开始）
    fps_numerator: u32,     // 帧率分子（例如 24000）
    fps_denominator: u32,   // 帧率分母（例如 1001，表示 23.976 FPS）
}

impl FramePosition {
    pub fn new(frame_index: usize, fps_numerator: u32, fps_denominator: u32) -> Self {
        assert!(fps_numerator > 0, "FPS numerator must be positive");
        assert!(fps_denominator > 0, "FPS denominator must be positive");

        Self {
            frame_index,
            fps_numerator,
            fps_denominator,
        }
    }

    pub fn from_f32_fps(frame_index: usize, fps: f32) -> Self {
        let (num, den) = Self::fps_to_rational(fps);
        Self::new(frame_index, num, den)
    }

    pub fn fps_to_rational(fps: f32) -> (u32, u32) {
        const EPSILON: f32 = 0.001;

        if (fps - 23.976).abs() < EPSILON {
            (24000, 1001)
        } else if (fps - 29.97).abs() < EPSILON {
            (30000, 1001)
        } else if (fps - 59.94).abs() < EPSILON {
            (60000, 1001)
        } else if (fps - 24.0).abs() < EPSILON {
            (24, 1)
        } else if (fps - 25.0).abs() < EPSILON {
            (25, 1)
        } else if (fps - 30.0).abs() < EPSILON {
            (30, 1)
        } else if (fps - 50.0).abs() < EPSILON {
            (50, 1)
        } else if (fps - 60.0).abs() < EPSILON {
            (60, 1)
        } else if (fps - 120.0).abs() < EPSILON {
            (120, 1)
        } else {
            ((fps * 1000.0).round() as u32, 1000)
        }
    }

    pub fn fps_as_f32(&self) -> f32 {
        self.fps_numerator as f32 / self.fps_denominator as f32
    }

    // duration = frame_index * (1 / fps) = frame_index * fps_denominator / fps_numerator
    pub fn duration_from_start(&self) -> Duration {
        let nanos = (self.frame_index as u128 * 1_000_000_000 * self.fps_denominator as u128)
            / self.fps_numerator as u128;
        Duration::from_nanos(nanos as u64)
    }

    pub fn frame_duration(&self) -> Duration {
        Duration::from_nanos(
            (1_000_000_000 * self.fps_denominator as u64) / self.fps_numerator as u64,
        )
    }

    pub fn frame_index(&self) -> usize {
        self.frame_index
    }

    pub fn add_frames(&self, count: usize) -> Self {
        Self {
            frame_index: self.frame_index + count,
            fps_numerator: self.fps_numerator,
            fps_denominator: self.fps_denominator,
        }
    }

    pub fn sub_frames(&self, count: usize) -> Option<Self> {
        self.frame_index.checked_sub(count).map(|idx| Self {
            frame_index: idx,
            fps_numerator: self.fps_numerator,
            fps_denominator: self.fps_denominator,
        })
    }
}

impl fmt::Display for FramePosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Frame #{} ({:.3}s @ {:.2} FPS)",
            self.frame_index,
            self.duration_from_start().as_secs_f64(),
            self.fps_as_f32()
        )
    }
}

// 精度安全的帧范围
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameRange {
    pub start: FramePosition,
    pub end: FramePosition, // 不包含
}

impl FrameRange {
    pub fn new(start: FramePosition, end: FramePosition) -> Self {
        assert!(
            start.frame_index <= end.frame_index,
            "FrameRange start must be <= end"
        );
        assert_eq!(
            start.fps_numerator, end.fps_numerator,
            "FrameRange must use same FPS"
        );
        assert_eq!(
            start.fps_denominator, end.fps_denominator,
            "FrameRange must use same FPS"
        );

        Self { start, end }
    }

    pub fn from_start_count(start: FramePosition, frame_count: usize) -> Self {
        Self {
            start,
            end: start.add_frames(frame_count),
        }
    }

    pub fn frame_count(&self) -> usize {
        self.end.frame_index - self.start.frame_index
    }

    pub fn duration(&self) -> Duration {
        self.end.duration_from_start() - self.start.duration_from_start()
    }

    pub fn overlaps(&self, other: &FrameRange) -> bool {
        self.start.frame_index < other.end.frame_index
            && other.start.frame_index < self.end.frame_index
    }

    // 迭代此范围内的帧索引
    pub fn iter_indices(&self) -> Range<usize> {
        self.start.frame_index..self.end.frame_index
    }
}

// 精度安全的时间到帧转换器
#[derive(Debug, Clone)]
pub struct TimeToFrameConverter {
    fps_numerator: u32,
    fps_denominator: u32,
}

impl TimeToFrameConverter {
    pub fn new(fps_numerator: u32, fps_denominator: u32) -> Self {
        assert!(fps_numerator > 0, "FPS numerator must be positive");
        assert!(fps_denominator > 0, "FPS denominator must be positive");

        Self {
            fps_numerator,
            fps_denominator,
        }
    }

    // 从 f32 FPS 创建转换器（向后兼容）
    pub fn from_f32(fps: f32) -> Self {
        let (num, den) = FramePosition::fps_to_rational(fps);
        Self::new(num, den)
    }

    // 将 Duration 转换为帧索引（四舍五入到最近的帧）
    //
    // 使用整数运算，避免浮点精度误差：
    // ```text
    // frame = duration * fps
    //       = duration_nanos * fps_numerator / (1_000_000_000 * fps_denominator)
    // ```
    pub fn duration_to_frame(&self, duration: Duration) -> usize {
        let nanos = duration.as_nanos();
        let frame_num = nanos * self.fps_numerator as u128;
        let frame_den = 1_000_000_000u128 * self.fps_denominator as u128;
        ((frame_num + frame_den / 2) / frame_den) as usize
    }

    // 将帧索引转换为 Duration
    //
    // 使用整数运算：
    // ```text
    // duration = frame_index * (1 / fps)
    //         = frame_index * fps_denominator / fps_numerator
    // ```
    pub fn frame_to_duration(&self, frame_index: usize) -> Duration {
        let nanos = (frame_index as u128 * 1_000_000_000 * self.fps_denominator as u128)
            / self.fps_numerator as u128;
        Duration::from_nanos(nanos as u64)
    }

    pub fn frame_position(&self, frame_index: usize) -> FramePosition {
        FramePosition::new(frame_index, self.fps_numerator, self.fps_denominator)
    }

    pub fn fps_as_f32(&self) -> f32 {
        self.fps_numerator as f32 / self.fps_denominator as f32
    }
}
