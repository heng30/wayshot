use crate::{Error, Result};
use ffmpeg_next as ffmpeg;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::ffi::CString;
use std::panic::catch_unwind;
use std::panic::AssertUnwindSafe;

/// 视频帧数据 (RGB格式)
#[derive(Debug, Clone)]
pub struct FrameData {
    /// 宽度 (像素)
    pub width: u32,
    /// 高度 (像素)
    pub height: u32,
    /// RGB数据 (每个像素3字节: R, G, B)
    pub data: Vec<u8>,
    /// 时间戳
    pub timestamp: Duration,
}

/// 音频数据
#[derive(Debug, Clone)]
pub struct AudioData {
    /// 音频样本 (浮点格式, 平面: [L, L, L, ..., R, R, R, ...])
    pub samples: Vec<f32>,
    /// 采样率 (Hz)
    pub sample_rate: u32,
    /// 声道数
    pub channels: u8,
    /// 时间戳
    pub timestamp: Duration,
}

/// H.264 压缩预设
#[derive(Debug, Clone, Copy)]
pub enum H264Preset {
    Ultrafast,
    Superfast,
    Veryfast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    Veryslow,
}

impl H264Preset {
    fn as_str(&self) -> &str {
        match self {
            H264Preset::Ultrafast => "ultrafast",
            H264Preset::Superfast => "superfast",
            H264Preset::Veryfast => "veryfast",
            H264Preset::Faster => "faster",
            H264Preset::Fast => "fast",
            H264Preset::Medium => "medium",
            H264Preset::Slow => "slow",
            H264Preset::Slower => "slower",
            H264Preset::Veryslow => "veryslow",
        }
    }
}

/// H.264 编码配置
#[derive(Debug, Clone)]
pub struct H264Config {
    /// 比特率 (bps)
    pub bitrate: u32,
    /// 压缩预设
    pub preset: H264Preset,
    /// CRF (恒定质量因子) - 范围 0-51，越小质量越高
    pub crf: Option<u8>,
}

impl Default for H264Config {
    fn default() -> Self {
        Self {
            bitrate: 2_000_000,
            preset: H264Preset::Medium,
            crf: Some(23),
        }
    }
}

/// AAC 编码配置
#[derive(Debug, Clone)]
pub struct AACConfig {
    /// 比特率 (bps)
    pub bitrate: u32,
    /// 采样率 (Hz)
    pub sample_rate: u32,
    /// 声道数
    pub channels: u8,
}

impl Default for AACConfig {
    fn default() -> Self {
        Self {
            bitrate: 128_000,
            sample_rate: 44_100,
            channels: 2,
        }
    }
}

/// MP4 编码器配置
#[derive(Debug, Clone)]
pub struct MP4EncoderConfig {
    /// 输出文件路径
    pub output_path: PathBuf,
    /// 视频帧率 (fps)
    pub frame_rate: u32,
    /// H.264 编码配置
    pub h264: H264Config,
    /// AAC 编码配置
    pub aac: AACConfig,
}

/// MP4 编码器
pub struct MP4Encoder {
    video_sender: Sender<FrameData>,
    audio_sender: Sender<AudioData>,
    join_handle: Option<JoinHandle<Result<()>>>,
}

impl MP4Encoder {
    /// 创建并启动 MP4 编码器
    pub fn start(config: MP4EncoderConfig) -> Result<(Self, Sender<FrameData>, Sender<AudioData>)> {
        let (video_sender, video_receiver) = channel();
        let (audio_sender, audio_receiver) = channel();

        let video_sender_for_user = video_sender.clone();
        let audio_sender_for_user = audio_sender.clone();

        let join_handle = thread::spawn(move || {
            // 捕获 panic 并转换为错误
            let result = catch_unwind(AssertUnwindSafe(|| {
                encode_mp4(config, video_receiver, audio_receiver)
            }));

            match result {
                Ok(r) => r,
                Err(e) => {
                    let msg = if let Some(s) = e.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = e.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "Unknown panic".to_string()
                    };
                    Err(Error::FFmpeg(format!("Encoder thread panic: {}", msg)))
                }
            }
        });

