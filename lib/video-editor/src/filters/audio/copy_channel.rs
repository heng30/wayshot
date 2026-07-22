//! # 复制声道滤镜 (Copy Channel)
//!
//! ## 作用
//! 将一个声道的内容复制到另一个声道，使两个声道输出相同的内容。
//!
//! ## 工作原理
//! - 音频数据以交错格式存储：[L, R, L, R, L, R, ...]
//! - `LeftToRight`: 将左声道样本（索引 0, 2, 4, ...）复制到右声道（索引 1, 3, 5, ...）
//! - `RightToLeft`: 将右声道样本（索引 1, 3, 5, ...）复制到左声道（索引 0, 2, 4, ...）
//!
//! ## 使用场景
//! 1. **音频修复**：当一个声道损坏或缺失时，用另一个声道替代
//! 2. **单声道效果**：将立体声转为双声道单声道
//! 3. **声道同步**：确保两个声道播放相同内容
//!
//! ## 注意事项
//! - 仅适用于立体声音频（channels = 2）
//! - 复制后输出的仍然是双声道音频

use crate::{
    Result,
    filters::traits::{AudioData, AudioFilter},
};

/// 复制方向
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
pub enum CopyDirection {
    /// 左声道 → 右声道
    #[default]
    LeftToRight = 0,
    /// 右声道 → 左声道
    RightToLeft = 1,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CopyChannelFilter {
    /// 复制方向
    pub direction: CopyDirection,
}

impl CopyChannelFilter {
    pub const NAME: &'static str = "copy channel";

    pub fn new(direction: CopyDirection) -> Self {
        Self { direction }
    }

    /// 创建左声道复制到右声道的滤镜
    pub fn left_to_right() -> Self {
        Self::new(CopyDirection::LeftToRight)
    }

    /// 创建右声道复制到左声道的滤镜
    pub fn right_to_left() -> Self {
        Self::new(CopyDirection::RightToLeft)
    }
}

impl Default for CopyChannelFilter {
    fn default() -> Self {
        Self::left_to_right()
    }
}

impl AudioFilter for CopyChannelFilter {
    crate::impl_default_audio_filter!(CopyChannelFilter);

    fn apply(&self, data: &mut AudioData) -> Result<()> {
        if data.samples.is_empty() {
            return Ok(());
        }

        if data.config.channels != 2 {
            return Err(crate::Error::InvalidConfig(
                "CopyChannelFilter only supports stereo audio (2 channels)".to_string(),
            ));
        }

        match self.direction {
            CopyDirection::LeftToRight => {
                // 将左声道（索引 0, 2, 4, ...）复制到右声道（索引 1, 3, 5, ...）
                for i in (0..data.samples.len()).step_by(2) {
                    if i + 1 < data.samples.len() {
                        data.samples[i + 1] = data.samples[i];
                    }
                }
            }
            CopyDirection::RightToLeft => {
                // 将右声道（索引 1, 3, 5, ...）复制到左声道（索引 0, 2, 4, ...）
                for i in (0..data.samples.len()).step_by(2) {
                    if i + 1 < data.samples.len() {
                        data.samples[i] = data.samples[i + 1];
                    }
                }
            }
        }

        Ok(())
    }
}
