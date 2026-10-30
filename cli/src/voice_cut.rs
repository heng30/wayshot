use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::path::Path;
use video_editor::commands::{
    HistoryManager,
    batch::BatchCommand,
    segment::{RemoveSegmentCommand, SplitSegmentCommand},
};

const SPLIT_PROMPT: &str = r#"You are a voice clip assistant. Given a list of transcribed speech segments from a voiceover, decide which segments should be kept and which should be removed.

Remove segments that are:
- Filler words (um, uh, ah, etc.)
- Repetitions or stuttering
- Off-topic or irrelevant content
- False starts or incomplete sentences

Keep segments that contain meaningful, coherent content.

Only output the JSON array, no additional text.

<Input format>
[{"index": 0, "text": "segment text"}, {"index": 1, "text": "segment text"}, ...]
</Input format>

<Output format>
[{"index": 0, "keep": true}, {"index": 1, "keep": false}, ...]
</Output format>
"#;

const REFERENCE_PROMPT: &str = r#"You are a voice clip assistant. The user has provided a reference text (e.g., a script or outline). Given a list of transcribed speech segments, decide which segments should be kept based on whether they match the reference content.

Remove segments that do not appear in or support the reference text.

Only output the JSON array, no additional text.

<Input format>
[{"index": 0, "text": "segment text"}, {"index": 1, "text": "segment text"}, ...]
</Input format>

<Output format>
[{"index": 0, "keep": true}, {"index": 1, "keep": false}, ...]
</Output format>
"#;

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub start_ms: u64,
    pub end_ms: u64,
    pub keep: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    project: &Path,
    track: usize,
    segment: usize,
    entries_json: Option<&str>,
    asr: Option<(&Path, &Path)>,
    api: &crate::llm::ApiConfig,
    reference: Option<&str>,
    vad_threshold: f32,
    max_tokens: u32,
    apply: bool,
) -> Result<Value> {
    let mut data = video_editor::project::load_project(project)?;
    let mut manager = data
        .inner
        .take()
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;

    let target_segment = segment_of(&manager, track, segment)?;
    let source_path = target_segment.metadata.path.clone();
    let segment_duration_ms = target_segment.duration.as_millis() as u64;

    let (entries, origin) = resolve_entries(
        entries_json,
        asr,
        &source_path,
        api,
        reference,
        vad_threshold,
        max_tokens,
    )?;
    if entries.is_empty() {
        return Err(anyhow!("没有任何语音片段，无法剪辑"));
    }

    let plan = build_plan(&entries, segment_duration_ms);
    let kept_ms: u64 = plan
        .iter()
        .filter(|(_, _, keep)| *keep)
        .map(|(start, end, _)| end - start)
        .sum();

    if apply {
        apply_plan(&mut manager, track, segment, &plan)?;
        manager.update_duration();
        data.inner = Some(manager);
        video_editor::project::save_project(&data, project)
            .map_err(|e| anyhow!("保存工程失败: {e}"))?;
    } else {
        data.inner = Some(manager);
    }

    let mut value = json!({
        "path": project.display().to_string(),
        "origin": origin,
        "source": source_path.display().to_string(),
        "segment_duration_ms": segment_duration_ms,
        "kept_ms": kept_ms,
        "applied": apply,
        "entries": entries.iter().map(entry_json).collect::<Vec<_>>(),
    });
    if !apply {
        value["cut_plan"] = json!(
            plan.iter()
                .map(|(start, end, keep)| json!({
                    "start_ms": start,
                    "end_ms": end,
                    "keep": keep,
                }))
                .collect::<Vec<_>>()
        );
    }

    Ok(value)
}

fn entry_json(entry: &Entry) -> Value {
    json!({
        "start_ms": entry.start_ms,
        "end_ms": entry.end_ms,
        "keep": entry.keep,
    })
}

fn segment_of(
    manager: &video_editor::tracks::Manager,
    track: usize,
    segment: usize,
) -> Result<std::sync::Arc<video_editor::tracks::segment::Segment>> {
    manager
        .tracks
        .get(track)
        .ok_or_else(|| anyhow!("轨道索引越界: {track}"))?
        .segments()
        .get(segment)
        .cloned()
        .ok_or_else(|| anyhow!("片段索引越界: {segment}"))
}

