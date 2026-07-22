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
pub struct FadeInFilter {
    #[derivative(Default(value = "Duration::from_secs(1)"))]
    pub duration: Duration,
}

impl FadeInFilter {
    pub const NAME: &'static str = "fade in";

    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }
}

impl VideoFilter for FadeInFilter {
    crate::impl_default_video_filter!(FadeInFilter);

    fn apply(&self, data: &mut VideoData) -> Result<()> {
        let frame_time_offset = data.relative_timeline_offset;
        for frame in data.frames.iter_mut() {
            if let VideoImage::Image { buffer, .. } = frame {
                let ratio = progress_ratio_from_offset(frame_time_offset, self.duration);

                buffer.par_pixels_mut().for_each(|pixel| {
                    // 淡入：alpha 从 0 渐变到原始值
                    pixel.0[3] = ((pixel.0[3] as f32) * ratio).clamp(0.0, 255.0) as u8;
                });
            }
        }

        Ok(())
    }
}
