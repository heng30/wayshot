use crate::{Error, Result};
use ffmpeg_next as ffmpeg;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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
            encode_mp4(config, video_receiver, audio_receiver)
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
    let video_stream = add_video_stream(&mut output, &config.h264, width, height, config.frame_rate)?;
    let video_stream_index = video_stream.index();

    // 添加音频流
    let audio_stream = add_audio_stream(&mut output, &config.aac)?;
    let audio_stream_index = audio_stream.index();

    // 写入头部
    output.write_header()
        .map_err(|e| Error::FFmpeg(format!("Failed to write header: {}", e)))?;

    // 获取编码器
    let mut video_encoder = {
        let ctx = video_stream.codec().encoder();
        ctx.video().map_err(|e| Error::FFmpeg(format!("Failed to get video encoder: {}", e)))?
    };

    let mut audio_encoder = {
        let ctx = audio_stream.codec().encoder();
        ctx.audio().map_err(|e| Error::FFmpeg(format!("Failed to get audio encoder: {}", e)))?
    };

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
    let mut rgb_frame = to_ffmpeg_rgb_frame(&first_frame)?;
    let mut yuv_frame = ffmpeg::frame::Video::empty();
    scaler.run(&rgb_frame, &mut yuv_frame)
        .map_err(|e| Error::FFmpeg(format!("Scaler failed: {}", e)))?;
    yuv_frame.set_pts(Some(0));

    encode_and_write_video(&mut video_encoder, &yuv_frame, &mut output, video_stream_index)?;

    let mut video_pts = 1i64;
    let mut audio_samples_written = 0u64;

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
                encode_and_write_video(&mut video_encoder, &yuv_frame, &mut output, video_stream_index)?;
                video_pts += 1;

                if video_pts % 30 == 0 {
                    log::debug!("Encoded {} video frames", video_pts);
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => video_done = true,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }

        // 处理音频
        match audio_receiver.try_recv() {
            Ok(audio) => {
                encode_and_write_audio(
                    &audio,
                    &mut audio_encoder,
                    &mut output,
                    audio_stream_index,
                    audio_samples_written,
                )?;
                audio_samples_written += (audio.samples.len() / audio.channels as usize) as u64;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => audio_done = true,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }

        if video_done && audio_done {
            break;
        }

        thread::sleep(Duration::from_millis(1));
    }

    // 刷新编码器
    log::debug!("Flushing encoders...");
    flush_video_encoder(&mut video_encoder, &mut output, video_stream_index)?;
    flush_audio_encoder(&mut audio_encoder, &mut output, audio_stream_index)?;

    // 写入尾部
    output.write_trailer()
        .map_err(|e| Error::FFmpeg(format!("Failed to write trailer: {}", e)))?;

    log::info!("MP4 encoding completed. Frames: {}", video_pts);

    Ok(())
}

/// 添加视频流
fn add_video_stream(
    output: &mut ffmpeg::format::context::Output,
    config: &H264Config,
    width: u32,
    height: u32,
    frame_rate: u32,
) -> Result<ffmpeg::StreamMut> {
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::H264)
        .ok_or_else(|| Error::FFmpeg("H264 encoder not found".to_string()))?;

    let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec);

    unsafe {
        let ctx = encoder.as_mut_ptr();
        if let Some(crf) = config.crf {
            ffmpeg::sys::av_opt_set_int(
                (*ctx).priv_data,
                b"crf\0".as_ptr() as *const _,
                crf as i64,
                0,
            );
        }
        ffmpeg::sys::av_opt_set(
            (*ctx).priv_data,
            b"preset\0".as_ptr() as *const _,
            config.preset.as_str().as_ptr() as *const _,
            0,
        );
    }

    let global_quality = if let Some(crf) = config.crf {
        ffmpeg::codec::flags::Flags::QSCALE as i32
    } else {
        0
    };

    encoder.set_bit_rate(config.bitrate as usize);
    encoder.set_width(width);
    encoder.set_height(height);
    encoder.set_time_base(ffmpeg::Rational(1, frame_rate as i32));
    encoder.set_frame_rate(ffmpeg::Rational(frame_rate as i32, 1));
    encoder.set_format(ffmpeg::format::Pixel::YUV420P);
    encoder.set_flags(global_quality);

    let stream = output.add_stream(encoder)
        .map_err(|e| Error::FFmpeg(format!("Failed to add video stream: {}", e)))?;

    Ok(stream)
}

/// 添加音频流
fn add_audio_stream(
    output: &mut ffmpeg::format::context::Output,
    config: &AACConfig,
) -> Result<ffmpeg::StreamMut> {
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::AAC)
        .ok_or_else(|| Error::FFmpeg("AAC encoder not found".to_string()))?;

    let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec);
    encoder.set_bit_rate(config.bitrate as usize);
    encoder.set_format(ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar));

    let stream = output.add_stream(encoder)
        .map_err(|e| Error::FFmpeg(format!("Failed to add audio stream: {}", e)))?;

    Ok(stream)
}

