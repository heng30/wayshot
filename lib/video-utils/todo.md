# Video Utils TODO List

## 功能列表

### ✅ 已完成
1. ✅ 音频处理 (audio_process.rs) - 音量调整、AAC编码
2. ✅ 字幕处理 (subtitle.rs)
3. ✅ 字幕烧录 (subtitle_burn.rs)

### 🚧 进行中

#### 1. 获取视频文件元信息函数
- [ ] 创建 metadata.rs 模块
- [ ] 实现 `get_metadata()` 函数
- [ ] 返回信息：
  - 文件路径、格式、时长、比特率、大小
  - 视频流信息（编码、分辨率、帧率、像素格式、宽高比）
  - 音频流信息（编码、采样率、声道、布局、格式）
- [ ] 单元测试
- [ ] 示例代码
- [ ] 验证测试

#### 2. 获取视频中指定时间间隔音频数据函数
- [ ] 创建 audio_extraction.rs 模块
- [ ] 实现 `extract_audio_interval()` 函数
- [ ] 参数：视频路径、开始时间、持续时间
- [ ] 返回：采样率、声道数、样本格式、原始音频数据
- [ ] 实现 `extract_all_audio()` 辅助函数
- [ ] 单元测试
- [ ] 示例代码
- [ ] 验证测试

#### 3. 获取视频指定时间点的图片
- [ ] 创建 video_frame.rs 模块
- [ ] 实现 `extract_frame_at_time()` 函数
- [ ] 参数：视频路径、时间点（秒）
- [ ] 返回：VideoFrame结构体（宽度、高度、像素格式、RGB数据）
- [ ] 实现 `save_frame_as_image()` 保存为PNG/JPG
- [ ] 单元测试
- [ ] 示例代码
- [ ] 验证测试

#### 4. 获取视频指定时间间隔的所有图片
- [ ] 在 video_frame.rs 模块中
- [ ] 实现 `extract_frames_interval()` 函数
- [ ] 参数：视频路径、开始时间、结束时间、间隔（秒）
- [ ] 返回：Vec<VideoFrame>
- [ ] 实现 `extract_all_frames()` 辅助函数
- [ ] 支持批量保存图片
- [ ] 单元测试
- [ ] 示例代码
- [ ] 验证测试

## 实现进度

- [ ] 功能 1: get_metadata
- [ ] 功能 2: extract_audio_interval
- [ ] 功能 3: extract_frame_at_time
- [ ] 功能 4: extract_frames_interval

## 技术要点

### FFmpeg API使用
- 使用 `ffmpeg-next` crate
- 正确处理时间戳 (PTS/DTS)
- 视频解码器配置
- 软件缩放 (software scaling)
- 音频采样处理

### 错误处理
- 使用统一的 `Result<T>` 类型
- 适当的错误消息
- 文件存在性检查

### 性能优化
- 避免不必要的内存拷贝
- 正确释放FFmpeg资源
- 批量处理优化
