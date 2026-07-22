use crate::{
    error::McpError,
    state::{self, UiAction},
    types::SegmentInfo,
};
use std::sync::Arc;
use std::time::Duration;
use video_editor::{
    commands::segment::{
        AddSegmentCommand, CopySegmentCommand, InsertSegmentAtTimeCommand,
        MoveSegmentToTimeCommand, RemoveSegmentCommand, SetSegmentDurationCommand,
        ShrinkSegmentLeftCommand, ShrinkSegmentRightCommand, StretchSegmentLeftCommand,
        StretchSegmentRightCommand,
    },
    metadata::get_metadata as probe_metadata,
    tracks::segment::Segment,
};

/// List segments in a track
pub fn list_segments(track_index: usize) -> Result<Vec<SegmentInfo>, McpError> {
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    let track = manager
        .get(track_index)
        .ok_or(McpError::InvalidTrackIndex(track_index))?;

    let segments: Vec<SegmentInfo> = track
        .segments()
        .iter()
        .enumerate()
        .map(|(index, seg)| SegmentInfo {
            index,
            timeline_offset_ms: seg.timeline_offset.as_millis() as u64,
            duration_ms: seg.duration.as_millis() as u64,
            source_offset_ms: seg.source_offset.as_millis() as u64,
            original_duration_ms: seg.original_duration.as_millis() as u64,
            visible: !seg.hiding,
            audio_muted: seg.audio_muted,
            playback_speed: seg.playback_speed,
            source_path: Some(seg.metadata.path.to_string_lossy().to_string()),
        })
        .collect();

    Ok(segments)
}

/// Split a segment at the given position.
/// First selects the segment, then dispatches split action.
/// The UI split-segment callback splits the currently selected segment at the playhead position.
pub fn split_segment(
    track_index: usize,
    segment_index: usize,
    _position_ms: u64,
) -> Result<(), McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    // First select the segment so the UI knows which one to split
    state::dispatch_action(UiAction::AddSelectedSegment {
        track_index,
        segment_index,
    });
    // Then dispatch the split action (splits selected segment at playhead)
    state::dispatch_action(UiAction::SplitSegment);
    Ok(())
}

/// Move a segment to a new timeline offset
/// Dispatches commit-segment-move with the target offset.
pub fn move_segment(
    track_index: usize,
    segment_index: usize,
    offset_ms: u64,
) -> Result<(), McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    // First select the segment
    state::dispatch_action(UiAction::AddSelectedSegment {
        track_index,
        segment_index,
    });
    // Then commit the move
    state::dispatch_action(UiAction::CommitSegmentMove {
        track_index,
        segment_index,
        final_offset_ms: offset_ms as i32,
    });
    Ok(())
}

/// Delete a segment.
/// First selects the segment, then dispatches remove action.
pub fn delete_segment(track_index: usize, segment_index: usize) -> Result<(), McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    // First select the segment so the UI knows which one to remove
    state::dispatch_action(UiAction::AddSelectedSegment {
        track_index,
        segment_index,
    });
    // Then dispatch remove action (removes selected segments)
    state::dispatch_action(UiAction::RemoveSegments);
    Ok(())
}

/// Toggle segment visibility
pub fn toggle_visible(track_index: usize, segment_index: usize) -> Result<bool, McpError> {
    state::dispatch_action(UiAction::ToggleSegmentEnable {
        track_index,
        segment_index,
    });

    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    let is_visible = manager
        .get(track_index)
        .and_then(|t| t.segments().get(segment_index))
        .map(|s| !s.hiding)
        .unwrap_or(true);
    Ok(is_visible)
}

/// Toggle segment audio muted
pub fn toggle_audio(track_index: usize, segment_index: usize) -> Result<bool, McpError> {
    state::dispatch_action(UiAction::ToggleSegmentAudio {
        track_index,
        segment_index,
    });

    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    let is_muted = manager
        .get(track_index)
        .and_then(|t| t.segments().get(segment_index))
        .map(|s| s.audio_muted)
        .unwrap_or(false);
    Ok(is_muted)
}

/// Remove gap before a segment
pub fn remove_gap(
    track_index: usize,
    segment_index: usize,
    direction: &str,
) -> Result<(), McpError> {
    let action = match direction {
        "left" => UiAction::SegmentRemoveLeftGap {
            track_index,
            segment_index,
        },
        "right" => UiAction::SegmentRemoveRightGap {
            track_index,
            segment_index,
        },
        _ => {
            return Err(McpError::InvalidParameter(format!(
                "Invalid direction: '{direction}'. Must be 'left' or 'right'"
            )));
        }
    };
    state::dispatch_action(action);
    Ok(())
}

