//! # 噪声门滤镜 (Noise Gate)
//!
//! ## 作用
//! 噪声门用于自动静音或降低低于设定阈值的音频信号，主要用于消除背景噪声。
//! 当信号强度超过阈值时，门"打开"（通过信号）；当信号低于阈值时，门"关闭"（静音或降低）。
//!
//! ## 工作原理
//! - **门打开**：当信号电平超过阈值时，噪声门让音频以原始音量通过
//! - **门关闭**：当信号电平低于阈值时，噪声门降低或完全静音音频
//! - 启动（attack）和释放（release）时间控制门的开关速度，避免突兀的音量变化
//!
//! ## 使用场景
//! 1. **麦克风录音**：消除麦克风放大器的本底噪声（hiss）和房间环境噪声
//! 2. **乐器录音**：在乐器不演奏时静音，防止背景噪声混入
//! 3. **播客/配音**：在说话停顿时自动降低环境噪声
//! 4. **电吉他**：消除高增益放大器的本底噪声
//! 5. **鼓组**：减少串音（crosstalk），例如军鼓麦克风拾取到的底鼓声音
//!
//! ## 与压缩器的区别
//! - **压缩器**：降低响亮部分，使整体音量更平衡
//! - **噪声门**：静音或降低安静部分（主要是噪声）
//! - 压缩器处理"太响"的问题，噪声门处理"太静"（但有噪声）的问题
//!
//! ## 参数说明
//! - **threshold** (阈值): 触发门打开/关闭的电平门限，单位 dB
//!   - 范围：-60dB 到 0dB
//!   - 常用值：-40dB（安静环境）到 -20dB（嘈杂环境）
//!   - 设置技巧：设定在环境噪声电平之上，但在最弱的有效信号之下
//!   - 例如：如果环境噪声是 -50dB，最弱的有效信号是 -30dB，阈值可设为 -35dB
//!
//! - **ratio** (衰减比): 门关闭时降低信号的倍数
//!   - 范围：1.0（无衰减）到 100.0（几乎完全静音）
//!   - 常用值：10（-20dB 衰减）到 100（-40dB 衰减）
//!   - ratio = 1：无效果
//!   - ratio = 10：信号降低 20dB（log₂10 ≈ 20）
//!   - ratio = 100：信号降低 40dB（几乎完全静音）
//!
//! - **attack** (启动时间): 门打开的速度，单位毫秒
//!   - 范围：0.1ms 到 100ms
//!   - 常用值：0.5ms - 10ms
//!   - 较快的启动（< 1ms）：适合打击乐，保留瞬态冲击
//!   - 较慢的启动（> 5ms）：避免"咔哒"声，但可能截断音频起始
//!
//! - **hold** (保持时间): 门保持打开状态的时间，单位毫秒
//!   - 范围：0ms 到 1000ms
//!   - 常用值：50ms - 200ms
//!   - 防止信号在阈值附近波动时门快速开关
//!   - 较长的保持时间（> 100ms）：适合人声，避免在停顿期间误关门
//!   - 较短的保持时间（< 50ms）：适合快速变化的音频
//!
//! - **release** (释放时间): 门关闭的速度，单位毫秒
//!   - 范围：10ms 到 1000ms
//!   - 常用值：100ms - 500ms
//!   - 较快的释放（< 100ms）：门快速关闭，可能听到"抽吸"效应
//!   - 较慢的释放（> 200ms）：更自然的衰减，但可能在间隙中听到噪声
//!
//! ## 典型参数预设
//!
//! ### 人声/播客（轻度降噪）
//! ```rust
//! NoiseGateFilter::new(-35.0, 20.0, 5.0, 100.0, 200.0)
//! ```
//! - threshold: -35dB
//! - ratio: 20:1（-26dB 衰减）
//! - attack: 5ms
//! - hold: 100ms
//! - release: 200ms
//!
//! ### 麦克风录音（中度降噪）
//! ```rust
//! NoiseGateFilter::new(-40.0, 50.0, 1.0, 50.0, 150.0)
//! ```
//! - threshold: -40dB
//! - ratio: 50:1（-34dB 衰减）
//! - attack: 1ms
//! - hold: 50ms
//! - release: 150ms
//!
//! ### 电吉他（重度降噪）
//! ```rust
//! NoiseGateFilter::new(-30.0, 100.0, 0.5, 30.0, 100.0)
//! ```
//! - threshold: -30dB
//! - ratio: 100:1（-40dB 衰减，几乎完全静音）
//! - attack: 0.5ms
//! - hold: 30ms
//! - release: 100ms
//!
//! ### 鼓组（减少串音）
//! ```rust
//! NoiseGateFilter::new(-25.0, 30.0, 0.1, 20.0, 50.0)
//! ```
//! - threshold: -25dB
//! - ratio: 30:1（-29dB 衰减）
//! - attack: 0.1ms（极快，保留瞬态）
//! - hold: 20ms
//! - release: 50ms
//!
//! ## 使用技巧
//!
//! 1. **找到合适的阈值**
//!    - 播放音频，观察最安静的有效信号电平
//!    - 将阈值设定在环境噪声和最弱信号之间
//!    - 从低阈值开始，逐渐提高直到噪声消失但有效信号仍能触发
//!
//! 2. **调整启动和释放时间**
//!    - 如果听到"咔哒"声或"抽吸"效应：增加启动和释放时间
//!    - 如果音频起始被截断：减少启动时间
//!    - 如果在停顿期间听到噪声：减少释放时间
//!
//! 3. **使用保持时间**
//!    - 如果门在持续信号中错误关闭：增加保持时间
//!    - 对于人声和延音乐器，使用较长的保持时间（100-200ms）
//!
//! 4. **调整衰减比**
//!    - 对于轻度降噪，使用较低的 ratio（10-20）
//!    - 对于完全静音，使用较高的 ratio（50-100）
//!    - 过高的 ratio可能导致不自然的静音效果
//!
//! 5. **监控效果**
//!    - 监听"门关闭"时的声音，确保没有产生噪声
//!    - 检查门是否在正确的时机打开和关闭
//!    - 确保有效音频没有被截断
//!
//! ## 常见问题
//!
//! ### Q: 噪声门会改变有效音频的音量吗？
//! A: 不会。噪声门只影响低于阈值的信号（通常是噪声），超过阈值的信号以原始音量通过。
//!
//! ### Q: 应该使用多高的衰减比？
//! A: 取决于应用。对于轻度降噪，ratio = 10-20；对于几乎完全静音，ratio = 50-100。
//!
//! ### Q: 如何避免"咔哒"声？
//! A: 使用较快的启动时间（0.5-5ms）和适当的释放时间（100-300ms），让门的开关更平滑。
//!
//! ### Q: 噪声门和降噪插件有什么区别？
//! A: 噪声门只静音低于阈值的信号，适合消除持续的背景噪声。降噪插件可以处理与信号混合的噪声，更复杂但可能产生伪影。
//!
//! ### Q: 可以串联使用多个噪声门吗？
//! A: 理论上可以，但通常不必要。一个正确配置的噪声门已经足够。多个门可能导致过度处理和音质损失。

