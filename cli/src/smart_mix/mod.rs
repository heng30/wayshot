use std::path::Path;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

mod apply;
mod matching;
mod resolve;
mod transcribe;

use apply::apply_matches;
use matching::ai_match;
use resolve::{resolve_audio_entries, resolve_tracks, resolve_visuals};

const DEFAULT_VL_PROMPT: &str =
    "Describe the main subject and action in this image/video frame in one short sentence.";

#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum)]
pub enum PrecisionArg {
    Fp32,
    Fp16,
    Q4,
    Q8,
}

impl PrecisionArg {
    pub fn to_precision(self) -> Precision {
        match self {
            PrecisionArg::Fp32 => Precision::Fp32,
            PrecisionArg::Fp16 => Precision::Fp16,
            PrecisionArg::Q4 => Precision::Q4,
            PrecisionArg::Q8 => Precision::Q8,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Precision {
    Fp32,
    Fp16,
    Q4,
    Q8,
}

impl Precision {
    pub(super) fn to_lfm(self) -> lfm_vl_onnx::Precision {
        match self {
            Precision::Fp32 => lfm_vl_onnx::Precision::Fp32,
            Precision::Fp16 => lfm_vl_onnx::Precision::Fp16,
            Precision::Q4 => lfm_vl_onnx::Precision::Q4,
            Precision::Q8 => lfm_vl_onnx::Precision::Q8,
        }
    }
}
#[derive(Clone, Debug)]
struct AudioEntry {
    start_ms: u64,
    end_ms: u64,
    text: String,
}
#[derive(Clone, Debug)]
struct VisualItem {
    track_index: usize,
    segment_index: usize,
    is_image: bool,
    description: String,
    original_duration_ms: u64,
    duration_ms: u64,
}
#[derive(Clone, Copy, Debug)]
struct Match {
    audio_index: usize,
    visual_index: usize,
    source_offset_ms: u64,
    duration_ms: u64,
}
#[allow(clippy::too_many_arguments)]
pub fn run(
    project: &Path,
    audio_track: Option<usize>,
    visual_tracks: Option<&str>,
    audio_entries_json: Option<&str>,
    srt: Option<&Path>,
    asr: Option<(&Path, &Path)>,
    visuals_json: Option<&str>,
    vision: Option<(&Path, Precision)>,
    api: &crate::llm::ApiConfig,
    reuse_media: bool,
    sequential: bool,
    cover_all: bool,
    apply: bool,
) -> Result<Value> {
    let mut data = video_editor::project::load_project(project)?;
    let manager = data
        .inner
        .take()
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;

    let (mut manager, audio_track, visual_indices) =
        resolve_tracks(manager, audio_track, visual_tracks)?;

    let audio_entries = resolve_audio_entries(&manager, audio_track, audio_entries_json, srt, asr)?;
    let mut visuals = resolve_visuals(&manager, &visual_indices, visuals_json, vision)?;
    // 视觉片段按轨道、片段顺序稳定排序，保证 index 可复现
    visuals.sort_by_key(|v| (v.track_index, v.segment_index));

    let matches = ai_match(
        &audio_entries,
        &visuals,
        api,
        reuse_media,
        sequential,
        cover_all,
    )?;
    if matches.is_empty() {
        return Err(anyhow!("模型没有返回任何匹配结果"));
    }

    if apply {
        apply_matches(&mut manager, &visuals, &audio_entries, &matches)?;
        manager.update_duration();
    }
    data.inner = Some(manager);
    if apply {
        video_editor::project::save_project(&data, project)
            .map_err(|e| anyhow!("保存工程失败: {e}"))?;
    }

    Ok(json!({
        "path": project.display().to_string(),
        "audio_track": audio_track,
        "visual_tracks": visual_indices,
        "audio_segments": audio_entries.len(),
        "visual_segments": visuals.len(),
        "applied": apply,
        "matches": matches.iter().map(|m| match_json(m, &visuals)).collect::<Vec<_>>(),
    }))
}
fn match_json(item: &Match, visuals: &[VisualItem]) -> Value {
    json!({
        "audio_index": item.audio_index,
        "visual_index": item.visual_index,
        "source_offset_ms": item.source_offset_ms,
        "duration_ms": item.duration_ms,
        "visual": visuals.get(item.visual_index).map(|v| json!({
            "track": v.track_index,
            "segment": v.segment_index,
            "description": v.description,
            "is_image": v.is_image,
        })),
    })
}