/// Get segment metadata
pub fn get_metadata(
    track_index: usize,
    segment_index: usize,
) -> Result<serde_json::Value, McpError> {
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    let segment = manager
        .get(track_index)
        .and_then(|t| t.segments().get(segment_index))
        .ok_or(McpError::InvalidSegmentIndex {
            track: track_index,
            segment: segment_index,
        })?
        .clone();

    let meta = &segment.metadata;
    Ok(serde_json::json!({
        "path": meta.path.to_string_lossy(),
        "duration_ms": meta.duration.as_millis() as u64,
        "videos": meta.videos.iter().map(|v| serde_json::json!({
            "width": v.width,
            "height": v.height,
            "fps": v.fps,
        })).collect::<Vec<_>>(),
        "audios": meta.audios.iter().map(|a| serde_json::json!({
            "channels": a.channels,
            "sample_rate": a.sample_rate,
        })).collect::<Vec<_>>(),
    }))
}

/// Add a segment to a track from a file path.
/// Uses the command system so undo/redo works.
pub fn add_segment(
    track_index: usize,
    file_path: String,
    timeline_offset_ms: Option<u64>,
) -> Result<serde_json::Value, McpError> {
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;

    // Verify track exists
    let _track = manager
        .get(track_index)
        .ok_or(McpError::InvalidTrackIndex(track_index))?;

    // Probe the file to get metadata
    let mut file_metadata = probe_metadata(&file_path).map_err(|e| {
        McpError::InvalidParameter(format!("Failed to probe file '{}': {}", file_path, e))
    })?;

    // For images (duration=0), set a default duration of 5 seconds
    if file_metadata.duration.is_zero() && file_metadata.is_image() {
        file_metadata.duration = Duration::from_secs(5);
    }

    let global_speed = manager.get_global_speed();
    let segment = Arc::new(Segment::new(
        timeline_offset_ms
            .map(Duration::from_millis)
            .unwrap_or(Duration::ZERO),
        file_metadata.duration,
        Arc::new(file_metadata),
        global_speed,
    ));

    let affected = if timeline_offset_ms.is_some() {
        // Use InsertSegmentAtTimeCommand for specific timeline offset
        // Find the correct insert position based on timeline_offset
        let insert_index = manager
            .get(track_index)
            .map(|t| {
                t.segments()
                    .iter()
                    .position(|s| s.timeline_offset >= segment.timeline_offset)
                    .unwrap_or(t.segments_count())
            })
            .unwrap_or(0);
        let cmd = InsertSegmentAtTimeCommand::new(track_index, insert_index, segment, false);
        state::execute_command(Box::new(cmd))
    } else {
        // Use AddSegmentCommand for adding at the end (auto-offset)
        let cmd = AddSegmentCommand::new(track_index, segment);
        state::execute_command(Box::new(cmd))
    }
    .map_err(|e| McpError::Internal(format!("Add segment command failed: {}", e)))?;

    state::sync_ui();

    Ok(serde_json::json!({
        "success": true,
        "track_index": track_index,
        "timeline_offset_ms": timeline_offset_ms,
        "affected_segments": affected.segments.len(),
    }))
}

/// Resize a segment — set its duration.
/// Uses SetSegmentDurationCommand so undo/redo works.
/// Checks for overlap with subsequent segments before executing.
pub fn resize_segment(
    track_index: usize,
    segment_index: usize,
    duration_ms: u64,
    shift_timeline: bool,
) -> Result<serde_json::Value, McpError> {
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;

    // Check for overlap: new end = segment.timeline_offset + new_duration
    // Must not overlap with the next segment (if shift_timeline is false)
    if !shift_timeline
        && let Some(track) = manager.get(track_index)
        && let Ok(segment) = track.get_segment(segment_index)
    {
        let new_end = segment.timeline_offset + Duration::from_millis(duration_ms);
        // Check against the next segment
        if let Some(next_seg) = track.segments().get(segment_index + 1)
            && new_end > next_seg.timeline_offset
        {
            return Err(McpError::InvalidParameter(format!(
                "Resize would cause overlap: segment {} new end ({}ms) would overlap with segment {} start ({}ms). Use shift_timeline=true to shift subsequent segments.",
                segment_index,
                new_end.as_millis(),
                segment_index + 1,
                next_seg.timeline_offset.as_millis()
            )));
        }
    }

    let cmd = SetSegmentDurationCommand::new(
        track_index,
        segment_index,
        Duration::from_millis(duration_ms),
        shift_timeline,
    );
    let affected = state::execute_command(Box::new(cmd))
        .map_err(|e| McpError::Internal(format!("SetSegmentDurationCommand failed: {}", e)))?;

    state::sync_ui();

    Ok(serde_json::json!({
        "success": true,
        "track_index": track_index,
        "segment_index": segment_index,
        "new_duration_ms": duration_ms,
        "shift_timeline": shift_timeline,
        "affected_segments": affected.segments.len(),
    }))
}

