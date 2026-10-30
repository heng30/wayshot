use std::path::Path;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use video_editor::{
    project::{load_project, save_project},
    tracks::{Manager, Track},
};

fn subtitle_entries(
    manager: &Manager,
) -> Vec<(usize, video_editor::filters::traits::SubtitleEntry)> {
    let mut entries = Vec::new();
    for (index, track) in manager.tracks.iter().enumerate() {
        if let Track::Subtitle(subtitle_track) = track {
            for entry in subtitle_track.get_subtitle_entries() {
                entries.push((index, entry));
            }
        }
    }
    entries.sort_by_key(|(_, entry)| entry.start);
    entries
}
fn subtitles(project: &Path) -> Result<Value> {
    let data = load_project(project)?;
    let manager = data
        .inner
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;

    let entries: Vec<Value> = subtitle_entries(&manager)
        .into_iter()
        .map(|(track, entry)| {
            json!({
                "track": track,
                "start_secs": entry.start.as_secs_f64(),
                "end_secs": entry.end.as_secs_f64(),
                "text": entry.text,
            })
        })
        .collect();

    Ok(json!({ "path": project.display().to_string(), "count": entries.len(), "entries": entries }))
}
fn subtitle_search(project: &Path, keyword: &str) -> Result<Value> {
    let data = load_project(project)?;
    let manager = data
        .inner
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;

    let keyword_lower = keyword.to_lowercase();
    let hits: Vec<Value> = subtitle_entries(&manager)
        .into_iter()
        .filter(|(_, entry)| entry.text.to_lowercase().contains(&keyword_lower))
        .map(|(track, entry)| {
            json!({
                "track": track,
                "start_secs": entry.start.as_secs_f64(),
                "end_secs": entry.end.as_secs_f64(),
                "text": entry.text,
            })
        })
        .collect();

    Ok(json!({
        "path": project.display().to_string(),
        "keyword": keyword,
        "count": hits.len(),
        "hits": hits,
    }))
}
fn chapter_summary(
    project: &Path,
    api: &crate::llm::ApiConfig,
    extra_prompt: &str,
    save: bool,
) -> Result<Value> {
    let mut data = load_project(project)?;
    let manager = data
        .inner
        .as_ref()
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?
        .clone();

    let entries = subtitle_entries(&manager);
    if entries.is_empty() {
        return Err(anyhow!(
            "工程里没有字幕条目，先用 `transcribe --srt` 或字幕轨道生成字幕"
        ));
    }

    let question = serde_json::to_string(&chapter_input(&entries, manager.duration))?;
    let response = crate::llm::chat(&chapter_prompt(extra_prompt), &question, api)?;
    let chapters = parse_chapters(&response)?;

    if save {
        data.chapter_summary = chapters.clone();
        save_project(&data, project).map_err(|e| anyhow!("保存工程失败: {e}"))?;
    }

    Ok(json!({
        "path": project.display().to_string(),
        "count": chapters.len(),
        "saved": save,
        "chapters": chapters.iter().map(|chapter| json!({
            "start_ms": chapter.start_ms,
            "end_ms": chapter.end_ms,
            "title": chapter.title,
        })).collect::<Vec<_>>(),
    }))
}

fn chapter_input(
    entries: &[(usize, video_editor::filters::traits::SubtitleEntry)],
    duration: std::time::Duration,
) -> Value {
    let subtitles: Vec<Value> = entries
        .iter()
        .enumerate()
        .map(|(index, (_, entry))| {
            json!({
                "index": index,
                "start_ms": entry.start.as_millis() as u64,
                "end_ms": entry.end.as_millis() as u64,
                "text": entry.text,
            })
        })
        .collect();

    json!({
        "subtitles": subtitles,
        "total_duration_ms": duration.as_millis() as u64,
    })
}

fn chapter_prompt(extra_prompt: &str) -> String {
    format!(
        r#"You are a video chapter summary assistant. Given subtitle entries with timestamps, group them into meaningful chapters. Each chapter should cover a coherent topic or section of the video.
{extra_prompt}

<Input format>
{{ "total_duration_ms": 300000, "subtitles": [{{"index":0,"start_ms":0,"end_ms":3000,"text":"..."}}] }}
</Input format>

<Output format>
{{ "chapters": [{{"start_ms":0,"end_ms":30000,"title":"Introduction"}},...] }}
</Output format>

Rules:
- Chapters should cover the entire video duration without gaps
- Each chapter title should be concise (less than 100 words)
- Group subtitles that belong to the same topic into one chapter
- The first chapter's start_ms should be 0
- The last chapter's end_ms should match total_duration_ms
- Only output the JSON, no additional text
"#
    )
}

fn parse_chapters(response: &str) -> Result<Vec<video_editor::project::ChapterSummaryData>> {
    let trimmed = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: Value = serde_json::from_str(trimmed)
        .map_err(|e| anyhow!("解析章节摘要失败: {e}。原始返回: {trimmed}"))?;
    let chapters: Vec<video_editor::project::ChapterSummaryData> = parsed["chapters"]
        .as_array()
        .ok_or_else(|| anyhow!("返回里没有 chapters 数组"))?
        .iter()
        .map(|chapter| {
            Ok(video_editor::project::ChapterSummaryData {
                start_ms: chapter["start_ms"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("章节缺少 start_ms"))?,
                end_ms: chapter["end_ms"]
                    .as_u64()
                    .ok_or_else(|| anyhow!("章节缺少 end_ms"))?,
                title: chapter["title"]
                    .as_str()
                    .ok_or_else(|| anyhow!("章节缺少 title"))?
                    .to_string(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    if chapters.is_empty() {
        return Err(anyhow!("模型没有返回任何章节"));
    }
    Ok(chapters)
}

pub(super) fn run(cmd: &super::Cmd) -> Result<Value> {
    match cmd {
        super::Cmd::Subtitles { project } => subtitles(project),
        super::Cmd::SubtitleSearch { project, keyword } => subtitle_search(project, keyword),
        super::Cmd::ChapterSummary {
            project,
            api_base,
            api_key,
            model,
            prompt,
            temperature,
            save,
        } => chapter_summary(
            project,
            &crate::llm::ApiConfig {
                base_url: api_base.clone(),
                api_key: api_key.clone(),
                model: model.clone(),
                temperature: *temperature,
            },
            prompt,
            *save,
        ),
        _ => unreachable!(),
    }
}
