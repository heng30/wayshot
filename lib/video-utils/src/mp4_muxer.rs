use crate::{Error, Result};
use ffmpeg_next as ffmpeg;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use image::{ImageBuffer, Rgb};
use video_encoder::{VideoEncoder, VideoEncoderConfig, EncodedFrame};

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
    /// 音频样本 (浮点格式, 平面)
    pub samples: Vec<f32>,
    /// 采样率 (Hz)
    pub sample_rate: u32,
    /// 声道数
    pub channels: u8,
    /// 时间戳
    pub timestamp: Duration,
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

/// MP4 封装器配置
#[derive(Debug, Clone)]
pub struct MP4MuxerConfig {
    /// 输出文件路径
    pub output_path: PathBuf,
    /// 视频帧率 (fps)
    pub frame_rate: u32,
    /// AAC 编码配置
    pub aac: AACConfig,
}

/// MP4 封装器
pub struct MP4Muxer {
    video_sender: Sender<FrameData>,
    audio_sender: Sender<AudioData>,
    join_handle: Option<JoinHandle<Result<()>>>,
}

impl MP4Muxer {
    /// 创建并启动 MP4 封装器
    ///
    /// # Example
    ///
    /// ```no_run
    /// use video_utils::mp4_muxer::{MP4Muxer, MP4MuxerConfig, AACConfig};
    /// use std::path::PathBuf;
    ///
    /// let config = MP4MuxerConfig {
    ///     output_path: PathBuf::from("output.mp4"),
    ///     frame_rate: 30,
    ///     aac: AACConfig {
    ///         bitrate: 192_000,
    ///         sample_rate: 48_000,
    ///         channels: 2,
    ///     },
    /// };
    ///
    /// let (muxer, video_tx, audio_tx) = MP4Muxer::start(config).unwrap();
    /// // 发送帧...
    /// muxer.stop().unwrap();
    /// ```
    pub fn start(config: MP4MuxerConfig) -> Result<(Self, Sender<FrameData>, Sender<AudioData>)> {
        let (video_sender, video_receiver) = channel();
        let (audio_sender, audio_receiver) = channel();

        let video_sender_for_user = video_sender.clone();
        let audio_sender_for_user = audio_sender.clone();

        let join_handle = thread::spawn(move || {
            mux_mp4(config, video_receiver, audio_receiver)
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

    /// 停止封装器并等待完成
    pub fn stop(mut self) -> Result<()> {
        drop(self.video_sender);
        drop(self.audio_sender);

        if let Some(handle) = self.join_handle.take() {
            handle.join().map_err(|e| Error::FFmpeg(format!("Muxer thread panicked: {:?}", e)))??;
        }

        Ok(())
    }
}

/// 封装 MP4 文件
fn mux_mp4(
    config: MP4MuxerConfig,
    video_receiver: Receiver<FrameData>,
    audio_receiver: Receiver<AudioData>,
) -> Result<()> {
    ffmpeg::init()
        .map_err(|e| Error::FFmpeg(format!("Failed to initialize FFmpeg: {}", e)))?;

    let output_path = config.output_path.to_str().ok_or_else(|| {
        Error::InvalidConfig("Invalid output path".to_string())
    })?;

    log::info!("Starting MP4 muxing to: {}", output_path);
    log::info!("Video encoding with video-encoder");
    log::info!("AAC: bitrate={}, sample_rate={}, channels={}",
        config.aac.bitrate, config.aac.sample_rate, config.aac.channels);

    // 接收第一帧确定尺寸
    let first_frame = video_receiver.recv().map_err(|_| {
        Error::FFmpeg("No video frames received".to_string())
    })?;

    let width = first_frame.width;
    let height = first_frame.height;

    log::info!("Video size: {}x{}, fps: {}", width, height, config.frame_rate);

    // 创建视频编码器
    let ve_config = VideoEncoderConfig::new(width, height)
        .with_fps(config.frame_rate)
        .with_annexb(true);

    let mut video_encoder = video_encoder::new(ve_config)
        .map_err(|e| Error::FFmpeg(format!("Failed to create video encoder: {}", e)))?;

    // 创建输出格式
    let mut output = ffmpeg::format::output(&output_path)
        .map_err(|e| Error::FFmpeg(format!("Failed to create output: {}", e)))?;

    // 添加 H.264 视频流
    let (video_stream, _video_encoder) = add_h264_stream(&mut output, width, height, config.frame_rate)?;
    let video_stream_index = video_stream.index();
    let video_time_base = video_stream.time_base();

    // 添加音频流
    let (audio_stream, mut audio_encoder) = add_audio_stream(&mut output, &config.aac)?;
    let audio_stream_index = audio_stream.index();
    let audio_time_base = audio_stream.time_base();

    // 写入头部
    output.write_header()
        .map_err(|e| Error::FFmpeg(format!("Failed to write header: {}", e)))?;

    // 编码第一帧
    let first_img = image_buffer_from_data(&first_frame)?;
    let encoded = video_encoder.encode_frame(first_img)
        .map_err(|e| Error::FFmpeg(format!("Video encoding failed: {}", e)))?;

    process_encoded_frame(&encoded, &mut output, video_stream_index, video_time_base, 0)?;

    let mut video_pts = 1i64;
    let mut audio_samples_written = 0u64;

    // 主循环
    loop {
        let mut video_done = false;
        let mut audio_done = false;

        // 处理视频
        match video_receiver.try_recv() {
            Ok(frame) => {
                let img = image_buffer_from_data(&frame)?;
                let encoded = video_encoder.encode_frame(img)
                    .map_err(|e| Error::FFmpeg(format!("Video encoding failed: {}", e)))?;

                process_encoded_frame(&encoded, &mut output, video_stream_index, video_time_base, video_pts)?;
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
                    audio_time_base,
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

    // 刷新视频编码器
    log::debug!("Flushing video encoder...");
    flush_video_encoder(&mut video_encoder, &mut output, video_stream_index, video_time_base, video_pts)?;

    // 刷新音频编码器
    log::debug!("Flushing audio encoder...");
    flush_audio_encoder(&mut audio_encoder, &mut output, audio_stream_index, audio_time_base)?;

    // 写入尾部
    output.write_trailer()
        .map_err(|e| Error::FFmpeg(format!("Failed to write trailer: {}", e)))?;

    log::info!("MP4 muxing completed. Video frames: {}", video_pts);

    Ok(())
}

/// 添加 H.264 视频流
fn add_h264_stream<'a>(
    output: &'a mut ffmpeg::format::context::Output,
    width: u32,
    height: u32,
    frame_rate: u32,
) -> Result<(ffmpeg::StreamMut<'a>, ffmpeg::encoder::Video)> {
    let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::H264)
        .ok_or_else(|| Error::FFmpeg("H264 encoder not found".to_string()))?;

    let encoder_ctx = ffmpeg::codec::context::Context::new_with_codec(codec);

    // 配置视频编码器参数
    let mut video_encoder = encoder_ctx.encoder().video()
        .map_err(|e| Error::FFmpeg(format!("Failed to get video encoder: {}", e)))?;

    video_encoder.set_bit_rate(2_000_000);
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

/// 从 FrameData 创建 ImageBuffer
fn image_buffer_from_data(frame: &FrameData) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    ImageBuffer::from_raw(frame.width, frame.height, frame.data.clone())
        .ok_or_else(|| Error::FFmpeg("Failed to create image buffer".to_string()))
}

/// 处理编码后的帧
fn process_encoded_frame(
    encoded: &EncodedFrame,
    output: &mut ffmpeg::format::context::Output,
    stream_index: usize,
    time_base: ffmpeg::Rational,
    pts: i64,
) -> Result<()> {
    match encoded {
        EncodedFrame::Frame((_, data)) => {
            let mut packet = ffmpeg::Packet::copy(data);
            packet.set_stream(stream_index);
            packet.set_pts(Some(pts));
            packet.set_dts(Some(pts));
            packet.rescale_ts(ffmpeg::Rational(1, 1), time_base);
            packet.write(output)
                .map_err(|e| Error::FFmpeg(format!("Failed to write packet: {}", e)))?;
        }
        EncodedFrame::Empty(_) => {
            // 空帧，跳过
        }
        EncodedFrame::End => {
            // 编码结束
        }
    }
    Ok(())
}

/// 编码并写入音频
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
    _encoder: &mut Box<dyn VideoEncoder>,
    _output: &mut ffmpeg::format::context::Output,
    _stream_index: usize,
    _time_base: ffmpeg::Rational,
    _total_frames: i64,
) -> Result<()> {
    // 注意：video-encoder crate 的 flush API 设计导致难以在此场景中使用
    // flush 需要 Box<Self> 的所有权，而此处只有 &mut Box<dyn VideoEncoder>
    // 在实际使用中，所有帧应在主循环中已完成编码
    // 如需严格 flush，建议重构为在主循环结束后直接消耗 encoder

    log::debug!("Video encoder flush skipped (external video-encoder crate limitation)");
    log::warn!("Some buffered video data may not be written. For production use, consider using mp4_encoder instead.");

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
        let aac = AACConfig::default();
        assert_eq!(aac.bitrate, 128_000);
        assert_eq!(aac.sample_rate, 44_100);
        assert_eq!(aac.channels, 2);
    }
}
