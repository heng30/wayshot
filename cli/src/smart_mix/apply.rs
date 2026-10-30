use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use video_editor::tracks::{Manager, segment::Segment};

use super::{AudioEntry, Match, VisualItem};

pub(super) fn apply_matches(
    manager: &mut Manager,
    visuals: &[VisualItem],
    audio: &[AudioEntry],
    matches: &[Match],
) -> Result<()> {
    // 注意：不能用 Track::add_segment，它会把 timeline_offset 改成追加位置，
    // 混剪需要保留「音频起点 + 偏移」的绝对时间轴位置。
    let originals: HashMap<(usize, usize), Arc<Segment>> = {
        let mut map = HashMap::new();
        for visual in visuals {
            let key = (visual.track_index, visual.segment_index);
            if map.contains_key(&key) {
                continue;
            }
            if let Some(segment) = manager
                .tracks
                .get(visual.track_index)
                .and_then(|track| track.segments().get(visual.segment_index))
            {
                map.insert(key, segment.clone());
            }
        }
        map
    };

    let track_indices: BTreeSet<usize> = visuals.iter().map(|v| v.track_index).collect();
    for &track_index in &track_indices {
        if let Some(track) = manager.tracks.get_mut(track_index) {
            crate::project::track_segments_mut(track).clear();
        }
    }

    let mut audio_offset: HashMap<usize, u64> = HashMap::new();
    for item in matches {
        let audio_start = audio[item.audio_index].start_ms;
        let offset = audio_offset.entry(item.audio_index).or_insert(0);
        let timeline_start = audio_start + *offset;
        *offset += item.duration_ms;

        let visual = &visuals[item.visual_index];
        let key = (visual.track_index, visual.segment_index);
        let Some(original) = originals.get(&key) else {
            continue;
        };

        let new_segment = Arc::new(Segment::new_with_source_offset(
            Duration::from_millis(timeline_start),
            if visual.is_image {
                Duration::ZERO
            } else {
                Duration::from_millis(item.source_offset_ms)
            },
            Duration::from_millis(item.duration_ms),
            1.0,
            original.global_speed,
            original.metadata.clone(),
        ));

        if let Some(track) = manager.tracks.get_mut(visual.track_index) {
            crate::project::track_segments_mut(track).push(new_segment);
        }
    }

    for &track_index in &track_indices {
        if let Some(track) = manager.tracks.get_mut(track_index) {
            let segments = crate::project::track_segments_mut(track);
            segments.sort_by_key(|segment| segment.timeline_offset);
            crate::project::refresh_track_duration(track);
        }
    }

    Ok(())
}
