use std::path::Path;

use anyhow::{Context, Result, anyhow};
use ffmpeg_next as ffmpeg;
use serde_json::{Value, json};

use super::probe::{decode_video, open_video, rgb_frame_to_image};
use crate::util;

#[allow(clippy::too_many_arguments)]
pub(crate) fn frames(
    input: &Path,
    output_dir: &Path,
    every_secs: f64,
    fps: Option<f64>,
    count: Option<u32>,
    start: f64,
    duration: Option<f64>,
    max_frames: u32,
) -> Result<Value> {
    let mut source = open_video(input)?;

    let end = duration
        .map(|d| start + d)
        .unwrap_or(source.duration_secs.max(start));
    let interval = match (fps, count) {
        (Some(fps), _) if fps > 0.0 => 1.0 / fps,
        (_, Some(count)) if count > 0 => ((end - start) / count as f64).max(0.001),
        _ => every_secs.max(0.001),
    };

    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("无法创建目录: {}", output_dir.display()))?;

    let prefix = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("frame")
        .to_string();

    let mut paths = Vec::new();
    let mut next_at = start;

    decode_video(&mut source, |rgb, ts| {
        if ts + 1e-6 < start {
            return Ok(true);
        }
        if ts >= end {
            return Ok(false);
        }
        if ts + 1e-6 < next_at {
            return Ok(true);
        }
        next_at = ts + interval;

        let img = rgb_frame_to_image(rgb)?;
        let path = output_dir.join(format!("{}_{:05}.png", prefix, paths.len()));
        util::save_rgba(&img, &path)?;
        paths.push(path);

        Ok(paths.len() < max_frames as usize)
    })?;

    let files: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    Ok(json!({
        "count": files.len(),
        "interval_secs": interval,
        "files": files,
        "truncated": files.len() >= max_frames as usize,
    }))
}
pub(crate) fn thumbnail(input: &Path, output: &Path, at: f64) -> Result<Value> {
    let mut source = open_video(input)?;
    if at > 0.0 {
        let ts = (at * ffmpeg::sys::AV_TIME_BASE as f64) as i64;
        source.ictx.seek(ts, ..).ok();
        source.decoder.flush();
    }

    let mut saved = None;
    decode_video(&mut source, |rgb, ts| {
        if ts + 0.05 < at {
            return Ok(true);
        }
        let img = rgb_frame_to_image(rgb)?;
        util::save_rgba(&img, output)?;
        saved = Some(ts);
        Ok(false)
    })?;

    match saved {
        Some(ts) => {
            let mut value = util::image_json(output, &util::load_rgba(output)?);
            value["timestamp_secs"] = json!(ts);
            Ok(value)
        }
        None => Err(anyhow!("在 {at}s 之后没有解码到任何视频帧")),
    }
}
