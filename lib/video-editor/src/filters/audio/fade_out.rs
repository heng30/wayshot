use crate::{
    Result,
    filters::{
        progress_ratio_from_offset,
        traits::{AudioData, AudioFilter},
    },
};
use std::time::Duration;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FadeOutFilter {
    pub duration: Duration,
}

impl Default for FadeOutFilter {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(1),
        }
    }
}

impl FadeOutFilter {
    pub const NAME: &'static str = "fade out";

    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }
}

impl AudioFilter for FadeOutFilter {
    crate::impl_default_audio_filter!(FadeOutFilter);

    fn apply(&self, data: &mut AudioData) -> Result<()> {
        if data.samples.is_empty() {
            return Ok(());
        }

        let sample_rate = data.config.sample_rate as f64;
        let channels = data.config.channels as f64;
        let segment_duration = data.from_segment.duration;
        let fade_out_start = segment_duration.saturating_sub(self.duration);

        // 当前批次开始的时间偏移
        let batch_start_offset = data.relative_timeline_offset;

        for (i, sample) in data.samples.iter_mut().enumerate() {
            // 计算当前采样在 segment 中的时间偏移（考虑声道）
            let sample_offset = batch_start_offset
                + Duration::from_secs_f64(i as f64 / (sample_rate * channels));

            // 如果采样不在淡出时间范围内，跳过
            if sample_offset < fade_out_start {
                continue;
            }

            // 计算淡出幅度：从 1.0 渐变到 0.0
            let time_in_fade = sample_offset.saturating_sub(fade_out_start);
            let ratio = 1.0 - progress_ratio_from_offset(time_in_fade, self.duration);

            *sample = (*sample * ratio).clamp(-1.0, 1.0);
        }

        Ok(())
    }
}
