use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use video_editor::{project::load_project, tracks::Track};

use super::common::{load_manager, refresh_track_duration, save_manager, track_segments_mut};

fn texts(project: &Path) -> Result<Value> {
    let data = load_project(project)?;
    let manager = data
        .inner
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;

    let mut tracks = Vec::new();
    for (index, track) in manager.tracks.iter().enumerate() {
        if let Track::Text(text_track) = track {
            let segments: Vec<Value> = text_track
                .track
                .segments
                .iter()
                .enumerate()
                .map(|(i, segment)| {
                    let mut value = serde_json::to_value(
                        video_editor::project::project::TextSegmentData::from(segment.as_ref()),
                    )
                    .unwrap_or(Value::Null);
                    value["segment"] = json!(i);
                    value
                })
                .collect();
            tracks.push(json!({ "index": index, "name": text_track.name, "segments": segments }));
        }
    }

    Ok(json!({ "path": project.display().to_string(), "tracks": tracks }))
}
#[allow(clippy::too_many_arguments)]
fn text_add(
    project: &Path,
    text: Option<&str>,
    start: f64,
    duration: f64,
    font_size: u32,
    font_family: &str,
    font_path: Option<&PathBuf>,
    color: &str,
    position: &str,
    alignment: Option<u32>,
    element_json: Option<&str>,
    track_index: Option<usize>,
    track_name: &str,
) -> Result<Value> {
    if duration <= 0.0 {
        return Err(anyhow!("duration 必须大于 0"));
    }

    let element = build_text_element(
        text,
        font_size,
        font_family,
        font_path,
        color,
        position,
        alignment,
        element_json,
    )?;

    let (mut data, mut manager) = load_manager(project)?;
    let index = resolve_text_track(&mut manager, track_index, track_name)?;

    let metadata = Arc::new(video_editor::metadata::Metadata {
        path: PathBuf::from(format!("text://{}", uuid::Uuid::new_v4())),
        ..Default::default()
    });
    let segment = video_editor::tracks::segment::Segment::new(
        std::time::Duration::from_secs_f64(start),
        std::time::Duration::from_secs_f64(duration),
        metadata,
        1.0,
    )
    .with_text_element((&element).into());

    if let Some(track) = manager.tracks.get_mut(index) {
        if let Track::Text(text_track) = track {
            std::sync::Arc::make_mut(text_track)
                .track
                .segments
                .push(Arc::new(segment));
        }
        refresh_track_duration(track);
    }

    save_manager(&mut data, manager, project)?;

    Ok(json!({
        "path": project.display().to_string(),
        "track": index,
        "text": element.text,
    }))
}

#[allow(clippy::too_many_arguments)]
fn build_text_element(
    text: Option<&str>,
    font_size: u32,
    font_family: &str,
    font_path: Option<&PathBuf>,
    color: &str,
    position: &str,
    alignment: Option<u32>,
    element_json: Option<&str>,
) -> Result<video_editor::project::project::TextElementData> {
    if let Some(json_text) = element_json {
        return serde_json::from_str(json_text).map_err(|e| anyhow!("文字样式 JSON 解析失败: {e}"));
    }

    let text = text.ok_or_else(|| anyhow!("需要 --text 或 --json"))?;
    let [r, g, b, a] = crate::image_ops::parse_rgba(color)?;
    let (x, y) = parse_position(position)?;

    Ok(video_editor::project::project::TextElementData {
        text: text.to_string(),
        position: (x, y),
        opacity: 1.0,
        rotation: 0.0,
        font_size,
        font_path: font_path.map(|p| p.display().to_string()),
        font_family: font_family.to_string(),
        font_style: "Normal".to_string(),
        primary_color: Some((r, g, b, a)),
        background_color: None,
        outline_color: None,
        outline_width: None,
        border_radius: None,
        alignment,
        margin_vertical: None,
        margin_horizontal: None,
        padding: None,
        border_width: None,
        border_color: None,
        typewriter: false,
        text_alignment: None,
    })
}

/// 指定了轨道就校验它是文字轨，否则复用/新建第一条文字轨。
fn resolve_text_track(
    manager: &mut video_editor::tracks::Manager,
    track_index: Option<usize>,
    track_name: &str,
) -> Result<usize> {
    if let Some(index) = track_index {
        match manager.tracks.get(index) {
            Some(Track::Text(_)) => {}
            Some(_) => return Err(anyhow!("轨道 {index} 不是文字轨道")),
            None => return Err(anyhow!("轨道索引越界: {index}")),
        }
        return Ok(index);
    }

    if let Some(index) = manager
        .tracks
        .iter()
        .position(|track| matches!(track, Track::Text(_)))
    {
        return Ok(index);
    }

    let track =
        video_editor::tracks::text_track::TextTrack::new().with_name(track_name.to_string());
    Ok(manager.add_track(Track::Text(Arc::new(track))))
}
fn text_remove(project: &Path, track: usize, segment: usize) -> Result<Value> {
    let (mut data, mut manager) = load_manager(project)?;
    match manager.tracks.get_mut(track) {
        Some(track) => {
            if !matches!(track, Track::Text(_)) {
                return Err(anyhow!("轨道不是文字轨道"));
            }
            let list = track_segments_mut(track);
            if segment >= list.len() {
                return Err(anyhow!("片段索引越界: {segment}"));
            }
            list.remove(segment);
            refresh_track_duration(track);
        }
        None => return Err(anyhow!("轨道索引越界: {track}")),
    }

    save_manager(&mut data, manager, project)?;
    Ok(json!({ "path": project.display().to_string(), "track": track }))
}
fn parse_position(value: &str) -> Result<(f32, f32)> {
    let (x, y) = value
        .split_once(',')
        .ok_or_else(|| anyhow!("position 格式应为 x,y: {value}"))?;
    Ok((
        x.trim().parse().map_err(|_| anyhow!("x 不是数字: {x}"))?,
        y.trim().parse().map_err(|_| anyhow!("y 不是数字: {y}"))?,
    ))
}

pub(super) fn run(cmd: &super::Cmd) -> Result<Value> {
    match cmd {
        super::Cmd::Texts { project } => texts(project),
        super::Cmd::TextAdd {
            project,
            text,
            start,
            duration,
            font_size,
            font_family,
            font_path,
            color,
            position,
            alignment,
            json,
            track,
            name,
        } => text_add(
            project,
            text.as_deref(),
            *start,
            *duration,
            *font_size,
            font_family,
            font_path.as_ref(),
            color,
            position,
            *alignment,
            json.as_deref(),
            *track,
            name,
        ),
        super::Cmd::TextRemove {
            project,
            track,
            segment,
        } => text_remove(project, *track, *segment),
        _ => unreachable!(),
    }
}
