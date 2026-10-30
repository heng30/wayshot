use anyhow::{Result, anyhow};
use image::RgbImage;
use ndarray::Array2;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::video::{decode_video, open_video, source_duration, source_fps, source_size};
use subtitle_remover::{Inpainter, inpaint::composite_back};

/// 逐帧编码成 mp4（可选 AAC 音轨），封装 mp4m + video_encoder 的最小外壳。
struct Encoder {
    video_encoder: Arc<Mutex<Option<Box<dyn video_encoder::VideoEncoder>>>>,
    h264_sender: mp4m::Sender<mp4m::VideoFrameType>,
    audio_sender: Option<mp4m::Sender<mp4m::AudioFrameType>>,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
}

impl Encoder {
    fn new(
        output: &Path,
        width: u32,
        height: u32,
        fps: u32,
        audio: Option<(u16, u32)>,
    ) -> Result<Self> {
        let mut processor = mp4m::Mp4Processor::new(
            mp4m::Mp4ProcessorConfigBuilder::default()
                .save_path(output.to_path_buf())
                .video_config(mp4m::VideoConfig { width, height, fps })
                .channel_size(1024)
                .build()
                .map_err(|e| anyhow!("mp4 配置失败: {e}"))?,
        );

        let audio_sender = match audio {
            Some((channels, sample_rate)) => Some(
                processor
                    .add_audio_track(mp4m::AudioConfig {
                        convert_to_mono: false,
                        spec: hound::WavSpec {
                            channels,
                            sample_rate,
                            bits_per_sample: 32,
                            sample_format: hound::SampleFormat::Float,
                        },
                    })
                    .map_err(|e| anyhow!("添加音轨失败: {e}"))?,
            ),
            None => None,
        };

        let mut encoder =
            video_encoder::new(video_encoder::VideoEncoderConfig::new(width, height).with_fps(fps))
                .map_err(|e| anyhow!("创建编码器失败: {e}"))?;
        let headers = encoder
            .headers()
            .map_err(|e| anyhow!("读取编码器头失败: {e}"))?;

        let h264_sender = processor.h264_sender();
        let thread = std::thread::spawn(move || {
            processor
                .run_processing_loop(Some(headers))
                .map_err(|e| e.to_string())
        });

        Ok(Self {
            video_encoder: Arc::new(Mutex::new(Some(encoder))),
            h264_sender,
            audio_sender,
            thread: Some(thread),
        })
    }

    fn frame(&mut self, frame: &RgbImage) -> Result<()> {
        let encoded = self
            .video_encoder
            .lock()
            .map_err(|_| anyhow!("编码器锁失败"))?
            .as_mut()
            .ok_or_else(|| anyhow!("编码器不可用"))?
            .encode_frame(frame.clone())
            .map_err(|e| anyhow!("编码帧失败: {e}"))?;

        if let video_encoder::EncodedFrame::Frame {
            data, is_keyframe, ..
        } = encoded
        {
            self.h264_sender
                .send(mp4m::VideoFrameType::Frame {
                    data,
                    is_sync: is_keyframe,
                })
                .map_err(|e| anyhow!("发送视频帧失败: {e}"))?;
        }

        Ok(())
    }

    fn audio(&self, samples: Vec<f32>) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        if let Some(sender) = &self.audio_sender {
            sender
                .send(mp4m::AudioFrameType::Samples(samples))
                .map_err(|e| anyhow!("发送音频失败: {e}"))?;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        let encoder = self
            .video_encoder
            .lock()
            .map_err(|_| anyhow!("编码器锁失败"))?
            .take()
            .ok_or_else(|| anyhow!("编码器不可用"))?;

        let flushed = Arc::new(Mutex::new(Vec::new()));
        let sink = flushed.clone();
        encoder
            .flush(Box::new(move |data, is_keyframe| {
                if let Ok(mut guard) = sink.lock() {
                    guard.push((data, is_keyframe));
                }
            }))
            .map_err(|e| anyhow!("刷新编码器失败: {e}"))?;

        if let Ok(mut guard) = flushed.lock() {
            for (data, is_keyframe) in guard.drain(..) {
                self.h264_sender
                    .send(mp4m::VideoFrameType::Frame {
                        data,
                        is_sync: is_keyframe,
                    })
                    .map_err(|e| anyhow!("发送尾帧失败: {e}"))?;
            }
        }

        if let Some(sender) = &self.audio_sender {
            let _ = sender.send(mp4m::AudioFrameType::End);
        }
        self.h264_sender
            .send(mp4m::VideoFrameType::End)
            .map_err(|e| anyhow!("结束视频流失败: {e}"))?;
        drop(self.h264_sender);
        drop(self.audio_sender.take());

        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| anyhow!("mp4 写盘线程异常退出"))?
                .map_err(|e| anyhow!("mp4 写盘失败: {e}"))?;
        }

        Ok(())
    }
}