use crate::{
    Result,
    filters::traits::{AudioData, AudioFilter},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NoiseGateFilter {
    /// 触发阈值 (dB)，低于此电平的信号将被衰减
    /// 范围: -60 到 0
    /// 常用值: -40 (安静环境) 到 -20 (嘈杂环境)
    pub threshold: f32,

    /// 衰减比，门关闭时降低信号的倍数
    /// 范围: 1 (无衰减) 到 100 (几乎完全静音)
    /// 常用值: 20 (中度) 到 100 (重度)
    pub ratio: f32,

    /// 启动时间 (毫秒)，门打开的速度
    /// 范围: 0.1 到 100
    /// 常用值: 0.5 - 10
    pub attack: f32,

    /// 保持时间 (毫秒)，门保持打开状态的时间
    /// 范围: 0 到 1000
    /// 常用值: 50 - 200
    pub hold: f32,

    /// 释放时间 (毫秒)，门关闭的速度
    /// 范围: 10 到 1000
    /// 常用值: 100 - 500
    pub release: f32,
}

impl Default for NoiseGateFilter {
    fn default() -> Self {
        Self {
            threshold: -35.0,
            ratio: 20.0,
            attack: 5.0,
            hold: 100.0,
            release: 200.0,
        }
    }
}

impl NoiseGateFilter {
    pub const NAME: &'static str = "noise gate";

    pub fn new(threshold: f32, ratio: f32, attack: f32, hold: f32, release: f32) -> Self {
        Self {
            threshold,
            ratio,
            attack,
            hold,
            release,
        }
    }
}

impl AudioFilter for NoiseGateFilter {
    crate::impl_default_audio_filter!(NoiseGateFilter);

    fn apply(&self, data: &mut AudioData) -> Result<()> {
        if data.samples.is_empty() {
            return Ok(());
        }

        let sample_rate = data.config.sample_rate;
        let threshold_linear = 10f32.powf(self.threshold / 20.0);
        let attack_samples = (self.attack / 1000.0 * sample_rate as f32) as usize;
        let hold_samples = (self.hold / 1000.0 * sample_rate as f32) as usize;
        let release_samples = (self.release / 1000.0 * sample_rate as f32) as usize;

        let mut gate_open = false;
        let mut hold_counter = 0usize;
        let mut gain = 1.0f32;

        for sample in &mut data.samples {
            let input_level = sample.abs();

            // Check if threshold exceeded
            if input_level > threshold_linear {
                gate_open = true;
                hold_counter = hold_samples;
            }

            // Update gate state
            if gate_open {
                if hold_counter > 0 {
                    hold_counter -= 1;
                } else {
                    gate_open = false;
                }
            }

            // Calculate gain
            let target_gain = if gate_open { 1.0 } else { 1.0 / self.ratio };
            let gain_diff = target_gain - gain;

            if gate_open && gain < 1.0 {
                // Attack
                gain = (gain + gain_diff / attack_samples as f32).min(1.0);
            } else if !gate_open && gain > target_gain {
                // Release
                gain = (gain + gain_diff / release_samples as f32).max(target_gain);
            }

            *sample *= gain;
        }

        Ok(())
    }
}
