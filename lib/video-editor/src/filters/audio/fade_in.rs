use crate::{
    Result,
    filters::{
        progress_ratio_from_offset,
        traits::{AudioData, AudioFilter},
    },
};
use std::time::Duration;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FadeInFilter {
    pub duration: Duration,
}

impl Default for FadeInFilter {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(1),
        }
    }
}

impl FadeInFilter {
    pub const NAME: &'static str = "fade in";

    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }
}

impl AudioFilter for FadeInFilter {
    crate::impl_default_audio_filter!(FadeInFilter);

    fn apply(&self, data: &mut AudioData) -> Result<()> {
        if data.samples.is_empty() {
            return Ok(());
        }

        let sample_rate = data.config.sample_rate as f64;
        let channels = data.config.channels as f64;
        let batch_start_offset = data.relative_timeline_offset;

        // 如果批次开始时间已经超过淡入时长，不应用淡入
        if batch_start_offset >= self.duration {
            return Ok(());
        }

        for (i, sample) in data.samples.iter_mut().enumerate() {
            // 计算当前采样在 segment 中的时间偏移（考虑声道）
            let sample_offset =
                batch_start_offset + Duration::from_secs_f64(i as f64 / (sample_rate * channels));

            // 如果采样时间已超过淡入时长，不修改
            if sample_offset >= self.duration {
                continue;
            }

            // 计算淡入幅度：从 0.0 渐变到 1.0
            let ratio = progress_ratio_from_offset(sample_offset, self.duration);
            *sample = (*sample * ratio).clamp(-1.0, 1.0);
        }

        Ok(())
    }
}
