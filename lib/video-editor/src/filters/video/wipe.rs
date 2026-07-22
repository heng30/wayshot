use crate::{
    Result,
    filters::{
        progress_ratio_from_offset,
        traits::{VideoData, VideoFilter},
    },
    tracks::video_frame_cache::VideoImage,
};
use rayon::prelude::*;
use std::time::Duration;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    num_enum::TryFromPrimitive,
    num_enum::IntoPrimitive,
)]
#[repr(u8)]
pub enum WipeDirection {
    LeftToRight = 0, // 从左向右擦除
    RightToLeft,     // 从右向左擦除
    TopToBottom,     // 从上向下擦除
    BottomToTop,     // 从下向上擦除
}

impl Default for WipeDirection {
    fn default() -> Self {
        Self::LeftToRight
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WipeFilter {
    pub direction: WipeDirection,
    pub duration: Duration,
}

impl Default for WipeFilter {
    fn default() -> Self {
        Self {
            direction: WipeDirection::LeftToRight,
            duration: Duration::from_secs(1),
        }
    }
}

impl WipeFilter {
    pub const NAME: &'static str = "wipe";

    pub fn new(direction: WipeDirection, duration: Duration) -> Self {
        Self {
            direction,
            duration,
        }
    }
}

impl VideoFilter for WipeFilter {
    crate::impl_default_video_filter!(WipeFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let frame_time_offset = data.relative_timeline_offset;
        let segment_duration = data.from_segment.duration;

        // Wipe 作用于 segment 结尾，需要计算距离结束的时间
        let time_until_end = segment_duration.saturating_sub(frame_time_offset);

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                // 如果不在效果时间范围内，跳过（不修改）
                if time_until_end > self.duration {
                    continue;
                }

                // ratio 从 1.0（效果开始，图像可见）到 0.0（效果结束，图像隐藏）
                let ratio = progress_ratio_from_offset(time_until_end, self.duration);

                // 当接近 segment 结尾时，强制完全隐藏，避免最后几帧的不完全擦除
                let is_last_frame = time_until_end < Duration::from_millis(100);

                let width = buffer.width();
                let height = buffer.height();

                // ratio=1: 图像完全可见，ratio=0: 图像完全隐藏
                // boundary 使用 (1.0 - ratio) 计算：ratio=1 -> boundary=0, ratio=0 -> boundary=width/height
                match self.direction {
                    WipeDirection::LeftToRight => {
                        // 从左向右擦除：隐藏区域从左边开始扩展
                        // ratio=1: boundary=0 (不隐藏)
                        // ratio=0: boundary=width (全部隐藏)
                        let boundary = if is_last_frame {
                            width
                        } else {
                            ((width as f32 * (1.0 - ratio) + 1.0).ceil() as u32).min(width)
                        };
                        buffer
                            .par_enumerate_pixels_mut()
                            .for_each(|(x, _y, pixel)| {
                                if x < boundary {
                                    pixel.0 = [0, 0, 0, pixel.0[3]];
                                }
                            });
                    }
                    WipeDirection::RightToLeft => {
                        // 从右向左擦除：隐藏区域从右边开始扩展
                        // ratio=1: hide_start=width (不隐藏)
                        // ratio=0: hide_start=0 (全部隐藏)
                        let hide_start = if is_last_frame {
                            0
                        } else {
                            width.saturating_sub(
                                ((width as f32 * (1.0 - ratio) + 1.0).ceil() as u32).min(width),
                            )
                        };
                        buffer
                            .par_enumerate_pixels_mut()
                            .for_each(|(x, _y, pixel)| {
                                if x >= hide_start {
                                    pixel.0 = [0, 0, 0, pixel.0[3]];
                                }
                            });
                    }
                    WipeDirection::TopToBottom => {
                        // 从上向下擦除：隐藏区域从顶部开始扩展
                        // ratio=1: boundary=0 (不隐藏)
                        // ratio=0: boundary=height (全部隐藏)
                        let boundary = if is_last_frame {
                            height
                        } else {
                            ((height as f32 * (1.0 - ratio) + 1.0).ceil() as u32).min(height)
                        };
                        buffer
                            .par_enumerate_pixels_mut()
                            .for_each(|(_x, y, pixel)| {
                                if y < boundary {
                                    pixel.0 = [0, 0, 0, pixel.0[3]];
                                }
                            });
                    }
                    WipeDirection::BottomToTop => {
                        // 从下向上擦除：隐藏区域从底部开始扩展
                        // ratio=1: hide_start=height (不隐藏)
                        // ratio=0: hide_start=0 (全部隐藏)
                        let hide_start = if is_last_frame {
                            0
                        } else {
                            height.saturating_sub(
                                ((height as f32 * (1.0 - ratio) + 1.0).ceil() as u32).min(height),
                            )
                        };
                        buffer
                            .par_enumerate_pixels_mut()
                            .for_each(|(_x, y, pixel)| {
                                if y >= hide_start {
                                    pixel.0 = [0, 0, 0, pixel.0[3]];
                                }
                            });
                    }
                }
            }
        }

        Ok(())
    }
}
