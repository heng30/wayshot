use std::path::Path;

use anyhow::{Context, Result, anyhow};
use ffmpeg::format::Pixel;
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{Context as Scaler, flag::Flags};
use ffmpeg_next as ffmpeg;
use serde_json::{Value, json};

pub(crate) fn fps_of(rate: ffmpeg::Rational) -> f64 {
    if rate.numerator() > 0 && rate.denominator() > 0 {
        rate.numerator() as f64 / rate.denominator() as f64
    } else {
        0.0
    }
}
pub(crate) fn info(input: &Path) -> Result<Value> {
    let ictx = ffmpeg::format::input(&input)
        .with_context(|| format!("无法打开文件: {}", input.display()))?;

    let duration_secs = ictx.duration() as f64 / f64::from(ffmpeg::sys::AV_TIME_BASE);

    let mut streams = Vec::new();
    for stream in ictx.streams() {
        let params = stream.parameters();
        let medium = params.medium();
        let mut stream_json = json!({
            "index": stream.index(),
            "type": match medium {
                Type::Video => "video",
                Type::Audio => "audio",
                Type::Subtitle => "subtitle",
                _ => "other",
            },
            "codec": format!("{:?}", params.id()),
            "bit_rate": params.bit_rate(),
        });

        if medium == Type::Video {
            stream_json["fps"] = json!(fps_of(stream.avg_frame_rate()));
            if let Ok(ctx) = ffmpeg::codec::context::Context::from_parameters(params.clone())
                && let Ok(decoder) = ctx.decoder().video()
            {
                stream_json["width"] = json!(decoder.width());
                stream_json["height"] = json!(decoder.height());
                stream_json["pixel_format"] = json!(format!("{:?}", decoder.format()));
            }
        } else if medium == Type::Audio
            && let Ok(ctx) = ffmpeg::codec::context::Context::from_parameters(params.clone())
            && let Ok(decoder) = ctx.decoder().audio()
        {
            stream_json["sample_rate"] = json!(decoder.rate());
            stream_json["channels"] = json!(decoder.channels());
            stream_json["sample_format"] = json!(format!("{:?}", decoder.format()));
        }

        streams.push(stream_json);
    }

    Ok(json!({
        "path": input.display().to_string(),
        "duration_secs": duration_secs,
        "streams": streams,
    }))
}
pub(crate) struct VideoSource {
    pub(crate) ictx: ffmpeg::format::context::Input,
    pub(crate) decoder: ffmpeg::decoder::Video,
    stream_index: usize,
    time_base: ffmpeg::Rational,
    fps: f64,
    pub(crate) duration_secs: f64,
}
pub(crate) fn open_video(path: &Path) -> Result<VideoSource> {
    let ictx = ffmpeg::format::input(&path)
        .with_context(|| format!("无法打开视频: {}", path.display()))?;

    let duration_secs = ictx.duration() as f64 / f64::from(ffmpeg::sys::AV_TIME_BASE);

    let found = ictx
        .streams()
        .find(|s| s.parameters().medium() == Type::Video)
        .map(|s| {
            (
                s.index(),
                s.time_base(),
                fps_of(s.avg_frame_rate()),
                s.parameters().clone(),
            )
        })
        .ok_or_else(|| anyhow!("文件中没有视频流: {}", path.display()))?;

    let (stream_index, time_base, fps, params) = found;
    let decoder = ffmpeg::codec::context::Context::from_parameters(params)?
        .decoder()
        .video()
        .map_err(|e| anyhow!("无法创建视频解码器: {e}"))?;

    Ok(VideoSource {
        ictx,
        decoder,
        stream_index,
        time_base,
        fps,
        duration_secs,
    })
}
pub(crate) fn decode_video<F>(source: &mut VideoSource, emit: F) -> Result<()>
where
    F: FnMut(&ffmpeg::frame::Video, f64) -> Result<bool>,
{
    let VideoSource {
        ictx,
        decoder,
        stream_index,
        time_base,
        fps,
        ..
    } = source;
    let (stream_index, time_base, fps) = (*stream_index, *time_base, *fps);

    let mut scaler = Scaler::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        decoder.width(),
        decoder.height(),
        Flags::BILINEAR,
    )
    .map_err(|e| anyhow!("无法创建像素格式转换器: {e}"))?;

    let mut decoded = ffmpeg::frame::Video::empty();
    let mut rgb = ffmpeg::frame::Video::empty();
    let mut fallback_index: u64 = 0;
    let mut stopped = false;
    let mut emit = emit;

    let mut handle = |decoded: &ffmpeg::frame::Video,
                      rgb: &mut ffmpeg::frame::Video,
                      scaler: &mut Scaler,
                      emit: &mut F|
     -> Result<bool> {
        let ts = decoded
            .timestamp()
            .map(|ts| {
                ts as f64 * f64::from(time_base.numerator()) / f64::from(time_base.denominator())
            })
            .unwrap_or_else(|| {
                if fps > 0.0 {
                    fallback_index as f64 / fps
                } else {
                    fallback_index as f64
                }
            });
        fallback_index += 1;

        scaler
            .run(decoded, rgb)
            .map_err(|e| anyhow!("像素格式转换失败: {e}"))?;
        emit(rgb, ts)
    };

    for (stream, packet) in ictx.packets() {
        if stream.index() != stream_index {
            continue;
        }
        if decoder.send_packet(&packet).is_err() {
            continue;
        }
        while decoder.receive_frame(&mut decoded).is_ok() {
            if !handle(&decoded, &mut rgb, &mut scaler, &mut emit)? {
                stopped = true;
                break;
            }
        }
        if stopped {
            break;
        }
    }

    if !stopped {
        let _ = decoder.send_eof();
        while decoder.receive_frame(&mut decoded).is_ok() {
            if !handle(&decoded, &mut rgb, &mut scaler, &mut emit)? {
                break;
            }
        }
    }

    Ok(())
}
pub(crate) fn source_size(source: &VideoSource) -> (u32, u32) {
    (source.decoder.width(), source.decoder.height())
}
pub(crate) fn source_fps(source: &VideoSource) -> f64 {
    if source.fps > 0.0 { source.fps } else { 25.0 }
}
pub(crate) fn source_duration(source: &VideoSource) -> f64 {
    source.duration_secs
}
pub(crate) fn rgb_frame_to_image(frame: &ffmpeg::frame::Video) -> Result<image::RgbaImage> {
    let rgb = video_utils::ffmpeg::extract_rgb24(frame)?;
    Ok(video_utils::convert::rgb_into_rgba(rgb))
}
