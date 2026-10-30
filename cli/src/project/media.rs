use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use video_editor::{
    project::{ManagerData, load_project, save_project},
    tracks::{Manager, Track},
};

type TrackSegments = (String, Vec<(usize, f64, f64, f64)>);

use super::common::{refresh_track_duration, track_inner, track_name, track_segments_mut};

fn info(input: &Path) -> Result<Value> {
    let project = load_project(input)?;
    let manager = project
        .inner
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", input.display()))?;

    let tracks: Vec<Value> = manager
        .tracks
        .iter()
        .map(|track| {
            let (kind, name, hiding, locked) = match track {
                Track::Video(t) => ("video", t.name.clone(), t.hiding, t.locked),
                Track::Audio(t) => ("audio", t.name.clone(), t.hiding, t.locked),
                Track::Image(t) => ("image", t.name.clone(), t.hiding, t.locked),
                Track::Subtitle(t) => ("subtitle", t.name.clone(), t.hiding, t.locked),
                Track::Text(t) => ("text", t.name.clone(), t.hiding, t.locked),
            };
            let inner = track_inner(track);

            json!({
                "type": kind,
                "name": name,
                "duration_secs": inner.duration.as_secs_f64(),
                "segments": inner.segments.len(),
                "hiding": hiding,
                "locked": locked,
            })
        })
        .collect();

    Ok(json!({
        "path": input.display().to_string(),
        "version": project.version,
        "duration_secs": manager.duration.as_secs_f64(),
        "global_filters": manager.global_filters.len(),
        "chapter_summary": project.chapter_summary.len(),
        "bookmarks": project.bookmarks.len(),
        "memo": project.memo,
        "tracks": tracks,
    }))
}
fn segments(project: &Path, only_track: Option<usize>) -> Result<Value> {
    let data = load_project(project)?;
    let manager = data
        .inner
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;

    let mut tracks = Vec::new();
    for (index, track) in manager.tracks.iter().enumerate() {
        if only_track.is_some_and(|t| t != index) {
            continue;
        }
        let segments = track_inner(track)
            .segments
            .iter()
            .enumerate()
            .map(|(i, segment)| {
                json!({
                    "index": i,
                    "timeline_offset_secs": segment.timeline_offset.as_secs_f64(),
                    "source_offset_secs": segment.source_offset.as_secs_f64(),
                    "duration_secs": segment.duration.as_secs_f64(),
                    "source": segment.metadata.path.display().to_string(),
                })
            })
            .collect::<Vec<_>>();

        tracks.push(json!({
            "index": index,
            "name": track_name(track),
            "segments": segments,
        }));
    }

    Ok(json!({ "path": project.display().to_string(), "tracks": tracks }))
}
fn trim(project: &Path, track: usize, segment: usize, start: f64, duration: f64) -> Result<Value> {
    if duration <= 0.0 {
        return Err(anyhow!("duration 必须大于 0"));
    }
    edit(project, track, segment, move |seg| {
        seg.source_offset = std::time::Duration::from_secs_f64(start.max(0.0));
        seg.duration = std::time::Duration::from_secs_f64(duration);
        seg.original_duration = seg.duration;
        Ok(())
    })
}
fn move_segment(project: &Path, track: usize, segment: usize, to: f64) -> Result<Value> {
    edit(project, track, segment, move |seg| {
        seg.timeline_offset = std::time::Duration::from_secs_f64(to.max(0.0));
        Ok(())
    })
}
fn remove_segment(project: &Path, track: usize, segment: usize) -> Result<Value> {
    let mut data = load_project(project)?;
    let mut manager = data
        .inner
        .take()
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;

    let target = manager
        .tracks
        .get_mut(track)
        .ok_or_else(|| anyhow!("轨道索引越界: {track}"))?;
    let list = track_segments_mut(target);
    if segment >= list.len() {
        return Err(anyhow!(
            "片段索引越界: {segment}（轨道只有 {} 个片段）",
            list.len()
        ));
    }
    list.remove(segment);
    refresh_track_duration(target);

    manager.update_duration();
    data.inner = Some(manager);
    save_project(&data, project).map_err(|e| anyhow!("保存工程失败: {e}"))?;

    Ok(json!({ "path": project.display().to_string(), "track": track }))
}
fn create(output: &Path, media: &[PathBuf], speed: f32) -> Result<Value> {
    let mut manager = Manager::new();
    append_media(&mut manager, media, speed)?;
    manager.update_duration();

    let data = ManagerData::new(manager);
    save_project(&data, output).map_err(|e| anyhow!("保存工程失败: {e}"))?;

    info(output)
}
fn add_media(project: &Path, media: &[PathBuf], speed: f32) -> Result<Value> {
    let mut data = load_project(project)?;
    let mut manager = data
        .inner
        .take()
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;

    append_media(&mut manager, media, speed)?;
    manager.update_duration();
    data.inner = Some(manager);

    save_project(&data, project).map_err(|e| anyhow!("保存工程失败: {e}"))?;

    info(project)
}
fn append_media(manager: &mut Manager, media: &[PathBuf], speed: f32) -> Result<()> {
    if media.is_empty() {
        return Err(anyhow!("至少需要一个媒体文件"));
    }

    for path in media {
        let tracks =
            Track::new(path, speed).map_err(|e| anyhow!("导入 {} 失败: {e}", path.display()))?;
        for track in tracks {
            manager.add_track(track);
        }
    }

    Ok(())
}
fn edit<F>(project: &Path, track: usize, segment: usize, edit: F) -> Result<Value>
where
    F: FnOnce(&mut video_editor::tracks::segment::Segment) -> Result<()>,
{
    let mut data = load_project(project)?;
    let mut manager = data
        .inner
        .take()
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;

    let target = manager
        .tracks
        .get_mut(track)
        .ok_or_else(|| anyhow!("轨道索引越界: {track}"))?;
    let segments: TrackSegments = (
        track_name(target),
        track_inner(target)
            .segments
            .iter()
            .map(|s| {
                (
                    0,
                    s.timeline_offset.as_secs_f64(),
                    s.source_offset.as_secs_f64(),
                    s.duration.as_secs_f64(),
                )
            })
            .collect(),
    );

    let list = track_segments_mut(target);
    let len = list.len();
    let segment = list
        .get_mut(segment)
        .ok_or_else(|| anyhow!("片段索引越界，轨道只有 {len} 个片段"))?;
    edit(std::sync::Arc::make_mut(segment))?;
    refresh_track_duration(target);

    manager.update_duration();
    data.inner = Some(manager);
    save_project(&data, project).map_err(|e| anyhow!("保存工程失败: {e}"))?;

    Ok(json!({
        "path": project.display().to_string(),
        "track": track,
        "track_name": segments.0,
        "segments_before": segments.1.len(),
    }))
}

pub(super) fn run(cmd: &super::Cmd) -> Result<Value> {
    match cmd {
        super::Cmd::Info { input } => info(input),
        super::Cmd::Create {
            output,
            media,
            speed,
        } => create(output, media, *speed),
        super::Cmd::AddMedia {
            project,
            media,
            speed,
        } => add_media(project, media, *speed),
        super::Cmd::Segments { project, track } => segments(project, *track),
        super::Cmd::Trim {
            project,
            track,
            segment,
            start,
            duration,
        } => trim(project, *track, *segment, *start, *duration),
        super::Cmd::Move {
            project,
            track,
            segment,
            to,
        } => move_segment(project, *track, *segment, *to),
        super::Cmd::Remove {
            project,
            track,
            segment,
        } => remove_segment(project, *track, *segment),
        _ => unreachable!(),
    }
}