/// 转换为 FFmpeg RGB 帧
fn to_ffmpeg_rgb_frame(frame: &FrameData) -> Result<ffmpeg::frame::Video> {
    let mut ff_frame = ffmpeg::frame::Video::empty();
    ff_frame.set_width(frame.width);
    ff_frame.set_height(frame.height);

    let stride = (frame.width as usize) * 3;
    let data = &frame.data[..(stride * frame.height as usize)];
    ff_frame.data_mut(0).copy_from_slice(data);

    Ok(ff_frame)
}

/// 编码并写入视频
fn encode_and_write_video(
    encoder: &mut ffmpeg::encoder::Video,
    frame: &ffmpeg::frame::Video,
    output: &mut ffmpeg::format::context::Output,
    stream_index: usize,
) -> Result<()> {
    let mut packet = ffmpeg::Packet::empty();
    encoder.send_frame(frame, &mut packet)
        .map_err(|e| Error::FFmpeg(format!("Video encoding failed: {}", e)))?;

    if packet.size() > 0 {
        packet.set_stream(stream_index);
        packet.rescale_ts(encoder.time_base(), output.stream(stream_index).unwrap().time_base());
        packet.set_position(-1);
        output.write_interleaved_packet(&packet)
            .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
    }

    Ok(())
}

/// 编码并写入音频
fn encode_and_write_audio(
    audio: &AudioData,
    encoder: &mut ffmpeg::encoder::Audio,
    output: &mut ffmpeg::format::context::Output,
    stream_index: usize,
    sample_count: u64,
) -> Result<()> {
    let frame_size = encoder.frame_size() as usize;
    let channels = encoder.channels() as usize;

    if audio.samples.len() < frame_size * channels {
        return Ok(());
    }

    let mut frame = ffmpeg::frame::Audio::empty();
    let mut samples_data: Vec<Vec<f32>> = vec![vec![0.0f32; frame_size]; channels];

    for ch in 0..channels {
        for i in 0..frame_size {
            let src_idx = i * channels + ch;
            if src_idx < audio.samples.len() {
                samples_data[ch][i] = audio.samples[src_idx];
            }
        }
    }

    unsafe {
        for (ch, channel_data) in samples_data.iter().enumerate() {
            let ptr = channel_data.as_ptr() as *const _;
            (*frame.as_mut_ptr()).data[ch] = ptr as *mut _;
            (*frame.as_mut_ptr()).linesize[ch] = (frame_size * std::mem::size_of::<f32>()) as i32;
        }
        (*frame.as_mut_ptr()).nb_samples = frame_size as i32;
    }

    frame.set_pts(Some(sample_count as i64));

    let mut packet = ffmpeg::Packet::empty();
    encoder.send_frame(&frame, &mut packet)
        .map_err(|e| Error::FFmpeg(format!("Audio encoding failed: {}", e)))?;

    if packet.size() > 0 {
        packet.set_stream(stream_index);
        packet.rescale_ts(encoder.time_base(), output.stream(stream_index).unwrap().time_base());
        packet.set_position(-1);
        output.write_interleaved_packet(&packet)
            .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
    }

    Ok(())
}

/// 刷新视频编码器
fn flush_video_encoder(
    encoder: &mut ffmpeg::encoder::Video,
    output: &mut ffmpeg::format::context::Output,
    stream_index: usize,
) -> Result<()> {
    let mut packet = ffmpeg::Packet::empty();

    while encoder.send_frame_eof(&mut packet).is_ok() && packet.size() > 0 {
        packet.set_stream(stream_index);
        packet.rescale_ts(encoder.time_base(), output.stream(stream_index).unwrap().time_base());
        packet.set_position(-1);
        output.write_interleaved_packet(&packet)
            .map_err(|e| Error::FFmpeg(format!("Failed to write flush packet: {}", e)))?;
    }

    Ok(())
}

/// 刷新音频编码器
fn flush_audio_encoder(
    encoder: &mut ffmpeg::encoder::Audio,
    output: &mut ffmpeg::format::context::Output,
    stream_index: usize,
) -> Result<()> {
    let mut packet = ffmpeg::Packet::empty();

    while encoder.send_frame_eof(&mut packet).is_ok() && packet.size() > 0 {
        packet.set_stream(stream_index);
        packet.rescale_ts(encoder.time_base(), output.stream(stream_index).unwrap().time_base());
        packet.set_position(-1);
        output.write_interleaved_packet(&packet)
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