        Ok((
            Self {
                video_sender,
                audio_sender,
                join_handle: Some(join_handle),
            },
            video_sender_for_user,
            audio_sender_for_user,
        ))
    }

    /// 停止编码器并等待完成
    pub fn stop(mut self) -> Result<()> {
        drop(self.video_sender);
        drop(self.audio_sender);

        if let Some(handle) = self.join_handle.take() {
            handle.join().map_err(|e| Error::FFmpeg(format!("Encoder thread panicked: {:?}", e)))??;
        }

        Ok(())
    }
}

/// 编码 MP4 文件
fn encode_mp4(
    config: MP4EncoderConfig,
    video_receiver: Receiver<FrameData>,
    audio_receiver: Receiver<AudioData>,
) -> Result<()> {
    ffmpeg::init()
        .map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    let output_path = config.output_path.to_str().ok_or_else(|| {
        Error::InvalidConfig("Invalid output path".to_string())
    })?;

    log::info!("Starting MP4 encoding to: {}", output_path);
    log::info!("H.264: bitrate={}, preset={:?}, crf={:?}",
        config.h264.bitrate, config.h264.preset, config.h264.crf);
    log::info!("AAC: bitrate={}, sample_rate={}, channels={}",
        config.aac.bitrate, config.aac.sample_rate, config.aac.channels);

    // 接收第一帧确定尺寸
    let first_frame = video_receiver.recv().map_err(|_| {
        Error::FFmpeg("No video frames received".to_string())
    })?;

    let width = first_frame.width;
    let height = first_frame.height;

    log::info!("Video size: {}x{}, fps: {}", width, height, config.frame_rate);

    // 创建输出格式
    let mut output = ffmpeg::format::output(&output_path)
        .map_err(|e| Error::FFmpeg(format!("Failed to create output: {}", e)))?;

    // 添加视频流
    let (video_stream, mut video_encoder) = add_video_stream(&mut output, &config.h264, width, height, config.frame_rate)?;
    let video_stream_index = video_stream.index();
    let video_time_base = video_stream.time_base();

    // 添加音频流
    let (audio_stream, mut audio_encoder) = add_audio_stream(&mut output, &config.aac)?;
    let audio_stream_index = audio_stream.index();
    let audio_time_base = audio_stream.time_base();

    // 写入头部
    output.write_header()
        .map_err(|e| Error::FFmpeg(format!("Failed to write header: {}", e)))?;

    // 创建RGB到YUV转换器
    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        ffmpeg::format::Pixel::RGB24,
        width,
        height,
        ffmpeg::format::Pixel::YUV420P,
        width,
        height,
        ffmpeg::software::scaling::Flags::BILINEAR,
    ).map_err(|e| Error::FFmpeg(format!("Failed to create scaler: {}", e)))?;

    // 编码第一帧
    log::debug!("Encoding first frame...");
    let mut rgb_frame = to_ffmpeg_rgb_frame(&first_frame)?;
    let mut yuv_frame = ffmpeg::frame::Video::empty();
    scaler.run(&rgb_frame, &mut yuv_frame)
        .map_err(|e| Error::FFmpeg(format!("Scaler failed: {}", e)))?;
    yuv_frame.set_pts(Some(0));

    log::debug!("Sending first frame to encoder...");
    encode_and_write_video(&mut video_encoder, &yuv_frame, &mut output, video_stream_index, video_time_base)?;
    log::debug!("First frame encoded successfully");

    let mut video_pts = 1i64;
    let audio_samples_written = 0u64;
    let mut loop_iterations = 0u64;
    const MAX_LOOP_ITERATIONS: u64 = 10000; // 10 seconds at 1ms sleep

    // 主循环
    loop {
        let mut video_done = false;
        let mut audio_done = false;

        // 处理视频
        match video_receiver.try_recv() {
            Ok(frame) => {
                rgb_frame = to_ffmpeg_rgb_frame(&frame)?;
                scaler.run(&rgb_frame, &mut yuv_frame)
                    .map_err(|e| Error::FFmpeg(format!("Scaler failed: {}", e)))?;
                yuv_frame.set_pts(Some(video_pts));
                encode_and_write_video(&mut video_encoder, &yuv_frame, &mut output, video_stream_index, video_time_base)?;
                video_pts += 1;

                if video_pts % 30 == 0 {
                    log::debug!("Encoded {} video frames", video_pts);
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                log::debug!("Video channel disconnected");
                video_done = true;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }

        // 处理音频
        match audio_receiver.try_recv() {
            Ok(audio) => {
                log::debug!("Received audio data ({} samples) - audio encoding temporarily disabled", audio.samples.len());
                // TODO: Implement proper audio encoding
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                log::debug!("Audio channel disconnected");
                audio_done = true;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }

        // 正常退出条件：视频和音频都已完成
        if video_done && audio_done {
            log::debug!("Both video and audio done - exiting main loop");
            break;
        }

        // 视频完成后最多再等待2秒 (2000 iterations) 给音频时间
        if video_done && loop_iterations > video_pts as u64 + 2000 {
            log::debug!("Video done, audio timeout - exiting main loop");
            break;
        }

        thread::sleep(Duration::from_millis(1));

        loop_iterations += 1;
        if loop_iterations > MAX_LOOP_ITERATIONS {
            log::warn!("Main loop timeout after {} iterations", loop_iterations);
            return Err(Error::FFmpeg(format!("Main loop timeout - video_pts={}, audio_samples_written={}", video_pts, audio_samples_written)));
        }
    }

    // 刷新编码器
    log::debug!("Flushing encoders...");
    flush_video_encoder(&mut video_encoder, &mut output, video_stream_index, video_time_base)?;
    flush_audio_encoder(&mut audio_encoder, &mut output, audio_stream_index, audio_time_base)?;

    // 写入尾部
    output.write_trailer()
        .map_err(|e| Error::FFmpeg(format!("Failed to write trailer: {}", e)))?;

    log::info!("MP4 encoding completed. Frames: {}", video_pts);

    Ok(())
}

/// 添加视频流
fn add_video_stream<'a>(
    output: &'a mut ffmpeg::format::context::Output,
    config: &H264Config,
    width: u32,
    height: u32,
    frame_rate: u32,
) -> Result<(ffmpeg::StreamMut<'a>, ffmpeg::encoder::Video)> {
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::H264)
        .ok_or_else(|| Error::FFmpeg("H264 encoder not found".to_string()))?;

    let mut encoder_ctx = ffmpeg::codec::context::Context::new_with_codec(codec);

    unsafe {
        let ctx = encoder_ctx.as_mut_ptr();
        if let Some(crf) = config.crf {
            ffmpeg::sys::av_opt_set_int(
                (*ctx).priv_data,
                b"crf\0".as_ptr() as *const _,
                crf as i64,
                0,
            );
        }
        // 使用 CString 确保 null 终止和正确的生命周期
        let preset_str = CString::new(config.preset.as_str())
            .map_err(|e| Error::FFmpeg(format!("Failed to create CString: {}", e)))?;
        ffmpeg::sys::av_opt_set(
            (*ctx).priv_data,
            b"preset\0".as_ptr() as *const _,
            preset_str.as_ptr() as *const _,
            0,
        );
    }

    // 配置视频编码器参数
    let mut video_encoder = encoder_ctx.encoder().video()
        .map_err(|e| Error::FFmpeg(format!("Failed to get video encoder: {}", e)))?;

    video_encoder.set_bit_rate(config.bitrate as usize);
    video_encoder.set_width(width);
    video_encoder.set_height(height);
    video_encoder.set_time_base(ffmpeg::Rational(1, frame_rate as i32));
    video_encoder.set_frame_rate(Some(ffmpeg::Rational(frame_rate as i32, 1)));
    video_encoder.set_format(ffmpeg::format::Pixel::YUV420P);

    // 打开编码器
    let video_encoder = video_encoder.open_as(codec)
        .map_err(|e| Error::FFmpeg(format!("Failed to open video encoder: {}", e)))?;

    let mut stream = output.add_stream(codec)
        .map_err(|e| Error::FFmpeg(format!("Failed to add video stream: {}", e)))?;

    stream.set_parameters(&video_encoder);

    Ok((stream, video_encoder))
}

