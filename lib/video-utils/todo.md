# Video Utils TODO List

## 功能列表

### ✅ 已完成

1. ✅ 音频处理 (audio_process.rs) - 音量调整、AAC编码
2. ✅ 字幕处理 (subtitle.rs)
3. ✅ 字幕烧录 (subtitle_burn.rs)

#### 1. 获取视频文件元信息函数 ✅
- ✅ 创建 metadata.rs 模块
- ✅ 实现 `get_metadata()` 函数
- ✅ 返回信息：
  - 文件路径、格式、时长、比特率、大小
  - 视频流数量、音频流数量
- ✅ 示例代码 (video_utils_demo.rs)
- ✅ 验证测试通过

**测试结果：**
```
✓ 成功获取元信息
  文件: data/test.mp4
  格式: mov,mp4,m4a,3gp,3g2,mj2
  时长: 5.01 秒
  比特率: 1014731 bps (1.01 Mbps)
  大小: 635222 bytes (0.61 MB)
  视频流: 1 个
  音频流: 1 个
```

#### 2. 获取视频中指定时间间隔音频数据函数 ✅
- ✅ 创建 audio_extraction.rs 模块
- ✅ 实现 `extract_audio_interval()` 函数
- ✅ 参数：视频路径、开始时间、持续时间
- ✅ 返回：采样率、声道数、样本格式、原始音频数据
- ✅ 实现 `extract_all_audio()` 辅助函数
- ✅ 示例代码 (video_utils_demo.rs)
- ✅ 验证测试通过

**测试结果：**
```
✓ 成功提取音频数据
  采样率: 48000 Hz
  声道数: 2
  样本格式: fltp
  开始时间: 1.00 秒
  持续时间: 3.00 秒
```

#### 3. 获取视频指定时间点的图片 ✅
- ✅ 创建 video_frame.rs 模块
- ✅ 实现 `extract_frame_at_time()` 函数
- ✅ 参数：视频路径、时间点（秒）
- ✅ 返回：VideoFrame结构体（宽度、高度、像素格式、RGB24数据）
- ✅ 实现 `save_frame_as_image()` 保存为PNG
- ✅ 示例代码 (video_utils_demo.rs)
- ✅ 验证测试通过 - 成功提取并保存图片

**测试结果：**
```
✓ 成功提取帧
  尺寸: 1920x1080
  像素格式: rgb24
  时间戳: 2.52 秒
  数据大小: 6220800 bytes
  已保存到: tmp/frame_at_2.5s.png (691KB)
```

#### 4. 获取视频指定时间间隔的所有图片 ✅
- ✅ 在 video_frame.rs 模块中
- ✅ 实现 `extract_frames_interval()` 函数
- ✅ 参数：视频路径、开始时间、结束时间、间隔（秒）
- ✅ 返回：Vec<VideoFrame>
- ✅ 实现 `extract_all_frames()` 辅助函数
- ✅ 支持批量保存图片
- ✅ 示例代码 (video_utils_demo.rs)
- ✅ 验证测试通过 - 成功提取并保存多帧

**测试结果：**
```
✓ 成功提取 4 帧
  帧 1: 1920x1080, 时间: 1.00s, 大小: 6220800 bytes
    已保存到: tmp/frame_1_at_1.0s.png (560KB)
  帧 2: 1920x1080, 时间: 2.00s, 大小: 6220800 bytes
    已保存到: tmp/frame_2_at_2.0s.png (691KB)
  帧 3: 1920x1080, 时间: 3.00s, 大小: 6220800 bytes
    已保存到: tmp/frame_3_at_3.0s.png (716KB)
  ... (还有 1 帧)
```

## 实现进度

- ✅ 功能 1: get_metadata - 完成
- ✅ 功能 2: extract_audio_interval - 完成
- ✅ 功能 3: extract_frame_at_time - 完成
- ✅ 功能 4: extract_frames_interval - 完成

**所有功能已完成并测试通过！**

## 技术要点

### FFmpeg API使用
- ✅ 使用 `ffmpeg-next` crate
- ✅ 正确处理时间戳 (PTS)
- ✅ 视频解码器配置
- ✅ 软件缩放 (software scaling) - YUV420P to RGB24
- ✅ 音频采样处理

### 类型安全改进
- ✅ 所有时间参数使用 `std::time::Duration` 而不是 `f64`
- ✅ 更类型安全，避免单位混淆
- ✅ 清晰的 API：`Duration::from_secs()`, `Duration::from_secs_f64()`, `Duration::from_millis()` 等

### API兼容性修复
1. **解码器创建** - 使用 `Context::from_parameters()` 然后 `.decoder().video()`
2. **像素格式** - 使用 `Pixel::RGB24` (3 bytes per pixel)
3. **借用检查器** - 提前提取 `time_base` 和 `codec_par`

### 错误处理
- ✅ 使用统一的 `Result<T>` 类型
- ✅ 适当的错误消息
- ✅ 文件存在性检查

### 测试
- ✅ 创建完整示例 `video_utils_demo.rs`
- ✅ 所有4个功能均验证通过
- ✅ 成功提取并保存视频帧为PNG图片

## 导出的公共API

```rust
// 元数据
pub use metadata::{get_metadata, VideoMetadata};

// 音频提取
pub use audio_extraction::{extract_audio_interval, extract_all_audio, AudioSamples};

// 视频帧
pub use video_frame::{
    extract_all_frames,
    extract_frame_at_time,
    extract_frames_interval,
    save_frame_as_image,
    VideoFrame,
};
```

### API 使用示例

```rust
use std::time::Duration;
use video_utils::{
    get_metadata,
    extract_audio_interval,
    extract_frame_at_time,
    extract_frames_interval,
};

// 1. 获取视频元信息
let metadata = get_metadata("video.mp4")?;
println!("时长: {:.2}s", metadata.duration);

// 2. 提取音频间隔 (1秒到3秒)
let audio = extract_audio_interval(
    "video.mp4",
    Duration::from_secs(1),
    Duration::from_secs(2)
)?;

// 3. 提取指定时间点的帧 (2.5秒)
let frame = extract_frame_at_time(
    "video.mp4",
    Duration::from_secs_f64(2.5)
)?;

// 4. 提取多个帧 (每1秒提取一次，从1秒到4秒)
let frames = extract_frames_interval(
    "video.mp4",
    Duration::from_secs(1),
    Duration::from_secs(4),
    Duration::from_secs(1)
)?;
```
