//! # 限幅器滤镜 (Limiter)
//!
//! ## 作用
//! 限幅器是一种特殊类型的压缩器，用于防止音频信号超过设定的最大电平。
//! 它是保护音频免受削波失真的最后一道防线，可以看作是压缩比为∞:1的压缩器。
//!
//! ## 工作原理
//! - 当信号超过阈值时，限幅器会"硬性"地将信号限制在阈值处
//! - 与压缩器不同，限幅器使用硬削波，超过阈值的信号直接被切断
//! - 这种处理可以防止数字音频削波（clipping），避免产生难听的失真
//!
//! ## 与压缩器的区别
//! - **压缩器**：平滑地降低超过阈值的信号，保留一些动态范围
//! - **限幅器**：硬性切断超过阈值的信号，作为安全保护措施
//! - 压缩器用于艺术处理，限幅器用于技术保护
//!
//! ## 使用场景
//! 1. **母带处理**：作为最后的处理环节，确保整首曲目不超过 0dB
//! 2. **广播/电视**：满足响度标准（如 EBU R128），防止超出法规限制
//! 3. **现场音响**：保护功率放大器和扬声器免受突发大音量损坏
//! 4. **录音安全**：防止录音电平过高导致数字削波
//! 5. **视频制作**：确保音频轨道符合平台标准（如 YouTube、Netflix）
//!
//! ## 参数说明
//! - **threshold** (阈值): 限制的最大电平，单位 dB
//!   - 范围：-60dB 到 0dB
//!   - 常用值：-0.1dB 到 -3.0dB
//!   - 设置为 -0.1dB 或 -0.3dB：提供"净空"（headroom），防止数字削波
//!   - 设置为 -3.0dB 或更低：显著限制动态范围，用于特殊效果
//!
//! ## 典型参数预设
//!
//! ### 母带处理（标准保护）
//! ```rust
//! LimiterFilter::new(-0.3)
//! ```
//! - threshold: -0.3dB
//! - 用途：提供轻微的保护净空，几乎不影响音质
//!
//! ### 广播标准
//! ```rust
//! LimiterFilter::new(-1.0)
//! ```
//! - threshold: -1.0dB
//! - 用途：满足广播响度标准
//!
//! ### 保守限制
//! ```rust
//! LimiterFilter::new(-3.0)
//! ```
//! - threshold: -3.0dB
//! - 用途：确保远低于削波电平，安全但可能损失一些动态
//!
//! ### 严格限制（用于网络平台）
//! ```rust
//! LimiterFilter::new(-6.0)
//! ```
//! - threshold: -6.0dB
//! - 用途：满足某些在线视频平台的严格响度要求
//!
//! ## 使用技巧
//!
//! 1. **作为最后的处理步骤**
//!    - 限幅器应该是效果链中的最后一个处理器
//!    - 顺序：均衡器 → 压缩器 → 其他效果 → 限幅器
//!
//! 2. **使用合理的阈值**
//!    - 母带处理：-0.1dB 到 -0.5dB（保留少量净空）
//!    - 不要设置为 0dB：可能会导致 intersample 削波
//!
//! 3. **与压缩器配合使用**
//!    - 先用压缩器控制动态范围
//!    - 再用限幅器作为最后的"保险"
//!    - 这样可以减少限幅器的工作强度，保持更好的音质
//!
//! 4. **避免过度限制**
//!    - 过低的阈值（如 -10dB）会严重压缩动态
//!    - 会导致音频听起来"疲劳"和"扁平"
//!    - 仅用于满足技术要求，不应作为音量提升的主要手段
//!
//! 5. **监控电平**
//!    - 使用峰值表监控输出电平
//!    - 确保没有触发削波（clipping）
//!    - 理想情况下，峰值应略微低于 0dB
//!
//! ## 常见问题
//!
//! ### Q: 限幅器和压缩器应该同时使用吗？
//! A: 是的，这是常见做法。压缩器用于控制动态范围，限幅器作为最后的安全网。
//!
//! ### Q: 限幅器会损害音质吗？
//! A: 轻微的限制（-0.3dB）几乎听不出影响，但过度限制会造成动态损失和失真。
//!
//! ### Q: 阈值设置为多少合适？
//! A: 对于母带处理，-0.1dB 到 -0.5dB 是标准做法。对于广播，参考具体标准（如 EBU R128）。
//!
//! ### Q: 为什么不设置为 0dB？
//! A: 0dB 可能导致 intersample 削波（峰值采样点之间的失真），建议留 -0.1dB 到 -0.5dB 的净空。

use crate::{
    Result,
    filters::traits::{AudioData, AudioFilter},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LimiterFilter {
    /// 限制阈值 (dB)，范围: -60 到 0
    /// 超过此电平的信号将被限制，防止削波失真
    /// 常用值: -0.3 (母带处理), -1.0 (广播), -3.0 (保守限制)
    pub threshold: f32,
}

impl Default for LimiterFilter {
    fn default() -> Self {
        Self { threshold: -0.3 }
    }
}

impl LimiterFilter {
    pub const NAME: &'static str = "limiter";

    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }
}

impl AudioFilter for LimiterFilter {
    crate::impl_default_audio_filter!(LimiterFilter);

    fn apply(&self, data: &mut AudioData) -> Result<()> {
        if data.samples.is_empty() {
            return Ok(());
        }

        let threshold_linear = 10f32.powf(self.threshold / 20.0);

        for sample in &mut data.samples {
            let abs_sample = sample.abs();
            if abs_sample > threshold_linear {
                *sample = sample.signum() * threshold_linear;
            }
        }

        Ok(())
    }
}
