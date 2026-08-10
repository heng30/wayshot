use super::{
    command::{
        sync_and_refresh, sync_and_refresh_simple, sync_and_refresh_tracks_only,
        sync_manager_to_ui, with_history_manager,
    },
    common_type::SubtitleStyleConfig,
    export::{
        add_export_task, next_export_task_id, picker_save_file, register_cancellation_token,
        remove_cancellation_token, update_export_task_progress,
    },
    filters::subtitle::create_subtitle_style_filters_from_config,
    project::PROJECT_STATE,
    track::{get_selected_segment_indices, is_track_locked},
    vad::{detect_voice_segments, to_mono},
};
use crate::{
    global_logic, global_store, global_ve_filter,
    logic::{recorder::picker_directory, toast, tr::tr},
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, FilterEntry as UIFilterEntry, FilterType as UIFilterType,
        MediaType as UIMediaType, PresetFilter as UIPresetFilter,
        SelectedSegmentIndex as UISelectedSegmentIndex, SnapResult as UISnapResult,
        VideoEditorSegmentMetadata as UIVideoEditorSegmentMetadata,
        VideoEditorSubtitle as UIVideoEditorSubtitle,
        VideoEditorTrackSegment as UIVideoEditorTrackSegment,
    },
    store_video_editor_selected_segments_index,
};
use hound::{SampleFormat, WavSpec, WavWriter};
use image::{Delay, Frame, RgbaImage, codecs::gif::GifEncoder};
use slint::{ComponentHandle, Model, ModelRc, VecModel, Weak};
use std::{collections::HashMap, fs::File, sync::Arc, time::Duration};
use video_editor::{
    Error,
    commands::{
        AffectedSegment, AffectedSegments, ExecuteResult,
        batch::BatchCommand,
        filter::AddFilterCommand,
        segment::{
            ClearSegmentFiltersCommand, ClearSegmentKeyframesCommand, DetachSegmentAudioCommand,
            DetachSegmentSubtitleCommand, InsertSegmentAtTimeCommand, MergeSegmentsCommand,
            MoveSegmentToTimeCommand, RemoveSegmentCommand, RemoveSegmentGapCommand,
            RemoveSegmentLeftGapCommand, RemoveSegmentRightGapCommand, SetPlaybackSpeedCommand,
            ShrinkSegmentLeftCommand, ShrinkSegmentRightCommand, SplitSegmentCommand,
            StretchSegmentLeftCommand, StretchSegmentRightCommand, ToggleSegmentAudioMutedCommand,
            ToggleSegmentVisibilityCommand,
        },
    },
    export::{SegmentExportConfig, SegmentExporter, progress::CancellationToken},
    filters::{audio::AudioSpeedFilter, video::SpeedFilter},
    tracks::{
        audio_track::extract_segment_audio, manager::Manager, segment::Segment, track::Track,
        video_frame_cache::VideoImage,
    },
};
use video_utils::{convert::resize_rgba_image, subtitle::ms_to_srt_timestamp};

/// 显示用波形采样率（每声道每秒采样数），供 UI 波形渲染使用。
/// 缓存层（lib）仍为 60Hz，此处只是请求的目标显示采样率。
const DISPLAY_AUDIO_SAMPLES_PER_SECOND: u32 = 30;
const THUMBNAIL_HEIGHT: u32 = 90;
const GIF_MAX_WIDTH: u32 = 854;
const GIF_MAX_HEIGHT: u32 = 480;
const GIF_FPS: f64 = 10.0;