/// Shrink a segment from the left or right side.
pub fn shrink_segment(
    track_index: usize,
    segment_index: usize,
    shrink_ms: u64,
    direction: &str, // "left" or "right"
    shift_timeline: bool,
) -> Result<serde_json::Value, McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;

    let shrink_duration = Duration::from_millis(shrink_ms);
    let affected = match direction {
        "left" => {
            let cmd = ShrinkSegmentLeftCommand::new(
                track_index,
                segment_index,
                shrink_duration,
                shift_timeline,
            );
            state::execute_command(Box::new(cmd))
        }
        "right" => {
            let cmd = ShrinkSegmentRightCommand::new(
                track_index,
                segment_index,
                shrink_duration,
                shift_timeline,
            );
            state::execute_command(Box::new(cmd))
        }
        _ => {
            return Err(McpError::InvalidParameter(format!(
                "Invalid direction: '{}'. Must be 'left' or 'right'",
                direction
            )));
        }
    }
    .map_err(|e| McpError::Internal(format!("ShrinkSegmentCommand failed: {}", e)))?;

    state::sync_ui();

    Ok(serde_json::json!({
        "success": true,
        "track_index": track_index,
        "segment_index": segment_index,
        "shrink_ms": shrink_ms,
        "direction": direction,
        "shift_timeline": shift_timeline,
        "affected_segments": affected.segments.len(),
    }))
}

/// Stretch a segment from the left or right side.
/// Checks for overlap when shift_timeline is false.
pub fn stretch_segment(
    track_index: usize,
    segment_index: usize,
    stretch_ms: u64,
    direction: &str, // "left" or "right"
    shift_timeline: bool,
) -> Result<serde_json::Value, McpError> {
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;

    // Check for overlap when shift_timeline is false
    if !shift_timeline
        && let Some(track) = manager.get(track_index)
        && let Ok(segment) = track.get_segment(segment_index)
    {
        let stretch_duration = Duration::from_millis(stretch_ms);
        match direction {
            "right" => {
                let new_end = segment.timeline_offset + segment.duration + stretch_duration;
                // Check overlap with next segment
                if let Some(next_seg) = track.segments().get(segment_index + 1)
                    && new_end > next_seg.timeline_offset
                {
                    return Err(McpError::InvalidParameter(format!(
                        "Stretch right would cause overlap: segment {} new end ({}ms) would overlap with segment {} start ({}ms). Use shift_timeline=true to shift subsequent segments.",
                        segment_index,
                        new_end.as_millis(),
                        segment_index + 1,
                        next_seg.timeline_offset.as_millis()
                    )));
                }
            }
            "left" => {
                let new_start = segment.timeline_offset - stretch_duration;
                // Check overlap with previous segment
                if segment_index > 0
                    && let Some(prev_seg) = track.segments().get(segment_index - 1)
                {
                    let prev_end = prev_seg.timeline_offset + prev_seg.duration;
                    if new_start < prev_end {
                        return Err(McpError::InvalidParameter(format!(
                            "Stretch left would cause overlap: segment {} new start ({}ms) would overlap with segment {} end ({}ms). Use shift_timeline=true to shift subsequent segments.",
                            segment_index,
                            new_start.as_millis(),
                            segment_index - 1,
                            prev_end.as_millis()
                        )));
                    }
                }
            }
            _ => {
                return Err(McpError::InvalidParameter(format!(
                    "Invalid direction: '{}'. Must be 'left' or 'right'",
                    direction
                )));
            }
        }
    }

    let stretch_duration = Duration::from_millis(stretch_ms);
    let affected = match direction {
        "left" => {
            let cmd = StretchSegmentLeftCommand::new(
                track_index,
                segment_index,
                stretch_duration,
                shift_timeline,
            );
            state::execute_command(Box::new(cmd))
        }
        "right" => {
            let cmd = StretchSegmentRightCommand::new(
                track_index,
                segment_index,
                stretch_duration,
                shift_timeline,
            );
            state::execute_command(Box::new(cmd))
        }
        _ => {
            return Err(McpError::InvalidParameter(format!(
                "Invalid direction: '{}'. Must be 'left' or 'right'",
                direction
            )));
        }
    }
    .map_err(|e| McpError::Internal(format!("StretchSegmentCommand failed: {}", e)))?;

    state::sync_ui();

    Ok(serde_json::json!({
        "success": true,
        "track_index": track_index,
        "segment_index": segment_index,
        "stretch_ms": stretch_ms,
        "direction": direction,
        "shift_timeline": shift_timeline,
        "affected_segments": affected.segments.len(),
    }))
}

