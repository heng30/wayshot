# deepfilter-rs

基于 [DeepFilterNet](https://github.com/Rikorose/DeepFilterNet) 的实时音频降噪与去混响库，使用 ONNX Runtime 进行神经网络推理。

Fork 自 [deepfilter-rt](https://github.com/shimondoodkin/deepfilter-rt)，重写了推理管线，支持分流式/组合式流推理、CUDA 加速、多模型变体自动检测。

## 算法原理

DeepFilterNet3 采用两阶段降噪架构：

1. **ERB 频谱掩码** — 在 32 个 ERB（等效矩形带宽）频段上学习频谱掩码，应用于全部 481 个频率 bin，抑制宽带噪声
2. **深度滤波** — 对前 96 个频率 bin（0–4.8 kHz，语音关键频段）预测复数 FIR 滤波器系数，进行 5 阶复数卷积，比掩码更好地保留相位信息，有效去除混响

### 处理流程

```
音频帧 [480 采样, 48kHz, 10ms]
        │
        ▼
  STFT 分析（Vorbis 窗 + FFT, 960 点, 50% 重叠）
        │
        ▼
  复数频谱 [481 bin]
        │
   ┌────┴────────────┐
   ▼                  ▼
 ERB 特征提取      复数频谱特征
 [1, 32] dB 归一化  [2, 96] 单位归一化
   │                  │
   └────┬─────────────┘
        ▼
  ┌─────────────────────────────┐
  │         ONNX 神经网络推理      │
  │                               │
  │  编码器 → GRU 时序建模 → LSNR  │
  │     │              │          │
  │  ERB 解码器      DF 解码器     │
  │     │              │          │
  │  频谱掩码 [32]   滤波系数 [96×5]│
  └─────┼──────────────┼─────────┘
        │              │
        ▼              ▼
  ┌─────────────────────────────┐
  │       频谱重建                │
  │  1. ERB 掩码 × 频谱（全部 bin）│
  │  2. 深度滤波（前 96 bin）      │
  │  3. 合并：DF [0:96] + 掩码 [96:481]│
  └──────────────┬──────────────┘
                 │
                 ▼
  ISTFT 合成（IFFT + Vorbis 窗 + 重叠相加）
                 │
                 ▼
  增强音频帧 [480 采样]
```

## 模型变体

| 变体 | 目录 | 延迟 | 推理模式 | 适用场景 |
|------|------|------|----------|----------|
| DeepFilterNet2 | `dfn2` | 30ms | 无状态 | 通用 |
| DeepFilterNet2-LL | `dfn2_ll` | 10ms | 无状态 | 低延迟 |
| DeepFilterNet2-H0 | `dfn2_h0` | 30ms | 有状态（GRU） | 最佳质量 |
| DeepFilterNet3 | `dfn3` | 30ms | 无状态 | 高质量 |
| DeepFilterNet3-LL | `dfn3_ll` | 10ms | 无状态 | 实时 |
| DeepFilterNet3-H0 | `dfn3_h0` | 30ms | 有状态（GRU） | 最佳质量 |

变体根据模型目录名和 `config.ini` 自动检测。

## 快速开始

### 添加依赖

```toml
[dependencies]
deepfilter-rs = { path = "..." }
```

### 流式处理

使用 `DeepFilterStream` 是最简单的方式，内部自动处理帧缓冲：

```rust
use deepfilter_rs::DeepFilterStream;
use std::path::Path;

// 创建流处理器（自动检测模型变体）
let mut stream = DeepFilterStream::new(Path::new("models/dfn3_h0"))?;

// 预热推理引擎，避免首次推理的冷启动延迟
stream.warmup()?;

// 处理任意长度的音频（必须是 48kHz 单声道 f32）
let enhanced = stream.process(&input_samples)?;

// 流结束时刷新缓冲区
let remaining = stream.flush()?;
```

### 逐帧处理

使用 `DeepFilterProcessor` 可以精确控制每帧处理，适合音频回调集成：

```rust
use deepfilter_rs::{DeepFilterProcessor, HOP_SIZE};
use std::path::Path;

let mut processor = DeepFilterProcessor::new(Path::new("models/dfn3_h0"))?;
processor.warmup()?;

// 每帧恰好 480 个采样（10ms @ 48kHz）
fn audio_callback(input: &[f32; 480], output: &mut [f32; 480]) {
    processor.process_frame(input, output).unwrap();
}
```

### 命令行处理音频文件

```bash
# 基本用法
cargo run --release --example process_file -- input.wav output.wav models/dfn3_h0

# 补偿算法延迟（对齐输出时间轴，等同于 Python 的 pad=True）
cargo run --release --example process_file -- input.wav output.wav models/dfn3_h0 -D

# 指定推理模式
cargo run --release --example process_file -- input.wav output.wav models/dfn3_h0 -D --mode combined
```

### 实时模拟

```bash
cargo run --release --example realtime -- input.wav output.wav models/dfn3_ll -D
```

### 多线程流水线

将推理放在独立线程上，与音频 I/O 线程解耦：

```bash
cargo run --release --example pipelined -- input.wav output.wav models/dfn3_ll -D
```

## 音频要求

- **采样率**：48 kHz（如输入不是 48kHz，需先重采样）
- **格式**：单声道 f32 采样，范围 [-1.0, 1.0]

## 推理模式

| 模式 | 说明 | 质量 | 速度 |
|------|------|------|------|
| 分流式（Split Streaming） | 4 个独立 ONNX 会话，GRU 状态显式传递 | 最佳（corr=0.999991） | RTF ~0.13x |
| 组合式（Combined Streaming） | 单个 `combined_streaming.onnx`，GRU 状态作为 I/O | 最佳（corr=0.999991） | RTF ~0.04x |
| 无状态窗口（Stateless Window） | 单个 `combined.onnx`，每帧 40 帧窗口预热 GRU | 较差 | RTF > 1x |

自动检测优先级：分流式 > 组合式 > 无状态窗口。

可通过 `SessionMode` 手动指定：

```rust
use deepfilter_rs::{DeepFilterStream, SessionMode};

let mut stream = DeepFilterStream::with_mode(
    Path::new("models/dfn3_h0"),
    SessionMode::CombinedStreaming,
    Some(2), // ONNX Runtime 线程数
)?;
```

## 线程数

通过 `with_threads` 构造器控制 ONNX Runtime 的算子内并行度：

- **实时音频**：使用 1-2 线程，减少延迟抖动
- **批量/离线**：使用 4-8 线程，提高吞吐量
- **默认**：ONNX Runtime 根据 CPU 核心数自动选择

```rust
let mut stream = DeepFilterStream::with_threads(
    Path::new("models/dfn3_h0"),
    1, // 实时场景：单线程
)?;
```

## CUDA 加速

默认启用 CUDA 支持（通过 `ort` 的 `cuda` feature）。ONNX Runtime 会自动检测 GPU，若不可用则回退到 CPU。

如需禁用 CUDA（纯 CPU 部署），修改 `Cargo.toml`：

```toml
ort = { version = "2.0.0-rc.12", default-features = false, features = [
  "std",
  "tls-rustls",
  "download-binaries",
] }
```

## DeepFilterStream API 参考

| 方法 | 说明 |
|------|------|
| `new(path)` | 从模型目录创建，自动检测变体 |
| `with_threads(path, n)` | 指定 ONNX 线程数 |
| `with_mode(path, mode, threads)` | 指定推理模式 |
| `with_variant_and_threads(path, variant, n)` | 指定模型变体 |
| `warmup()` | 预热推理引擎，消除冷启动延迟 |
| `process(&samples)` | 处理任意长度音频，返回增强采样 |
| `flush()` | 刷新缓冲区，获取剩余采样 |
| `reset()` | 重置处理器状态，清除缓冲区 |
| `variant()` | 获取检测到的模型变体 |
| `sample_rate()` | 采样率（48000） |
| `latency_ms()` | 算法延迟（毫秒） |
| `delay_samples()` | 算法延迟（采样数），用于对齐输出 |
| `lookahead()` | 模型前瞻帧数（0=LL, 2=标准） |
| `inference_mode_name()` | 当前推理模式名称 |
| `processor_mut()` | 访问底层 `DeepFilterProcessor` |

## 性能基准

测试条件：2 秒音频，Linux x86_64，ONNX Runtime CPU

| 模型 | 推理模式 | 相对 Tract 相关系数 | RTF |
|------|----------|---------------------|-----|
| dfn3_h0 | 分流式 | 0.999991 | ~0.13x |
| dfn3_h0 | 组合式 | 0.999991 | ~0.04x |
| dfn3_ll | 分流式 | 0.999605 | ~0.24x |

RTF < 1.0 表示快于实时（可以实时处理）。

## 关键参数

| 参数 | 值 | 说明 |
|------|-----|------|
| 采样率 | 48000 Hz | |
| FFT 大小 | 960 | Vorbis 窗 |
| 帧长（hop） | 480 采样 | 10ms |
| 频率 bin 数 | 481 | FFT/2 + 1 |
| ERB 频段数 | 32 | |
| DF 频率 bin 数 | 96 | 0–4.8 kHz |
| DF 滤波器阶数 | 5 | 5 阶复数 FIR |
| GRU 隐藏维度 | 256 | |
| 算法延迟（标准） | 1440 采样 | 30ms |
| 算法延迟（LL） | 480 采样 | 10ms |

## 项目结构

```
deepfilter-rs/
├── Cargo.toml              # 依赖配置 + [patch.crates-io]
├── src/
│   ├── lib.rs              # 核心推理代码（DeepFilterProcessor / DeepFilterStream）
│   └── rolling.rs          # 独立 STFT/ISTFT/GRU/Norm（rolling feature，当前未使用）
├── vender/
│   └── deep_filter/        # deep_filter 0.2.5 + 补丁（补全 DFState 方法）
├── models/                 # ONNX 模型文件
│   ├── dfn2/               # DeepFilterNet2（无状态，30ms）
│   ├── dfn2_h0/            # DeepFilterNet2-H0（有状态，30ms，最佳质量）
│   ├── dfn2_ll/            # DeepFilterNet2-LL（无状态，10ms）
│   ├── dfn3/               # DeepFilterNet3（无状态，30ms）
│   ├── dfn3_h0/            # DeepFilterNet3-H0（有状态，30ms，最佳质量）
│   └── dfn3_ll/            # DeepFilterNet3-LL（无状态，10ms）
├── examples/
│   ├── process_file.rs     # 文件处理示例
│   ├── realtime.rs         # 实时流模拟示例
│   └── pipelined.rs        # 多线程流水线示例
└── fork.md                 # 上游 fork 来源
```

## 依赖说明

- **`deep_filter`** — 通过 `[patch.crates-io]` 使用本地 `vender/deep_filter/`，在 crates.io 0.2.5 基础上补全了 `DFState` 缺失的方法（`feat_erb`、`feat_cplx`、`apply_mask`、`init_norm_states` 等），实现来自 [DeepFilterNet/libDF](https://github.com/Rikorose/DeepFilterNet/tree/main/libDF)
- **`ort`** — ONNX Runtime Rust 绑定（2.0.0-rc.12），启用 `download-binaries` 自动下载 ORT 共享库，启用 `cuda` 支持 GPU 加速
- **`ndarray`** — 张量操作（ort 依赖）
- **`realfft`** — FFT 实现（`rolling` feature 可选，当前未使用）

## Feature Flags

| Feature | 默认 | 说明 |
|---------|------|------|
| `rolling` | 否 | 启用独立 STFT/ISTFT/GRU/Norm 实现（`rolling` 模块），当前未使用，保留用于无 ONNX Runtime 的平台 |

## 许可证

本项目基于 DeepFilterNet，遵循 MIT / Apache-2.0 双许可证。