/// 掩码的外接矩形，按 8 像素对齐（LaMa 要求）。
fn inpaint_area(mask: &Array2<u8>) -> Option<(i32, i32, i32, i32)> {
    let rows = mask.nrows();
    let cols = mask.ncols();
    let (mut ymin, mut ymax, mut xmin, mut xmax) = (rows as i32, 0i32, cols as i32, 0i32);
    let mut found = false;

    for r in 0..rows {
        for c in 0..cols {
            if mask[[r, c]] > 0 {
                found = true;
                ymin = ymin.min(r as i32);
                ymax = ymax.max(r as i32 + 1);
                xmin = xmin.min(c as i32);
                xmax = xmax.max(c as i32 + 1);
            }
        }
    }

    if !found {
        return None;
    }

    let align = 8;
    Some((
        (ymin / align) * align,
        ((ymax + align - 1) / align) * align,
        (xmin / align) * align,
        ((xmax + align - 1) / align) * align,
    ))
}

fn paste_area(canvas: &mut RgbImage, cropped: &RgbImage, area: (i32, i32, i32, i32)) {
    let (ymin, _, xmin, _) = area;
    for y in 0..cropped.height() {
        for x in 0..cropped.width() {
            let ty = y as i32 + ymin;
            let tx = x as i32 + xmin;
            if ty < 0 || tx < 0 || ty as u32 >= canvas.height() || tx as u32 >= canvas.width() {
                continue;
            }
            canvas.put_pixel(tx as u32, ty as u32, *cropped.get_pixel(x, y));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &Path,
    output: &Path,
    model: &Path,
    region: (u32, u32, u32, u32),
    start: f64,
    duration: Option<f64>,
    skip_inpaint: bool,
) -> Result<serde_json::Value> {
    let setup = Setup::new(input, output, model, region, start, duration, skip_inpaint)?;
    let mut setup = setup;
    let frames = setup.process()?;

    let bytes = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    Ok(serde_json::json!({
        "path": output.display().to_string(),
        "frames": frames,
        "width": setup.out_width,
        "height": setup.out_height,
        "fps": setup.fps as u32,
        "audio": setup.audio_format.is_some(),
        "bytes": bytes,
    }))
}

/// 一次擦除任务的全部准备状态（视频、掩码、模型、音轨、编码器）。
struct Setup {
    source: Option<crate::video::VideoSource>,
    mask: Array2<u8>,
    area: (i32, i32, i32, i32),
    inpainter: Option<subtitle_remover::LamaInpainter>,
    audio: Option<(u16, u32, Vec<f32>)>,
    audio_format: Option<(u16, u32)>,
    samples_per_frame: usize,
    audio_cursor: usize,
    encoder: Option<Encoder>,
    start: f64,
    end: f64,
    out_width: u32,
    out_height: u32,
    fps: f64,
}

impl Setup {
    #[allow(clippy::too_many_arguments)]
    fn new(
        input: &Path,
        output: &Path,
        model: &Path,
        region: (u32, u32, u32, u32),
        start: f64,
        duration: Option<f64>,
        skip_inpaint: bool,
    ) -> Result<Self> {
        let (x, y, width, height) = region;
        if width == 0 || height == 0 {
            return Err(anyhow!("擦除区域宽高必须大于 0"));
        }

        let source = open_video(input)?;
        let (src_width, src_height) = source_size(&source);
        if x + width > src_width || y + height > src_height {
            return Err(anyhow!(
                "擦除区域超出画面范围: 画面 {src_width}x{src_height}"
            ));
        }

        let fps = source_fps(&source);
        let end = duration
            .map(|d| start + d)
            .unwrap_or_else(|| source_duration(&source).max(start));

        let config = subtitle_remover::Config::default();
        let mask = subtitle_remover::create_mask(
            src_height as usize,
            src_width as usize,
            &[(x as i32, (x + width) as i32, y as i32, (y + height) as i32)],
            config.subtitle_area_deviation_pixel,
        );
        let area =
            inpaint_area(&mask).ok_or_else(|| anyhow!("擦除区域生成了空掩码，检查坐标与尺寸"))?;

        let inpainter = if skip_inpaint {
            None
        } else {
            Some(
                subtitle_remover::LamaInpainter::new(
                    model
                        .to_str()
                        .ok_or_else(|| anyhow!("模型路径不是有效 UTF-8"))?,
                    1,
                )
                .map_err(|e| anyhow!("加载 LaMa 模型失败: {e}"))?,
            )
        };

        // 用 ffmpeg 取音轨：symphonia 对 mp4m 写出的 mp4 偶发探测失败。
        let audio = crate::video::decode_audio(input)
            .ok()
            .filter(|(channels, rate, samples)| *channels > 0 && *rate > 0 && !samples.is_empty());
        let audio_format = audio.as_ref().map(|(channels, rate, _)| (*channels, *rate));

        let out_width = src_width - src_width % 2;
        let out_height = src_height - src_height % 2;
        let encoder = Encoder::new(output, out_width, out_height, fps as u32, audio_format)?;

        let samples_per_frame = audio
            .as_ref()
            .map(|(channels, rate, _)| (*rate as f64 / fps * *channels as f64).round() as usize)
            .unwrap_or(0);

        Ok(Self {
            source: Some(source),
            mask,
            area,
            inpainter,
            audio,
            audio_format,
            samples_per_frame,
            audio_cursor: 0,
            encoder: Some(encoder),
            start,
            end,
            out_width,
            out_height,
            fps,
        })
    }

    fn process(&mut self) -> Result<u64> {
        let mut frames = 0u64;
        let (start, end) = (self.start, self.end);
        let mut source = self
            .source
            .take()
            .ok_or_else(|| anyhow!("视频源已被消耗"))?;

        decode_video(&mut source, |rgb_frame, ts| {
            if ts + 1e-6 < start {
                return Ok(true);
            }
            if ts >= end {
                return Ok(false);
            }

            self.process_frame(rgb_frame)?;
            frames += 1;
            Ok(true)
        })?;

        self.encoder
            .take()
            .ok_or_else(|| anyhow!("编码器已被消耗"))?
            .finish()?;
        Ok(frames)
    }

    fn process_frame(&mut self, rgb_frame: &ffmpeg_next::frame::Video) -> Result<()> {
        let frame = video_utils::ffmpeg::extract_rgb24(rgb_frame)?;
        let mut canvas = frame.clone();

        if let Some(inpainter) = self.inpainter.as_mut() {
            let inpainted = inpainter
                .inpaint(&[frame.clone()], &self.mask, &self.area)
                .map_err(|e| anyhow!("修复失败: {e}"))?;
            if let Some(cropped) = inpainted.into_iter().next() {
                paste_area(&mut canvas, &cropped, self.area);
                composite_back(&mut canvas, &frame, &self.mask, &self.area);
            }
        }

        let frame = if (self.out_width, self.out_height) != (canvas.width(), canvas.height()) {
            image::imageops::crop_imm(&canvas, 0, 0, self.out_width, self.out_height).to_image()
        } else {
            canvas
        };

        self.encoder
            .as_mut()
            .ok_or_else(|| anyhow!("编码器不可用"))?
            .frame(&frame)?;
        self.push_audio()?;
        Ok(())
    }

    fn push_audio(&mut self) -> Result<()> {
        if let Some((_, _, samples)) = &self.audio {
            let end_index = (self.audio_cursor + self.samples_per_frame).min(samples.len());
            if self.audio_cursor < end_index {
                if let Some(encoder) = self.encoder.as_ref() {
                    encoder.audio(samples[self.audio_cursor..end_index].to_vec())?;
                }
            }
            self.audio_cursor = end_index;
        }
        Ok(())
    }
}