/// 添加音频流
fn add_audio_stream<'a>(
    output: &'a mut ffmpeg::format::context::Output,
    config: &AACConfig,
) -> Result<(ffmpeg::StreamMut<'a>, ffmpeg::encoder::Audio)> {
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::AAC)
        .ok_or_else(|| Error::FFmpeg("AAC encoder not found".to_string()))?;

    let encoder_ctx = ffmpeg::codec::context::Context::new_with_codec(codec);

    // 配置音频编码器参数
    let mut audio_encoder = encoder_ctx.encoder().audio()
        .map_err(|e| Error::FFmpeg(format!("Failed to get audio encoder: {}", e)))?;

    audio_encoder.set_bit_rate(config.bitrate as usize);
    audio_encoder.set_rate(config.sample_rate as i32);
    audio_encoder.set_format(ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar));

    // 设置声道布局
    let channel_layout = if config.channels == 2 {
        ffmpeg::channel_layout::ChannelLayout::STEREO
    } else {
        ffmpeg::channel_layout::ChannelLayout::MONO
    };
    audio_encoder.set_channel_layout(channel_layout);

    // 打开编码器
    let audio_encoder = audio_encoder.open_as(codec)
        .map_err(|e| Error::FFmpeg(format!("Failed to open audio encoder: {}", e)))?;

    let mut stream = output.add_stream(codec)
        .map_err(|e| Error::FFmpeg(format!("Failed to add audio stream: {}", e)))?;

    stream.set_parameters(&audio_encoder);

    Ok((stream, audio_encoder))
}

