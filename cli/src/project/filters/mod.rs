use std::path::Path;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use video_editor::{
    filters::traits::{AudioFilterWrapper, SubtitleFilterWrapper, VideoFilterWrapper},
    project::{
        filters::{
            AudioFilterData, GlobalFilterData, SubtitleFilterData, VideoFilterData,
            audio_filter_wrapper_to_data, data_to_audio_filter, data_to_global_filter,
            data_to_subtitle_filter, data_to_video_filter, global_filter_wrapper_to_data,
            image_filter_wrapper_to_data, subtitle_filter_wrapper_to_data,
            video_filter_wrapper_to_data,
        },
        load_project,
    },
};

mod catalog;

use super::common::{find_segment, load_manager, save_manager, track_inner, track_name};
use crate::project::args::FilterKind;
use catalog::filter_catalog;

fn list_filters(
    project: &Path,
    only_track: Option<usize>,
    only_segment: Option<usize>,
) -> Result<Value> {
    let data = load_project(project)?;
    let manager = data
        .inner
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;

    let mut tracks = Vec::new();
    for (track_index, track) in manager.tracks.iter().enumerate() {
        if only_track.is_some_and(|t| t != track_index) {
            continue;
        }
        let segments: Vec<Value> = track_inner(track)
            .segments
            .iter()
            .enumerate()
            .filter(|(i, _)| only_segment.is_none_or(|s| s == *i))
            .map(|(i, segment)| {
                let mut value = segment_filters_json(segment);
                value["segment"] = json!(i);
                value
            })
            .collect();

        tracks.push(json!({
            "index": track_index,
            "name": track_name(track),
            "segments": segments,
        }));
    }

    Ok(json!({
        "path": project.display().to_string(),
        "tracks": tracks,
        "global": manager
            .get_global_filters()
            .iter()
            .map(|f| global_filter_wrapper_to_data(f))
            .collect::<Vec<_>>(),
    }))
}
fn filter_add(
    project: &Path,
    track: usize,
    segment: usize,
    json_text: &str,
    kind: Option<FilterKind>,
) -> Result<Value> {
    let (mut data, mut manager) = load_manager(project)?;
    let target = find_segment(&mut manager, track, segment)?;

    let kinds: Vec<FilterKind> = match kind {
        Some(kind) => vec![kind],
        None => vec![
            FilterKind::Video,
            FilterKind::Audio,
            FilterKind::Subtitle,
            FilterKind::Image,
        ],
    };

    let mut applied = None;
    let mut errors = Vec::new();
    for kind in kinds {
        match add_filter_of_kind(target, kind, json_text)? {
            Ok(result) => {
                applied = Some(result);
                break;
            }
            Err(e) => errors.push(e),
        }
    }

    let (applied, stored) = applied.ok_or_else(|| {
        anyhow!(
            "滤镜 JSON 无法解析，请用 `project filter-catalog` 查类型名。\n{}",
            errors.join("\n")
        )
    })?;

    save_manager(&mut data, manager, project)?;

    Ok(json!({
        "path": project.display().to_string(),
        "track": track,
        "segment": segment,
        "kind": applied,
        "filter": stored,
    }))
}

