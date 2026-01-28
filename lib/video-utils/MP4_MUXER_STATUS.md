# MP4 封装器实现说明

## 当前状态

`src/mp4_muxer.rs` 使用 `video-encoder` 库进行 H.264 编码，并使用 ffmpeg-next 进行 MP4 封装。**但由于 ffmpeg-next API 的复杂性，当前实现存在编译错误。**

## 架构设计

```
┌─────────────┐
│ FrameData  │ (RGB)
└──────┬──────┘
       │
       ▼
┌─────────────────┐
│ video-encoder   │ (H.264 编码)
│  - x264         │
│  - openh264    │
│  - ffmpeg      │
└──────┬──────────┘
       │ EncodedFrame
       ▼
┌─────────────────┐
│  MP4 Muxer      │ (ffmpeg-next)
│  - H.264 流    │
│  - AAC 流      │
└──────┬──────────┘
       │
       ▼
   output.mp4
```

## 遇到的问题

1. **ffmpeg-next 编码器 API** - `set_bit_rate`, `set_width` 等方法不可用
2. **包写入** - `write_interleaved_packet` 方法不存在
3. **类型不匹配** - 编码器上下文的类型转换问题

## 实用替代方案

### 方案 1: 使用 MP4 封装库

推荐使用 `mp4` crate:
```toml
[dependencies]
mp4 = "0.14"
```

```rust
use mp4::{Mp4Config, Mp4Writer, TrackType};

let config = Mp4Config {
    track_type: TrackType::Video,
    timescale: 90000,
    ..Default::default()
};

let mut writer = Mp4Writer::write(&output, config)?;
// 写入 H.264 和 AAC 数据
```

### 方案 2: 分步处理 (推荐)

```rust
// 1. 使用 video-encoder 编码视频
let encoder = video_encoder::new(config)?;
let encoded = encoder.encode_frame(frame)?;

// 2. 将编码后的数据保存到文件
std::fs::write("video.h264", &encoded_data)?;

// 3. 使用 ffmpeg CLI 进行最终封装
Command::new("ffmpeg")
    .args(&[
        "-f", "h264",
        "-i", "video.h264",
        "-f", "aac",
        "-i", "audio.aac",
        "-c", "copy",
        "output.mp4"
    ])
    .output()?;
```

### 方案 3: 直接使用 mp4 封装库

```rust
use mp4::{Mp4Writer, Mp4Config, TrackType, AvcConfig, Mp4Sample};

// 创建 MP4 writer
let config = Mp4Config {
    major_brand: str::parse("isom").unwrap(),
    timescale: 90000,
    ..Default::default()
};

let mut writer = Mp4Writer::write_start(&output, config)?;

// 添加 H.264 track
let video_track_id = writer.add_track(&TrackType::Video {
    width: 1920,
    height: 1080,
    ..Default::default()
})?;

// 写入 H.264 样本
let sample = Mp4Sample {
    start_time: 0,
    bytes: &h264_data,
    ..Default::default()
};
writer.write_sample(video_track_id, &sample)?;

// 写入尾部
writer.write_end()?;
```

## 当前代码状态

### ✅ 已完成
- 数据结构定义
- 通道接口设计
- video-encoder 集成框架
- AAC 音频编码部分实现

### ⚠️ 需要修复
- ffmpeg-next API 调用问题
- H.264 流配置
- 包写入方法

## 建议的开发路径

### 短期 (推荐)
1. 使用方案2（分步处理）快速实现功能
2. 创建临时文件存储编码数据
3. 使用 ffmpeg CLI 进行最终封装

### 中期
1. 研究使用 `mp4` crate
2. 替换 ffmpeg-next 的封装部分
3. 保留 video-encoder 进行编码

### 长期
1. 深入研究 ffmpeg-next 源码
2. 贡献修复到 ffmpeg-next 项目
3. 完整实现纯 Rust 方案

## 代码可用性

当前代码框架完整，主要问题在于：
- ffmpeg-next API 的正确调用方式
- 需要参考其他成功使用 ffmpeg-next 的项目

一旦 API 问题解决，完整实现可以快速完成。