/// 转换为 FFmpeg RGB 帧
fn to_ffmpeg_rgb_frame(frame: &FrameData) -> Result<ffmpeg::frame::Video> {
    let mut ff_frame = ffmpeg::frame::Video::empty();
    ff_frame.set_width(frame.width);
    ff_frame.set_height(frame.height);
    // 必须在分配缓冲区之前设置格式
    ff_frame.set_format(ffmpeg::format::Pixel::RGB24);

    // 分配帧数据
    let stride = (frame.width as usize) * 3;
    let required_size = stride * frame.height as usize;

    unsafe {
        // 分配数据缓冲区 - av_frame_get_buffer 只需要 2 个参数
        let ret = ffmpeg::sys::av_frame_get_buffer(ff_frame.as_mut_ptr(), 0);
        if ret < 0 {
            return Err(Error::FFmpeg(format!("Failed to allocate frame buffer: error {}", ret)));
        }

        // 获取数据指针并复制数据
        let data_ptr = (*ff_frame.as_mut_ptr()).data[0];
        if data_ptr.is_null() {
            return Err(Error::FFmpeg("Frame data pointer is null".to_string()));
        }

        // 设置行大小
        (*ff_frame.as_mut_ptr()).linesize[0] = stride as i32;

        // 复制数据
        let dst_slice = std::slice::from_raw_parts_mut(data_ptr as *mut u8, required_size);
        dst_slice.copy_from_slice(&frame.data[..required_size]);
    }

    Ok(ff_frame)
}

/// 编码并写入视频
fn encode_and_write_video(
    encoder: &mut ffmpeg::encoder::Video,
    frame: &ffmpeg::frame::Video,
    output: &mut ffmpeg::format::context::Output,
    stream_index: usize,
    time_base: ffmpeg::Rational,
) -> Result<()> {
    encoder.send_frame(frame)
        .map_err(|e| Error::FFmpeg(format!("Video encoding failed: {}", e)))?;

    let mut packet = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(stream_index);
        packet.rescale_ts(encoder.time_base(), time_base);
        packet.write(output)
            .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
    }

    Ok(())
}