/// Delete a segment using the command system (proper undo support).
pub fn delete_segment_cmd(
    track_index: usize,
    segment_index: usize,
    shift_timeline: bool,
) -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;

    let cmd = RemoveSegmentCommand::new(track_index, segment_index, shift_timeline);
    let affected = state::execute_command(Box::new(cmd))
        .map_err(|e| McpError::Internal(format!("RemoveSegmentCommand failed: {}", e)))?;

    state::sync_ui();

    Ok(serde_json::json!({
        "success": true,
        "track_index": track_index,
        "segment_index": segment_index,
        "shift_timeline": shift_timeline,
        "affected_segments": affected.segments.len(),
    }))
}

/// Move a segment to a new timeline offset using the command system.
/// Checks for overlap with other segments when shift_timeline is false.
pub fn move_segment_cmd(
    track_index: usize,
    segment_index: usize,
    new_timeline_offset_ms: u64,
    shift_timeline: bool,
) -> Result<serde_json::Value, McpError> {
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;

    // Check for overlap when shift_timeline is false
    if !shift_timeline
        && let Some(track) = manager.get(track_index)
        && let Ok(segment) = track.get_segment(segment_index)
    {
        let new_start = Duration::from_millis(new_timeline_offset_ms);
        let new_end = new_start + segment.duration;

        // Check overlap with all other segments
        for (i, other) in track.segments().iter().enumerate() {
            if i == segment_index {
                continue;
            }
            let other_end = other.timeline_offset + other.duration;
            if new_start < other_end && other.timeline_offset < new_end {
                return Err(McpError::InvalidParameter(format!(
                    "Move would cause overlap: segment {} ({}-{}ms) would overlap with segment {} ({}-{}ms). Use shift_timeline=true to shift subsequent segments.",
                    segment_index,
                    new_start.as_millis(),
                    new_end.as_millis(),
                    i,
                    other.timeline_offset.as_millis(),
                    other_end.as_millis()
                )));
            }
        }
    }

    let cmd = MoveSegmentToTimeCommand::new(
        track_index,
        segment_index,
        Duration::from_millis(new_timeline_offset_ms),
        shift_timeline,
    );
    let affected = state::execute_command(Box::new(cmd))
        .map_err(|e| McpError::Internal(format!("MoveSegmentToTimeCommand failed: {}", e)))?;

    state::sync_ui();

    Ok(serde_json::json!({
        "success": true,
        "track_index": track_index,
        "segment_index": segment_index,
        "new_timeline_offset_ms": new_timeline_offset_ms,
        "shift_timeline": shift_timeline,
        "affected_segments": affected.segments.len(),
    }))
}

/// Copy a segment to a new position in the same track.
pub fn copy_segment(
    track_index: usize,
    segment_index: usize,
    target_index: Option<usize>,
) -> Result<serde_json::Value, McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;

    let cmd = CopySegmentCommand::new(track_index, segment_index, target_index);
    let affected = state::execute_command(Box::new(cmd))
        .map_err(|e| McpError::Internal(format!("CopySegmentCommand failed: {}", e)))?;

    state::sync_ui();

    Ok(serde_json::json!({
        "success": true,
        "track_index": track_index,
        "source_segment_index": segment_index,
        "target_index": target_index,
        "affected_segments": affected.segments.len(),
    }))
}
