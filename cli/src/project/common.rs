use std::path::Path;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use video_editor::{
    project::{ManagerData, load_project, save_project},
    tracks::{Manager, Track},
};

/// 载入工程并把轨道管理器取出来，后续修改完再 save_manager 写回。
pub(crate) fn load_manager(project: &Path) -> Result<(ManagerData, Manager)> {
    let mut data = load_project(project)?;
    let manager = data
        .inner
        .take()
        .ok_or_else(|| anyhow!("工程文件里没有轨道数据: {}", project.display()))?;
    Ok((data, manager))
}

pub(crate) fn save_manager(data: &mut ManagerData, manager: Manager, project: &Path) -> Result<()> {
    let mut manager = manager;
    manager.update_duration();
    data.inner = Some(manager);
    save_project(data, project).map_err(|e| anyhow!("保存工程失败: {e}"))
}

pub(crate) fn track_inner_mut(track: &mut Track) -> &mut video_editor::tracks::track::InnerTrack {
    match track {
        Track::Video(t) => &mut std::sync::Arc::make_mut(t).track,
        Track::Audio(t) => &mut std::sync::Arc::make_mut(t).track,
        Track::Image(t) => &mut std::sync::Arc::make_mut(t).track,
        Track::Subtitle(t) => &mut std::sync::Arc::make_mut(t).track,
        Track::Text(t) => &mut std::sync::Arc::make_mut(t).track,
    }
}
pub(crate) fn refresh_track_duration(track: &mut Track) {
    let inner = track_inner_mut(track);
    inner.duration = inner
        .segments
        .iter()
        .map(|segment| segment.timeline_offset + segment.duration)
        .max()
        .unwrap_or_default();
}
pub(crate) fn track_inner(track: &Track) -> &video_editor::tracks::track::InnerTrack {
    match track {
        Track::Video(t) => &t.track,
        Track::Audio(t) => &t.track,
        Track::Image(t) => &t.track,
        Track::Subtitle(t) => &t.track,
        Track::Text(t) => &t.track,
    }
}
pub(crate) fn track_name(track: &Track) -> String {
    match track {
        Track::Video(t) => t.name.clone(),
        Track::Audio(t) => t.name.clone(),
        Track::Image(t) => t.name.clone(),
        Track::Subtitle(t) => t.name.clone(),
        Track::Text(t) => t.name.clone(),
    }
}
pub(crate) fn find_segment(
    manager: &mut Manager,
    track: usize,
    segment: usize,
) -> Result<&mut video_editor::tracks::segment::Segment> {
    let target = manager
        .tracks
        .get_mut(track)
        .ok_or_else(|| anyhow!("轨道索引越界: {track}"))?;
    let len = track_inner(target).segments.len();
    let segments = track_segments_mut(target);
    segments
        .get_mut(segment)
        .map(std::sync::Arc::make_mut)
        .ok_or_else(|| anyhow!("片段索引越界，轨道只有 {len} 个片段"))
}

pub(crate) fn track_segments_mut(
    track: &mut Track,
) -> &mut Vec<Arc<video_editor::tracks::segment::Segment>> {
    match track {
        Track::Video(t) => &mut std::sync::Arc::make_mut(t).track.segments,
        Track::Audio(t) => &mut std::sync::Arc::make_mut(t).track.segments,
        Track::Image(t) => &mut std::sync::Arc::make_mut(t).track.segments,
        Track::Subtitle(t) => &mut std::sync::Arc::make_mut(t).track.segments,
        Track::Text(t) => &mut std::sync::Arc::make_mut(t).track.segments,
    }
}
