# MP4 编码和封装功能 - 完整总结

## 📋 功能需求

从通道接收 RGB 图片和音频数据，进行 H.264 编码和 AAC 编码，打包成 MP4 文件，支持压缩比率控制。

## ✅ 已完成的工作

### 1. 数据结构设计
- **FrameData**: RGB 视频帧数据 (`src/mp4_muxer.rs:10-21`)
- **AudioData**: 浮点音频样本数据 (`src/mp4_muxer.rs:23-34`)
- **AACConfig**: AAC 编码配置 (`src/mp4_muxer.rs:36-55`)
- **MP4MuxerConfig**: 封装器配置 (`src/mp4_muxer.rs:57-66`)

### 2. API 设计
- **通道接口**: Rust `std::sync::mpsc` 通道 (`src/mp4_muxer.rs:98-118`)
- **启动方法**: `MP4Muxer::start()` (`src/mp4_muxer.rs:98`)
- **停止方法**: `MP4Muxer::stop()` (`src/mp4_muxer.rs:121-130`)

### 3. video-encoder 集成
- 依赖添加到 `Cargo.toml`
- 使用 `VideoEncoder` trait (`src/mp4_muxer.rs:8`)
- 调用 `video_encoder::new()` (`src/mp4_muxer.rs:166`)

### 4. 文档
- **MP4_MUXER_STATUS.md**: 当前状态和替代方案
- **MP4_ENCODER.md**: 编码器详细说明
- **mp4_muxer_demo.rs**: 使用示例

## ⚠️ 当前限制

### 技术挑战

1. **ffmpeg-next API 问题**
   ```
   error[E0599]: no method named `set_bit_rate` found for struct `ffmpeg_next::codec::Context`
   error[E0599]: no method named `set_width` found for struct `ffmpeg_next::codec::Context`
   ```

2. **包写入方法**
   ```
   error[E0599]: no method named `write_interleaved_packet` found
   ```

3. **类型系统**
   - ffmpeg-next 的封装 API 与 Rust 类型系统存在摩擦
   - 需要正确的时间戳转换和包管理

## 🔄 可用的替代方案

### 方案 1: 分步处理 (最简单)

```rust
use video_encoder::VideoEncoder;

// 步骤 1: 编码视频
let encoder = video_encoder::new(config)?;
let encoded = encoder.encode_frame(frame)?;
std::fs::write("video.h264", &data)?;

// 步骤 2: 编码音频 (使用 audio_process)
// ...

// 步骤 3: 使用 ffmpeg CLI 封装
Command::new("ffmpeg")
    .args(&["-f", "h264", "-i", "video.h264",
              "-f", "aac", "-i", "audio.aac",
              "-c", "copy", "output.mp4"])
    .output()?;
```

### 方案 2: 使用 mp4 crate

```toml
[dependencies]
mp4 = "0.14"
video-encoder = { path = "../video-encoder" }
```

```rust
use mp4::{Mp4Writer, Mp4Config, TrackType};

// 编码视频
let encoder = video_encoder::new(ve_config)?;
let encoded = encoder.encode_frame(frame)?;

// 封装到 MP4
let mut writer = Mp4Writer::write_start(&output, config)?;
let video_track = writer.add_track(&TrackType::Video { ... })?;
writer.write_sample(video_track, &sample)?;
writer.write_end()?;
```

### 方案 3: 暂时禁用功能

当前实现已经在 `src/lib.rs` 中注释掉：
```rust
// MP4 编码器处于开发中，暂时禁用
// #[cfg(feature = "ffmpeg")]
// pub mod mp4_encoder;

// 使用 mp4_muxer 替代
#[cfg(feature = "ffmpeg")]
pub mod mp4_muxer;
```

## 📊 项目结构

```
video-utils/
├── src/
│   ├── lib.rs                 # 模块导出
│   ├── mp4_muxer.rs         # MP4 封装器 (当前编译错误)
│   ├── mp4_encoder.rs        # MP4 编码器 (原始实现，已禁用)
│   ├── audio_process.rs       # 音频处理 (AAC 编码可用)
│   ├── metadata.rs            # 视频元信息
│   ├── audio_extraction.rs   # 音频提取
│   └── video_frame.rs         # 视频帧提取
├── examples/
│   └── mp4_muxer_demo.rs    # 使用示例
├── Cargo.toml                 # 已添加 video-encoder 依赖
├── MP4_MUXER_STATUS.md       # 状态文档
└── MP4_ENCODER.md            # 编码器文档
```

## 🎯 下一步行动

### 立即可行

1. **使用分步方案**:
   - 利用现有的 `video-encoder` 编码视频
   - 利用现有的 `audio_process` 编码音频
   - 使用 ffmpeg CLI 进行 MP4 封装

2. **研究 mp4 crate**:
   - 更简单直接的 MP4 封装 API
   - 可能更稳定的类型系统

### 长期规划

1. **等待 ffmpeg-next 改进**
2. **贡献修复到 ffmpeg-next**
3. **参考其他项目**:
   - [ffmpeg-sidecar](https://github.comrescia/ffmpeg-sidecar)
   - 其他使用 ffmpeg-next 的 Rust 项目

## 📝 代码使用示例

当实现完成后，使用方式如下：

```rust
use video_utils::mp4_muxer::{MP4Muxer, MP4MuxerConfig, AACConfig};
use std::path::PathBuf;

let config = MP4MuxerConfig {
    output_path: PathBuf::from("output.mp4"),
    frame_rate: 30,
    aac: AACConfig {
        bitrate: 192_000,  // 192 kbps
        sample_rate: 48_000,
        channels: 2,
    },
};

let (muxer, video_tx, audio_tx) = MP4Muxer::start(config)?;

// 发送视频帧
video_tx.send(FrameData { ... })?;

// 发送音频数据
audio_tx.send(AudioData { ... })?;

// 完成
muxer.stop()?;
```

## 🔧 当前 video-utils 可用功能

虽然 MP4 封装器还在开发中，但以下功能已完全可用：

1. ✅ **视频元信息提取** - `get_metadata()`
2. ✅ **音频数据提取** - `extract_audio_interval()`, `extract_all_audio()`
3. ✅ **视频帧提取** - `extract_frame_at_time()`, `extract_frames_interval()`
4. ✅ **帧保存** - `save_frame_as_image()`
5. ✅ **音频处理** - `process_audio()` (音量调整、AAC 编码)
6. ✅ **字幕烧录** - `add_subtitles()`
7. ✅ **字幕处理** - SRT 解析和生成

## 📚 相关资源

- `video-encoder` 库: `/home/blue/Code/rust/wayshot/lib/video-encoder/`
- `ffmpeg-next` 文档: https://github.com/zencoder/rust-ffmpeg-next
- MP4 格式规范: ISO/IEC 14496-14
