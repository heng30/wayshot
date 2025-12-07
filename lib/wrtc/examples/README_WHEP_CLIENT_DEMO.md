# WHEP Client Demo

这个演示程序展示了如何使用WHEP客户端连接到WHEP服务器并接收音视频流。

## 功能特性

- 连接到WHEP服务器 (默认: localhost:9090)
- 接收H264视频流并解码为RGB格式
- 接收Opus音频流并解码为f32格式
- 保存前10张视频帧到 `/tmp/whep-client/` 目录
- 保存前10秒音频数据到 `/tmp/whep-client/` 目录
- 实时显示接收进度和统计信息

## 使用方法

### 1. 启动WHEP服务器

首先需要在另一个终端启动WHEP服务器：

```bash
cd /home/blue/Code/rust/wayshot/lib/wrtc
cargo run --example whep_server2_demo
```

服务器将在 `localhost:9090` 启动，并提供测试音视频流。

### 2. 运行WHEP客户端演示

在新的终端中运行客户端演示：

```bash
cd /home/blue/Code/rust/wayshot/lib/wrtc
cargo run --example whep_client_demo
```

### 3. 查看输出结果

程序将在 `/tmp/whep-client/` 目录下创建以下文件：

#### 视频文件
- `frame_001_1920x1080.rgb` - 第1帧RGB数据
- `frame_001_1920x1080.txt` - 第1帧信息 (宽度、高度、数据大小)
- `frame_002_1920x1080.rgb` - 第2帧RGB数据
- `frame_002_1920x1080.txt` - 第2帧信息
- ... (共10帧)

#### 音频文件
- `first_10_seconds_audio.raw` - 前10秒原始音频数据 (f32格式)
- `first_10_seconds_audio_info.txt` - 音频信息 (采样率、声道数、时长等)

## 文件格式说明

### RGB视频帧格式
- 格式: RGB24 (每像素3字节)
- 分辨率: 1920x1080 (可变)
- 文件大小: 宽度 × 高度 × 3 字节

### 音频数据格式
- 格式: 32位浮点数 (little-endian)
- 采样率: 48000 Hz
- 声道: 2 (立体声)
- 文件大小: 样本数 × 4 字节

## 查看和转换输出文件

### 转换RGB帧为图片

使用FFmpeg将RGB数据转换为PNG图片：

```bash
ffmpeg -f rawvideo -pixel_format rgb24 -video_size 1920x1080 \
  -i /tmp/whep-client/frame_001_1920x1080.rgb \
  /tmp/whep-client/frame_001.png
```

### 转换音频为WAV

使用FFmpeg将原始音频转换为WAV文件：

```bash
ffmpeg -f f32le -ar 48000 -ac 2 \
  -i /tmp/whep-client/first_10_seconds_audio.raw \
  /tmp/whep-client/first_10_seconds_audio.wav
```

## 预期输出

程序运行时会显示类似以下的输出：

```
WHEP Client Demo
================
Connecting to WHEP server at: http://localhost:9090
Attempting to connect to WHEP server: http://localhost:9090
H264 video decoder initialized
Opus audio decoder initialized
WHEP connection established successfully
Received video frame #1: 1920x1080 (6220800 bytes)
  Saved frame #1 to: /tmp/whep-client/frame_001_1920x1080.rgb
Received audio packet #1: 48000 Hz, 960 samples (20.00ms)
Progress: 1 video frames, 0.02 seconds audio
...
Successfully saved 10 video frames
Collected 10.00 seconds of audio, saving...
Saved audio data to: /tmp/whep-client/first_10_seconds_audio.raw
==================================================
WHEP Client Demo Summary
=========================
Total video frames received: 10
Total audio duration received: 10.00 seconds
Output directory: /tmp/whep-client
Files created:
  - frame_001_1920x1080.rgb (6220800 bytes)
  - frame_001_1920x1080.txt
  - frame_002_1920x1080.rgb (6220800 bytes)
  - ...
  - first_10_seconds_audio.raw (3840000 bytes)
  - first_10_seconds_audio_info.txt
```

## 故障排除

### 连接失败
- 确保WHEP服务器正在运行在 `localhost:9090`
- 检查防火墙设置
- 确保端口9090未被其他程序占用

### 编译错误
- 确保所有依赖项已正确安装
- 运行 `cargo check` 检查代码问题
- 确保在正确的目录中运行命令

### 没有收到数据
- 检查服务器是否正确启动
- 确认服务器正在发送数据
- 查看程序输出中的错误信息

## 技术实现

- **WHEP协议**: 实现了标准的WebRTC-HTTP Egress Protocol
- **H264解码**: 使用OpenH264库进行Annex-B格式解码
- **Opus解码**: 使用audiopus库进行音频解码
- **YUV转换**: 实现YUV420到RGB24的实时转换
- **并发处理**: 使用tokio异步编程处理多个数据流

## 依赖项

- `tokio` - 异步运行时
- `openh264` - H264编解码器
- `audiopus` - Opus音频编解码器
- `anyhow` - 错误处理
- `env_logger` - 日志记录
- `serde_json` - JSON序列化
- `reqwest` - HTTP客户端