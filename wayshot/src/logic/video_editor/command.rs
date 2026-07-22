use super::{
    preview::seek_to_position,
    project::{PROJECT_STATE, ProjectState},
    segment::refresh_affected_segments,
};
use crate::{
    global_store, global_ve_filter,
    logic::tr::tr,
    logic_cb,
    slint_generatedAppWindow::{AppWindow, SelectedSegmentIndex as UISelectedSegmentIndex},
};
use slint::{ComponentHandle, Model, VecModel};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;
use video_editor::{
    commands::{
        AffectedSegment, AffectedSegments, BatchCommand, segment::InsertSegmentAtTimeCommand,
        segment::InsertSegmentCommand, segment::RemoveSegmentCommand,
        segment::ShiftSubsequentSegmentsCommand, segment::SplitSegmentCommand,
    },
    metadata::MetadataType,
    tracks::{segment::Segment, track::Track},
};

static CLIPBOARD: Mutex<Option<ClipboardContent>> = Mutex::new(None);

struct ClipboardContent {
    segments: Vec<ClipboardSegment>,
}

#[derive(Clone)]
struct ClipboardSegment {
    track_index: usize,
    segment_index: usize, // Original index for removal operations
    segment: Segment,
}

#[macro_export]
macro_rules! store_video_editor_selected_segments_index {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_selected_segments_index()
            .as_any()
            .downcast_ref::<VecModel<UISelectedSegmentIndex>>()
            .expect("We know we set a VecModel<UISelectedSegmentIndex> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_undo, ui);
    logic_cb!(video_editor_redo, ui);
    logic_cb!(video_editor_cut, ui);
    logic_cb!(video_editor_copy, ui);
    logic_cb!(video_editor_paste, ui);
    logic_cb!(video_editor_append, ui);
}

fn video_editor_undo(ui: &AppWindow) {
    let result =
        with_history_manager(|state| state.history_manager.undo(&mut state.tracks_manager));

    match result {
        Ok(undo_result) => {
            sync_and_refresh(ui, undo_result.affected_segments.clone(), Some(true));

            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );

            global_ve_filter!(ui)
                .set_toggle_keyframe_flag(!global_ve_filter!(ui).get_toggle_keyframe_flag());

            crate::toast_success!(ui, format!("{}: {}", tr("Undo"), undo_result.description));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Undo failed"), e)),
    }
}

fn video_editor_redo(ui: &AppWindow) {
    let result =
        with_history_manager(|state| state.history_manager.redo(&mut state.tracks_manager));

    match result {
        Ok(redo_result) => {
            sync_and_refresh(ui, redo_result.affected_segments.clone(), Some(true));

            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );

            global_ve_filter!(ui)
                .set_toggle_keyframe_flag(!global_ve_filter!(ui).get_toggle_keyframe_flag());

            crate::toast_success!(ui, format!("{}: {}", tr("Redo"), redo_result.description));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Redo failed"), e)),
    }
}

fn video_editor_cut(ui: &AppWindow) {
    let selected_segments = get_selected_segments(&ui);

    if selected_segments.is_empty() {
        crate::toast_warn!(ui, tr("No segments selected"));
        return;
    }

    let shift_timeline = global_store!(ui)
        .get_video_editor_ui_state()
        .enabled_link_track;

    let clipboard_content = ClipboardContent {
        segments: selected_segments.clone(),
    };
    *CLIPBOARD.lock().unwrap() = Some(clipboard_content);

    let mut batch_command = BatchCommand::new("Cut segments".to_string());
    let mut segments_per_track: HashMap<usize, Vec<usize>> = HashMap::new();

    for seg in &selected_segments {
        segments_per_track
            .entry(seg.track_index)
            .or_default()
            .push(seg.segment_index);
    }

    // Add remove commands in reverse order (to maintain valid indices)
    for (track_idx, mut seg_indices) in segments_per_track {
        seg_indices.sort_by(|a, b| b.cmp(a)); // Reverse sort
        for seg_idx in seg_indices {
            batch_command.add_command(Box::new(RemoveSegmentCommand::new(
                track_idx,
                seg_idx,
                shift_timeline,
            )));
        }
    }

    // Execute through HistoryManager for undo/redo support
    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(_execute_result) => {
            sync_manager_to_ui(ui);
            store_video_editor_selected_segments_index!(ui).set_vec(vec![]);
            crate::toast_success!(ui, format!("{} {} {}", tr("Cut"), selected_segments.len(), tr("segments")));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Cut failed"), e)),
    }
}

fn video_editor_copy(ui: &AppWindow) {
    let selected_segments = get_selected_segments(&ui);

    if selected_segments.is_empty() {
        crate::toast_warn!(ui, tr("No segments selected"));
        return;
    }

    let clipboard_content = ClipboardContent {
        segments: selected_segments.clone(),
    };
    *CLIPBOARD.lock().unwrap() = Some(clipboard_content);

    crate::toast_success!(ui, format!("{} {} {}", tr("Copied"), selected_segments.len(), tr("segments")));
}

fn video_editor_paste(ui: &AppWindow) {
    let timeline_offset_ms = global_store!(ui).get_video_editor_timeline_offset();
    let track_idx = global_store!(ui).get_video_editor_current_edited_track_index();
    let shift_timeline = global_store!(ui)
        .get_video_editor_ui_state()
        .enabled_link_track;

    if track_idx < 0 {
        crate::toast_warn!(ui, tr("No found current edited track"));
        return;
    }

    let track_idx = track_idx as usize;

    let timeline_offset = Duration::from_millis(timeline_offset_ms as u64);
    let clipboard_segments = CLIPBOARD
        .lock()
        .unwrap()
        .as_ref()
        .map(|c| c.segments.clone());

    let segments_to_paste = match clipboard_segments {
        Some(segments) if !segments.is_empty() => segments,
        _ => {
            crate::toast_warn!(ui, tr("Nothing to paste"));
            return;
        }
    };

    let first_metadata_type = segments_to_paste[0].segment.metadata.get_type();
    for seg in &segments_to_paste {
        if seg.segment.metadata.get_type() != first_metadata_type {
            crate::toast_warn!(ui, tr("Cannot paste segments of different types"));
            return;
        }
    }

    let is_text_segment = segments_to_paste[0].segment.text_element.is_some();

    let pasted_count = segments_to_paste.len();
    let mut batch_command = BatchCommand::new("Paste segments".to_string());

    let result = with_history_manager(|state| {
        // Check track type matches segment type
        let track = state.tracks_manager.get(track_idx).ok_or_else(|| {
            video_editor::Error::IndexOutOfBounds(track_idx, state.tracks_manager.len())
        })?;

        let track_matches = match track {
            Track::Video(_) => first_metadata_type == MetadataType::Video,
            Track::Audio(_) => first_metadata_type == MetadataType::Audio,
            Track::Subtitle(_) => first_metadata_type == MetadataType::Subtitle,
            Track::Image(_) => first_metadata_type == MetadataType::Image,
            Track::Text(_) => is_text_segment,
        };

        if !track_matches {
            return Err(video_editor::Error::InvalidConfig(
                format!(
                    "Cannot paste {:?} segments to {:?} track. Track types do not match.",
                    first_metadata_type,
                    match track {
                        Track::Video(_) => "Video",
                        Track::Audio(_) => "Audio",
                        Track::Subtitle(_) => "Subtitle",
                        Track::Image(_) => "Image",
                        Track::Text(_) => "Text",
                    }
                )
                .into(),
            ));
        }

        let mut need_split = false;
        let mut split_segment_index = 0;
        let mut insert_index = track.segments_count();
        let mut split_time = Duration::ZERO;

        for (i, segment) in track.segments().iter().enumerate() {
            let segment_start = segment.timeline_offset;
            let segment_end = segment.timeline_offset + segment.duration;

            if timeline_offset >= segment_start && timeline_offset < segment_end {
                // Pasting in the middle of a segment, need to split
                insert_index = i + 1;
                need_split = true;
                split_segment_index = i;
                split_time = timeline_offset - segment_start;
                break;
            } else if timeline_offset < segment_start {
                // Pasting before this segment
                insert_index = i;
                break;
            }
        }

        let total_pasted_duration: Duration =
            segments_to_paste.iter().map(|s| s.segment.duration).sum();

        // Check for potential overlap when not splitting and link mode disabled
        // If there's a next segment, calculate available space and truncate if needed
        let mut last_segment_new_duration: Option<Duration> = None;

        if !need_split && !shift_timeline && insert_index < track.segments_count() {
            let next_segment = &track.segments()[insert_index];
            let next_segment_start = next_segment.timeline_offset;
            let available_space = next_segment_start.saturating_sub(timeline_offset);

            if total_pasted_duration > available_space {
                if available_space == Duration::ZERO {
                    return Err(video_editor::Error::InvalidConfig(
                        "No space to paste: playhead is at the start of the next segment".into(),
                    ));
                }

                // Calculate how much we need to truncate from the last segment
                let overflow = total_pasted_duration.saturating_sub(available_space);
                let last_seg_idx = segments_to_paste.len() - 1;
                let last_seg_original_duration = segments_to_paste[last_seg_idx].segment.duration;

                if overflow >= last_seg_original_duration {
                    return Err(video_editor::Error::InvalidConfig(
                        format!(
                            "Not enough space: pasted content ({}ms) exceeds available space ({}ms)",
                            total_pasted_duration.as_millis(),
                            available_space.as_millis()
                        )
                        .into(),
                    ));
                }

                last_segment_new_duration =
                    Some(last_seg_original_duration.saturating_sub(overflow));
            }
        }

        // If splitting is needed
        if need_split {
            batch_command.add_command(Box::new(SplitSegmentCommand::new(
                track_idx,
                split_segment_index,
                split_time,
            )));

            // The right segment after split will be at index split_segment_index + 1 initially,
            // but after inserting segments_to_paste.len() segments, its final position will be
            // split_segment_index + 1 + segments_to_paste.len()
            let right_segment_final_index = split_segment_index + 1 + segments_to_paste.len();
            batch_command.add_extra_affected_segment(AffectedSegment::with_both_thumbnails(
                track_idx,
                right_segment_final_index,
            ));
        }

        // Insert segments at playhead position
        let mut current_offset = timeline_offset;
        for (i, clipboard_seg) in segments_to_paste.iter().enumerate() {
            let mut new_segment = clipboard_seg.segment.clone();
            new_segment.uuid = Uuid::new_v4().to_string();
            new_segment.timeline_offset = current_offset;

            // Truncate last segment if needed (when link mode disabled and overlap detected)
            let is_last_segment = i == segments_to_paste.len() - 1;
            if is_last_segment && let Some(new_duration) = last_segment_new_duration {
                new_segment.duration = new_duration;
            }

            let segment_arc = Arc::new(new_segment);

            // Use InsertSegmentAtTimeCommand to preserve timeline_offset
            // For split case, shift_timeline=true for all segments to shift the split_part2
            // For gap case, shift_timeline=false, we handle shifting separately
            let should_shift = need_split;
            batch_command.add_command(Box::new(InsertSegmentAtTimeCommand::new(
                track_idx,
                insert_index,
                segment_arc,
                should_shift,
            )));

            // Update offset for next segment to be placed after current one
            // Use the actual duration (possibly truncated for last segment)
            current_offset += if is_last_segment {
                last_segment_new_duration.unwrap_or(clipboard_seg.segment.duration)
            } else {
                clipboard_seg.segment.duration
            };
            insert_index += 1;
        }

        // Calculate actual total duration (may be different if truncated)
        let actual_total_duration = if let Some(new_dur) = last_segment_new_duration {
            let last_idx = segments_to_paste.len() - 1;
            total_pasted_duration - segments_to_paste[last_idx].segment.duration + new_dur
        } else {
            total_pasted_duration
        };

        // If link mode is enabled and not splitting, shift subsequent segments by total duration
        if shift_timeline && !need_split && actual_total_duration > Duration::ZERO {
            batch_command.add_command(Box::new(ShiftSubsequentSegmentsCommand::new(
                track_idx,
                insert_index, // Shift from the index after all pasted segments
                actual_total_duration,
            )));
        }

        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_manager_to_ui(ui);
            refresh_affected_segments(ui, execute_result.affected_segments);
            crate::toast_success!(
                ui,
                format!(
                    "{} {} {} {}ms",
                    tr("Pasted"), pasted_count, tr("segments at"), timeline_offset_ms
                )
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_append(ui: &AppWindow) {
    let track_idx = global_store!(ui).get_video_editor_current_edited_track_index();

    if track_idx < 0 {
        crate::toast_warn!(ui, tr("No found current edited track"));
        return;
    }

    let track_idx = track_idx as usize;
    let clipboard_segments = CLIPBOARD
        .lock()
        .unwrap()
        .as_ref()
        .map(|c| c.segments.clone());

    let segments_to_append = match clipboard_segments {
        Some(segments) if !segments.is_empty() => segments,
        _ => {
            crate::toast_warn!(ui, tr("Nothing in clipboard to append"));
            return;
        }
    };

    // Check all segments are of consistent type
    let first_metadata_type = segments_to_append[0].segment.metadata.get_type();
    for seg in &segments_to_append {
        if seg.segment.metadata.get_type() != first_metadata_type {
            crate::toast_warn!(ui, tr("Cannot append segments of different types"));
            return;
        }
    }

    let is_text_segment = segments_to_append[0].segment.text_element.is_some();

    let append_count = segments_to_append.len();
    let mut batch_command = BatchCommand::new("Append segments".to_string());

    let result = with_history_manager(|state| {
        // Check track type matches segment type
        let track = state.tracks_manager.get(track_idx).ok_or_else(|| {
            video_editor::Error::IndexOutOfBounds(track_idx, state.tracks_manager.len())
        })?;

        let track_matches = match track {
            Track::Video(_) => first_metadata_type == MetadataType::Video,
            Track::Audio(_) => first_metadata_type == MetadataType::Audio,
            Track::Subtitle(_) => first_metadata_type == MetadataType::Subtitle,
            Track::Image(_) => first_metadata_type == MetadataType::Image,
            Track::Text(_) => is_text_segment,
        };

        if !track_matches {
            return Err(video_editor::Error::InvalidConfig(
                format!(
                    "Cannot append {:?} segments to {:?} track. Track types do not match.",
                    first_metadata_type,
                    match track {
                        Track::Video(_) => "Video",
                        Track::Audio(_) => "Audio",
                        Track::Subtitle(_) => "Subtitle",
                        Track::Image(_) => "Image",
                        Track::Text(_) => "Text",
                    }
                )
                .into(),
            ));
        }

        let timeline_offset = track.duration();
        let mut insert_index = track.segments_count();

        for clipboard_seg in &segments_to_append {
            let mut new_segment = clipboard_seg.segment.clone();
            new_segment.uuid = Uuid::new_v4().to_string();
            new_segment.timeline_offset = timeline_offset;
            let segment_arc = Arc::new(new_segment);

            batch_command.add_command(Box::new(InsertSegmentCommand::new(
                track_idx,
                insert_index,
                segment_arc,
            )));

            insert_index += 1;
        }

        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(_execute_result) => {
            sync_manager_to_ui(ui);
            crate::toast_success!(
                ui,
                format!("{} {} {} {}", tr("Appended"), append_count, tr("segments to track"), track_idx)
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn get_selected_segments(ui: &AppWindow) -> Vec<ClipboardSegment> {
    let mut result = Vec::new();

    for selected_idx in store_video_editor_selected_segments_index!(ui).iter() {
        let track_idx = selected_idx.track_index as usize;
        let seg_idx = selected_idx.index as usize;

        let segment_opt = with_history_manager(|state| {
            state
                .tracks_manager
                .get(track_idx)
                .and_then(|track| match track {
                    Track::Video(inner) => inner.track.segments.get(seg_idx).cloned(),
                    Track::Audio(inner) => inner.track.segments.get(seg_idx).cloned(),
                    Track::Subtitle(inner) => inner.track.segments.get(seg_idx).cloned(),
                    Track::Image(inner) => inner.track.segments.get(seg_idx).cloned(),
                    Track::Text(inner) => inner.track.segments.get(seg_idx).cloned(),
                })
        });

        if let Some(segment) = segment_opt {
            result.push(ClipboardSegment {
                track_index: track_idx,
                segment_index: seg_idx,
                segment: (*segment).clone(),
            });
        }
    }

    result
}

pub fn sync_manager_to_ui(ui: &AppWindow) {
    let (new_duration, ui_manager) = with_history_manager(|state| {
        if let Some(ref autosave) = state.autosave_manager {
            autosave.mark_dirty();
        }

        let manager = state.tracks_manager.clone();
        (manager.duration, manager.into())
    });

    let current_offset = global_store!(ui).get_video_editor_timeline_offset();
    let new_offset_ms = new_duration.as_millis() as i32;
    if current_offset > new_offset_ms {
        global_store!(ui).set_video_editor_timeline_offset(new_offset_ms);
    }

    global_store!(ui).set_video_editor_tracks_manager(ui_manager);
    global_store!(ui).set_video_editor_is_unsaved(true);
}

pub fn with_history_manager<F, R>(f: F) -> R
where
    F: FnOnce(&mut ProjectState) -> R,
{
    let mut state = PROJECT_STATE.lock().unwrap();
    if let Some(ref mut s) = *state {
        f(s)
    } else {
        panic!("Project state not initialized");
    }
}

pub fn refresh_preview(ui: &AppWindow) {
    if global_store!(ui).get_video_editor_is_previewing() {
        return;
    }

    let position_ms = global_store!(ui).get_video_editor_timeline_offset();
    let position = Duration::from_millis(position_ms as u64);
    seek_to_position(ui, position, false);
}

pub fn sync_and_refresh(ui: &AppWindow, affected: AffectedSegments, force_preview: Option<bool>) {
    sync_manager_to_ui(ui);

    let should_refresh_preview = force_preview.unwrap_or(affected.tracks_changed);

    if !affected.is_empty() {
        refresh_affected_segments(ui, affected);
    }

    if should_refresh_preview {
        refresh_preview(ui);
    }
}

pub fn sync_and_refresh_simple(ui: &AppWindow) {
    sync_manager_to_ui(ui);
    refresh_preview(ui);
}

pub fn sync_and_refresh_tracks_changed(ui: &AppWindow, tracks_changed: bool) {
    sync_manager_to_ui(ui);
    if tracks_changed {
        refresh_preview(ui);
    }
}

pub fn sync_and_refresh_tracks_only(ui: &AppWindow, affected: AffectedSegments) {
    sync_manager_to_ui(ui);
    if affected.tracks_changed {
        refresh_preview(ui);
    }
}
