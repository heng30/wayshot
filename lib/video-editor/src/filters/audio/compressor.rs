//! # 压缩器滤镜 (Compressor)
//!
//! ## 作用
//! 压缩器用于减小音频的动态范围，即降低响亮部分和安静部分之间的音量差异。
//! 它可以让音频的整体音量更加平衡、一致，避免某些片段过响或过轻。
//!
//! ## 工作原理
//! - 当信号超过设定的阈值（threshold）时，压缩器会按照压缩比（ratio）降低信号电平
//! - 例如，4:1 的压缩比意味着超过阈值部分每增加 4dB，输出只增加 1dB
//! - 这种处理可以让响亮的声音变小，但不会改变安静的声音
//!
//! ## 使用场景
//! 1. **人声处理**：让说话声更加平稳，避免某些词语过响
//! 2. **音乐制作**：平衡各乐器间的音量差异
//! 3. **播客/配音**：确保整体音量一致，提升听感
//! 4. **现场录音**：防止突发的响亮声音导致失真
//!
//! ## 参数说明
//! - **threshold** (阈值): 触发压缩的电平门限，单位 dB
//!   - 范围：-60dB 到 0dB
//!   - 常用值：-20dB（轻度压缩）到 -10dB（重度压缩）
//!   - 值越低，被压缩的音频越多
//!
//! - **ratio** (压缩比): 超过阈值后的信号压缩比例
//!   - 脉围：1.0（无压缩）到 20.0（重度压缩）
//!   - 常用值：2:1（轻度）到 8:1（重度）
//!   - 示例：4.0 表示 4:1 压缩比
//!
//! - **attack** (启动时间): 信号超过阈值后，压缩器开始生效的速度
//!   - 单位：毫秒 (ms)
//!   - 脉围：0.1ms 到 100ms
//!   - 常用值：5ms - 20ms
//!   - 较快的启动（<5ms）：适合打击乐，能快速捕捉瞬态
//!   - 较慢的启动（>10ms）：保留更多原始冲击力
//!
//! - **release** (释放时间): 信号低于阈值后，压缩器恢复的速度
//!   - 单位：毫秒 (ms)
//!   - 脉围：10ms 到 1000ms
//!   - 常用值：50ms - 250ms
//!   - 太快会导致"抽吸"效应（音量忽大忽小）
//!   - 太慢会导致压缩器无法及时恢复
//!
//! - **makeup_gain** (补偿增益): 压缩后增加的音量，用于补偿压缩造成的整体音量下降
//!   - 单位：dB
//!   - 脉围：0dB 到 20dB
//!   - 常用值：3dB - 10dB
//!   - 压缩后会降低整体音量，需要补偿以维持响度
//!
//! ## 典型参数预设
//!
//! ### 人声（轻度压缩）
//! ```rust
//! CompressorFilter::new(-20.0, 2.0, 10.0, 100.0, 3.0)
//! ```
//! - threshold: -20dB
//! - ratio: 2:1
//! - attack: 10ms
//! - release: 100ms
//! - makeup_gain: 3dB
//!
//! ### 人声（中度压缩）
//! ```rust
//! CompressorFilter::new(-16.0, 4.0, 5.0, 80.0, 6.0)
//! ```
//! - threshold: -16dB
//! - ratio: 4:1
//! - attack: 5ms
//! - release: 80ms
//! - makeup_gain: 6dB
//!
//! ### 播客/配音
//! ```rust
//! CompressorFilter::new(-18.0, 3.0, 8.0, 150.0, 5.0)
//! ```
//! - threshold: -18dB
//! - ratio: 3:1
//! - attack: 8ms
//! - release: 150ms
//! - makeup_gain: 5dB
//!
//! ### 激烈压缩（广播风格）
//! ```rust
//! CompressorFilter::new(-12.0, 8.0, 3.0, 50.0, 10.0)
//! ```
//! - threshold: -12dB
//! - ratio: 8:1
//! - attack: 3ms
//! - release: 50ms
//! - makeup_gain: 10dB
//!
//! ## 使用技巧
//! 1. **先调阈值**：从 -20dB 开始，逐渐降低直到听到明显的压缩效果
//! 2. **再调压缩比**：根据需要的压缩程度调整
//! 3. **最后调补偿增益**：让压缩后的音量与原始音量相当
//! 4. **注意过度压缩**：过度压缩会让音频失去动态感，听起来"扁平"

