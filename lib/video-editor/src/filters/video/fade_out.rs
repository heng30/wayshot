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
    derivative::Derivative,
    derive_setters::Setters,
    serde::Serialize,
    serde::Deserialize,
)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct FadeOutFilter {
    #[derivative(Default(value = "Duration::from_secs(1)"))]
    pub duration: Duration,
}

impl FadeOutFilter {
    pub const NAME: &'static str = "fade out";

    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }
}

impl VideoFilter for FadeOutFilter {
    crate::impl_default_video_filter!(FadeOutFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let frame_time_offset = data.relative_timeline_offset;
        let segment_duration = data.from_segment.duration;

        // 计算距离 segment 结束的剩余时间
        let time_until_end = segment_duration.saturating_sub(frame_time_offset);

        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                // 如果不在淡出时间范围内，跳过（不修改）
                if time_until_end > self.duration {
                    continue;
                }

                // ratio 从 1.0（淡出开始）到 0.0（淡出结束）
                let ratio = progress_ratio_from_offset(time_until_end, self.duration);

                buffer.par_pixels_mut().for_each(|pixel| {
                    // 淡出：alpha 从原始值渐变到 0
                    pixel.0[3] = ((pixel.0[3] as f32) * ratio).clamp(0.0, 255.0) as u8;
                });
            }
        }

        Ok(())
    }
}
