use std::path::Path;

use anyhow::{Context, Result, anyhow};
use ffmpeg::media::Type;
use ffmpeg_next as ffmpeg;
use serde_json::{Value, json};

use crate::util;

pub(crate) fn decode_audio(input: &Path) -> Result<(u16, u32, Vec<f32>)> {
    let mut ictx = ffmpeg::format::input(&input)
        .with_context(|| format!("无法打开文件: {}", input.display()))?;

    let found = ictx
        .streams()
        .find(|s| s.parameters().medium() == Type::Audio)
        .map(|s| (s.index(), s.parameters().clone()))
        .ok_or_else(|| anyhow!("文件中没有音频流: {}", input.display()))?;
    let (stream_index, params) = found;

    let mut decoder = ffmpeg::codec::context::Context::from_parameters(params)?
        .decoder()
        .audio()
        .map_err(|e| anyhow!("无法创建音频解码器: {e}"))?;
    let src_rate = decoder.rate();
    let src_channels = decoder.channels();

    let mut samples = Vec::new();
    let mut decoded = ffmpeg::frame::Audio::empty();

    for (stream, packet) in ictx.packets() {
        if stream.index() != stream_index {
            continue;
        }
        if decoder.send_packet(&packet).is_err() {
            continue;
        }
        while decoder.receive_frame(&mut decoded).is_ok() {
            samples.extend(video_utils::ffmpeg::audio_frame_to_f32(&decoded));
        }
    }
    let _ = decoder.send_eof();
    while decoder.receive_frame(&mut decoded).is_ok() {
        samples.extend(video_utils::ffmpeg::audio_frame_to_f32(&decoded));
    }

    if samples.is_empty() {
        return Err(anyhow!("没有解码到任何音频数据"));
    }

    Ok((src_channels, src_rate, samples))
}
pub(crate) fn extract_audio(
    input: &Path,
    output: &Path,
    sample_rate: u32,
    channels: u16,
) -> Result<Value> {
    let (src_channels, src_rate, samples) = decode_audio(input)?;

    let samples = audio_utils::audio::resample_audio_with_channel(
        &samples,
        src_rate,
        src_channels,
        sample_rate,
        channels,
    )
    .map_err(|e| anyhow!("重采样失败: {e}"))?;

    util::write_wav(output, &samples, sample_rate, channels)?;

    let duration = samples.len() as f64 / (channels as f64 * sample_rate as f64);
    Ok(json!({
        "path": output.display().to_string(),
        "sample_rate": sample_rate,
        "channels": channels,
        "duration_secs": duration,
    }))
}