#[allow(clippy::too_many_arguments)]
fn resolve_entries(
    entries_json: Option<&str>,
    asr: Option<(&Path, &Path)>,
    source_path: &Path,
    api: &crate::llm::ApiConfig,
    reference: Option<&str>,
    vad_threshold: f32,
    max_tokens: u32,
) -> Result<(Vec<Entry>, &'static str)> {
    if let Some(json_text) = entries_json {
        return Ok((parse_entries(json_text)?, "entries"));
    }

    let (weights, tokenizer) = asr.ok_or_else(|| {
        anyhow!("需要 --entries，或用 --asr-weights/--asr-tokenizer + LLM 参数自动生成")
    })?;
    let entries = decide_with_ai(
        source_path,
        weights,
        tokenizer,
        api,
        reference,
        vad_threshold,
        max_tokens,
    )?;
    Ok((entries, "ai"))
}

fn parse_entries(json_text: &str) -> Result<Vec<Entry>> {
    let value: Value =
        serde_json::from_str(json_text).map_err(|e| anyhow!("--entries 解析失败: {e}"))?;
    let items = value
        .as_array()
        .ok_or_else(|| anyhow!("--entries 需要是数组"))?;

    items
        .iter()
        .map(|item| {
            Ok(Entry {
                start_ms: item["start_ms"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("条目缺少 start_ms"))?,
                end_ms: item["end_ms"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("条目缺少 end_ms"))?,
                keep: item["keep"].as_bool().unwrap_or(true),
            })
        })
        .collect()
}

/// 与 GUI 一致：以所有条目的起止点为边界切片，只有完整落在 kept 条目内的片段才保留。
fn build_plan(entries: &[Entry], duration_ms: u64) -> Vec<(u64, u64, bool)> {
    let mut boundaries: Vec<u64> = vec![0];
    for entry in entries {
        if entry.start_ms > 0 && entry.start_ms < duration_ms {
            boundaries.push(entry.start_ms);
        }
        if entry.end_ms > 0 && entry.end_ms < duration_ms {
            boundaries.push(entry.end_ms);
        }
    }
    boundaries.push(duration_ms);
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut plan = Vec::new();
    for pair in boundaries.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        if start >= end {
            continue;
        }
        let keep = entries
            .iter()
            .any(|e| e.keep && start >= e.start_ms && end <= e.end_ms);
        plan.push((start, end, keep));
    }
    plan
}

fn apply_plan(
    manager: &mut video_editor::tracks::Manager,
    track: usize,
    segment: usize,
    plan: &[(u64, u64, bool)],
) -> Result<()> {
    let mut history = HistoryManager::new();
    let mut batch = BatchCommand::new("smart voice clip".to_string());

    // 在除末尾以外的每个边界处切分（逆序，保持 index 稳定）。
    for (start, _, _) in plan.iter().skip(1).rev() {
        let split = std::time::Duration::from_millis(*start);
        if split > std::time::Duration::ZERO {
            batch.add_command(Box::new(SplitSegmentCommand::new(track, segment, split)));
        }
    }

    // 逆序删除不保留的切片（shift_timeline 收紧时间轴）。
    for (index, (_, _, keep)) in plan.iter().enumerate().rev() {
        if !keep {
            batch.add_command(Box::new(RemoveSegmentCommand::new(
                track,
                segment + index,
                true,
            )));
        }
    }

    history
        .execute(manager, Box::new(batch))
        .map_err(|e| anyhow!("应用剪辑失败: {e}"))?;

    Ok(())
}

/// VAD 切分语音段 → FunASR 转写 → LLM 判断保留/删除。
fn decide_with_ai(
    source: &Path,
    weights: &Path,
    tokenizer: &Path,
    api: &crate::llm::ApiConfig,
    reference: Option<&str>,
    vad_threshold: f32,
    max_tokens: u32,
) -> Result<Vec<Entry>> {
    let regions = transcribe_regions(source, weights, tokenizer, vad_threshold, max_tokens)?;
    decide_keep_flags(&regions, api, reference)
}