/// 按类别解析并挂一个滤镜；解析失败返回给调用方继续尝试下一类。
fn add_filter_of_kind(
    target: &mut video_editor::tracks::segment::Segment,
    kind: FilterKind,
    json_text: &str,
) -> Result<std::result::Result<(&'static str, Value), String>> {
    match kind {
        FilterKind::Video => match serde_json::from_str::<VideoFilterData>(json_text) {
            Ok(filter_data) => {
                let filter = data_to_video_filter(&filter_data)
                    .map_err(|e| anyhow!("构建视频滤镜失败: {e}"))?;
                target.video_filters.push(Arc::new(VideoFilterWrapper::new(
                    filter_data.enabled,
                    filter,
                )));
                Ok(Ok(("video", serde_json::to_value(&filter_data)?)))
            }
            Err(e) => Ok(Err(format!("video: {e}"))),
        },
        FilterKind::Audio => match serde_json::from_str::<AudioFilterData>(json_text) {
            Ok(filter_data) => {
                let filter = data_to_audio_filter(&filter_data)
                    .map_err(|e| anyhow!("构建音频滤镜失败: {e}"))?;
                target.audio_filters.push(Arc::new(AudioFilterWrapper::new(
                    filter_data.enabled,
                    filter,
                )));
                Ok(Ok(("audio", serde_json::to_value(&filter_data)?)))
            }
            Err(e) => Ok(Err(format!("audio: {e}"))),
        },
        FilterKind::Subtitle => match serde_json::from_str::<SubtitleFilterData>(json_text) {
            Ok(filter_data) => {
                let filter = data_to_subtitle_filter(&filter_data)
                    .map_err(|e| anyhow!("构建字幕滤镜失败: {e}"))?;
                target
                    .subtitle_filters
                    .push(Arc::new(SubtitleFilterWrapper::new(
                        filter_data.enabled,
                        filter,
                    )));
                Ok(Ok(("subtitle", serde_json::to_value(&filter_data)?)))
            }
            Err(e) => Ok(Err(format!("subtitle: {e}"))),
        },
        FilterKind::Image => match serde_json::from_str::<VideoFilterData>(json_text) {
            Ok(filter_data) => {
                let filter = data_to_video_filter(&filter_data)
                    .map_err(|e| anyhow!("构建图片滤镜失败: {e}"))?;
                target.image_filters.push(Arc::new(VideoFilterWrapper::new(
                    filter_data.enabled,
                    filter,
                )));
                Ok(Ok(("image", serde_json::to_value(&filter_data)?)))
            }
            Err(e) => Ok(Err(format!("image: {e}"))),
        },
        FilterKind::Global => Err(anyhow!("global 滤镜请用 `project global-filter-add`")),
    }
}
fn filter_remove(
    project: &Path,
    track: usize,
    segment: usize,
    kind: FilterKind,
    index: usize,
) -> Result<Value> {
    let (mut data, mut manager) = load_manager(project)?;
    let target = find_segment(&mut manager, track, segment)?;

    let removed = match kind {
        FilterKind::Video => remove_at(&mut target.video_filters, index),
        FilterKind::Audio => remove_at(&mut target.audio_filters, index),
        FilterKind::Subtitle => remove_at(&mut target.subtitle_filters, index),
        FilterKind::Image => remove_at(&mut target.image_filters, index),
        FilterKind::Global => return Err(anyhow!("全局滤镜请用 `project global-filter-remove`")),
    };

    if !removed {
        return Err(anyhow!("滤镜索引越界: {index}"));
    }

    save_manager(&mut data, manager, project)?;
    Ok(json!({ "path": project.display().to_string(), "track": track, "segment": segment }))
}
fn remove_at<T>(items: &mut Vec<T>, index: usize) -> bool {
    if index < items.len() {
        items.remove(index);
        true
    } else {
        false
    }
}
fn list_global_filters(project: &Path) -> Result<Value> {
    let data = load_project(project)?;
    let manager = data
        .inner
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;

    Ok(json!({
        "path": project.display().to_string(),
        "filters": manager
            .get_global_filters()
            .iter()
            .map(|f| global_filter_wrapper_to_data(f))
            .collect::<Vec<_>>(),
    }))
}
fn global_filter_add(project: &Path, json_text: &str) -> Result<Value> {
    let data = serde_json::from_str::<GlobalFilterData>(json_text)
        .map_err(|e| anyhow!("全局滤镜 JSON 解析失败: {e}"))?;
    let wrapper = data_to_global_filter(&data).map_err(|e| anyhow!("构建全局滤镜失败: {e}"))?;

    let (mut manager_data, mut manager) = load_manager(project)?;
    manager.add_global_filter(Arc::new(wrapper));
    save_manager(&mut manager_data, manager, project)?;

    Ok(json!({ "path": project.display().to_string() }))
}
fn global_filter_remove(project: &Path, index: usize) -> Result<Value> {
    let (mut data, mut manager) = load_manager(project)?;
    manager
        .remove_global_filter(index)
        .map_err(|e| anyhow!("删除全局滤镜失败: {e}"))?;
    save_manager(&mut data, manager, project)?;
    Ok(json!({ "path": project.display().to_string(), "index": index }))
}
fn segment_filters_json(segment: &video_editor::tracks::segment::Segment) -> Value {
    json!({
        "video": segment
            .video_filters
            .iter()
            .map(|f| video_filter_wrapper_to_data(f))
            .collect::<Vec<_>>(),
        "audio": segment
            .audio_filters
            .iter()
            .map(|f| audio_filter_wrapper_to_data(f))
            .collect::<Vec<_>>(),
        "subtitle": segment
            .subtitle_filters
            .iter()
            .map(|f| subtitle_filter_wrapper_to_data(f))
            .collect::<Vec<_>>(),
        "image": segment
            .image_filters
            .iter()
            .map(|f| image_filter_wrapper_to_data(f))
            .collect::<Vec<_>>(),
    })
}

pub(crate) fn run(cmd: &crate::project::Cmd) -> Result<Value> {
    match cmd {
        crate::project::Cmd::Filters {
            project,
            track,
            segment,
        } => list_filters(project, *track, *segment),
        crate::project::Cmd::FilterAdd {
            project,
            track,
            segment,
            json,
            kind,
        } => filter_add(project, *track, *segment, json, *kind),
        crate::project::Cmd::FilterRemove {
            project,
            track,
            segment,
            kind,
            index,
        } => filter_remove(project, *track, *segment, *kind, *index),
        crate::project::Cmd::FilterCatalog => Ok(filter_catalog()),
        crate::project::Cmd::GlobalFilters { project } => list_global_filters(project),
        crate::project::Cmd::GlobalFilterAdd { project, json } => global_filter_add(project, json),
        crate::project::Cmd::GlobalFilterRemove { project, index } => {
            global_filter_remove(project, *index)
        }
        _ => unreachable!(),
    }
}
