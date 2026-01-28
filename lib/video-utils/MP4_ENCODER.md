# MP4 编码器实现说明

## 状态

MP4 编码器 (`src/mp4_encoder.rs`) 目前**处于开发中**，完整实现需要解决 ffmpeg-next API 的复杂性问题。

## 当前实现

### 已完成
- ✅ 数据结构: `FrameData`, `AudioData`
- ✅ 配置结构: `H264Config`, `AACConfig`, `MP4EncoderConfig`
- ✅ 压缩预设: `H264Preset` (Ultrafast 到 Veryslow)
- ✅ 通道接口: 视频和音频的 Sender/Receiver
- ✅ 编码器框架: `MP4Encoder::start()` 和 `stop()`

### 待解决的技术问题

ffmpeg-next 的编码器 API 存在以下挑战：

1. **编码器配置** - `set_bit_rate`, `set_width` 等方法在某些版本中不可用
2. **流管理** - `add_stream`, `codec()` 等API的调用顺序
3. **包写入** - `write_interleaved_packet` vs `write_packet`
4. **编码器刷新** - `send_frame_eof` vs `flush`

## 建议的替代方案

### 选项 1: 使用现有功能 + ffmpeg CLI

```rust
// 1. 使用 video_utils 提取帧和音频
let frames = extract_frames_interval("input.mp4", ...)?;
let audio = extract_all_audio("input.mp4")?;

// 2. 使用 ffmpeg CLI 进行编码（如果允许）
// 或者使用其他 Rust 库
```

### 选项 2: 直接使用 sys 危险 FFmpeg 绑定

```rust
use ffmpeg_sys_next::ffmpeg as sys;

// 直接使用 FFmpeg C API
// 需要大量 unsafe 代码
```

### 选项 3: 等待 ffmpeg-next 改进

关注 ffmpeg-next 项目的新版本，可能会有更好的 API。

## 完整实现的需求

要完整实现 MP4 编码器，需要：

1. 深入研究 FFmpeg C API 文档
2. 阅读 ffmpeg-next 源码
3. 参考其他使用 ffmpeg-next 的项目（如 ffmpeg-sidecar）
4. 逐步测试和调试

## 压缩比率控制

当编码器完全实现后，将支持以下压缩控制：

### H.264 CRF 模式 (推荐)
```rust
H264Config {
    crf: Some(20),  // 18-23 为高质量
    preset: H264Preset::Slow,
    ..Default::default()
}
```

### H.264 比特率模式
```rust
H264Config {
    bitrate: 3_000_000,  // 3 Mbps
    crf: None,
    ..Default::default()
}
```

### AAC 音频
```rust
AACConfig {
    bitrate: 192_000,  // 192 kbps
    ..Default::default()
}
```

## CRF 值参考

- 0-16: 近无损（文件很大）
- 18-23: 高质量（推荐）
- 28: 中等质量
- 35+: 低质量

## 预设速度参考

- Ultrafast: 编码最快，文件最大
- Fast: 快速编码
- Medium: 默认
- Slow: 高质量，编码较慢
- Veryslow: 最高质量，编码最慢