/// VAD 切分 + 逐段 ASR，返回 (start_ms, end_ms, text)。
fn transcribe_regions(
    source: &Path,
    weights: &Path,
    tokenizer: &Path,
    vad_threshold: f32,
    max_tokens: u32,
) -> Result<Vec<(u64, u64, String)>> {
    let (channels, rate, samples) = crate::video::decode_audio(source)?;
    let samples =
        audio_utils::audio::resample_audio_with_channel(&samples, rate, channels, 16_000, 1)
            .map_err(|e| anyhow!("重采样失败: {e}"))?;

    let vad_config = audio_utils::vad::VadConfig::default()
        .with_sample_rate(16_000)
        .with_speech_threshold(vad_threshold);
    let segments = audio_utils::vad::detect_speech_segments(&samples, &vad_config);
    if segments.is_empty() {
        return Ok(Vec::new());
    }

    let mut model = load_asr(weights, tokenizer)?;
    let mut regions = Vec::with_capacity(segments.len());
    for segment in &segments {
        let text = transcribe_segment(&mut model, &segment.audio_data, max_tokens);
        regions.push((
            (segment.start_sample as f64 / 16_000.0 * 1000.0) as u64,
            (segment.end_sample as f64 / 16_000.0 * 1000.0) as u64,
            text,
        ));
    }
    Ok(regions)
}

fn load_asr(weights: &Path, tokenizer: &Path) -> Result<fun_ast_nano::FunAsrNanoGenerateModel> {
    let config = fun_ast_nano::FunASRModelConfig::default()
        .with_model_weights(
            weights
                .to_str()
                .ok_or_else(|| anyhow!("ASR 权重路径不是有效 UTF-8"))?
                .to_string(),
        )
        .with_tokenizer_path(
            tokenizer
                .to_str()
                .ok_or_else(|| anyhow!("ASR tokenizer 路径不是有效 UTF-8"))?
                .to_string(),
        );
    fun_ast_nano::FunAsrNanoGenerateModel::new(config, None, None)
        .map_err(|e| anyhow!("加载 ASR 模型失败: {e}"))
}

fn transcribe_segment(
    model: &mut fun_ast_nano::FunAsrNanoGenerateModel,
    samples: &[f32],
    max_tokens: u32,
) -> String {
    let audio = audio_utils::loader::AudioConfig::default()
        .with_sample_rate(16_000)
        .with_channel(1)
        .with_duration(std::time::Duration::from_secs_f64(
            samples.len() as f64 / 16_000.0,
        ))
        .with_samples(samples.to_vec());

    let request = fun_ast_nano::TranscriptionRequest::default()
        .with_audio_config(audio)
        .with_max_tokens(max_tokens);
    model
        .generate(request, None, |_| Ok(()))
        .map(|response| fast2s::convert(&response.text))
        .unwrap_or_default()
}

fn decide_keep_flags(
    regions: &[(u64, u64, String)],
    api: &crate::llm::ApiConfig,
    reference: Option<&str>,
) -> Result<Vec<Entry>> {
    let input: Vec<Value> = regions
        .iter()
        .enumerate()
        .map(|(index, (_, _, text))| json!({ "index": index, "text": text }))
        .collect();

    let prompt = match reference {
        Some(reference) => format!("{REFERENCE_PROMPT}\n<Reference text>\n{reference}\n"),
        None => SPLIT_PROMPT.to_string(),
    };

    let response = crate::llm::chat(&prompt, &serde_json::to_string(&input)?, api)?;
    let trimmed = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let decisions: Value = serde_json::from_str(trimmed)
        .map_err(|e| anyhow!("解析模型返回失败: {e}。原始返回: {trimmed}"))?;
    let decisions = decisions
        .as_array()
        .ok_or_else(|| anyhow!("模型返回不是数组"))?;

    let mut keep_flags = vec![true; regions.len()];
    for decision in decisions {
        if let Some(index) = decision["index"].as_u64()
            && let Some(slot) = keep_flags.get_mut(index as usize)
        {
            *slot = decision["keep"].as_bool().unwrap_or(true);
        }
    }

    Ok(regions
        .iter()
        .zip(keep_flags)
        .map(|((start, end, _), keep)| Entry {
            start_ms: *start,
            end_ms: *end,
            keep,
        })
        .collect())
}
