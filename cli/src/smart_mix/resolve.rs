use std::path::Path;

use anyhow::{Result, anyhow};
use serde_json::Value;
use video_editor::tracks::{Manager, Track, segment::Segment};

use super::transcribe::transcribe_track;
use super::{AudioEntry, DEFAULT_VL_PROMPT, Precision, VisualItem};

pub(super) fn resolve_tracks(
    manager: Manager,
    audio_track: Option<usize>,
    visual_tracks: Option<&str>,
) -> Result<(Manager, usize, Vec<usize>)> {
    let audio_track = match audio_track {
        Some(index) => index,
        None => manager
            .tracks
            .iter()
            .position(|track| matches!(track, Track::Audio(_)))
            .ok_or_else(|| anyhow!("工程里没有音频轨道，请用 --audio-track 指定"))?,
    };
    if !matches!(manager.tracks.get(audio_track), Some(Track::Audio(_))) {
        return Err(anyhow!("轨道 {audio_track} 不是音频轨道"));
    }

    let visual_indices = match visual_tracks {
        Some(list) => list
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<usize>()
                    .map_err(|_| anyhow!("--visual-tracks 解析失败: {s}"))
            })
            .collect::<Result<Vec<_>>>()?,
        None => visual_track_indices(&manager),
    };
    if visual_indices.is_empty() {
        return Err(anyhow!("工程里没有视频/图片轨道可混剪"));
    }

    Ok((manager, audio_track, visual_indices))
}
pub(super) fn visual_track_indices(manager: &Manager) -> Vec<usize> {
    manager
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| matches!(track, Track::Video(_) | Track::Image(_)))
        .map(|(index, _)| index)
        .collect()
}
pub(super) fn resolve_audio_entries(
    manager: &Manager,
    audio_track: usize,
    audio_entries_json: Option<&str>,
    srt: Option<&Path>,
    asr: Option<(&Path, &Path)>,
) -> Result<Vec<AudioEntry>> {
    let entries = match (audio_entries_json, srt) {
        (Some(json_text), _) => parse_audio_entries(json_text)?,
        (None, Some(srt_path)) => audio_entries_from_srt(srt_path)?,
        (None, None) => {
            let (weights, tokenizer) = asr.ok_or_else(|| {
                anyhow!("需要 --audio-entries / --srt，或 --asr-weights + --asr-tokenizer")
            })?;
            transcribe_track(manager, audio_track, weights, tokenizer)?
        }
    };

    if entries.is_empty() {
        return Err(anyhow!("没有可用的音频片段"));
    }
    Ok(entries)
}
pub(super) fn resolve_visuals(
    manager: &Manager,
    visual_indices: &[usize],
    visuals_json: Option<&str>,
    vision: Option<(&Path, Precision)>,
) -> Result<Vec<VisualItem>> {
    let visuals = match visuals_json {
        Some(json_text) => parse_visuals(json_text, manager, visual_indices)?,
        None => {
            let (model_dir, precision) = vision
                .ok_or_else(|| anyhow!("需要 --visuals，或用 --vision-model 自动识别画面内容"))?;
            describe_visuals(manager, visual_indices, model_dir, precision)?
        }
    };

    if visuals.is_empty() {
        return Err(anyhow!("没有可用的画面片段"));
    }
    Ok(visuals)
}
pub(super) fn parse_audio_entries(json_text: &str) -> Result<Vec<AudioEntry>> {
    let value: Value =
        serde_json::from_str(json_text).map_err(|e| anyhow!("--audio-entries 解析失败: {e}"))?;
    let items = value
        .as_array()
        .ok_or_else(|| anyhow!("--audio-entries 需要是数组"))?;
    Ok(items
        .iter()
        .map(|item| {
            Ok(AudioEntry {
                start_ms: item["start_ms"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("条目缺少 start_ms"))?,
                end_ms: item["end_ms"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("条目缺少 end_ms"))?,
                text: item["text"].as_str().unwrap_or_default().to_string(),
            })
        })
        .collect::<Result<Vec<_>>>()?)
}
pub(super) fn audio_entries_from_srt(path: &Path) -> Result<Vec<AudioEntry>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("无法读取字幕 {}: {e}", path.display()))?;
    let normalized = content.replace("\r\n", "\n");
    let mut entries = Vec::new();

    for block in normalized.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        let mut lines = block.lines();
        let mut timeline = lines.next().unwrap_or("").trim();
        if timeline.parse::<u32>().is_ok() {
            timeline = lines.next().unwrap_or("").trim();
        }
        let Some((start, end)) = timeline.split_once("-->") else {
            continue;
        };
        let start_ms = video_utils::subtitle::srt_timestamp_to_ms(start.trim())
            .map_err(|e| anyhow!("字幕时间轴解析失败: {e}"))?;
        let end_ms = video_utils::subtitle::srt_timestamp_to_ms(end.trim())
            .map_err(|e| anyhow!("字幕时间轴解析失败: {e}"))?;
        entries.push(AudioEntry {
            start_ms,
            end_ms,
            text: lines.collect::<Vec<_>>().join(" "),
        });
    }

    Ok(entries)
}
pub(super) fn parse_visuals(
    json_text: &str,
    manager: &Manager,
    allowed_tracks: &[usize],
) -> Result<Vec<VisualItem>> {
    let value: Value =
        serde_json::from_str(json_text).map_err(|e| anyhow!("--visuals 解析失败: {e}"))?;
    let items = value
        .as_array()
        .ok_or_else(|| anyhow!("--visuals 需要是数组"))?;

    let mut visuals = Vec::new();
    for item in items {
        let track_index = item["track"]
            .as_u64()
            .ok_or_else(|| anyhow!("条目缺少 track"))? as usize;
        let segment_index = item["segment"].as_u64().unwrap_or(0) as usize;
        if !allowed_tracks.contains(&track_index) {
            return Err(anyhow!("--visuals 里的轨道 {track_index} 不在混剪范围内"));
        }
        visuals.push(build_visual_item(
            manager,
            track_index,
            segment_index,
            item["description"].as_str().unwrap_or_default().to_string(),
        )?);
    }

    Ok(visuals)
}
pub(super) fn build_visual_item(
    manager: &Manager,
    track_index: usize,
    segment_index: usize,
    description: String,
) -> Result<VisualItem> {
    let track = manager
        .tracks
        .get(track_index)
        .ok_or_else(|| anyhow!("轨道索引越界: {track_index}"))?;
    let segment = track
        .segments()
        .get(segment_index)
        .ok_or_else(|| anyhow!("轨道 {track_index} 片段索引越界: {segment_index}"))?;

    Ok(VisualItem {
        track_index,
        segment_index,
        is_image: matches!(track, Track::Image(_)),
        description,
        original_duration_ms: segment.original_duration.as_millis() as u64,
        duration_ms: segment.duration.as_millis() as u64,
    })
}
pub(super) fn describe_visuals(
    manager: &Manager,
    visual_indices: &[usize],
    model_dir: &Path,
    precision: Precision,
) -> Result<Vec<VisualItem>> {
    let mut model = lfm_vl_onnx::LfmVlModel::load(model_dir, precision.to_lfm())
        .map_err(|e| anyhow!("加载视觉模型失败: {e}"))?;
    let tokenizer = lfm_vl_onnx::LfmTokenizer::from_file(&model_dir.join("tokenizer.json"))
        .map_err(|e| anyhow!("加载 tokenizer 失败: {e}"))?;

    let mut visuals = Vec::new();
    for &track_index in visual_indices {
        let Some(track) = manager.tracks.get(track_index) else {
            continue;
        };
        for (segment_index, segment) in track.segments().iter().enumerate() {
            let image = match representative_image(segment) {
                Ok(image) => image,
                Err(_) => continue,
            };
            let description =
                lfm_vl_onnx::generate(&mut model, &tokenizer, &image, DEFAULT_VL_PROMPT, 128)
                    .unwrap_or_default();
            visuals.push(VisualItem {
                track_index,
                segment_index,
                is_image: matches!(track, Track::Image(_)),
                description,
                original_duration_ms: segment.original_duration.as_millis() as u64,
                duration_ms: segment.duration.as_millis() as u64,
            });
        }
    }

    Ok(visuals)
}
pub(super) fn representative_image(segment: &Segment) -> Result<image::DynamicImage> {
    if segment.metadata.is_image() {
        return image::open(&segment.metadata.path)
            .map_err(|e| anyhow!("无法打开图片 {}: {e}", segment.metadata.path.display()));
    }

    let frames = segment
        .extract_video(segment.source_offset.as_secs_f64().max(0.0) as usize, 1)
        .map_err(|e| anyhow!("取帧失败: {e}"))?;
    for frame in frames {
        if let video_editor::tracks::video_frame_cache::VideoImage::Image { buffer } = frame {
            return Ok(image::DynamicImage::ImageRgba8(buffer));
        }
    }

    Err(anyhow!("取不到代表帧"))
}