use crate::{
    Result,
    filters::{
        interpolation::get_float_at_time,
        keyframe::{AnimatableProperty, KeyframeTracks, KeyframeValue},
        traits::{AudioData, AudioFilter},
    },
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompressorFilter {
    /// 触发压缩的阈值 (dB)，范围: -60 到 0
    pub threshold: f32,

    /// 压缩比，例如 4.0 表示 4:1 的压缩比
    pub ratio: f32,

    /// 启动时间 (毫秒)，信号超过阈值后压缩器开始生效的速度
    pub attack: f32,

    /// 释放时间 (毫秒)，信号低于阈值后压缩器恢复的速度
    pub release: f32,

    /// 补偿增益 (dB)，用于补偿压缩造成的整体音量下降
    pub makeup_gain: f32,

    #[serde(skip_serializing_if = "KeyframeTracks::is_empty", default)]
    pub keyframe_tracks: KeyframeTracks,
}

impl Default for CompressorFilter {
    fn default() -> Self {
        Self {
            threshold: -20.0,
            ratio: 4.0,
            attack: 10.0,
            release: 100.0,
            makeup_gain: 3.0,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }
}

impl CompressorFilter {
    pub const NAME: &'static str = "compressor";

    pub fn new(threshold: f32, ratio: f32, attack: f32, release: f32, makeup_gain: f32) -> Self {
        Self {
            threshold,
            ratio,
            attack,
            release,
            makeup_gain,
            keyframe_tracks: KeyframeTracks::default(),
        }
    }

    /// Get the animatable properties for this filter
    pub fn animatable_properties() -> Vec<AnimatableProperty> {
        vec![AnimatableProperty::float(
            "makeup_gain",
            "Makeup Gain",
            0.0,
            20.0,
            3.0,
        )]
    }

    /// Get interpolated makeup_gain at a specific time
    fn get_makeup_gain_at_time(&self, time_ms: i64) -> f32 {
        self.keyframe_tracks
            .get_track("makeup_gain")
            .map(|track| get_float_at_time(track, time_ms, self.makeup_gain))
            .unwrap_or(self.makeup_gain)
    }
}

impl AudioFilter for CompressorFilter {
    crate::impl_default_audio_filter!(CompressorFilter);

    fn apply(&self, data: &mut AudioData) -> Result<()> {
        if data.samples.is_empty() {
            return Ok(());
        }

        let sample_rate = data.config.sample_rate;
        let channels = data.config.channels as usize;
        let start_time_ms = data.relative_timeline_offset.as_millis() as i64;

        // 计算每个样本对应的时间（毫秒）
        let samples_per_ms = (sample_rate as f64 * channels as f64 / 1000.0) as f64;

        let threshold_linear = 10f32.powf(self.threshold / 20.0);
        let slope = 1.0 - (1.0 / self.ratio);
        let attack_coeff = (-1.0 / (self.attack / 1000.0 * sample_rate as f32)).exp();
        let release_coeff = (-1.0 / (self.release / 1000.0 * sample_rate as f32)).exp();

        let mut envelope = 0.0f32;

        for (i, sample) in data.samples.iter_mut().enumerate() {
            // 计算当前样本相对于 segment 开始的时间
            let sample_time_ms = start_time_ms + (i as f64 / samples_per_ms) as i64;
            let makeup_gain = self.get_makeup_gain_at_time(sample_time_ms);
            let makeup_linear = 10f32.powf(makeup_gain / 20.0);

            let input_level = sample.abs();

            // Update envelope
            let target = if input_level > envelope {
                input_level
            } else {
                envelope
            };
            let coeff = if input_level > envelope {
                attack_coeff
            } else {
                release_coeff
            };
            envelope = coeff * envelope + (1.0 - coeff) * target;

            // Apply compression
            let gain = if envelope > threshold_linear {
                10f32.powf((slope * (20.0 * envelope.log10() - self.threshold)) / 20.0)
            } else {
                1.0
            };

            *sample = (*sample * gain * makeup_linear).clamp(-1.0, 1.0);
        }

        Ok(())
    }

    fn get_animatable_properties(&self) -> Vec<AnimatableProperty> {
        Self::animatable_properties()
    }

    fn get_keyframe_tracks(&self) -> KeyframeTracks {
        self.keyframe_tracks.clone()
    }

    fn set_keyframe_tracks(&mut self, tracks: KeyframeTracks) {
        self.keyframe_tracks = tracks;
    }

    fn supports_keyframes(&self) -> bool {
        true
    }

    fn update_keyframes_at_time(&self, tracks: &mut KeyframeTracks, time_ms: i64) -> bool {
        if let Some(track) = tracks.get_track("makeup_gain")
            && track.keyframes.iter().any(|k| k.time_ms == time_ms)
        {
            tracks.update_keyframe_value(
                "makeup_gain",
                time_ms,
                KeyframeValue::Float(self.makeup_gain),
            );
            return true;
        }
        false
    }
}

