//! # 静音滤镜 (Mute)
//!
//! ## 作用
//! 将音频的一个或多个声道静音。
//!
//! ## 工作原理
//! - 音频数据以交错格式存储：[L, R, L, R, L, R, ...]
//! - `Left`: 将左声道样本（索引 0, 2, 4, ...）设置为 0
//! - `Right`: 将右声道样本（索引 1, 3, 5, ...）设置为 0
//! - `Both`: 将所有样本设置为 0（完全静音）
//!
//! ## 使用场景
//! 1. **音频修复**：当某个声道有噪音或杂音时
//! 2. **创意效果**：仅播放特定声道的内容
//! 3. **声道隔离**：提取特定声道音频
//! 4. **平衡调整**：临时禁用某声道进行对比试听
//!
//! ## 注意事项
//! - `Left` 和 `Right` 模式仅适用于立体声音频（channels = 2）
//! - `Both` 模式适用于任意声道数的音频
//! - 静音后输出的音频声道数保持不变

use crate::{
    Result,
    filters::traits::{AudioData, AudioFilter},
};

/// 静音声道选择
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    num_enum::TryFromPrimitive,
    num_enum::IntoPrimitive,
)]
#[repr(u8)]
pub enum MuteChannel {
    /// 仅静音左声道
    #[default]
    Left = 0,
    /// 仅静音右声道
    Right = 1,
    /// 静音所有声道（完全静音）
    Both = 2,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MuteFilter {
    /// 静音的声道
    pub channel: MuteChannel,
}

impl MuteFilter {
    pub const NAME: &'static str = "mute";

    pub fn new(channel: MuteChannel) -> Self {
        Self { channel }
    }

    /// 创建静音左声道的滤镜
    pub fn left() -> Self {
        Self::new(MuteChannel::Left)
    }

    /// 创建静音右声道的滤镜
    pub fn right() -> Self {
        Self::new(MuteChannel::Right)
    }

    /// 创建完全静音的滤镜
    pub fn both() -> Self {
        Self::new(MuteChannel::Both)
    }
}

impl Default for MuteFilter {
    fn default() -> Self {
        Self {
            channel: MuteChannel::default(),
        }
    }
}

impl AudioFilter for MuteFilter {
    crate::impl_default_audio_filter!(MuteFilter);

    fn apply(&self, data: &mut AudioData) -> Result<()> {
        if data.samples.is_empty() {
            return Ok(());
        }

        match self.channel {
            MuteChannel::Left => {
                if data.config.channels != 2 {
                    return Err(crate::Error::InvalidConfig(
                        "MuteFilter with Left channel only supports stereo audio (2 channels)"
                            .to_string(),
                    ));
                }
                // 静音左声道（索引 0, 2, 4, ...）
                for i in (0..data.samples.len()).step_by(2) {
                    data.samples[i] = 0.0;
                }
            }
            MuteChannel::Right => {
                if data.config.channels != 2 {
                    return Err(crate::Error::InvalidConfig(
                        "MuteFilter with Right channel only supports stereo audio (2 channels)"
                            .to_string(),
                    ));
                }
                // 静音右声道（索引 1, 3, 5, ...）
                for i in (1..data.samples.len()).step_by(2) {
                    data.samples[i] = 0.0;
                }
            }
            MuteChannel::Both => {
                // 完全静音（适用于任意声道数）
                data.samples.fill(0.0);
            }
        }

        Ok(())
    }
}