#[macro_export]
macro_rules! store_video_editor_tracks_manager_track_segment {
    ($segments:expr) => {
        $segments
            .as_any()
            .downcast_ref::<VecModel<UIVideoEditorTrackSegment>>()
            .expect("We know we set a VecModel<UIVideoEditorTrackSegment> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_add_selected_segment, ui, index);
    logic_cb!(video_editor_select_all_segments, ui);
    logic_cb!(video_editor_remove_segments, ui);
    logic_cb!(video_editor_linked_remove_segments, ui);
    logic_cb!(video_editor_split_segment, ui);
    logic_cb!(video_editor_commit_segment_move, ui, index, final_offset_ms);
    logic_cb!(
        video_editor_commit_segment_resize,
        ui,
        index,
        is_left,
        new_duration_ms,
        new_offset_ms
    );
    logic_cb!(
        video_editor_commit_segment_cross_track_move,
        ui,
        source_track_index,
        source_segment_index,
        target_track_index,
        target_timeline_offset_ms,
        will_split,
        split_segment_index
    );
    logic_cb!(
        video_editor_find_segment_at_time,
        ui,
        track_index,
        timeline_offset_ms
    );
    logic_cb!(
        video_editor_find_segment_at_time_excluding,
        ui,
        track_index,
        timeline_offset_ms,
        exclude_segment_index
    );
    logic_cb!(
        video_editor_find_snap_position,
        ui,
        offset_ms,
        threshold_ms,
        exclude_track_index,
        exclude_segment_index,
        target_track_index
    );
    logic_cb!(video_editor_segment_detach_audio, ui, index);
    logic_cb!(video_editor_segment_detach_subtitle, ui, index);
    logic_cb!(video_editor_segment_remove_gap, ui, index);
    logic_cb!(video_editor_segment_remove_left_gap, ui, index);
    logic_cb!(video_editor_segment_remove_right_gap, ui, index);
    logic_cb!(video_editor_segment_snap_to_previous, ui);
    logic_cb!(video_editor_segment_snap_to_playhead, ui);
    logic_cb!(video_editor_segment_merge, ui);
    logic_cb!(video_editor_segment_resize_to_playhead, ui, index);
    logic_cb!(video_editor_segment_resize_to_previous_segment, ui, index);
    logic_cb!(video_editor_segment_resize_to_next_segment, ui, index);
    logic_cb!(
        video_editor_segment_has_keyframes,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(
        video_editor_remove_all_segment_keyframes,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(
        video_editor_video_segment_thumbnail,
        ui,
        track_index,
        index,
        is_left
    );
    logic_cb!(
        video_editor_is_selected_segment,
        ui,
        selected_segment,
        index,
        _flag
    );
    logic_cb!(video_editor_selected_segment, ui, _flag, _flag2);
    logic_cb!(video_editor_selected_segment_metadata, ui, _flag);
    logic_cb!(video_editor_selected_segment_relative_start, ui);
    logic_cb!(video_editor_get_min_all_tracks_offset, ui);
    logic_cb_pure!(
        video_editor_is_link_all_movable,
        ui,
        track_index,
        segment_index,
        drag_original_offset,
        drag_original_duration
    );
    logic_cb!(
        video_editor_update_edited_subtitle_from_segment,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(video_editor_segment_has_filter, ui, entry);
    logic_cb!(video_editor_segment_has_preset_filter, ui, entry);
    logic_cb!(
        video_editor_segment_contain_filter,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(
        video_editor_segment_remove_all_filters,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(video_editor_segment_export_gif, ui, index);
    logic_cb!(video_editor_segment_export_selected_mp4, ui);
    logic_cb!(video_editor_segment_extract_frames, ui, index);
    logic_cb!(video_editor_segment_export_audio, ui, index);
    logic_cb!(video_editor_segment_toggle_enable, ui, index);
    logic_cb!(video_editor_segment_toggle_audio, ui, index);
    logic_cb!(
        video_editor_segment_intelligent_voice_segmentation,
        ui,
        index
    );
}

fn video_editor_add_selected_segment(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let is_selected = store_video_editor_selected_segments_index!(ui)
        .iter()
        .any(|s| s.track_index == index.track_index && s.index == index.index);

    let mut selected_indices: Vec<UISelectedSegmentIndex> =
        store_video_editor_selected_segments_index!(ui)
            .iter()
            .collect();

    if index.modifiers.control {
        if is_selected {
            selected_indices
                .retain(|s| !(s.track_index == index.track_index && s.index == index.index));
        } else {
            selected_indices.push(index.clone());
        }
    } else if index.modifiers.shift {
        if let Some(last) = selected_indices.last() {
            if last.track_index == index.track_index {
                let start = last.index.min(index.index);
                let end = last.index.max(index.index);
                for i in start..=end {
                    if !selected_indices
                        .iter()
                        .any(|s| s.track_index == index.track_index && s.index == i)
                    {
                        selected_indices.push(UISelectedSegmentIndex {
                            track_index: index.track_index,
                            index: i,
                            modifiers: Default::default(),
                        });
                    }
                }
            } else {
                selected_indices.push(index.clone());
            }
        } else {
            selected_indices.push(index.clone());
        }
    } else {
        if is_selected && selected_indices.len() == 1 {
            selected_indices.clear();
        } else {
            selected_indices.clear();
            selected_indices.push(index.clone());
        }
    }

    selected_indices.sort_by(|a, b| match a.track_index.cmp(&b.track_index) {
        std::cmp::Ordering::Equal => a.index.cmp(&b.index),
        other => other,
    });

    global_ve_filter!(ui).set_selected_filter_index(-1);
    store_video_editor_selected_segments_index!(ui).set_vec(selected_indices);

    // Trigger segment-selected-flag change to ensure SegmentFilter component updates
    let segment_flag = global_store!(ui).get_video_editor_segment_selected_flag();
    global_store!(ui).set_video_editor_segment_selected_flag(!segment_flag);
}

fn video_editor_select_all_segments(ui: &AppWindow) {
    let all_segments: Vec<UISelectedSegmentIndex> = with_history_manager(|state| {
        let mut result = Vec::new();

        for (track_idx, track) in state.tracks_manager.iter().enumerate() {
            let segments_len = match track {
                Track::Video(inner) => inner.track.segments.len(),
                Track::Audio(inner) => inner.track.segments.len(),
                Track::Subtitle(inner) => inner.track.segments.len(),
                Track::Image(inner) => inner.track.segments.len(),
                Track::Text(inner) => inner.track.segments.len(),
            };

            for seg_idx in 0..segments_len {
                result.push(UISelectedSegmentIndex {
                    track_index: track_idx as i32,
                    index: seg_idx as i32,
                    modifiers: Default::default(),
                });
            }
        }

        result
    });

    store_video_editor_selected_segments_index!(ui).set_vec(all_segments);
}

fn video_editor_is_selected_segment(
    _ui: &AppWindow,
    selected_segment: ModelRc<UISelectedSegmentIndex>,
    index: UISelectedSegmentIndex,
    _flag: bool,
) -> bool {
    selected_segment
        .iter()
        .find(|s| s.track_index == index.track_index && s.index == index.index)
        .is_some()
}

fn video_editor_selected_segment(
    ui: &AppWindow,
    _flag: bool,
    _flag2: bool,
) -> UIVideoEditorTrackSegment {
    let selected_indices = get_selected_segment_indices(ui);

    if let Some((track_idx, seg_idx)) = selected_indices.last()
        && let Some(segment_arc) = with_history_manager(|state| {
            state
                .tracks_manager
                .get(*track_idx)
                .and_then(|track| match track {
                    Track::Video(inner) => inner.track.segments.get(*seg_idx).cloned(),
                    Track::Audio(inner) => inner.track.segments.get(*seg_idx).cloned(),
                    Track::Subtitle(inner) => inner.track.segments.get(*seg_idx).cloned(),
                    Track::Image(inner) => inner.track.segments.get(*seg_idx).cloned(),
                    Track::Text(inner) => inner.track.segments.get(*seg_idx).cloned(),
                })
        })
    {
        let ui_segment: UIVideoEditorTrackSegment = segment_arc.into();
        return ui_segment;
    }

    UIVideoEditorTrackSegment::default()
}

pub fn get_selected_segment_metadata(ui: &AppWindow) -> UIVideoEditorSegmentMetadata {
    let selected_indices = get_selected_segment_indices(ui);

    if let Some((track_idx, seg_idx)) = selected_indices.last()
        && let Some(segment_arc) = with_history_manager(|state| {
            state
                .tracks_manager
                .get(*track_idx)
                .and_then(|track| match track {
                    Track::Video(inner) => inner.track.segments.get(*seg_idx).cloned(),
                    Track::Audio(inner) => inner.track.segments.get(*seg_idx).cloned(),
                    Track::Subtitle(inner) => inner.track.segments.get(*seg_idx).cloned(),
                    Track::Image(inner) => inner.track.segments.get(*seg_idx).cloned(),
                    Track::Text(inner) => inner.track.segments.get(*seg_idx).cloned(),
                })
        })
    {
        return (&*segment_arc.metadata).into();
    }

    UIVideoEditorSegmentMetadata::default()
}

fn video_editor_selected_segment_metadata(
    ui: &AppWindow,
    _flag: bool,
) -> UIVideoEditorSegmentMetadata {
    get_selected_segment_metadata(ui)
}

fn video_editor_selected_segment_relative_start(ui: &AppWindow) -> i32 {
    let playhead_ms = global_store!(ui).get_video_editor_timeline_offset();
    let segment = video_editor_selected_segment(ui, false, false);
    (playhead_ms - segment.timeline_offset).max(0)
}

fn video_editor_remove_segments(ui: &AppWindow) {
    let shift_timeline = global_store!(ui)
        .get_video_editor_ui_state()
        .enabled_link_track;

    inner_video_editor_remove_segments(ui, shift_timeline);
}

fn video_editor_linked_remove_segments(ui: &AppWindow) {
    inner_video_editor_remove_segments(ui, true);
}

fn inner_video_editor_remove_segments(ui: &AppWindow, shift_timeline: bool) {
    let selected_segment_indices = get_selected_segment_indices(ui);

    if selected_segment_indices.is_empty() {
        crate::toast_warn!(ui, tr("No segments selected"));
        return;
    }

    let mut batch_command = BatchCommand::new(format!(
        "Remove {} segments",
        selected_segment_indices.len()
    ));

    let mut segments_per_track: HashMap<usize, Vec<usize>> = HashMap::new();

    for (track_idx, seg_idx) in &selected_segment_indices {
        segments_per_track
            .entry(*track_idx)
            .or_default()
            .push(*seg_idx);
    }

    // Add remove commands in reverse order (to maintain valid indices)
    for (track_idx, mut seg_indices) in segments_per_track {
        seg_indices.sort_by(|a, b| b.cmp(a)); // Sort descending
        for seg_idx in seg_indices {
            batch_command.add_command(Box::new(RemoveSegmentCommand::new(
                track_idx,
                seg_idx,
                shift_timeline,
            )));
        }
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(_) => {
            sync_and_refresh_simple(ui);
            store_video_editor_selected_segments_index!(ui).set_vec(vec![]);
            crate::toast_success!(
                ui,
                format!("Removed {} segments", selected_segment_indices.len())
            );
        }
        Err(e) => {
            crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove segments"), e));
        }
    }

    global_ve_filter!(ui).set_selected_filter_index(-1);
}

fn video_editor_split_segment(ui: &AppWindow) {
    let ui_state = global_store!(ui).get_video_editor_ui_state();
    let timeline_offset_ms = global_store!(ui).get_video_editor_timeline_offset();
    let current_track_index = global_store!(ui).get_video_editor_current_edited_track_index();

    if is_track_locked(ui, current_track_index) {
        crate::toast_warn!(ui, tr("Cannot split a locked tracks' segment"));
        return;
    }

    // 记录分割前的segment数量，用于确定新segment
    let segments_to_split: Vec<(usize, usize)> = with_history_manager(|state| {
        let timeline_position = Duration::from_millis(timeline_offset_ms as u64);
        let mut result = Vec::new();

        if ui_state.enabled_link_all_tracks {
            for (track_idx, track) in state.tracks_manager.iter().enumerate() {
                if matches!(track, Track::Text(_)) {
                    continue;
                }
                if track.is_locked() {
                    continue;
                }

                let segments: &[Arc<Segment>] = match track {
                    Track::Video(inner) => &inner.track.segments,
                    Track::Audio(inner) => &inner.track.segments,
                    Track::Subtitle(inner) => &inner.track.segments,
                    Track::Image(inner) => &inner.track.segments,
                    Track::Text(inner) => &inner.track.segments,
                };

                for (seg_idx, seg) in segments.iter().enumerate() {
                    let seg_start = seg.timeline_offset;
                    let seg_end = seg_start + seg.duration;

                    if timeline_position > seg_start && timeline_position < seg_end {
                        result.push((track_idx, seg_idx));
                    }
                }
            }
        } else {
            if current_track_index < 0 {
                return result;
            }

            let track_idx = current_track_index as usize;
            if track_idx >= state.tracks_manager.len() {
                return result;
            }

            let track = state.tracks_manager.get(track_idx).unwrap();
            let segments: &[Arc<Segment>] = match track {
                Track::Video(inner) => &inner.track.segments,
                Track::Audio(inner) => &inner.track.segments,
                Track::Subtitle(inner) => &inner.track.segments,
                Track::Image(inner) => &inner.track.segments,
                Track::Text(inner) => &inner.track.segments,
            };

            for (seg_idx, seg) in segments.iter().enumerate() {
                let seg_start = seg.timeline_offset;
                let seg_end = seg_start + seg.duration;

                if timeline_position > seg_start && timeline_position < seg_end {
                    result.push((track_idx, seg_idx));
                }
            }
        }
        result
    });

    if segments_to_split.is_empty() {
        if current_track_index < 0 {
            crate::toast_warn!(ui, tr("Please select a track or segment"));
        } else {
            crate::toast_warn!(ui, tr("No segment found at the current timeline position"));
        }
        return;
    }

    let split_count = segments_to_split.len();

    let result: Result<ExecuteResult, String> = with_history_manager(|state| {
        let mut batch_command = BatchCommand::new(format!("Split {} segment(s)", split_count));

        for (track_idx, seg_idx) in &segments_to_split {
            let track = state.tracks_manager.get(*track_idx).unwrap();
            let segments: &[Arc<Segment>] = match track {
                Track::Video(inner) => &inner.track.segments,
                Track::Audio(inner) => &inner.track.segments,
                Track::Subtitle(inner) => &inner.track.segments,
                Track::Image(inner) => &inner.track.segments,
                Track::Text(inner) => &inner.track.segments,
            };

            if let Some(seg) = segments.get(*seg_idx) {
                let seg_start = seg.timeline_offset;
                let split_position = timeline_offset_ms - seg_start.as_millis() as i32;

                if split_position > 0 {
                    let split_duration = Duration::from_millis(split_position as u64);
                    batch_command.add_command(Box::new(SplitSegmentCommand::new(
                        *track_idx,
                        *seg_idx,
                        split_duration,
                    )));
                }
            }
        }

        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(false));
            crate::toast_success!(
                ui,
                format!("{} {} {}", tr("Split"), split_count, tr("segment(s)"))
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_video_segment_thumbnail(
    ui: &AppWindow,
    track_index: i32,
    index: i32,
    is_left: bool,
) {
    let tracks_manager = with_history_manager(|state| state.tracks_manager.clone());

    let Some(uuid) = with_history_manager(|state| {
        state
            .tracks_manager
            .get(track_index as usize)
            .and_then(|track| match track {
                Track::Video(inner) => inner
                    .track
                    .segments
                    .get(index as usize)
                    .map(|s| s.uuid.clone()),
                Track::Image(inner) => inner
                    .track
                    .segments
                    .get(index as usize)
                    .map(|s| s.uuid.clone()),
                _ => None,
            })
    }) else {
        return;
    };

    async_load_segment_thumbnail(
        ui.as_weak(),
        tracks_manager,
        track_index as usize,
        index as usize,
        uuid,
        is_left,
    );
}

pub fn async_load_segment_audio(
    ui: Weak<AppWindow>,
    tracks_manager: Manager,
    track_index: usize,
    segment_index: usize,
    uuid: String,
) {
    tokio::task::spawn_blocking(move || {
        let seg = tracks_manager
            .get(track_index)
            .and_then(|track| match track {
                Track::Video(inner) => inner.track.segments.get(segment_index).cloned(),
                Track::Audio(inner) => inner.track.segments.get(segment_index).cloned(),
                Track::Subtitle(inner) => inner.track.segments.get(segment_index).cloned(),
                Track::Image(inner) => inner.track.segments.get(segment_index).cloned(),
                Track::Text(inner) => inner.track.segments.get(segment_index).cloned(),
            });

        let Some(seg) = seg else {
            log::warn!(
                "Segment not found: track={}, segment={}",
                track_index,
                segment_index
            );
            return;
        };

        let (channels, audio_samples) = seg.audio_resampling_for_display(
            (seg.duration.as_secs_f64() * DISPLAY_AUDIO_SAMPLES_PER_SECOND as f64).ceil() as u32,
        );

        log::debug!(
            "segment[{uuid}]: load {} audio samples",
            audio_samples.len()
        );

        with_history_manager(|state| {
            for i in 0..state.tracks_manager.len() {
                let track = state.tracks_manager.get_mut(i);
                let Some(track) = track else { continue };

                let segments: &mut Vec<Arc<Segment>> = match track {
                    Track::Video(inner) => &mut Arc::make_mut(inner).track.segments,
                    Track::Audio(inner) => &mut Arc::make_mut(inner).track.segments,
                    Track::Subtitle(inner) => &mut Arc::make_mut(inner).track.segments,
                    Track::Image(inner) => &mut Arc::make_mut(inner).track.segments,
                    Track::Text(inner) => &mut Arc::make_mut(inner).track.segments,
                };
                for seg in segments.iter_mut() {
                    if seg.uuid != uuid {
                        continue;
                    }

                    Arc::make_mut(seg).set_display_audio_samples(channels, audio_samples.clone());
                    log::debug!("Updated audio cache for segment uuid={}", uuid);
                    return;
                }
            }
        });

        _ = ui.upgrade_in_event_loop(move |ui| {
            update_segment_audio_ui_by_uuid(&ui, &uuid, channels, audio_samples);
        });
    });
}

pub fn async_load_segment_thumbnail(
    ui: Weak<AppWindow>,
    tracks_manager: Manager,
    track_index: usize,
    segment_index: usize,
    uuid: String,
    is_left: bool, // 判断需要更新的thumbnail, 左边还是右边的
) {
    tokio::spawn(async move {
        let Some(seg) = tracks_manager
            .get(track_index)
            .and_then(|track| match track {
                Track::Video(inner) => inner.track.segments.get(segment_index).cloned(),
                Track::Image(inner) => inner.track.segments.get(segment_index).cloned(),
                _ => None,
            })
        else {
            return;
        };

        // For image segments, load image directly from path
        // For animated WebP on Video tracks, FFmpeg can't decode it, so also use image::open
        let is_image = matches!(tracks_manager.get(track_index), Some(Track::Image(_)));
        let is_webp = seg
            .metadata
            .path
            .extension()
            .map(|e| e == "webp")
            .unwrap_or(false);

        let rgba_image = if is_image || is_webp {
            tokio::task::spawn_blocking(move || {
                image::open(&seg.metadata.path)
                    .map(|img| img.to_rgba8())
                    .ok()
            })
            .await
            .unwrap_or(None)
        } else {
            tokio::task::spawn_blocking(move || {
                if is_left {
                    seg.first_frame_image().ok()
                } else {
                    seg.last_frame_image().ok()
                }
            })
            .await
            .unwrap_or(None)
        };

        let Some(rgba) = rgba_image else {
            log::debug!("No thumbnail generated for segment uuid={}", uuid);
            return;
        };

        // Downscale to display size before storing/copying
        let rgba = resize_thumbnail(rgba);

        with_history_manager(|state| {
            for i in 0..state.tracks_manager.len() {
                let track = state.tracks_manager.get_mut(i);
                let Some(track) = track else { continue };

                let segments: &mut Vec<Arc<Segment>> = match track {
                    Track::Video(inner) => &mut Arc::make_mut(inner).track.segments,
                    Track::Audio(inner) => &mut Arc::make_mut(inner).track.segments,
                    Track::Subtitle(inner) => &mut Arc::make_mut(inner).track.segments,
                    Track::Image(inner) => &mut Arc::make_mut(inner).track.segments,
                    Track::Text(inner) => &mut Arc::make_mut(inner).track.segments,
                };
                for seg in segments.iter_mut() {
                    if seg.uuid != uuid {
                        continue;
                    }

                    let seg = Arc::make_mut(seg);
                    if is_left {
                        seg.set_display_thumbnail_left(rgba.clone());
                    } else {
                        seg.set_display_thumbnail_right(rgba.clone());
                    }
                    log::debug!(
                        "Updated {} thumbnail cache for segment uuid={}",
                        if is_left { "left" } else { "right" },
                        uuid
                    );
                    return;
                }
            }
        });

        let (width, height, pixels) = (rgba.width(), rgba.height(), rgba.into_raw());

        _ = ui.upgrade_in_event_loop(move |ui| {
            let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                &pixels, width, height,
            );
            let thumbnail = slint::Image::from_rgba8(buffer);

            update_segment_thumbnail_ui_by_uuid(&ui, &uuid, thumbnail, is_left);
        });
    });
}

fn update_segment_thumbnail_ui_by_uuid(
    ui: &AppWindow,
    uuid: &str,
    thumbnail: slint::Image,
    is_left: bool,
) {
    let manager = global_store!(ui).get_video_editor_tracks_manager();

    for (_track_idx, track) in manager.tracks.iter().enumerate() {
        for (seg_idx, mut segment) in track.segments.iter().enumerate() {
            if segment.uuid != uuid {
                continue;
            }

            if is_left {
                segment.left_thumbnail = thumbnail;
            } else {
                segment.right_thumbnail = thumbnail;
            }

            store_video_editor_tracks_manager_track_segment!(track.segments)
                .set_row_data(seg_idx, segment);

            log::trace!(
                "Updated {} thumbnail UI for segment uuid={}",
                if is_left { "left" } else { "right" },
                uuid
            );
            return;
        }
    }

    log::warn!(
        "Segment with uuid={} not found in UI for thumbnail update",
        uuid
    );
}

fn update_segment_audio_ui_by_uuid(ui: &AppWindow, uuid: &str, channels: u16, samples: Vec<f32>) {
    let manager = global_store!(ui).get_video_editor_tracks_manager();

    for (_track_idx, track) in manager.tracks.iter().enumerate() {
        for (seg_idx, mut segment) in track.segments.iter().enumerate() {
            if segment.uuid != uuid {
                continue;
            }

            segment.preview_audio_channels = channels as i32;
            segment.preview_audio_samples = ModelRc::new(VecModel::from_slice(&samples));

            store_video_editor_tracks_manager_track_segment!(track.segments)
                .set_row_data(seg_idx, segment);

            log::trace!("Updated audio UI for segment uuid={}", uuid);
            return;
        }
    }

    log::warn!("Segment with uuid={} not found in UI", uuid);
}

// 提交segment移动操作 (拖拽释放时调用)
fn video_editor_commit_segment_move(
    ui: &AppWindow,
    index: UISelectedSegmentIndex,
    final_offset_ms: i32,
) {
    let ui_state = global_store!(ui).get_video_editor_ui_state();
    let shift_timeline = ui_state.enabled_link_track;
    let link_all_tracks = ui_state.enabled_link_all_tracks;

    let seg_idx = index.index as usize;
    let track_idx = index.track_index as usize;
    let new_offset = Duration::from_millis(final_offset_ms as u64);

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        // 获取原始偏移量，计算 delta（使用有符号整数避免 saturating_sub 导致负数变0）
        let track = state.tracks_manager.get(track_idx).unwrap();
        let segment = track.get_segment(seg_idx)?;
        let original_offset_ms = segment.timeline_offset.as_millis() as i64;
        let delta_ms = final_offset_ms as i64 - original_offset_ms;

        let mut batch_command = BatchCommand::new("Move segments".to_string());

        if link_all_tracks {
            // 全部联动模式：
            // 同轨道：被拖拽 segment 之后的 segment 统一移动
            // 跨轨道：与拖拽 segment 时间重叠的 segment 及其后续 segment 统一移动（排除锁定轨道）
            batch_command.add_command(Box::new(MoveSegmentToTimeCommand::new(
                track_idx, seg_idx, new_offset, false,
            )));

            // 同轨道：只移动拖拽 segment 之后的 segment
            let segments_count = track.segments_count();
            for other_seg_idx in (seg_idx + 1)..segments_count {
                let other_seg = track.get_segment(other_seg_idx)?;
                let other_new_offset_ms = other_seg.timeline_offset.as_millis() as i64 + delta_ms;
                let other_new_offset = Duration::from_millis(other_new_offset_ms as u64);

                batch_command.add_command(Box::new(MoveSegmentToTimeCommand::new(
                    track_idx,
                    other_seg_idx,
                    other_new_offset,
                    false,
                )));
            }

            // 跨轨道：排除锁定轨道，找到与拖拽 segment 时间重叠的 segment 及其后续 segment
            let drag_start_ms = original_offset_ms as i32;
            let drag_end_ms = drag_start_ms + segment.duration.as_millis() as i32;

            for other_track_idx in 0..state.tracks_manager.len() {
                if other_track_idx == track_idx {
                    continue;
                }

                let other_track = state.tracks_manager.get(other_track_idx).unwrap();
                if other_track.is_locked() {
                    continue;
                }

                let segments_count = other_track.segments_count();
                let mut found_overlap = false;

                for other_seg_idx in 0..segments_count {
                    let other_seg = other_track.get_segment(other_seg_idx).unwrap();
                    let seg_start = other_seg.timeline_offset.as_millis() as i32;
                    let seg_end = seg_start + other_seg.duration.as_millis() as i32;

                    if !found_overlap && seg_start < drag_end_ms && drag_start_ms < seg_end {
                        found_overlap = true;
                    }

                    if found_overlap {
                        let other_new_offset_ms =
                            other_seg.timeline_offset.as_millis() as i64 + delta_ms;
                        let other_new_offset = Duration::from_millis(other_new_offset_ms as u64);

                        batch_command.add_command(Box::new(MoveSegmentToTimeCommand::new(
                            other_track_idx,
                            other_seg_idx,
                            other_new_offset,
                            false,
                        )));
                    }
                }
            }
        } else {
            // 非全部联动模式：只有当前 segment 移动，可能触发 shift_timeline
            batch_command.add_command(Box::new(MoveSegmentToTimeCommand::new(
                track_idx,
                seg_idx,
                new_offset,
                shift_timeline,
            )));
        }

        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

// 提交跨轨道 segment 移动操作（也支持同轨道移动）
fn video_editor_commit_segment_cross_track_move(
    ui: &AppWindow,
    source_track_index: i32,
    source_segment_index: i32,
    target_track_index: i32,
    target_timeline_offset_ms: i32,
    will_split: bool,
    split_segment_index: i32,
) {
    let result = with_history_manager(|state| {
        if source_track_index < 0 || source_track_index as usize >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                source_track_index as usize,
                state.tracks_manager.len(),
            ));
        }
        if target_track_index < 0 || target_track_index as usize >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                target_track_index as usize,
                state.tracks_manager.len(),
            ));
        }

        let source_track_idx = source_track_index as usize;
        let source_seg_idx = source_segment_index as usize;
        let target_track_idx = target_track_index as usize;
        let target_timeline_offset = Duration::from_millis(target_timeline_offset_ms as u64);

        let is_same_track = source_track_idx == target_track_idx;
        let source_track = state.tracks_manager.get(source_track_idx).unwrap();
        let source_segment = source_track.get_segment(source_seg_idx)?;
        let target_track = state.tracks_manager.get(target_track_idx).unwrap();

        let type_matches = match source_track {
            Track::Video(_) => matches!(target_track, Track::Video(_)),
            Track::Audio(_) => matches!(target_track, Track::Audio(_)),
            Track::Subtitle(_) => matches!(target_track, Track::Subtitle(_)),
            Track::Image(_) => matches!(target_track, Track::Image(_)),
            Track::Text(_) => matches!(target_track, Track::Text(_)),
        };

        if !type_matches {
            return Err(Error::InvalidConfig(
                "Source and target track types do not match".to_string(),
            ));
        }

        if target_track.is_locked() {
            return Err(Error::InvalidConfig("Target track is locked".to_string()));
        }

        let mut batch_command = BatchCommand::new("Move segment to track".to_string());

        let mut actual_split_happened = false;
        let split_seg_idx = split_segment_index as usize;

        // 同轨道时，如果分割点在源 segment 之后，需要调整 split_seg_idx
        // 因为移除源 segment 后，分割点索引会减少 1
        // 但分割操作需要在移除之前执行（因为分割是基于原始 track 状态）
        // 所以我们需要：
        // - 先执行分割（使用原始索引）
        // - 移除源 segment（如果源索引受分割影响，需要调整）
        // - 插入 segment（索引需要根据分割和移除调整）

        if will_split && split_segment_index >= 0 {
            let split_seg = target_track.get_segment(split_seg_idx)?;
            let split_time = target_timeline_offset - split_seg.timeline_offset;

            // 只有当目标时间在 segment 内部时才分割
            if split_time > Duration::ZERO && split_time < split_seg.duration {
                batch_command.add_command(Box::new(SplitSegmentCommand::new(
                    target_track_idx,
                    split_seg_idx,
                    split_time,
                )));
                actual_split_happened = true;
            }
        }

        let removed_segment = source_segment.clone();
        let mut new_segment = (*removed_segment).clone();
        new_segment.timeline_offset = target_timeline_offset;
        let new_segment = Arc::new(new_segment);

        // 计算移除时的索引调整：
        // 如果发生了分割，且分割点在源 segment 之后（或等于），源索引需要 +1
        // 因为分割会在 split_seg_idx 处插入一个新 segment，后续索引都增加
        let remove_seg_idx =
            if actual_split_happened && is_same_track && source_seg_idx >= split_seg_idx {
                source_seg_idx + 1
            } else {
                source_seg_idx
            };

        batch_command.add_command(Box::new(RemoveSegmentCommand::new(
            source_track_idx,
            remove_seg_idx,
            false,
        )));

        let mut insert_index;
        if actual_split_happened {
            // 当分割发生时，moved segment 应该插入在 split_seg_idx + 1
            // （左半部分之后、右半部分之前），确保 overlap 检查能正确工作
            //
            // 分割后的索引：
            // - split_seg_idx: 左半部分（保持在原位置）
            // - split_seg_idx + 1: 右半部分（新插入）
            //
            // 我们要插入 moved segment 在 split_seg_idx + 1，这样：
            // - 左半部分在 split_seg_idx
            // - moved segment 在 split_seg_idx + 1（插入后）
            // - 右半部分在 split_seg_idx + 2（被推后）
            //
            // InsertSegmentAtTimeCommand 的 overlap 检查会正确处理右半部分的位移

            insert_index = split_seg_idx + 1;

            // 同轨道时需要考虑移除源 segment 的影响
            // 移除源 segment 后，如果移除位置在 insert_index 之前，insert_index 需要减 1
            if is_same_track && remove_seg_idx < insert_index {
                insert_index -= 1;
            }
        } else {
            // 没有分割时，使用原有逻辑计算插入位置（基于原始 track 状态）
            let target_segments_count = target_track.segments_count();
            insert_index = target_segments_count;

            for (i, seg) in target_track.segments().iter().enumerate() {
                if target_timeline_offset < seg.timeline_offset {
                    insert_index = i;
                    break;
                }
            }

            if is_same_track && insert_index > source_seg_idx {
                insert_index -= 1;
            }
        }

        batch_command.add_command(Box::new(InsertSegmentAtTimeCommand::new(
            target_track_idx,
            insert_index,
            new_segment,
            true,
        )));

        //  添加额外的 affected segments 以确保 thumbnail/waveform 正确刷新
        // 分割后的两个 segment 需要刷新，但索引需要根据最终状态计算
        // 因为后续的 remove 和 insert 操作会改变索引位置
        if actual_split_happened {
            // 计算分割部分在最终状态中的索引
            // 分析：
            // 1. 同轨道且源在分割目标左边 (source_seg_idx < split_seg_idx):
            //    - remove 在 split 之前执行，索引左移
            //    - left 最终在 split_seg_idx - 1, right 最终在 split_seg_idx + 1
            // 2. 同轨道且源在分割目标右边 (source_seg_idx > split_seg_idx):
            //    - remove 在 split 之后执行，split 索引不变
            //    - insert 在 split_seg_idx + 1 位置，右半部分索引右移
            //    - left 最终在 split_seg_idx, right 最终在 split_seg_idx + 2
            // 3. 跨轨道 (is_same_track = false):
            //    - 只有分割操作影响目标轨道索引
            //    - left 在 split_seg_idx, right 在 split_seg_idx + 1

            let (left_split_final_idx, right_split_final_idx) =
                if is_same_track && source_seg_idx < split_seg_idx {
                    // 源在分割目标左边：移除操作使左半部分索引减1
                    // 插入操作在 split_seg_idx 位置，右半部分索引加1
                    (split_seg_idx - 1, split_seg_idx + 1)
                } else if is_same_track && source_seg_idx > split_seg_idx {
                    // 源在分割目标右边：移除操作不影响分割索引
                    // 插入操作在 split_seg_idx + 1 位置，右半部分索引加1
                    (split_seg_idx, split_seg_idx + 2)
                } else {
                    // 跨轨道：分割索引不受移除/插入影响
                    (split_seg_idx, split_seg_idx + 1)
                };

            batch_command.add_extra_affected_segment(AffectedSegment::with_both_thumbnails(
                target_track_idx,
                left_split_final_idx,
            ));
            batch_command.add_extra_affected_segment(AffectedSegment::with_both_thumbnails(
                target_track_idx,
                right_split_final_idx,
            ));
        }

        // 移动到目标轨道的 segment 也需要刷新
        batch_command.add_extra_affected_segment(AffectedSegment::with_both_thumbnails(
            target_track_idx,
            insert_index,
        ));

        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
            .map(|result| (result, is_same_track))
    });

    match result {
        Ok((execute_result, is_same_track)) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));
            if is_same_track {
                crate::toast_success!(ui, tr("Segment moved"));
            } else {
                crate::toast_success!(ui, tr("Segment moved to target track"));
            }
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

/// 二分：返回第一个 timeline_offset >= target_ms 的索引（0..=len）。
/// 前提：轨道内 segment 按 timeline_offset 升序。
fn partition_point_segment_start_lt(
    segments: &ModelRc<UIVideoEditorTrackSegment>,
    target_ms: i32,
) -> usize {
    let n = segments.row_count();
    let mut lo = 0usize;
    let mut hi = n;
    while lo < hi {
        let mid = (lo + hi) / 2;
        let start = segments
            .row_data(mid)
            .map(|s| s.timeline_offset)
            .unwrap_or(i32::MAX);
        if start < target_ms {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// 在按 timeline_offset 升序的段列表中二分查找包含 timeline_ms 的段。
/// 排序不变量被破坏时（罕见），在插入点附近做小窗口线性兜底。
fn binary_search_segment_containing(
    segments: &ModelRc<UIVideoEditorTrackSegment>,
    timeline_ms: i32,
) -> i32 {
    const TOLERANCE: usize = 8;

    // 第一个 start > timeline_ms 的位置 ⇒ start <= t 的段都在 [0, p)
    let p = partition_point_segment_start_lt(segments, timeline_ms.saturating_add(1));

    // 向前（索引更小）容错窗口
    let mut i = p;
    let mut checked = 0usize;
    while i > 0 && checked < TOLERANCE {
        i -= 1;
        checked += 1;
        let Some(s) = segments.row_data(i) else { break };
        if s.timeline_offset > timeline_ms {
            break;
        }
        if s.timeline_offset <= timeline_ms && timeline_ms < s.timeline_offset + s.duration {
            return i as i32;
        }
    }

    // 向后容错窗口（乱序时 start <= t 的段可能出现在 p 之后）
    for i in p..segments.row_count().min(p + TOLERANCE) {
        let Some(s) = segments.row_data(i) else { break };
        if s.timeline_offset > timeline_ms {
            break;
        }
        if s.timeline_offset <= timeline_ms && timeline_ms < s.timeline_offset + s.duration {
            return i as i32;
        }
    }

    -1
}

// 查找指定时间点所在的 segment 索引
fn video_editor_find_segment_at_time(
    ui: &AppWindow,
    track_index: i32,
    timeline_offset_ms: i32,
) -> i32 {
    let tracks_manager = global_store!(ui).get_video_editor_tracks_manager();
    if track_index < 0 || track_index as usize >= tracks_manager.tracks.row_count() {
        return -1;
    }

    let Some(track) = tracks_manager.tracks.row_data(track_index as usize) else {
        return -1;
    };

    binary_search_segment_containing(&track.segments, timeline_offset_ms)
}

// 查找指定时间点所在的 segment 索引，可排除指定 segment
fn video_editor_find_segment_at_time_excluding(
    ui: &AppWindow,
    track_index: i32,
    timeline_offset_ms: i32,
    exclude_segment_index: i32,
) -> i32 {
    let tracks_manager = global_store!(ui).get_video_editor_tracks_manager();
    if track_index < 0 || track_index as usize >= tracks_manager.tracks.row_count() {
        return -1;
    }

    let Some(track) = tracks_manager.tracks.row_data(track_index as usize) else {
        return -1;
    };

    let found = binary_search_segment_containing(&track.segments, timeline_offset_ms);
    if found >= 0 && found != exclude_segment_index {
        return found;
    }

    // 命中的恰好是被排除的段：在附近小窗口内找其他包含该时间点的段
    if found >= 0 {
        const WINDOW: usize = 8;
        let start = (found as usize).saturating_sub(WINDOW);
        let end = (found as usize + WINDOW).min(track.segments.row_count());
        for i in start..end {
            if i as i32 == exclude_segment_index {
                continue;
            }
            if let Some(s) = track.segments.row_data(i)
                && s.timeline_offset <= timeline_offset_ms
                && timeline_offset_ms < s.timeline_offset + s.duration
            {
                return i as i32;
            }
        }
    }

    -1
}

// 查找跨轨道的 snap 位置
// 当拖拽 segment 时，查找所有轨道上最近的 segment 边界（开始或结束位置）
// 返回 SnapResult 结构体，包含 snap 后的位置和是否找到了 snap 点
// target_track_index: >= 0 表示跨轨道模式（只检查目标轨道），< 0 表示普通模式（检查相邻轨道）
fn video_editor_find_snap_position(
    ui: &AppWindow,
    offset_ms: i32,
    threshold_ms: i32,
    exclude_track_index: i32,
    exclude_segment_index: i32,
    target_track_index: i32,
) -> UISnapResult {
    if threshold_ms <= 0 {
        return UISnapResult {
            position: offset_ms,
            snapped: false,
        };
    }

    let tracks_manager = global_store!(ui).get_video_editor_tracks_manager();
    let mut min_distance = threshold_ms;
    let mut closest_snap = offset_ms;
    let mut found_snap = false;

    // 优化：因为相同轨道的 segment 都是按时间排序的，
    // 只需要检查在 [offset_ms - threshold_ms, offset_ms + threshold_ms] 范围内的 segment
    let search_start = offset_ms - threshold_ms;
    let search_end = offset_ms + threshold_ms;

    // 确定要检查的轨道范围
    let tracks_to_check: Vec<usize> = if target_track_index >= 0 {
        // 跨轨道模式：只检查目标轨道
        vec![target_track_index as usize]
    } else {
        // 普通模式：检查相邻轨道（不包括当前轨道，因为同轨道的 segment 之间本身就能 snap）
        let current = exclude_track_index as usize;
        let mut tracks = vec![];
        if current > 0 {
            tracks.push(current - 1);
        }
        if current + 1 < tracks_manager.tracks.row_count() {
            tracks.push(current + 1);
        }
        tracks
    };

    for track_idx in tracks_to_check {
        let Some(track) = tracks_manager.tracks.row_data(track_idx) else {
            break;
        };

        let n = track.segments.row_count();
        if n == 0 {
            continue;
        }

        // 二分：第一个 start >= search_start 的索引 ⇒ 其 start 落入搜索窗口的段从 p 开始
        let p = partition_point_segment_start_lt(&track.segments, search_start);

        // 候选1：p-2、p-1（start < search_start，但 end 可能落在搜索窗口内）
        for idx in p.saturating_sub(2)..p {
            let Some(segment) = track.segments.row_data(idx) else {
                continue;
            };
            // 排除正在拖拽的 segment（同一轨道时）
            if track_idx as i32 == exclude_track_index && idx as i32 == exclude_segment_index {
                continue;
            }

            let seg_start = segment.timeline_offset;
            let seg_end = segment.timeline_offset + segment.duration;

            // 结束时间在搜索范围之前，两个边界点距离都 >= threshold，不会更新
            if seg_end < search_start {
                continue;
            }

            let distance_to_start = (offset_ms - seg_start).abs();
            if distance_to_start < min_distance {
                min_distance = distance_to_start;
                closest_snap = seg_start;
                found_snap = true;
            }

            let distance_to_end = (offset_ms - seg_end).abs();
            if distance_to_end < min_distance {
                min_distance = distance_to_end;
                closest_snap = seg_end;
                found_snap = true;
            }
        }

        // 候选2：从 p 向后走 while start <= search_end（按时间排序，可提前退出）
        let mut idx = p;
        while idx < n {
            let Some(segment) = track.segments.row_data(idx) else {
                break;
            };
            // 排除正在拖拽的 segment（同一轨道时）
            if track_idx as i32 == exclude_track_index && idx as i32 == exclude_segment_index {
                idx += 1;
                continue;
            }

            let seg_start = segment.timeline_offset;
            // 后续 segment 都更晚（按时间排序），可以停止遍历当前轨道
            if seg_start > search_end {
                break;
            }

            let seg_end = segment.timeline_offset + segment.duration;
            let distance_to_start = (offset_ms - seg_start).abs();
            if distance_to_start < min_distance {
                min_distance = distance_to_start;
                closest_snap = seg_start;
                found_snap = true;
            }

            let distance_to_end = (offset_ms - seg_end).abs();
            if distance_to_end < min_distance {
                min_distance = distance_to_end;
                closest_snap = seg_end;
                found_snap = true;
            }

            idx += 1;
        }
    }

    let playhead_ms = global_store!(ui).get_video_editor_timeline_offset();
    let distance_to_playhead = (offset_ms - playhead_ms).abs();
    if distance_to_playhead < min_distance {
        closest_snap = playhead_ms;
        found_snap = true;
    }

    UISnapResult {
        position: closest_snap,
        snapped: found_snap,
    }
}

// 提交segment尺寸调整操作 (拖拽释放时调用)
fn video_editor_commit_segment_resize(
    ui: &AppWindow,
    index: UISelectedSegmentIndex,
    is_left: bool,
    new_duration_ms: i32,
    new_offset_ms: i32,
) {
    if new_duration_ms <= 0 {
        crate::toast_warn!(ui, tr("Cannot resize segment to zero duration"));
        return;
    }

    let shift_timeline = global_store!(ui)
        .get_video_editor_ui_state()
        .enabled_link_track;

    let seg_idx = index.index as usize;
    let track_idx = index.track_index as usize;
    let new_duration = Duration::from_millis(new_duration_ms as u64);
    let new_offset = Duration::from_millis(new_offset_ms as u64);

    let result: Result<ExecuteResult, Error> = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let track = state
            .tracks_manager
            .get(track_idx)
            .ok_or_else(|| Error::IndexOutOfBounds(track_idx, state.tracks_manager.len()))?;

        let segment = track.get_segment(seg_idx)?;
        let original_offset = segment.timeline_offset;
        let original_duration = segment.duration;

        let mut batch_command = BatchCommand::new("Resize segment".to_string());

        if is_left {
            // 检查联动模式下offset是否不变
            let offset_unchanged = new_offset == original_offset;

            if shift_timeline && offset_unchanged {
                // 联动模式下左边缘拖拽且offset不变：timeline_offset不变，source_offset和duration改变
                let duration_diff = if new_duration > original_duration {
                    new_duration - original_duration
                } else {
                    original_duration - new_duration
                };

                if new_duration > original_duration {
                    // 向左拖拽：duration增加，source_offset减少，使用从左边扩展
                    batch_command.add_command(Box::new(StretchSegmentLeftCommand::new(
                        track_idx,
                        seg_idx,
                        duration_diff,
                        shift_timeline,
                    )));
                } else if new_duration < original_duration {
                    // 向右拖拽：duration减少，source_offset增加，使用从左边收缩
                    batch_command.add_command(Box::new(ShrinkSegmentLeftCommand::new(
                        track_idx,
                        seg_idx,
                        duration_diff,
                        shift_timeline,
                    )));
                }
            } else {
                // 非联动模式：offset和duration都变化
                let offset_diff = if new_offset > original_offset {
                    new_offset - original_offset
                } else {
                    original_offset - new_offset
                };

                if new_offset < original_offset {
                    // 向左拉伸: offset减小, duration增加
                    batch_command.add_command(Box::new(StretchSegmentLeftCommand::new(
                        track_idx,
                        seg_idx,
                        offset_diff,
                        shift_timeline,
                    )));
                } else if new_offset > original_offset {
                    // 向右收缩: offset增加, duration减小
                    batch_command.add_command(Box::new(ShrinkSegmentLeftCommand::new(
                        track_idx,
                        seg_idx,
                        offset_diff,
                        shift_timeline,
                    )));
                }
            }
        } else {
            // 右边缘调整: 只改变duration
            let duration_diff = if new_duration > original_duration {
                new_duration - original_duration
            } else {
                original_duration - new_duration
            };

            if new_duration > original_duration {
                batch_command.add_command(Box::new(StretchSegmentRightCommand::new(
                    track_idx,
                    seg_idx,
                    duration_diff,
                    shift_timeline,
                )));
            } else if new_duration < original_duration {
                batch_command.add_command(Box::new(ShrinkSegmentRightCommand::new(
                    track_idx,
                    seg_idx,
                    duration_diff,
                    shift_timeline,
                )));
            }
        }

        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_segment_detach_audio(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let seg_idx = index.index as usize;
    let track_idx = index.track_index as usize;

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let command = DetachSegmentAudioCommand::new(track_idx, seg_idx);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(false));
            crate::toast_success!(ui, tr("Audio detached from segment"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to detach audio"), e)),
    }
}

fn video_editor_segment_detach_subtitle(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let seg_idx = index.index as usize;
    let track_idx = index.track_index as usize;
    let subtitle_style: SubtitleStyleConfig = global_ve_filter!(ui).get_subtitle_style().into();
    let filters = create_subtitle_style_filters_from_config(&subtitle_style);

    let result: Result<ExecuteResult, Error> = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let num_subtitle_streams = state
            .tracks_manager
            .get(track_idx)
            .and_then(|track| {
                if matches!(track, Track::Video(_)) && seg_idx < track.segments().len() {
                    Some(track.segments()[seg_idx].metadata.subtitles.len())
                } else {
                    None
                }
            })
            .unwrap_or(0);

        state
            .history_manager
            .begin_batch("Detach segment subtitles with font style".to_string());

        let command = DetachSegmentSubtitleCommand::new(track_idx, seg_idx);
        let execute_result = state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))?;

        for new_track_idx in 0..num_subtitle_streams {
            let seg_count = state
                .tracks_manager
                .get(new_track_idx)
                .map(|t| t.segments_count())
                .unwrap_or(0);

            for seg_idx in 0..seg_count {
                for filter in &filters {
                    let cmd = AddFilterCommand::new_subtitle(
                        new_track_idx,
                        seg_idx,
                        filter.as_ref().clone_box(),
                    );
                    state
                        .history_manager
                        .execute(&mut state.tracks_manager, Box::new(cmd))?;
                }
            }
        }

        state.history_manager.end_batch()?;
        Ok(execute_result)
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh_tracks_only(ui, execute_result.affected_segments);
            crate::toast_success!(ui, tr("Subtitles detached from segment"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to detach subtitles"), e)),
    }
}

fn video_editor_segment_remove_gap(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let shift_timeline = global_store!(ui)
        .get_video_editor_ui_state()
        .enabled_link_track;

    let seg_idx = index.index as usize;
    let track_idx = index.track_index as usize;

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let command = RemoveSegmentGapCommand::new(track_idx, seg_idx, shift_timeline);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(_) => {
            sync_manager_to_ui(ui);
            crate::toast_success!(ui, tr("Gaps removed around segment"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove gaps"), e)),
    }
}

fn video_editor_segment_remove_left_gap(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let shift_timeline = global_store!(ui)
        .get_video_editor_ui_state()
        .enabled_link_track;

    let seg_idx = index.index as usize;
    let track_idx = index.track_index as usize;

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let command = RemoveSegmentLeftGapCommand::new(track_idx, seg_idx, shift_timeline);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(_) => {
            sync_manager_to_ui(ui);
            crate::toast_success!(ui, tr("Left gap removed"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove left gap"), e)),
    }
}

fn video_editor_segment_snap_to_previous(ui: &AppWindow) {
    let idx = store_video_editor_selected_segments_index!(ui).row_count();
    if idx == 0 {
        return crate::toast_warn!(ui, tr("No segments selected"));
    }

    match store_video_editor_selected_segments_index!(ui).row_data(idx - 1) {
        Some(index) => {
            if is_track_locked(ui, index.track_index) {
                crate::toast_warn!(ui, tr("Cannot snap segment in a locked track"));
                return;
            }
            global_logic!(ui).invoke_video_editor_segment_remove_left_gap(index);
        }
        _ => crate::toast_warn!(ui, tr("No segments selected")),
    }
}

fn video_editor_segment_snap_to_playhead(ui: &AppWindow) {
    let selected_indices = get_selected_segment_indices(ui);
    let Some((track_idx, seg_idx)) = selected_indices.last() else {
        crate::toast_warn!(ui, tr("No segments selected"));
        return;
    };

    if is_track_locked(ui, *track_idx as i32) {
        crate::toast_warn!(ui, tr("Cannot snap segment in a locked track"));
        return;
    }

    let playhead_ms = global_store!(ui).get_video_editor_timeline_offset();
    let playhead = Duration::from_millis(playhead_ms as u64);

    let result = with_history_manager(|state| {
        if *track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                *track_idx,
                state.tracks_manager.len(),
            ));
        }

        let track = state.tracks_manager.get(*track_idx).unwrap();

        if *seg_idx >= track.segments_count() {
            return Err(Error::IndexOutOfBounds(*seg_idx, track.segments_count()));
        }

        let segment = track.get_segment(*seg_idx).unwrap();
        let segment_duration = segment.duration;

        let new_start = playhead;
        let new_end = playhead + segment_duration;

        let segments = track.segments();
        let last_segment_end = segments
            .iter()
            .map(|s| s.timeline_offset + s.duration)
            .max()
            .unwrap_or(Duration::ZERO);

        if playhead >= last_segment_end {
            let command = MoveSegmentToTimeCommand::new(*track_idx, *seg_idx, playhead, false);
            return state
                .history_manager
                .execute(&mut state.tracks_manager, Box::new(command));
        }

        for (idx, seg) in segments.iter().enumerate() {
            if idx == *seg_idx {
                continue;
            }
            let seg_start = seg.timeline_offset;
            let seg_end = seg.timeline_offset + seg.duration;

            // Overlap check: seg overlaps new position if seg_start < new_end AND seg_end > new_start
            if seg_start < new_end && seg_end > new_start {
                return Err(Error::TrackSegment(
                    "Cannot move: another segment overlaps the target position".to_string(),
                ));
            }
        }

        // Check gap constraint only when moving forward: if segment duration > 1s AND gap to next segment < 1s, show error
        if playhead > segment.timeline_offset && segment_duration > Duration::from_secs(1) {
            for seg in segments.iter() {
                if seg.timeline_offset <= playhead {
                    continue;
                }

                if seg.timeline_offset - playhead < Duration::from_secs(1) {
                    return Err(Error::TrackSegment(
                        "Cannot move: segment duration > 1s and gap to next segment < 1s"
                            .to_string(),
                    ));
                }
                break;
            }
        }

        let command = MoveSegmentToTimeCommand::new(*track_idx, *seg_idx, playhead, false);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));
            crate::toast_success!(ui, tr("Segment snapped to playhead"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to snap segment"), e)),
    }
}

fn video_editor_segment_merge(ui: &AppWindow) {
    let selected_indices = get_selected_segment_indices(ui);

    if selected_indices.len() != 2 {
        crate::toast_warn!(ui, tr("Please select exactly 2 segments to merge"));
        return;
    }

    if selected_indices[0].0 != selected_indices[1].0 {
        crate::toast_warn!(ui, tr("Cannot merge segments from different tracks"));
        return;
    }

    let track_idx = selected_indices[0].0;
    if is_track_locked(ui, track_idx as i32) {
        crate::toast_warn!(ui, tr("Cannot merge segments in a locked track"));
        return;
    }

    // Get segment indices and ensure they're in order
    let (first_seg_idx, second_seg_idx) = if selected_indices[0].1 < selected_indices[1].1 {
        (selected_indices[0].1, selected_indices[1].1)
    } else {
        (selected_indices[1].1, selected_indices[0].1)
    };

    // Verify segments are adjacent (second index must be first + 1)
    if second_seg_idx != first_seg_idx + 1 {
        crate::toast_warn!(ui, tr("Segments must be adjacent on the track"));
        return;
    }

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let track = state.tracks_manager.get(track_idx).unwrap();

        if !track.is_video_or_audio() {
            return Err(Error::InvalidConfig(
                "Merge only works on Video or Audio tracks".into(),
            ));
        }

        let first_segment = track.get_segment(first_seg_idx)?;
        let second_segment = track.get_segment(second_seg_idx)?;

        if first_segment.metadata.path != second_segment.metadata.path {
            return Err(Error::InvalidConfig(
                "Segments must be from the same source file".into(),
            ));
        }

        // Determine which segment has smaller source_offset (should be "first")
        let (actual_first_idx, actual_second_idx) =
            if first_segment.source_offset < second_segment.source_offset {
                (first_seg_idx, second_seg_idx)
            } else {
                (second_seg_idx, first_seg_idx)
            };

        let command = MergeSegmentsCommand::new(track_idx, actual_first_idx, actual_second_idx);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(false));

            store_video_editor_selected_segments_index!(ui).set_vec(vec![]);
            global_ve_filter!(ui).set_selected_filter_index(-1);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
            crate::toast_success!(ui, tr("Segments merged"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to merge segments"), e)),
    }
}

fn video_editor_segment_resize_to_playhead(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let track_idx = index.track_index as usize;
    let seg_idx = index.index as usize;

    if is_track_locked(ui, index.track_index) {
        crate::toast_warn!(ui, tr("Cannot resize segment in a locked track"));
        return;
    }

    let playhead_ms = global_store!(ui).get_video_editor_timeline_offset();
    let playhead = Duration::from_millis(playhead_ms as u64);

    // let shift_timeline = global_store!(ui)
    //     .get_video_editor_ui_state()
    //     .enabled_link_track;

    let shift_timeline = false;

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let track = state.tracks_manager.get(track_idx).unwrap();

        if seg_idx >= track.segments_count() {
            return Err(Error::IndexOutOfBounds(seg_idx, track.segments_count()));
        }

        let segment = track.get_segment(seg_idx).unwrap();
        let segment_start = segment.timeline_offset;
        let segment_end = segment_start + segment.duration;

        // Case A: Playhead is before segment start - extend segment beginning
        if playhead < segment_start {
            if seg_idx > 0 {
                let prev_segment = track.get_segment(seg_idx - 1).unwrap();
                let prev_end = prev_segment.timeline_offset + prev_segment.duration;
                if playhead < prev_end {
                    return Err(Error::TrackSegment(
                        "Playhead is inside previous segment, cannot resize".to_string(),
                    ));
                }
            }

            let stretch_duration = segment_start - playhead;

            // Use StretchSegmentLeftCommand to extend beginning
            let command = StretchSegmentLeftCommand::new(
                track_idx,
                seg_idx,
                stretch_duration,
                shift_timeline,
            );
            state
                .history_manager
                .execute(&mut state.tracks_manager, Box::new(command))
        }
        // Case B: Playhead is after segment start (inside or after segment)
        else if playhead > segment_start {
            let new_duration = playhead - segment_start;
            if new_duration < Duration::from_millis(100) {
                return Err(Error::TrackSegment(
                    "Cannot resize segment to less than 100ms".to_string(),
                ));
            }

            // If playhead is after segment end, check next segment blocking
            if playhead > segment_end && seg_idx < track.segments_count() - 1 {
                let next_segment = track.get_segment(seg_idx + 1).unwrap();
                let next_start = next_segment.timeline_offset;
                if playhead > next_start {
                    return Err(Error::TrackSegment(
                        "Playhead is inside next segment, cannot resize".to_string(),
                    ));
                }
            }

            // Determine if we're shrinking or extending the right side
            if playhead < segment_end {
                // Shrinking - calculate shrink amount
                let shrink_duration = segment.duration - new_duration;
                let command = ShrinkSegmentRightCommand::new(
                    track_idx,
                    seg_idx,
                    shrink_duration,
                    shift_timeline,
                );
                state
                    .history_manager
                    .execute(&mut state.tracks_manager, Box::new(command))
            } else {
                // Extending - calculate stretch amount
                let stretch_duration = new_duration - segment.duration;
                let command = StretchSegmentRightCommand::new(
                    track_idx,
                    seg_idx,
                    stretch_duration,
                    shift_timeline,
                );
                state
                    .history_manager
                    .execute(&mut state.tracks_manager, Box::new(command))
            }
        } else {
            // Playhead exactly at segment start - no change needed
            return Err(Error::TrackSegment(
                "Playhead is at segment start, no resize needed".to_string(),
            ));
        }
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));
            crate::toast_success!(ui, tr("Segment resized to playhead"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to resize segment"), e)),
    }
}

fn video_editor_segment_resize_to_previous_segment(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let track_idx = index.track_index as usize;
    let seg_idx = index.index as usize;

    if is_track_locked(ui, index.track_index) {
        crate::toast_warn!(ui, tr("Cannot resize segment in a locked track"));
        return;
    }

    let shift_timeline = false;

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let track = state.tracks_manager.get(track_idx).unwrap();

        if seg_idx >= track.segments_count() {
            return Err(Error::IndexOutOfBounds(seg_idx, track.segments_count()));
        }

        // Check if there's a previous segment
        if seg_idx == 0 {
            return Err(Error::TrackSegment(
                "No previous segment to resize to".to_string(),
            ));
        }

        let segment = track.get_segment(seg_idx).unwrap();
        let segment_start = segment.timeline_offset;

        let prev_segment = track.get_segment(seg_idx - 1).unwrap();
        let prev_end = prev_segment.timeline_offset + prev_segment.duration;

        // Check if there's a gap to fill
        if prev_end >= segment_start {
            return Err(Error::TrackSegment(
                "No gap to fill - segments are adjacent".to_string(),
            ));
        }

        let stretch_duration = segment_start - prev_end;

        // Use StretchSegmentLeftCommand to extend beginning to previous segment's end
        let command =
            StretchSegmentLeftCommand::new(track_idx, seg_idx, stretch_duration, shift_timeline);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));
            crate::toast_success!(ui, tr("Segment resized to previous segment"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to resize segment"), e)),
    }
}

fn video_editor_segment_resize_to_next_segment(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let track_idx = index.track_index as usize;
    let seg_idx = index.index as usize;

    if is_track_locked(ui, index.track_index) {
        crate::toast_warn!(ui, tr("Cannot resize segment in a locked track"));
        return;
    }

    let shift_timeline = false;

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let track = state.tracks_manager.get(track_idx).unwrap();

        if seg_idx >= track.segments_count() {
            return Err(Error::IndexOutOfBounds(seg_idx, track.segments_count()));
        }

        // Check if there's a next segment
        if seg_idx >= track.segments_count() - 1 {
            return Err(Error::TrackSegment(
                "No next segment to resize to".to_string(),
            ));
        }

        let segment = track.get_segment(seg_idx).unwrap();
        let segment_end = segment.timeline_offset + segment.duration;

        let next_segment = track.get_segment(seg_idx + 1).unwrap();
        let next_start = next_segment.timeline_offset;

        // Check if there's a gap to fill
        if segment_end >= next_start {
            return Err(Error::TrackSegment(
                "No gap to fill - segments are adjacent".to_string(),
            ));
        }

        let stretch_duration = next_start - segment_end;

        // Use StretchSegmentRightCommand to extend end to next segment's start
        let command =
            StretchSegmentRightCommand::new(track_idx, seg_idx, stretch_duration, shift_timeline);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));
            crate::toast_success!(ui, tr("Segment resized to next segment"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to resize segment"), e)),
    }
}

fn video_editor_segment_remove_right_gap(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let shift_timeline = global_store!(ui)
        .get_video_editor_ui_state()
        .enabled_link_track;

    let seg_idx = index.index as usize;
    let track_idx = index.track_index as usize;

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let command = RemoveSegmentRightGapCommand::new(track_idx, seg_idx, shift_timeline);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(_) => {
            sync_manager_to_ui(ui);
            crate::toast_success!(ui, tr("Right gap removed"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove right gap"), e)),
    }
}

pub fn refresh_affected_segments(ui: &AppWindow, affected: AffectedSegments) {
    if affected.is_empty() {
        return;
    }

    for seg in affected.segments {
        if !seg.should_update() {
            continue;
        }

        let ui_weak = ui.as_weak();
        tokio::spawn(async move {
            let result: Option<(Manager, String, bool, bool, bool)> =
                with_history_manager(|state| {
                    let track = state.tracks_manager.get(seg.track_index)?;
                    let segment = track.get_segment(seg.segment_index).ok()?;
                    let is_video = matches!(track, Track::Video(_));
                    let is_audio = matches!(track, Track::Audio(_));
                    let is_image = matches!(track, Track::Image(_));
                    Some((
                        state.tracks_manager.clone(),
                        segment.uuid.clone(),
                        is_video,
                        is_audio,
                        is_image,
                    ))
                });

            let Some((manager, uuid, is_video, is_audio, is_image)) = result else {
                return;
            };

            if (is_audio || is_video) && seg.update_audio_sample {
                async_load_segment_audio(
                    ui_weak.clone(),
                    manager.clone(),
                    seg.track_index,
                    seg.segment_index,
                    uuid.clone(),
                );
            }

            if is_video || is_image {
                let (left_thumb, right_thumb) = seg.update_thumbnail;

                if left_thumb {
                    async_load_segment_thumbnail(
                        ui_weak.clone(),
                        manager.clone(),
                        seg.track_index,
                        seg.segment_index,
                        uuid.clone(),
                        true,
                    );
                }

                if right_thumb {
                    async_load_segment_thumbnail(
                        ui_weak,
                        manager,
                        seg.track_index,
                        seg.segment_index,
                        uuid,
                        false,
                    );
                }
            }
        });
    }
}

fn video_editor_update_edited_subtitle_from_segment(
    ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
) {
    let track_idx = track_index as usize;
    let seg_idx = segment_index as usize;
    let manager = global_store!(ui).get_video_editor_tracks_manager();

    if track_idx >= manager.tracks.row_count() {
        return;
    }

    let track = manager.tracks.row_data(track_idx);
    let Some(track) = track else {
        return;
    };

    if seg_idx >= track.segments.row_count() {
        return;
    }

    let segment = track.segments.row_data(seg_idx);
    let Some(segment) = segment else {
        return;
    };

    let start_ms = segment.timeline_offset as u64;
    let end_ms = (segment.timeline_offset + segment.duration) as u64;

    let start_timestamp = ms_to_srt_timestamp(start_ms);
    let end_timestamp = ms_to_srt_timestamp(end_ms);

    global_store!(ui).set_video_editor_current_edited_subtitle(UIVideoEditorSubtitle {
        start_timestamp: start_timestamp.into(),
        end_timestamp: end_timestamp.into(),
        // Convert \N (ASS line break) back to \n for display in text editor
        subtitle: segment.subtitle_text.replace("\\N", "\n").into(),
    });
}

fn video_editor_segment_has_filter(ui: &AppWindow, entry: UIFilterEntry) -> bool {
    let selected_segments = get_selected_segment_indices(ui);
    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return false;
    };
    segment_contains_filter(*track_idx, *seg_idx, &entry).unwrap_or_default()
}

fn video_editor_segment_has_preset_filter(ui: &AppWindow, entry: UIPresetFilter) -> bool {
    let selected_segments = get_selected_segment_indices(ui);
    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return false;
    };

    let result: Option<bool> = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;

        for item in entry.filters.iter() {
            match item.ty {
                UIFilterType::Video => {
                    if segment
                        .video_filters
                        .iter()
                        .any(|f| f.inner.name() == item.name.as_str())
                    {
                        return Some(true);
                    }
                }
                UIFilterType::Image => {
                    if segment
                        .image_filters
                        .iter()
                        .any(|f| f.inner.name() == item.name.as_str())
                    {
                        return Some(true);
                    }
                }
                UIFilterType::Audio => {
                    if segment
                        .audio_filters
                        .iter()
                        .any(|f| f.inner.name() == item.name.as_str())
                    {
                        return Some(true);
                    }
                }
                _ => continue,
            }
        }
        Some(false)
    });

    result.unwrap_or_default()
}

pub fn segment_contains_filter(
    track_idx: usize,
    seg_idx: usize,
    entry: &UIFilterEntry,
) -> Option<bool> {
    with_history_manager(|state| {
        let track = state.tracks_manager.get(track_idx)?;
        let segment = track.get_segment(seg_idx).ok()?;

        match entry.ty {
            UIFilterType::Video => Some(
                segment
                    .video_filters
                    .iter()
                    .any(|f| f.inner.name() == entry.name.as_str()),
            ),
            UIFilterType::Image => Some(
                segment
                    .image_filters
                    .iter()
                    .any(|f| f.inner.name() == entry.name.as_str()),
            ),
            UIFilterType::Audio => Some(
                segment
                    .audio_filters
                    .iter()
                    .any(|f| f.inner.name() == entry.name.as_str()),
            ),
            _ => None,
        }
    })
}

fn video_editor_segment_contain_filter(
    _ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
) -> bool {
    let track_idx = track_index as usize;
    let seg_idx = segment_index as usize;

    with_history_manager(|state| {
        let track = state.tracks_manager.get(track_idx)?;
        let segment = track.get_segment(seg_idx).ok()?;

        Some(
            !segment.video_filters.is_empty()
                || !segment.image_filters.is_empty()
                || !segment.audio_filters.is_empty(),
        )
    })
    .unwrap_or_default()
}

fn video_editor_segment_remove_all_filters(ui: &AppWindow, track_index: i32, segment_index: i32) {
    let track_idx = track_index as usize;
    let seg_idx = segment_index as usize;

    let speed_reset_info: Option<(f32, Duration)> = with_history_manager(|state| {
        let track = state.tracks_manager.get(track_idx)?;
        let segment = track.get_segment(seg_idx).ok()?;

        let has_enabled_speed = segment
            .video_filters
            .iter()
            .any(|f| f.inner.name() == SpeedFilter::NAME && f.enabled())
            || segment
                .audio_filters
                .iter()
                .any(|f| f.inner.name() == AudioSpeedFilter::NAME && f.enabled())
            || segment
                .image_filters
                .iter()
                .any(|f| f.inner.name() == SpeedFilter::NAME && f.enabled());

        if has_enabled_speed {
            Some((segment.playback_speed, segment.duration))
        } else {
            None
        }
    });

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let track = state.tracks_manager.get(track_idx).unwrap();
        if seg_idx >= track.segments_count() {
            return Err(Error::IndexOutOfBounds(seg_idx, track.segments_count()));
        }

        let mut batch_command = BatchCommand::new("Remove all filters from segment".to_string());
        batch_command.add_command(Box::new(ClearSegmentFiltersCommand::new(
            track_idx, seg_idx,
        )));

        if let Some((old_speed, old_duration)) = speed_reset_info {
            batch_command.add_command(Box::new(SetPlaybackSpeedCommand::new(
                track_idx,
                seg_idx,
                1.0,
                old_speed,
                old_duration,
            )));
        }

        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));
            global_ve_filter!(ui).set_selected_filter_index(-1);
            global_ve_filter!(ui).invoke_refresh_filter_list();
            crate::toast_success!(ui, tr("All filters removed from segment"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove filters"), e)),
    }
}

fn video_editor_is_link_all_movable(
    ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
    drag_original_offset: i32,
    drag_original_duration: i32,
) -> bool {
    let tracks_manager = global_store!(ui).get_video_editor_tracks_manager();
    let drag_track_index = global_store!(ui).get_video_editor_drag_segment_track_index();
    let drag_seg_index = global_store!(ui).get_video_editor_drag_segment_index();

    if track_index < 0 || segment_index < 0 {
        return false;
    }

    let track_idx = track_index as usize;
    let seg_idx = segment_index as usize;

    if track_idx >= tracks_manager.tracks.row_count() {
        return false;
    }

    let Some(track) = tracks_manager.tracks.row_data(track_idx) else {
        return false;
    };

    // 同轨道：只有被拖拽 segment 之后的 segment 才移动
    if track_idx == drag_track_index as usize {
        return seg_idx > drag_seg_index as usize;
    }

    // 跨轨道：排除锁定轨道
    if track.locked {
        return false;
    }

    let drag_end = drag_original_offset + drag_original_duration;

    // 找到该轨道中第一个与拖拽 segment 时间重叠的 segment
    // 从该 segment 开始，所有后续 segment 都应移动
    let mut found_overlap = false;
    for i in 0..track.segments.row_count() {
        let Some(seg) = track.segments.row_data(i) else {
            continue;
        };
        let seg_start = seg.timeline_offset;
        let seg_end = seg_start + seg.duration;

        if !found_overlap && seg_start < drag_end && drag_original_offset < seg_end {
            found_overlap = true;
        }

        if found_overlap && i == seg_idx {
            return true;
        }

        if found_overlap && i > seg_idx {
            break;
        }
    }

    false
}

fn video_editor_get_min_all_tracks_offset(ui: &AppWindow) -> i32 {
    let tracks_manager = global_store!(ui).get_video_editor_tracks_manager();
    let drag_track_index = global_store!(ui).get_video_editor_drag_segment_track_index();
    let drag_seg_index = global_store!(ui).get_video_editor_drag_segment_index();

    // 获取拖拽 segment 的原始时间范围
    let (drag_start_ms, drag_end_ms) = if drag_track_index >= 0
        && drag_track_index < tracks_manager.tracks.row_count() as i32
        && drag_seg_index >= 0
    {
        let Some(drag_track) = tracks_manager.tracks.row_data(drag_track_index as usize) else {
            return 0;
        };
        if let Some(seg) = drag_track.segments.row_data(drag_seg_index as usize) {
            let start = seg.timeline_offset;
            (start, start + seg.duration)
        } else {
            return 0;
        }
    } else {
        return 0;
    };

    // 计算所有受影响轨道中，第一个可移动 segment 与其前一个 segment 之间的最小间隙
    // 这个间隙限制了向左移动的最大距离
    let mut min_gap = i32::MAX;

    for track_idx in 0..tracks_manager.tracks.row_count() {
        let Some(track) = tracks_manager.tracks.row_data(track_idx) else {
            continue;
        };

        if track.locked {
            continue;
        }

        if track_idx == drag_track_index as usize {
            // 同轨道：拖拽 segment 与前一个 segment 之间的间隙
            if drag_seg_index > 0 {
                if let Some(prev_seg) = track.segments.row_data(drag_seg_index as usize - 1) {
                    let prev_end = prev_seg.timeline_offset + prev_seg.duration;
                    let gap = drag_start_ms - prev_end;
                    if gap < min_gap {
                        min_gap = gap;
                    }
                }
            } else {
                // 拖拽 segment 是第一个，间隙就是它的 offset（不能低于 0）
                if drag_start_ms < min_gap {
                    min_gap = drag_start_ms;
                }
            }
        } else {
            // 跨轨道：找到第一个与拖拽 segment 重叠的 segment
            for seg_idx in 0..track.segments.row_count() {
                if let Some(seg) = track.segments.row_data(seg_idx) {
                    let seg_start = seg.timeline_offset;
                    let seg_end = seg_start + seg.duration;

                    if seg_start < drag_end_ms && drag_start_ms < seg_end {
                        // 这是第一个重叠的 segment，计算它与前一个 segment 的间隙
                        if seg_idx > 0 {
                            if let Some(prev_seg) = track.segments.row_data(seg_idx - 1) {
                                let prev_end = prev_seg.timeline_offset + prev_seg.duration;
                                let gap = seg_start - prev_end;
                                if gap < min_gap {
                                    min_gap = gap;
                                }
                            }
                        } else {
                            // 第一个 segment，间隙就是它的 offset
                            if seg_start < min_gap {
                                min_gap = seg_start;
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    if min_gap == i32::MAX { 0 } else { min_gap }
}

fn video_editor_segment_has_keyframes(
    _ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
) -> bool {
    let track_idx = track_index as usize;
    let seg_idx = segment_index as usize;

    let result: Option<bool> = with_history_manager(|state| {
        let track = state.tracks_manager.get(track_idx)?;
        let segment = track.get_segment(seg_idx).ok()?;
        let has_video_filter_keyframes = segment
            .video_filters
            .iter()
            .any(|f| f.inner.get_keyframe_tracks().has_keyframes());
        let has_audio_filter_keyframes = segment
            .audio_filters
            .iter()
            .any(|f| f.inner.get_keyframe_tracks().has_keyframes());
        let has_image_filter_keyframes = segment
            .image_filters
            .iter()
            .any(|f| f.inner.get_keyframe_tracks().has_keyframes());
        let has_text_keyframes = segment
            .text_element
            .as_ref()
            .map(|te| te.keyframe_tracks.has_keyframes())
            .unwrap_or(false);

        Some(
            has_video_filter_keyframes
                || has_audio_filter_keyframes
                || has_image_filter_keyframes
                || has_text_keyframes,
        )
    });

    result.unwrap_or(false)
}

fn video_editor_remove_all_segment_keyframes(ui: &AppWindow, track_index: i32, segment_index: i32) {
    let track_idx = track_index as usize;
    let seg_idx = segment_index as usize;

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let track = state.tracks_manager.get(track_idx).unwrap();
        if seg_idx >= track.segments_count() {
            return Err(Error::IndexOutOfBounds(seg_idx, track.segments_count()));
        }

        let command = ClearSegmentKeyframesCommand::new(track_idx, seg_idx);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(_) => {
            sync_manager_to_ui(ui);
            crate::toast_success!(ui, tr("All keyframes removed from segment"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove keyframes"), e)),
    }
}

fn video_editor_segment_export_gif(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let ui_weak = ui.as_weak();
    let (track_idx, seg_idx) = (index.track_index as usize, index.index as usize);

    tokio::spawn(async move {
        let Some(output_path) = picker_save_file(
            ui_weak.clone(),
            &tr("Export GIF"),
            &tr("GIF Image"),
            &["gif"],
            "segment.gif",
        ) else {
            return;
        };

        let file_name = output_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("segment.gif")
            .to_string();

        let segment_info = with_history_manager(|state| {
            state.tracks_manager.get(track_idx).and_then(|track| {
                if seg_idx < track.segments().len() {
                    let seg = track.segments()[seg_idx].clone();
                    let video_meta = seg.metadata.first_video().cloned();
                    Some((seg, video_meta))
                } else {
                    None
                }
            })
        });

        let Some((segment, Some(video_meta))) = segment_info else {
            toast::async_toast_warn(ui_weak, tr("Segment has no video"));
            return;
        };

        let duration_secs = segment.duration.as_secs_f64();
        if duration_secs > 60.0 {
            toast::async_toast_warn(ui_weak, tr("GIF export limited to 60 seconds"));
            return;
        }

        let task_id = next_export_task_id();
        add_export_task(&ui_weak, task_id, file_name.clone(), UIMediaType::Image).await;

        let source_fps = video_meta.fps as f64;
        let sample_interval = (source_fps / GIF_FPS).ceil() as usize;
        let frames_per_chunk = source_fps.ceil() as usize; // ~1 second of source frames
        let total_chunks = duration_secs.ceil() as usize;
        let start_frame = (segment.source_offset.as_secs_f64() * source_fps) as usize;

        let file = match File::create(&output_path) {
            Ok(f) => f,
            Err(e) => {
                update_export_task_progress(&ui_weak, task_id, 0.0);
                toast::async_toast_warn(
                    ui_weak,
                    format!("{}. {}: {}", tr("Failed to create file"), tr("Reason"), e),
                );
                return;
            }
        };
        let mut encoder = GifEncoder::new(file);

        let mut any_frames = false;
        for chunk_idx in 0..total_chunks {
            let progress = chunk_idx as f32 / total_chunks as f32;
            update_export_task_progress(&ui_weak, task_id, progress);

            let chunk_start_frame = start_frame + (chunk_idx * frames_per_chunk);
            let remaining_frames =
                ((duration_secs - chunk_idx as f64) * source_fps).ceil() as usize;
            let chunk_frame_count = frames_per_chunk.min(remaining_frames);

            let chunk_frames = match segment.extract_video(chunk_start_frame, chunk_frame_count) {
                Ok(frames) => frames,
                Err(e) => {
                    toast::async_toast_warn(
                        ui_weak,
                        format!(
                            "{}. {}: {}",
                            tr("Failed to extract frames"),
                            tr("Reason"),
                            e
                        ),
                    );
                    return;
                }
            };

            // Sample frames from this chunk and resize to 480P
            let sampled_frames: Vec<Frame> = chunk_frames
                .into_iter()
                .enumerate()
                .filter_map(|(idx, vi)| {
                    if idx % sample_interval != 0 {
                        return None;
                    }

                    match vi {
                        VideoImage::Image { buffer } => {
                            let resized =
                                resize_rgba_image(buffer, GIF_MAX_WIDTH, GIF_MAX_HEIGHT).ok()?;
                            any_frames = true;
                            Some(Frame::from_parts(
                                resized,
                                0,
                                0,
                                Delay::from_saturating_duration(Duration::from_millis(100)),
                            ))
                        }
                        VideoImage::Empty => None,
                    }
                })
                .collect();

            if let Err(e) = encoder.encode_frames(sampled_frames) {
                toast::async_toast_warn(
                    ui_weak,
                    format!("{}. {}: {}", tr("GIF export failed"), tr("Reason"), e),
                );
                return;
            }
        }

        if !any_frames {
            toast::async_toast_warn(ui_weak, tr("No frames extracted"));
            return;
        }

        update_export_task_progress(&ui_weak, task_id, 1.0);
        toast::async_toast_success(ui_weak, tr("GIF exported successfully"));
    });
}

fn video_editor_segment_export_selected_mp4(ui: &AppWindow) {
    let selected_indices = get_selected_segment_indices(ui);

    if selected_indices.is_empty() {
        crate::toast_warn!(ui, tr("No segments selected"));
        return;
    }

    // Collect all selected video segments info
    let segments_info: Vec<(usize, usize, Arc<Segment>)> = selected_indices
        .iter()
        .filter_map(|&(track_idx, seg_idx)| {
            with_history_manager(|state| {
                state
                    .tracks_manager
                    .get(track_idx)
                    .and_then(|track| match track {
                        Track::Video(inner) => inner.track.segments.get(seg_idx).cloned(),
                        _ => None,
                    })
                    .map(|seg| (track_idx, seg_idx, seg))
            })
        })
        .collect();

    if segments_info.is_empty() {
        crate::toast_warn!(ui, tr("No video segments selected"));
        return;
    }

    crate::toast_info!(
        ui,
        tr("Exporting segments as MP4 will take some time, please be patient")
    );

    let ui_weak = ui.as_weak();

    // Get project name for the export directory prefix
    let project_name = {
        let state_guard = PROJECT_STATE.lock().unwrap();
        state_guard
            .as_ref()
            .and_then(|s| s.current_project_path.as_ref())
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("project")
            .to_string()
    };

    let task_id = next_export_task_id();
    let total_segments = segments_info.len();

    tokio::spawn(async move {
        add_export_task(
            &ui_weak,
            task_id,
            format!("{} ({} {})", project_name, total_segments, tr("segment(s)")),
            UIMediaType::Video,
        )
        .await;

        let cancellation_token = CancellationToken::new();
        register_cancellation_token(task_id, cancellation_token.clone());

        let Some(output_dir) = picker_directory(ui_weak.clone(), &tr("Export MP4")) else {
            remove_cancellation_token(task_id);
            update_export_task_progress(&ui_weak, task_id, 0.0);
            return;
        };

        // Create a subdirectory for the exports, using project name as prefix
        let export_dir = output_dir.join(format!("{}-mp4-export", project_name));
        if let Err(e) = std::fs::create_dir_all(&export_dir) {
            remove_cancellation_token(task_id);
            update_export_task_progress(&ui_weak, task_id, 0.0);
            toast::async_toast_warn(
                ui_weak,
                format!(
                    "{}. {}: {}",
                    tr("Failed to create directory"),
                    tr("Reason"),
                    e
                ),
            );
            return;
        }

        let mut exported_count = 0usize;
        for (seg_num, (track_idx, seg_idx, segment)) in segments_info.into_iter().enumerate() {
            if cancellation_token.is_cancelled() {
                break;
            }

            if segment.metadata.first_video().is_none() {
                log::warn!("Segment track={track_idx} seg={seg_idx} has no video, skipping");
                continue;
            }

            let source_stem = segment
                .metadata
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("segment")
                .to_string();

            let output_path = export_dir.join(format!(
                "{}-track{}-seg{}.mp4",
                source_stem, track_idx, seg_idx
            ));

            let ui_weak_for_progress = ui_weak.clone();
            let total = total_segments;
            let seg_index = seg_num;
            let ct = cancellation_token.clone();

            let result = tokio::task::spawn_blocking(move || {
                let config = SegmentExportConfig::new(output_path).with_cancellation_token(ct);

                SegmentExporter::export_with_progress(&segment, config, move |progress| {
                    // Map per-segment progress [0..1] to overall progress across all segments
                    let seg_progress = progress.progress();
                    let overall = (seg_index as f32 + seg_progress) / total as f32;
                    update_export_task_progress(&ui_weak_for_progress, task_id, overall);
                })
            })
            .await;

            match result {
                Ok(Ok(_)) => exported_count += 1,
                Ok(Err(Error::ExportCancelled)) | Err(_) => break,
                Ok(Err(e)) => {
                    log::warn!(
                        "MP4 export failed for segment track={track_idx} seg={seg_idx}: {e}"
                    );
                    continue;
                }
            }
        }

        remove_cancellation_token(task_id);

        if cancellation_token.is_cancelled() {
            update_export_task_progress(&ui_weak, task_id, 0.0);
            toast::async_toast_warn(ui_weak, tr("MP4 export cancelled"));
        } else {
            update_export_task_progress(&ui_weak, task_id, 1.0);
            toast::async_toast_success(
                ui_weak,
                format!(
                    "{} {} {}",
                    tr("Exported"),
                    exported_count,
                    tr("segment(s) as MP4")
                ),
            );
        }
    });
}

fn video_editor_segment_extract_frames(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let ui_weak = ui.as_weak();
    let (track_idx, seg_idx) = (index.track_index as usize, index.index as usize);

    tokio::spawn(async move {
        let Some(output_dir) = picker_directory(ui_weak.clone(), &tr("Extract Frames")) else {
            return;
        };

        let segment_info = with_history_manager(|state| {
            state.tracks_manager.get(track_idx).and_then(|track| {
                if seg_idx < track.segments().len() {
                    let seg = track.segments()[seg_idx].clone();
                    let video_meta = seg.metadata.first_video().cloned();
                    let source_stem = seg
                        .metadata
                        .path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("segment")
                        .to_string();
                    Some((seg, video_meta, source_stem))
                } else {
                    None
                }
            })
        });

        let Some((segment, Some(video_meta), source_stem)) = segment_info else {
            toast::async_toast_warn(ui_weak, tr("Segment has no video"));
            return;
        };

        toast::async_toast_info(
            ui_weak.clone(),
            tr("Exporting frames will take some time, please be patient"),
        );

        let source_fps = video_meta.fps as f64;
        let duration_secs = segment.duration.as_secs_f64();
        let total_frames = (source_fps * duration_secs).ceil() as usize;
        let digit_width = if total_frames >= 1000 { 4 } else { 3 };
        let frames_per_chunk = source_fps.ceil() as usize;
        let total_chunks = duration_secs.ceil() as usize;
        let start_frame = (segment.source_offset.as_secs_f64() * source_fps) as usize;

        let task_id = next_export_task_id();
        let task_name = format!("{}-{}", source_stem, seg_idx);
        let cancellation_token = CancellationToken::new();
        add_export_task(&ui_weak, task_id, task_name.clone(), UIMediaType::Image).await;
        register_cancellation_token(task_id, cancellation_token.clone());

        let mut saved_count = 0usize;
        let prefix = format!("{}-{}", source_stem, seg_idx);

        for chunk_idx in 0..total_chunks {
            if cancellation_token.is_cancelled() {
                remove_cancellation_token(task_id);
                toast::async_toast_warn(ui_weak, tr("Extract frames cancelled"));
                return;
            }

            let progress = chunk_idx as f32 / total_chunks as f32;
            update_export_task_progress(&ui_weak, task_id, progress);

            let chunk_start_frame = start_frame + (chunk_idx * frames_per_chunk);
            let remaining_frames =
                ((duration_secs - chunk_idx as f64) * source_fps).ceil() as usize;
            let chunk_frame_count = frames_per_chunk.min(remaining_frames);

            let chunk_frames = match segment.extract_video(chunk_start_frame, chunk_frame_count) {
                Ok(frames) => frames,
                Err(e) => {
                    remove_cancellation_token(task_id);
                    toast::async_toast_warn(
                        ui_weak,
                        format!(
                            "{}. {}: {}",
                            tr("Failed to extract frames"),
                            tr("Reason"),
                            e
                        ),
                    );
                    return;
                }
            };

            for vi in chunk_frames {
                match vi {
                    VideoImage::Image { buffer } => {
                        let frame_num = saved_count + 1;
                        let file_name =
                            format!("{}-{:0>w$}.png", prefix, frame_num, w = digit_width);
                        let output_path = output_dir.join(&file_name);
                        if let Err(e) = buffer.save(&output_path) {
                            remove_cancellation_token(task_id);
                            toast::async_toast_warn(
                                ui_weak,
                                format!("{}. {}: {}", tr("Failed to save frame"), tr("Reason"), e),
                            );
                            return;
                        }
                        saved_count += 1;
                    }
                    VideoImage::Empty => {}
                }
            }
        }

        remove_cancellation_token(task_id);

        if saved_count == 0 {
            update_export_task_progress(&ui_weak, task_id, 0.0);
            toast::async_toast_warn(ui_weak, tr("No frames extracted"));
            return;
        }

        update_export_task_progress(&ui_weak, task_id, 1.0);
        toast::async_toast_success(ui_weak, tr("Frames exported successfully"));
    });
}

fn video_editor_segment_export_audio(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let ui_weak = ui.as_weak();
    let (track_idx, seg_idx) = (index.track_index as usize, index.index as usize);

    tokio::spawn(async move {
        let Some(output_path) = picker_save_file(
            ui_weak.clone(),
            &tr("Export Audio"),
            &tr("WAV Audio"),
            &["wav"],
            "segment.wav",
        ) else {
            return;
        };

        let segment_info = with_history_manager(|state| {
            state.tracks_manager.get(track_idx).and_then(|track| {
                if seg_idx < track.segments().len() {
                    let seg = track.segments()[seg_idx].clone();
                    let path = seg.metadata.path.clone();
                    let audio_meta = seg.metadata.audios.first().cloned();
                    Some((seg, audio_meta, path))
                } else {
                    None
                }
            })
        });

        let Some((segment, Some(audio_meta), path)) = segment_info else {
            toast::async_toast_warn(ui_weak, tr("Segment has no audio"));
            return;
        };

        toast::async_toast_info(ui_weak.clone(), tr("Exporting audio, please wait..."));

        let result = extract_segment_audio(
            &path,
            audio_meta.index,
            &segment,
            segment.timeline_offset,
            segment.duration,
            audio_meta.channels,
            audio_meta.sample_rate,
            audio_meta.channels,    // keep original channels
            audio_meta.sample_rate, // keep original sample rate for full quality
        );

        match result {
            Ok(segment_samples) => {
                let samples: Vec<f32> = segment_samples
                    .samples
                    .into_iter()
                    .filter_map(|s| s) // Remove None (gap markers)
                    .collect();

                if samples.is_empty() {
                    toast::async_toast_warn(ui_weak, tr("No audio samples extracted"));
                    return;
                }

                let spec = WavSpec {
                    channels: audio_meta.channels,
                    sample_rate: audio_meta.sample_rate,
                    bits_per_sample: 32,
                    sample_format: SampleFormat::Float,
                };

                match WavWriter::create(&output_path, spec) {
                    Ok(mut writer) => {
                        for sample in &samples {
                            let clamped = sample.clamp(-1.0, 1.0);
                            if let Err(e) = writer.write_sample(clamped) {
                                toast::async_toast_warn(
                                    ui_weak,
                                    format!(
                                        "{}. {}: {}",
                                        tr("Failed to write audio"),
                                        tr("Reason"),
                                        e
                                    ),
                                );
                                return;
                            }
                        }
                        if let Err(e) = writer.finalize() {
                            toast::async_toast_warn(
                                ui_weak,
                                format!(
                                    "{}. {}: {}",
                                    tr("Failed to finalize WAV"),
                                    tr("Reason"),
                                    e
                                ),
                            );
                        } else {
                            toast::async_toast_success(ui_weak, tr("Audio exported successfully"));
                        }
                    }
                    Err(e) => {
                        toast::async_toast_warn(
                            ui_weak,
                            format!(
                                "{}. {}: {}",
                                tr("Failed to create WAV file"),
                                tr("Reason"),
                                e
                            ),
                        );
                    }
                }
            }
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak,
                    format!("{}. {}: {}", tr("Failed to extract audio"), tr("Reason"), e),
                );
            }
        }
    });
}

fn video_editor_segment_toggle_enable(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let seg_idx = index.index as usize;
    let track_idx = index.track_index as usize;

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let command = ToggleSegmentVisibilityCommand::new(track_idx, seg_idx);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to toggle enable"), e)),
    }
}

fn video_editor_segment_toggle_audio(ui: &AppWindow, index: UISelectedSegmentIndex) {
    let seg_idx = index.index as usize;
    let track_idx = index.track_index as usize;

    let result = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let command = ToggleSegmentAudioMutedCommand::new(track_idx, seg_idx);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to toggle audio"), e)),
    }
}

fn video_editor_segment_intelligent_voice_segmentation(
    ui: &AppWindow,
    index: UISelectedSegmentIndex,
) {
    let track_idx = index.track_index as usize;
    let seg_idx = index.index as usize;

    if is_track_locked(ui, index.track_index) {
        crate::toast_warn!(ui, tr("Cannot segment in a locked track"));
        return;
    }

    let segment = with_history_manager(|state| {
        state.tracks_manager.get(track_idx).and_then(|track| {
            if seg_idx < track.segments().len() {
                Some(track.segments()[seg_idx].clone())
            } else {
                None
            }
        })
    });

    let Some(segment) = segment else {
        return;
    };

    if segment.metadata.audios.is_empty() {
        crate::toast_warn!(ui, tr("Segment has no audio"));
        return;
    }

    crate::toast_info!(&ui, tr("Detecting voice segments, please wait..."));

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || detect_voice_split_points(&segment)).await;

        let Ok(Ok(split_points_ms)) = result else {
            toast::async_toast_warn(ui_weak, tr("Voice detection failed"));
            return;
        };

        if split_points_ms.is_empty() {
            toast::async_toast_warn(ui_weak, tr("No silence boundaries found"));
            return;
        }

        let mut sorted_points = split_points_ms;
        sorted_points.sort();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let result = with_history_manager(|state| {
                if track_idx >= state.tracks_manager.len() {
                    return Err(Error::IndexOutOfBounds(
                        track_idx,
                        state.tracks_manager.len(),
                    ));
                }

                let mut batch_command =
                    BatchCommand::new("Intelligent voice segmentation".to_string());
                for &split_ms in sorted_points.iter().rev() {
                    let split_duration = Duration::from_millis(split_ms as u64);
                    if split_duration > Duration::ZERO {
                        batch_command.add_command(Box::new(SplitSegmentCommand::new(
                            track_idx,
                            seg_idx,
                            split_duration,
                        )));
                    }
                }
                // N splits produce N+1 segments at indices seg_idx..seg_idx+N
                for i in 0..=sorted_points.len() {
                    batch_command.add_extra_affected_segment(
                        AffectedSegment::with_both_thumbnails(track_idx, seg_idx + i),
                    );
                }
                state
                    .history_manager
                    .execute(&mut state.tracks_manager, Box::new(batch_command))
            });

            match result {
                Ok(execute_result) => {
                    sync_and_refresh(&ui, execute_result.affected_segments, Some(false));
                    crate::toast_success!(&ui, tr("Voice segmentation completed"));
                }
                Err(e) => crate::toast_warn!(&ui, format!("{}: {}", tr("Failed to segment"), e)),
            }
        });
    });
}

fn detect_voice_split_points(segment: &Arc<Segment>) -> Result<Vec<i32>, String> {
    let audio_meta = segment.metadata.audios.first().unwrap();
    let samples_result = extract_segment_audio(
        &segment.metadata.path,
        audio_meta.index,
        segment,
        segment.timeline_offset,
        segment.duration,
        audio_meta.channels,
        audio_meta.sample_rate,
        audio_meta.channels,
        audio_meta.sample_rate,
    );
    let segment_samples = samples_result.map_err(|e| format!("Audio extraction failed: {e}"))?;

    let raw_samples: Vec<f32> = segment_samples
        .samples
        .into_iter()
        .map(|s| s.unwrap_or(0.0))
        .collect();

    if raw_samples.is_empty() {
        return Ok(vec![]);
    }

    let mono = to_mono(&raw_samples, audio_meta.channels);
    let speech_regions = detect_voice_segments(&mono, audio_meta.sample_rate)?;

    if speech_regions.len() <= 1 {
        return Ok(vec![]);
    }

    let seg_duration_ms = segment.duration.as_millis() as i32;
    let mut split_points: Vec<i32> = Vec::new();
    for i in 0..speech_regions.len() - 1 {
        let (_, end_ms) = speech_regions[i];
        let gap_start = end_ms as i32;
        if gap_start > 0 && gap_start < seg_duration_ms {
            split_points.push(gap_start);
        }
    }

    Ok(split_points)
}

fn resize_thumbnail(img: RgbaImage) -> RgbaImage {
    let (w, h) = (img.width(), img.height());
    if h <= THUMBNAIL_HEIGHT {
        return img;
    }
    let new_w = (w as f32 * THUMBNAIL_HEIGHT as f32 / h as f32).round() as u32;
    let new_w = new_w.max(1);
    resize_rgba_image(img.clone(), new_w, THUMBNAIL_HEIGHT).unwrap_or_else(|_| img)
}