/// 编码并写入音频
#[allow(dead_code)]
fn encode_and_write_audio(
    audio: &AudioData,
    encoder: &mut ffmpeg::encoder::Audio,
    output: &mut ffmpeg::format::context::Output,
    stream_index: usize,
    time_base: ffmpeg::Rational,
    sample_count: u64,
) -> Result<()> {
    let frame_size = encoder.frame_size() as usize;
    let channels = encoder.channels() as usize;

    if audio.samples.len() < frame_size * channels {
        return Ok(());
    }

    let mut frame = ffmpeg::frame::Audio::empty();
    // 必须设置格式和采样率
    frame.set_format(ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar));
    frame.set_rate(audio.sample_rate as u32);
    // 设置样本数量 (通过 unsafe 指针)
    unsafe {
        (*frame.as_mut_ptr()).nb_samples = frame_size as i32;
    }

    // 分配帧缓冲区 (使用对齐参数)
    const ALIGN: i32 = 0;  // 使用默认对齐
    unsafe {
        let ret = ffmpeg::sys::av_frame_get_buffer(frame.as_mut_ptr(), ALIGN);
        if ret < 0 {
            return Err(Error::FFmpeg(format!("Failed to allocate audio frame buffer: error {}", ret)));
        }
    }

    // 转换并复制数据到平面格式
    let mut samples_data: Vec<Vec<f32>> = vec![vec![0.0f32; frame_size]; channels];

    for ch in 0..channels {
        for i in 0..frame_size {
            let src_idx = i * channels + ch;
            if src_idx < audio.samples.len() {
                samples_data[ch][i] = audio.samples[src_idx];
            }
        }
    }

    // 复制数据到帧缓冲区
    unsafe {
        for (ch, channel_data) in samples_data.iter().enumerate() {
            let data_ptr = (*frame.as_mut_ptr()).data[ch];
            if data_ptr.is_null() {
                return Err(Error::FFmpeg(format!("Audio frame data[{}] pointer is null", ch)));
            }
            let dst_slice = std::slice::from_raw_parts_mut(data_ptr as *mut f32, frame_size);
            dst_slice.copy_from_slice(channel_data);
        }
    }

    frame.set_pts(Some(sample_count as i64));

    encoder.send_frame(&frame)
        .map_err(|e| Error::FFmpeg(format!("Audio encoding failed: {}", e)))?;

    let mut packet = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(stream_index);
        packet.rescale_ts(encoder.time_base(), time_base);
        packet.write(output)
            .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
    }

    Ok(())
}

/// 刷新视频编码器
fn flush_video_encoder(
    encoder: &mut ffmpeg::encoder::Video,
    output: &mut ffmpeg::format::context::Output,
    stream_index: usize,
    time_base: ffmpeg::Rational,
) -> Result<()> {
    encoder.send_eof()
        .map_err(|e| Error::FFmpeg(format!("Failed to send EOF to video encoder: {}", e)))?;

    let mut packet = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(stream_index);
        packet.rescale_ts(encoder.time_base(), time_base);
        packet.write(output)
            .map_err(|e| Error::FFmpeg(format!("Failed to write flush packet: {}", e)))?;
    }

    Ok(())
}

/// 刷新音频编码器
fn flush_audio_encoder(
    encoder: &mut ffmpeg::encoder::Audio,
    output: &mut ffmpeg::format::context::Output,
    stream_index: usize,
    time_base: ffmpeg::Rational,
) -> Result<()> {
    encoder.send_eof()
        .map_err(|e| Error::FFmpeg(format!("Failed to send EOF to audio encoder: {}", e)))?;

    let mut packet = ffmpeg::Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(stream_index);
        packet.rescale_ts(encoder.time_base(), time_base);
        packet.write(output)
            .map_err(|e| Error::FFmpeg(format!("Failed to write flush packet: {}", e)))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let h264 = H264Config::default();
        assert_eq!(h264.bitrate, 2_000_000);
        assert_eq!(h264.preset, H264Preset::Medium);
        assert_eq!(h264.crf, Some(23));

        let aac = AACConfig::default();
        assert_eq!(aac.bitrate, 128_000);
        assert_eq!(aac.sample_rate, 44_100);
        assert_eq!(aac.channels, 2);
    }
}
