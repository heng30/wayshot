use super::{
    command::{
        sync_and_refresh, sync_and_refresh_simple, sync_and_refresh_tracks_changed,
        sync_and_refresh_tracks_only, sync_manager_to_ui, with_history_manager,
    },
    common_type::SubtitleStyleConfig,
    conversion::track_to_filter_type,
    filters::{
        create_filter_command_with_detail, subtitle::create_subtitle_style_filters_from_config,
    },
    project::{PRESET_TEXT_STYLES_ID, PROJECT_STATE},
    segment::segment_contains_filter,
    transcribe::audio_player::set_current_audio_config,
};
use crate::{
    SelectedSegmentIndex as UISelectedSegmentIndex, SelectedTrackIndex as UISelectedTrackIndex,
    db::{PresetTextStyleConfig, PresetTextStyleData, TextStyleConfig, VIDEO_EDITOR_TABLE},
    global_logic, global_store, global_ve_filter,
    logic::{toast, tr::tr, video_editor::project::TEXT_STYLE_CONFIG_ID},
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, DrawCircleDetail as UIDrawCircleDetail, Keyframe as UIKeyframe,
        KeyframeValue as UIKeyframeValue, PresetTextStyle as UIPresetTextStyle,
        PropertyTrack as UIPropertyTrack, SelectedTrackIndex, TextElement as UITextElement,
        TranscribeProgressType as UITranscribeProgressType,
        VideoEditorTrackType as UIVideoEditorTrackType,
    },
    store_video_editor_selected_segments_index,
};
use audio_utils::loader::AudioConfig;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use video_editor::{
    Error,
    commands::{
        BatchCommand, ExecuteResult,
        filter::{AddFilterCommand, ClearFiltersCommand, FilterType},
        segment::{
            AddTextKeyframeCommand, AddTextSegmentCommand, MoveTextKeyframeCommand,
            RemoveTextKeyframeCommand, SetPlaybackSpeedCommand, ShiftSegmentsAfterTimeCommand,
            UpdateTextKeyframeValueCommand,
        },
        track::{
            AddTrackCommand, DetachAudioTracksCommand, DetachSubtitleTracksCommand,
            InsertTrackCommand, MoveTrackCommand, RemoveAllGapsCommand, RemoveTrackCommand,
            StretchTrackToEndCommand, ToggleTrackLockedCommand, ToggleTrackMutedCommand,
            ToggleTrackVisibilityCommand,
        },
    },
    export::{AudioExportConfig, AudioExporter},
    filters::{
        audio::AudioSpeedFilter,
        keyframe::KeyframeValue,
        video::{DrawCircleFilter, SpeedFilter},
    },
    metadata::Metadata,
    tracks::{
        Track,
        audio_track::AudioTrack,
        image_track::ImageTrack,
        subtitle_track::SubtitleTrack,
        text_track::{TextElement, TextTrack},
        track::InnerTrack,
        video_track::VideoTrack,
    },
};

static AUDIO_EXPORT_SIG: AtomicBool = AtomicBool::new(false);

#[macro_export]
macro_rules! store_video_editor_selected_tracks_index {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_selected_tracks_index()
            .as_any()
            .downcast_ref::<VecModel<UISelectedTrackIndex>>()
            .expect("We know we set a VecModel<UISelectedTrackIndex> earlier")
    };
}

#[macro_export]
macro_rules! store_video_editor_tracks_manager_tracks {
    ($tracks:expr) => {
        $tracks
            .as_any()
            .downcast_ref::<VecModel<UIVideoEditorTrack>>()
            .expect("We know we set a VecModel<UIVideoEditorTrack> earlier")
    };
}

#[macro_export]
macro_rules! store_preset_text_styles {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_preset_text_styles()
            .as_any()
            .downcast_ref::<VecModel<UIPresetTextStyle>>()
            .expect("We know we set a VecModel<UIPresetTextStyle> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_add_track, ui, ty);
    logic_cb!(video_editor_add_empty_video_track, ui);
    logic_cb!(video_editor_add_empty_audio_track, ui);
    logic_cb!(video_editor_add_empty_subtitle_track, ui);
    logic_cb!(video_editor_add_empty_image_track, ui);
    logic_cb!(video_editor_add_empty_text_track, ui);
    logic_cb!(video_editor_remove_tracks, ui);
    logic_cb!(video_editor_track_move_up, ui, index);
    logic_cb!(video_editor_track_move_down, ui, index);
    logic_cb!(video_editor_track_move_to_top, ui, index);
    logic_cb!(video_editor_track_move_to_bottom, ui, index);
    logic_cb!(video_editor_insert_video_track, ui, index);
    logic_cb!(video_editor_insert_audio_track, ui, index);
    logic_cb!(video_editor_insert_subtitle_track, ui, index);
    logic_cb!(video_editor_insert_image_track, ui, index);
    logic_cb!(video_editor_insert_text_track, ui, index);
    logic_cb!(video_editor_toggle_locked_track, ui, index);
    logic_cb!(video_editor_toggle_hiding_track, ui, index);
    logic_cb!(video_editor_toggle_muted_track, ui, index);
    logic_cb!(video_editor_detach_audio_track, ui, index);
    logic_cb!(video_editor_detach_subtitle_track, ui, index);
    logic_cb!(video_editor_paste_filter_to_track, ui, index);
    logic_cb!(video_editor_remove_all_filters_from_track, ui, index);
    logic_cb!(video_editor_track_stretch_to_end, ui, index);
    logic_cb!(video_editor_track_remove_all_gap, ui, index);
    logic_cb!(video_editor_select_all_tracks, ui);
    logic_cb!(video_editor_unselect_all_tracks, ui);
    logic_cb!(video_editor_add_selected_track, ui, index);
    logic_cb!(video_editor_is_video_track, ui, index);
    logic_cb!(video_editor_is_video_with_audio_track, ui, index);
    logic_cb!(video_editor_is_audio_track, ui, index);
    logic_cb!(video_editor_is_subtitle_track, ui, index);
    logic_cb!(video_editor_is_image_track, ui, index);
    logic_cb!(video_editor_is_text_track, ui, index);
    logic_cb!(video_editor_contain_audio_track, ui, index);
    logic_cb!(video_editor_contain_subtitle_track, ui, index);
    logic_cb!(
        video_editor_is_video_segment,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(
        video_editor_is_audio_segment,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(
        video_editor_is_subtitle_segment,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(
        video_editor_is_selected_track,
        ui,
        selected_tracks,
        index,
        _flag
    );
    logic_cb!(video_editor_can_move_track, ui, from_index, to_index);
    logic_cb!(video_editor_move_track_by_drag, ui, from_index, to_index);
    logic_cb!(
        video_editor_find_track_reorder_target,
        ui,
        relative_y,
        from_index
    );
    logic_cb!(video_editor_clear_all_selected_state, ui);
    logic_cb!(video_editor_transcribe_audio, ui);
    logic_cb!(video_editor_get_track_accumulated_height, ui, index);
    logic_cb!(video_editor_add_text_segment, ui);
    logic_cb!(
        video_editor_load_text_element,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(video_editor_update_text_element, ui, element);
    logic_cb!(video_editor_update_text_position, ui, x, y);
    logic_cb!(video_editor_add_text_keyframe, ui, property, time_ms, value);
    logic_cb!(video_editor_remove_text_keyframe, ui, property, time_ms);
    logic_cb!(
        video_editor_move_text_keyframe,
        ui,
        property,
        old_time_ms,
        new_time_ms
    );
    logic_cb!(
        video_editor_update_text_keyframe_value,
        ui,
        property,
        time_ms,
        value
    );
    logic_cb!(
        video_editor_get_text_keyframes,
        ui,
        track_index,
        segment_index,
        property
    );
    logic_cb!(
        video_editor_text_property_has_keyframe_at_playhead,
        ui,
        track_index,
        segment_index,
        property,
        _flag
    );
    logic_cb!(
        video_editor_get_text_property_tracks,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(
        video_editor_get_text_element_at_playhead,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(
        video_editor_get_draw_circle_at_playhead,
        ui,
        track_index,
        segment_index
    );
    logic_cb!(video_editor_exit_text_editing, ui);
    logic_cb!(video_editor_show_preset_text_style_panel, ui);
    logic_cb!(video_editor_show_preset_text_style_new_lineinput, ui);
    logic_cb!(video_editor_create_preset_text_style, ui, name);
    logic_cb!(video_editor_remove_preset_text_style, ui, index);
    logic_cb!(video_editor_apply_preset_text_style, ui, style);
}

fn inner_init(ui: &AppWindow) {
    load_text_style_config_on_startup(ui);
    load_preset_text_styles_from_db(ui);
}

fn video_editor_add_track(ui: &AppWindow, ty: UIVideoEditorTrackType) {
    let index = global_store!(ui)
        .get_video_editor_tracks_manager()
        .tracks
        .row_count();
    let track_name: String = global_logic!(ui)
        .invoke_defalut_video_editor_track_name(ty, index as i32)
        .into();

    let new_track = create_empty_track(ty, track_name.clone());
    let command = AddTrackCommand::new(new_track);

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh_tracks_only(ui, execute_result.affected_segments);
            reset_editor_selection_state(ui);
            crate::toast_success!(ui, format!("{} {}", tr("Added"), track_name));
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_add_empty_video_track(ui: &AppWindow) {
    video_editor_add_track(ui, UIVideoEditorTrackType::Video);
}

fn video_editor_add_empty_audio_track(ui: &AppWindow) {
    video_editor_add_track(ui, UIVideoEditorTrackType::Audio);
}

fn video_editor_add_empty_subtitle_track(ui: &AppWindow) {
    video_editor_add_track(ui, UIVideoEditorTrackType::Subtitle);
}

fn video_editor_add_empty_image_track(ui: &AppWindow) {
    video_editor_add_track(ui, UIVideoEditorTrackType::Image);
}

fn video_editor_add_empty_text_track(ui: &AppWindow) {
    video_editor_add_track(ui, UIVideoEditorTrackType::Text);
}

fn video_editor_remove_tracks(ui: &AppWindow) {
    let mut selected_track_indices = get_selected_track_indices(ui);

    if selected_track_indices.is_empty() {
        crate::toast_warn!(ui, tr("No tracks selected"));
        return;
    }

    // Remove tracks in reverse order to maintain indices
    selected_track_indices.sort_by(|a, b| b.cmp(a));

    let mut batch_command =
        BatchCommand::new(format!("Remove {} tracks", selected_track_indices.len()));

    for index in &selected_track_indices {
        batch_command.add_command(Box::new(RemoveTrackCommand::new(*index)));
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(_) => {
            sync_and_refresh_simple(ui);
            reset_editor_selection_state(ui);
            crate::toast_success!(
                ui,
                format!(
                    "{} {}",
                    tr("Removed"),
                    format!("{} {}", selected_track_indices.len(), tr("tracks"))
                )
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_track_move_up(ui: &AppWindow, index: i32) {
    if index <= 0 {
        return;
    }

    if is_track_locked(ui, index) {
        crate::toast_warn!(ui, tr("Cannot move a locked track"));
        return;
    }

    let idx = index as usize;

    let result = with_history_manager(|state| {
        if idx >= state.tracks_manager.len() {
            return Err(
                video_editor::Error::IndexOutOfBounds(idx, state.tracks_manager.len()).into(),
            );
        }

        let command = MoveTrackCommand::new(idx, idx - 1);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh_tracks_only(ui, execute_result.affected_segments);
            crate::toast_success!(
                ui,
                format!(
                    "{} {} {} {}",
                    tr("Moved track up from"),
                    index,
                    tr("to"),
                    index - 1
                )
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_track_move_down(ui: &AppWindow, index: i32) {
    if is_track_locked(ui, index) {
        crate::toast_warn!(ui, tr("Cannot move a locked track"));
        return;
    }

    let idx = index as usize;

    let result = with_history_manager(|state| {
        if idx >= state.tracks_manager.len() {
            return Err(
                video_editor::Error::IndexOutOfBounds(idx, state.tracks_manager.len()).into(),
            );
        }

        if idx + 1 >= state.tracks_manager.len() {
            return Err(
                Error::InvalidConfig("Cannot move track down from last position".into()).into(),
            );
        }

        let command = MoveTrackCommand::new(idx, idx + 1);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh_tracks_only(ui, execute_result.affected_segments);
            crate::toast_success!(
                ui,
                format!(
                    "{} {} {} {}",
                    tr("Moved track down from"),
                    index,
                    tr("to"),
                    index + 1
                )
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_track_move_to_top(ui: &AppWindow, index: i32) {
    if index <= 0 {
        return;
    }

    if is_track_locked(ui, index) {
        crate::toast_warn!(ui, tr("Cannot move a locked track"));
        return;
    }

    let idx = index as usize;

    let result = with_history_manager(|state| {
        if idx >= state.tracks_manager.len() {
            return Err(
                video_editor::Error::IndexOutOfBounds(idx, state.tracks_manager.len()).into(),
            );
        }

        // 使用新方法获取同优先级组的顶部位置
        let target_idx = state.tracks_manager.priority_group_top(idx);
        let target_idx = match target_idx {
            Some(t) => t,
            None => {
                return Err(
                    video_editor::Error::IndexOutOfBounds(idx, state.tracks_manager.len()).into(),
                );
            }
        };

        if idx == target_idx {
            return Ok(ExecuteResult {
                affected_segments: Default::default(),
            });
        }

        let command = MoveTrackCommand::new(idx, target_idx);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh_tracks_only(ui, execute_result.affected_segments);
            crate::toast_success!(
                ui,
                format!(
                    "{} {} {}",
                    tr("Moved track from"),
                    index,
                    tr("to top of priority group")
                )
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_track_move_to_bottom(ui: &AppWindow, index: i32) {
    if is_track_locked(ui, index) {
        crate::toast_warn!(ui, tr("Cannot move a locked track"));
        return;
    }

    let idx = index as usize;

    let result = with_history_manager(|state| {
        if idx >= state.tracks_manager.len() {
            return Err(
                video_editor::Error::IndexOutOfBounds(idx, state.tracks_manager.len()).into(),
            );
        }

        // 使用新方法获取同优先级组的底部位置
        let target_idx = state.tracks_manager.priority_group_bottom(idx);
        let target_idx = match target_idx {
            Some(t) => t,
            None => {
                return Err(
                    video_editor::Error::IndexOutOfBounds(idx, state.tracks_manager.len()).into(),
                );
            }
        };

        if idx == target_idx {
            return Ok(ExecuteResult {
                affected_segments: Default::default(),
            });
        }

        let command = MoveTrackCommand::new(idx, target_idx);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh_tracks_only(ui, execute_result.affected_segments);
            crate::toast_success!(
                ui,
                format!(
                    "{} {} {}",
                    tr("Moved track from"),
                    index,
                    tr("to bottom of priority group")
                )
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_insert_video_track(ui: &AppWindow, index: i32) {
    insert_track_by_type(ui, index, UIVideoEditorTrackType::Video);
}

fn video_editor_insert_audio_track(ui: &AppWindow, index: i32) {
    insert_track_by_type(ui, index, UIVideoEditorTrackType::Audio);
}

fn video_editor_insert_subtitle_track(ui: &AppWindow, index: i32) {
    insert_track_by_type(ui, index, UIVideoEditorTrackType::Subtitle);
}

fn video_editor_insert_image_track(ui: &AppWindow, index: i32) {
    insert_track_by_type(ui, index, UIVideoEditorTrackType::Image);
}

fn video_editor_insert_text_track(ui: &AppWindow, index: i32) {
    insert_track_by_type(ui, index, UIVideoEditorTrackType::Text);
}

fn insert_track_by_type(ui: &AppWindow, index: i32, ty: UIVideoEditorTrackType) {
    let rows = global_store!(ui)
        .get_video_editor_tracks_manager()
        .tracks
        .row_count();
    let track_name: String = global_logic!(ui)
        .invoke_defalut_video_editor_track_name(ty, rows as i32)
        .into();

    let insert_index = index as usize;
    let new_track = create_empty_track(ty, track_name.clone());

    let result: Result<(ExecuteResult, usize), Error> = with_history_manager(|state| {
        let actual_index = state
            .tracks_manager
            .find_valid_insert_position(insert_index, &new_track);
        let command = InsertTrackCommand::new(new_track.clone(), actual_index);
        let execute_result = state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))?;
        Ok((execute_result, actual_index))
    });

    match result {
        Ok((execute_result, actual_index)) => {
            sync_and_refresh_tracks_only(ui, execute_result.affected_segments);
            reset_editor_selection_state(ui);
            if actual_index == insert_index {
                crate::toast_success!(
                    ui,
                    format!(
                        "{} {} {} {}",
                        tr("Inserted"),
                        track_name,
                        tr("track at index"),
                        actual_index
                    )
                );
            } else {
                crate::toast_success!(
                    ui,
                    format!(
                        "{} {} {} {} ({} {})",
                        tr("Inserted"),
                        track_name,
                        tr("track at index"),
                        actual_index,
                        tr("adjusted from"),
                        insert_index
                    )
                );
            }
        }
        Err(e) => {
            crate::toast_warn!(ui, e.to_string());
        }
    }
}

fn video_editor_toggle_locked_track(ui: &AppWindow, index: i32) {
    let idx = index as usize;

    let result: Result<(bool, String), Error> = with_history_manager(|state| {
        if idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(idx, state.tracks_manager.len()));
        }

        let track_name = state
            .tracks_manager
            .get(idx)
            .map(|t| match t {
                Track::Video(inner) => inner.name.clone(),
                Track::Audio(inner) => inner.name.clone(),
                Track::Subtitle(inner) => inner.name.clone(),
                Track::Image(inner) => inner.name.clone(),
                Track::Text(inner) => inner.name.clone(),
            })
            .unwrap_or_default();

        let was_locked = state
            .tracks_manager
            .get(idx)
            .map(|t| t.is_locked())
            .unwrap_or(false);

        let command = ToggleTrackLockedCommand::new(idx);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))?;

        Ok((was_locked, track_name))
    });

    match result {
        Ok((was_locked, track_name)) => {
            sync_manager_to_ui(ui);
            crate::toast_success!(
                ui,
                format!(
                    "{} '{}' {}",
                    tr("Track"),
                    track_name,
                    if !was_locked {
                        tr("locked")
                    } else {
                        tr("unlocked")
                    }
                )
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_toggle_hiding_track(ui: &AppWindow, index: i32) {
    let idx = index as usize;

    let result: Result<(bool, String, bool), Error> = with_history_manager(|state| {
        if idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(idx, state.tracks_manager.len()));
        }

        let track_name = state
            .tracks_manager
            .get(idx)
            .map(|t| match t {
                Track::Video(inner) => inner.name.clone(),
                Track::Audio(inner) => inner.name.clone(),
                Track::Subtitle(inner) => inner.name.clone(),
                Track::Image(inner) => inner.name.clone(),
                Track::Text(inner) => inner.name.clone(),
            })
            .unwrap_or_default();

        let was_hiding = state
            .tracks_manager
            .get(idx)
            .map(|t| t.is_hiding())
            .unwrap_or(false);

        let command = ToggleTrackVisibilityCommand::new(idx);
        let execute_result = state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))?;

        Ok((
            was_hiding,
            track_name,
            execute_result.affected_segments.tracks_changed,
        ))
    });

    match result {
        Ok((was_hiding, track_name, tracks_changed)) => {
            sync_and_refresh_tracks_changed(ui, tracks_changed);

            crate::toast_success!(
                ui,
                format!(
                    "{} '{}' {}",
                    tr("Track"),
                    track_name,
                    if !was_hiding {
                        tr("hidden")
                    } else {
                        tr("visible")
                    }
                )
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_toggle_muted_track(ui: &AppWindow, index: i32) {
    let idx = index as usize;

    let result: Result<(bool, String), Error> = with_history_manager(|state| {
        if idx >= state.tracks_manager.len() {
            return Err(Error::IndexOutOfBounds(idx, state.tracks_manager.len()));
        }

        let track = state.tracks_manager.get(idx);
        let can_mute = match track {
            Some(Track::Video(_)) => true,
            _ => false,
        };

        if !can_mute {
            return Err(Error::InvalidConfig(
                "Only video tracks can be muted".into(),
            ));
        }

        let track_name = track
            .and_then(|t| match t {
                Track::Video(inner) => Some(inner.name.clone()),
                _ => None,
            })
            .unwrap_or_default();

        let was_muted = track.map(|t| t.is_muted()).unwrap_or(false);

        let command = ToggleTrackMutedCommand::new(idx);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))?;

        Ok((was_muted, track_name))
    });

    match result {
        Ok((was_muted, track_name)) => {
            sync_manager_to_ui(ui);

            crate::toast_success!(
                ui,
                format!(
                    "{} '{}' {}",
                    tr("Track"),
                    track_name,
                    if !was_muted {
                        tr("muted")
                    } else {
                        tr("unmuted")
                    }
                )
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_detach_audio_track(ui: &AppWindow, index: i32) {
    let track_idx = index as usize;

    let command = DetachAudioTracksCommand::new(track_idx);
    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(false));
            crate::toast_success!(
                ui,
                format!("{} {}", tr("Detached audio tracks from track"), index)
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to detach audio"), e)),
    }
}

fn video_editor_detach_subtitle_track(ui: &AppWindow, index: i32) {
    let track_idx = index as usize;
    let subtitle_style: SubtitleStyleConfig = global_ve_filter!(ui).get_subtitle_style().into();
    let filters = create_subtitle_style_filters_from_config(&subtitle_style);

    let result: Result<ExecuteResult, Error> = with_history_manager(|state| {
        let num_subtitle_streams = state
            .tracks_manager
            .get(track_idx)
            .and_then(|track| {
                if matches!(track, Track::Video(_)) {
                    Some(track.metadata().subtitles.len())
                } else {
                    None
                }
            })
            .unwrap_or(0);

        state
            .history_manager
            .begin_batch("Detach subtitle tracks with font style".to_string());

        let command = DetachSubtitleTracksCommand::new(track_idx);

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
            sync_and_refresh(ui, execute_result.affected_segments, Some(false));
            crate::toast_success!(
                ui,
                format!("{} {}", tr("Detached subtitle tracks from track"), index)
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to detach subtitles"), e)),
    }
}

fn video_editor_paste_filter_to_track(ui: &AppWindow, index: i32) {
    let track_idx = index as usize;
    let cached_filter = global_ve_filter!(ui).get_cache_copied_filter();
    let filter_entry = cached_filter.clone().into();
    let filter_name = cached_filter.name.to_string();

    if filter_name.is_empty() {
        crate::toast_warn!(ui, tr("No filter copied to paste"));
        return;
    }

    let filter_type: Option<FilterType> = with_history_manager(|state| {
        let track = state.tracks_manager.get(track_idx)?;
        Some(track_to_filter_type(&track))
    });

    let Some(filter_type) = filter_type else {
        crate::toast_warn!(ui, format!("{} {}", tr("Track not found"), index));
        return;
    };

    let segment_count = with_history_manager(|state| {
        state
            .tracks_manager
            .get(track_idx)
            .map(|t| t.segments_count())
            .unwrap_or(0)
    });

    if segment_count == 0 {
        crate::toast_warn!(ui, format!("{} {}", tr("Track has no segments"), index));
        return;
    }

    let mut batch_command =
        BatchCommand::new(format!("Paste filter '{}' to track {}", filter_name, index));

    let filter_detail = cached_filter.detail.to_string();

    for seg_idx in 0..segment_count {
        let has_filter = segment_contains_filter(track_idx, seg_idx, &filter_entry);
        if matches!(has_filter, Some(true)) {
            continue;
        }

        if let Some(cmd) = create_filter_command_with_detail(
            track_idx,
            seg_idx,
            filter_type.clone(),
            &filter_name,
            &filter_detail,
            cached_filter.enabled,
        ) {
            batch_command.add_command(cmd);
        }
    }

    if !batch_command.is_empty() {
        let result = with_history_manager(|state| {
            state
                .history_manager
                .execute(&mut state.tracks_manager, Box::new(batch_command))
        });

        match result {
            Ok(execute_result) => {
                sync_and_refresh(ui, execute_result.affected_segments, Some(true));

                global_ve_filter!(ui).invoke_refresh_filter_list();
                crate::toast_success!(
                    ui,
                    format!(
                        "{} '{}' {} {} {}",
                        tr("Pasted filter"),
                        filter_name,
                        tr("to"),
                        segment_count,
                        tr("segments")
                    )
                );
            }
            Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to paste filter"), e)),
        }
    } else {
        crate::toast_warn!(
            ui,
            format!(
                "{}: {} {}",
                tr("Failed to paste filter"),
                tr("all track's segments have contained filter"),
                filter_name
            )
        );
    }
}

fn video_editor_remove_all_filters_from_track(ui: &AppWindow, index: i32) {
    let track_idx = index as usize;

    let speed_reset_info: Vec<(usize, usize, f32, Duration)> = with_history_manager(|state| {
        let track = state.tracks_manager.get(track_idx)?;
        let segment_count = track.segments_count();

        Some(
            (0..segment_count)
                .filter_map(|seg_idx| {
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
                        Some((track_idx, seg_idx, segment.playback_speed, segment.duration))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>(),
        )
    })
    .unwrap_or_default();

    let result: Result<ExecuteResult, video_editor::Error> = with_history_manager(|state| {
        let track = state.tracks_manager.get(track_idx).ok_or_else(|| {
            video_editor::Error::IndexOutOfBounds(track_idx, state.tracks_manager.len())
        })?;

        let segment_count = track.segments_count();
        let mut batch_command = BatchCommand::new("Remove all filters from track".to_string());

        for seg_idx in 0..segment_count {
            batch_command.add_command(Box::new(ClearFiltersCommand::new_video(track_idx, seg_idx)));
            batch_command.add_command(Box::new(ClearFiltersCommand::new_audio(track_idx, seg_idx)));
            batch_command.add_command(Box::new(ClearFiltersCommand::new_subtitle(
                track_idx, seg_idx,
            )));
            batch_command.add_command(Box::new(ClearFiltersCommand::new_image(track_idx, seg_idx)));
        }

        for (t_idx, s_idx, old_speed, old_duration) in speed_reset_info {
            batch_command.add_command(Box::new(SetPlaybackSpeedCommand::new(
                t_idx,
                s_idx,
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
        Ok(_execute_result) => {
            sync_and_refresh_simple(ui);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
            crate::toast_success!(
                ui,
                format!("{} {}", tr("Removed all filters from track"), index)
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove filters"), e)),
    }
}

fn video_editor_track_stretch_to_end(ui: &AppWindow, index: i32) {
    let track_idx = index as usize;

    let result: Result<ExecuteResult, video_editor::Error> = with_history_manager(|state| {
        if track_idx >= state.tracks_manager.len() {
            return Err(video_editor::Error::IndexOutOfBounds(
                track_idx,
                state.tracks_manager.len(),
            ));
        }

        let command = StretchTrackToEndCommand::new(track_idx);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(false));
            crate::toast_success!(ui, format!("{} {}", tr("Stretched track to end"), index));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to stretch track"), e)),
    }
}

fn video_editor_track_remove_all_gap(ui: &AppWindow, index: i32) {
    let track_idx = index as usize;
    let command = RemoveAllGapsCommand::new(track_idx);
    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(_) => {
            sync_manager_to_ui(ui);
            crate::toast_success!(
                ui,
                format!("{} {}", tr("Removed all gaps from track"), index)
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove gaps"), e)),
    }
}

fn video_editor_select_all_tracks(ui: &AppWindow) {
    let all_track_indices =
        with_history_manager(|state| (0..state.tracks_manager.len()).collect::<Vec<_>>());

    let mut selected_indices = vec![];
    for track_index in &all_track_indices {
        selected_indices.push(SelectedTrackIndex {
            index: *track_index as i32,
            modifiers: Default::default(),
        });
    }

    store_video_editor_selected_tracks_index!(ui).set_vec(selected_indices);
    let flag = global_store!(ui).get_video_editor_track_selected_flag();
    global_store!(ui).set_video_editor_track_selected_flag(!flag);
}

fn video_editor_unselect_all_tracks(ui: &AppWindow) {
    store_video_editor_selected_tracks_index!(ui).set_vec(vec![]);
    let flag = global_store!(ui).get_video_editor_track_selected_flag();
    global_store!(ui).set_video_editor_track_selected_flag(!flag);
}

fn video_editor_add_selected_track(ui: &AppWindow, index: SelectedTrackIndex) {
    let is_selected = store_video_editor_selected_tracks_index!(ui)
        .iter()
        .any(|s| s.index == index.index);

    let mut selected_indices: Vec<SelectedTrackIndex> =
        store_video_editor_selected_tracks_index!(ui)
            .iter()
            .collect();

    if index.modifiers.control {
        if is_selected {
            selected_indices.retain(|s| s.index != index.index);
        } else {
            selected_indices.push(index.clone());
        }
    } else if index.modifiers.shift {
        // Range selection (from last selected to current)
        if let Some(last) = selected_indices.last() {
            let start = last.index.min(index.index);
            let end = last.index.max(index.index);
            for i in start..=end {
                if !selected_indices.iter().any(|s| s.index == i) {
                    selected_indices.push(SelectedTrackIndex {
                        index: i,
                        modifiers: Default::default(),
                    });
                }
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

    store_video_editor_selected_tracks_index!(ui).set_vec(selected_indices);
}

fn video_editor_is_video_track(_ui: &AppWindow, index: i32) -> bool {
    if index < 0 {
        return false;
    }

    with_history_manager(|state| {
        let idx = index as usize;
        state
            .tracks_manager
            .get(idx)
            .map_or(false, |track| matches!(track, Track::Video(_)))
    })
}

fn video_editor_is_video_with_audio_track(_ui: &AppWindow, index: i32) -> bool {
    if index < 0 {
        return false;
    }

    with_history_manager(|state| {
        let idx = index as usize;
        state
            .tracks_manager
            .get(idx)
            .map_or(false, |track| match track {
                Track::Video(vt) => {
                    !vt.track.metadata.audios.is_empty() || vt.has_audio_in_any_segment()
                }
                _ => false,
            })
    })
}

fn video_editor_is_audio_track(_ui: &AppWindow, index: i32) -> bool {
    if index < 0 {
        return false;
    }

    with_history_manager(|state| {
        let idx = index as usize;
        state
            .tracks_manager
            .get(idx)
            .map_or(false, |track| matches!(track, Track::Audio(_)))
    })
}

fn video_editor_is_subtitle_track(_ui: &AppWindow, index: i32) -> bool {
    if index < 0 {
        return false;
    }

    with_history_manager(|state| {
        let idx = index as usize;
        state
            .tracks_manager
            .get(idx)
            .map_or(false, |track| matches!(track, Track::Subtitle(_)))
    })
}

fn video_editor_is_image_track(_ui: &AppWindow, index: i32) -> bool {
    if index < 0 {
        return false;
    }

    with_history_manager(|state| {
        let idx = index as usize;
        state
            .tracks_manager
            .get(idx)
            .map_or(false, |track| matches!(track, Track::Image(_)))
    })
}

fn video_editor_is_text_track(_ui: &AppWindow, index: i32) -> bool {
    if index < 0 {
        return false;
    }

    with_history_manager(|state| {
        let idx = index as usize;
        state
            .tracks_manager
            .get(idx)
            .map_or(false, |track| matches!(track, Track::Text(_)))
    })
}

fn video_editor_contain_audio_track(_ui: &AppWindow, index: i32) -> bool {
    let idx = index as usize;
    with_history_manager(|state| {
        state
            .tracks_manager
            .get(idx)
            .map_or(false, |track| match track {
                Track::Video(vt) => vt.has_audio_in_any_segment(),
                Track::Audio(_) => true,
                _ => false,
            })
    })
}

fn video_editor_contain_subtitle_track(_ui: &AppWindow, index: i32) -> bool {
    let idx = index as usize;
    with_history_manager(|state| {
        state
            .tracks_manager
            .get(idx)
            .map_or(false, |track| match track {
                Track::Video(vt) => !vt.track.metadata.subtitles.is_empty(),
                Track::Subtitle(_) => true,
                _ => false,
            })
    })
}

fn video_editor_is_selected_track(
    _ui: &AppWindow,
    selected_tracks: ModelRc<SelectedTrackIndex>,
    index: i32,
    _flag: bool,
) -> bool {
    let count = selected_tracks.row_count();
    for i in 0..count {
        let s = selected_tracks.row_data(i).unwrap();
        if s.index == index {
            return true;
        }
    }
    false
}

fn create_empty_track(ty: UIVideoEditorTrackType, track_name: String) -> Track {
    let metadata = Arc::new(Metadata::default());
    let inner_track = InnerTrack::new(metadata, Duration::ZERO, vec![]);

    match ty {
        UIVideoEditorTrackType::Video => Track::Video(Arc::new(VideoTrack {
            name: track_name,
            hiding: false,
            muted: false,
            locked: false,
            track: inner_track,
        })),
        UIVideoEditorTrackType::Audio => Track::Audio(Arc::new(AudioTrack {
            name: track_name,
            hiding: false,
            locked: false,
            track: inner_track,
        })),
        UIVideoEditorTrackType::Subtitle => Track::Subtitle(Arc::new(SubtitleTrack {
            name: track_name,
            hiding: false,
            locked: false,
            track: inner_track,
        })),
        UIVideoEditorTrackType::Image => Track::Image(Arc::new(ImageTrack {
            name: track_name,
            hiding: false,
            locked: false,
            track: inner_track,
        })),
        UIVideoEditorTrackType::Text => {
            Track::Text(Arc::new(TextTrack::new().with_name(track_name)))
        }
    }
}

pub fn get_selected_track_indices(ui: &AppWindow) -> Vec<usize> {
    store_video_editor_selected_tracks_index!(ui)
        .iter()
        .filter_map(|s| {
            if s.index >= 0 {
                Some(s.index as usize)
            } else {
                None
            }
        })
        .collect()
}

pub fn get_selected_segment_indices(ui: &AppWindow) -> Vec<(usize, usize)> {
    store_video_editor_selected_segments_index!(ui)
        .iter()
        .filter_map(|s| {
            if s.track_index >= 0 && s.index >= 0 {
                Some((s.track_index as usize, s.index as usize))
            } else {
                None
            }
        })
        .collect()
}

pub fn is_track_locked(ui: &AppWindow, track_index: i32) -> bool {
    if track_index < 0 {
        return false;
    }

    let tracks = global_store!(ui).get_video_editor_tracks_manager().tracks;

    tracks
        .row_data(track_index as usize)
        .map(|t| t.locked)
        .unwrap_or(false)
}

pub fn reset_editor_selection_state(ui: &AppWindow) {
    global_store!(ui).set_video_editor_current_edited_track_index(-1);
    global_ve_filter!(ui).set_selected_filter_index(-1);
    store_video_editor_selected_tracks_index!(ui).set_vec(vec![]);
    store_video_editor_selected_segments_index!(ui).set_vec(vec![]);

    let track_flag = global_store!(ui).get_video_editor_track_selected_flag();
    global_store!(ui).set_video_editor_track_selected_flag(!track_flag);

    let segment_flag = global_store!(ui).get_video_editor_segment_selected_flag();
    global_store!(ui).set_video_editor_segment_selected_flag(!segment_flag);
}

fn video_editor_clear_all_selected_state(ui: &AppWindow) {
    reset_editor_selection_state(ui);
}

pub fn cancel_audio_export() {
    AUDIO_EXPORT_SIG.store(true, Ordering::Relaxed);
}

fn video_editor_transcribe_audio(ui: &AppWindow) {
    let mut entry = global_store!(ui).get_video_editor_transcribe();
    if entry.subtitles.row_count() == 0 {
        entry.subtitles = ModelRc::new(VecModel::from_slice(&[]));
    }
    entry.playing_index = -1;
    entry.progress_type = UITranscribeProgressType::ImportingAudio;
    entry.progress = 0.0;
    entry.media_duration_ms = 0.0;
    global_store!(ui).set_video_editor_transcribe(entry);
    global_store!(ui).set_video_editor_transcribe_audio_player_progress(0.0);
    global_store!(ui).set_video_editor_transcribe_audio_player_is_playing(false);
    global_store!(ui).set_video_editor_transcribe_is_show_dialog(true);
    crate::toast_info!(ui, tr("Exporting audio, please wait..."));

    AUDIO_EXPORT_SIG.store(false, Ordering::Relaxed);

    let ui_weak = ui.as_weak();
    std::thread::spawn(move || {
        let manager = {
            let state_guard = PROJECT_STATE.lock().unwrap();
            let state = state_guard.as_ref().unwrap();
            Arc::new(state.tracks_manager.clone())
        };

        if AUDIO_EXPORT_SIG.load(Ordering::Relaxed) {
            return;
        }

        let result = AudioExporter::new(manager, AudioExportConfig::default())
            .collect_audio_samples_with_progress(
                fun_ast_nano::INPUT_AUDIO_CHANNELS as u16,
                fun_ast_nano::INPUT_AUDIO_SAMPLE_RATE,
                |progress| {
                    if AUDIO_EXPORT_SIG.load(Ordering::Relaxed) {
                        return;
                    }
                    _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                        let mut entry = global_store!(ui).get_video_editor_transcribe();
                        entry.progress = progress;
                        global_store!(ui).set_video_editor_transcribe(entry);
                    });
                },
            );

        if AUDIO_EXPORT_SIG.load(Ordering::Relaxed) {
            return;
        }

        match result {
            Ok(audio_samples) => {
                let duration = Duration::from_secs_f64(
                    audio_samples.samples.len() as f64
                        / (audio_samples.channels as f64 * audio_samples.sample_rate as f64),
                );
                let audio_config = AudioConfig::default()
                    .with_sample_rate(audio_samples.sample_rate)
                    .with_channel(audio_samples.channels)
                    .with_duration(duration)
                    .with_samples(audio_samples.samples);

                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    set_current_audio_config(Some(audio_config.clone()));

                    let mut entry = global_store!(ui).get_video_editor_transcribe();
                    entry.progress_type = UITranscribeProgressType::None;
                    entry.media_duration_ms = audio_config.duration.as_millis() as f32;
                    global_store!(ui).set_video_editor_transcribe(entry);

                    crate::toast_success!(ui, tr("Audio export completed"));
                });
            }
            Err(e) => toast::async_toast_warn(
                ui_weak,
                format!("{}: {}", tr("Failed to collect audio"), e),
            ),
        }
    });
}

fn video_editor_can_move_track(_ui: &AppWindow, from_index: i32, to_index: i32) -> bool {
    if from_index < 0 || to_index < 0 {
        return false;
    }

    with_history_manager(|state| {
        let from_idx = from_index as usize;
        let to_idx = to_index as usize;

        if from_idx >= state.tracks_manager.len() || to_idx >= state.tracks_manager.len() {
            return false;
        }

        state.tracks_manager.can_move_track(from_idx, to_idx)
    })
}

fn video_editor_move_track_by_drag(ui: &AppWindow, from_index: i32, to_index: i32) {
    if from_index < 0 || to_index < 0 || from_index == to_index {
        return;
    }

    if is_track_locked(ui, from_index) {
        crate::toast_warn!(ui, tr("Cannot move a locked track"));
        return;
    }

    reset_editor_selection_state(ui);

    let from_idx = from_index as usize;
    let to_idx = to_index as usize;

    let result = with_history_manager(|state| {
        if from_idx >= state.tracks_manager.len() || to_idx >= state.tracks_manager.len() {
            return Err(video_editor::Error::IndexOutOfBounds(
                from_idx,
                state.tracks_manager.len(),
            )
            .into());
        }

        let command = MoveTrackCommand::new(from_idx, to_idx);
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh_tracks_only(ui, execute_result.affected_segments);
            crate::toast_success!(
                ui,
                format!(
                    "{} {} {} {}",
                    tr("Moved track from"),
                    from_index,
                    tr("to"),
                    to_index
                )
            );
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_find_track_reorder_target(ui: &AppWindow, relative_y: i32, from_index: i32) -> i32 {
    let from_idx = if from_index >= 0 {
        from_index as usize
    } else {
        0
    };

    with_history_manager(|state| {
        let tracks_count = state.tracks_manager.len();
        if tracks_count == 0 {
            return -1;
        }

        let mut accumulated_height: i32 = 0;

        for target_idx in 0..tracks_count {
            let track = state.tracks_manager.get(target_idx).unwrap();
            let track_height = global_logic!(ui)
                .invoke_video_editor_get_track_height_pixels(UIVideoEditorTrackType::from(track));

            if relative_y >= accumulated_height && relative_y < accumulated_height + track_height {
                if state.tracks_manager.can_move_track(from_idx, target_idx) {
                    return target_idx as i32;
                }
                return -1;
            }
            accumulated_height += track_height;
        }

        let last_idx = tracks_count - 1;
        if state.tracks_manager.can_move_track(from_idx, last_idx) {
            return last_idx as i32;
        }

        -1
    })
}

fn video_editor_get_track_accumulated_height(ui: &AppWindow, index: i32) -> i32 {
    if index <= 0 {
        return 0;
    }

    with_history_manager(|state| {
        let mut accumulated: i32 = 0;
        for i in 0..index as usize {
            let track = state.tracks_manager.get(i).unwrap();
            let track_height = global_logic!(ui)
                .invoke_video_editor_get_track_height_pixels(UIVideoEditorTrackType::from(track));
            accumulated += track_height;
        }
        accumulated
    })
}

fn calc_non_overlap_start_time(track_index: usize, playhead: Duration) -> Duration {
    with_history_manager(|state| {
        let track = state.tracks_manager.get(track_index);
        if track.is_none() {
            return playhead;
        }

        let track = track.unwrap();
        let segments = match track {
            Track::Text(tt) => &tt.track.segments,
            _ => return playhead, // 其他 track 类型不需要处理
        };

        segments
            .iter()
            .filter(|seg| {
                playhead >= seg.timeline_offset && playhead < seg.timeline_offset + seg.duration
            })
            .map(|seg| seg.timeline_offset + seg.duration)
            .max()
            .unwrap_or(playhead)
    })
}

fn calc_gap_duration(track_index: usize, start_time: Duration) -> Option<Duration> {
    with_history_manager(|state| {
        let track = state.tracks_manager.get(track_index)?;
        let segments = match track {
            Track::Text(tt) => &tt.track.segments,
            _ => return None,
        };

        let next_segment = segments
            .iter()
            .filter(|seg| seg.timeline_offset > start_time)
            .min_by_key(|seg| seg.timeline_offset);

        next_segment.map(|seg| seg.timeline_offset.saturating_sub(start_time))
    })
}

fn video_editor_add_text_segment(ui: &AppWindow) {
    let playhead_ms = global_store!(ui).get_video_editor_timeline_offset();
    let playhead = Duration::from_millis(playhead_ms as u64);
    let default_duration = Duration::from_secs(5); // Default 5 seconds
    let min_gap_for_fit = Duration::from_secs(1); // Minimum gap to fit segment without shift
    let current_track_index = global_store!(ui).get_video_editor_current_edited_track_index();

    let (track_index, should_create_track) = with_history_manager(|state| {
        if current_track_index >= 0 {
            let idx = current_track_index as usize;
            if let Some(track) = state.tracks_manager.get(idx)
                && matches!(track, Track::Text(_))
            {
                return (idx, false);
            }
        }
        (state.tracks_manager.len(), true)
    });

    let actual_track_index = if should_create_track {
        let rows = global_store!(ui)
            .get_video_editor_tracks_manager()
            .tracks
            .row_count();
        let track_name: String = global_logic!(ui)
            .invoke_defalut_video_editor_track_name(UIVideoEditorTrackType::Text, rows as i32)
            .into();
        let new_track = Track::Text(Arc::new(TextTrack::new().with_name(track_name)));
        let insert_idx =
            with_history_manager(|state| state.tracks_manager.calc_track_position(&new_track));

        let command = AddTrackCommand::new(new_track);
        let result = with_history_manager(|state| {
            state
                .history_manager
                .execute(&mut state.tracks_manager, Box::new(command))
        });

        match result {
            Ok(_) => sync_manager_to_ui(ui),
            Err(e) => {
                crate::toast_warn!(ui, e.to_string());
                return;
            }
        }

        insert_idx
    } else {
        track_index
    };

    let timeline_offset = if should_create_track {
        playhead
    } else {
        calc_non_overlap_start_time(actual_track_index, playhead)
    };

    let (segment_duration, should_shift) = if should_create_track {
        (default_duration, false)
    } else {
        match calc_gap_duration(actual_track_index, timeline_offset) {
            None => (default_duration, false), // No subsequent segments
            Some(gap) if gap < min_gap_for_fit => (default_duration, true), // Need to shift
            Some(gap) if gap < default_duration => (gap, false), // Use gap duration
            Some(_) => (default_duration, false), // Gap >= 5 seconds
        }
    };

    let element = global_store!(ui)
        .get_video_editor_current_text_element()
        .into();

    let result = if should_shift {
        with_history_manager(|state| {
            let mut batch = BatchCommand::new("Add text segment with shift".to_string());

            batch.add_command(Box::new(ShiftSegmentsAfterTimeCommand::new(
                actual_track_index,
                timeline_offset,
                default_duration,
            )));

            batch.add_command(Box::new(AddTextSegmentCommand::new(
                actual_track_index,
                element,
                timeline_offset,
                segment_duration,
            )));

            state
                .history_manager
                .execute(&mut state.tracks_manager, Box::new(batch))
        })
    } else {
        with_history_manager(|state| {
            state.history_manager.execute(
                &mut state.tracks_manager,
                Box::new(AddTextSegmentCommand::new(
                    actual_track_index,
                    element,
                    timeline_offset,
                    segment_duration,
                )),
            )
        })
    };

    match result {
        Ok(_) => {
            sync_and_refresh_simple(ui);
            global_logic!(ui).invoke_video_editor_clear_all_selected_state();
            crate::toast_success!(ui, tr("Added text segment"));
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_load_text_element(ui: &AppWindow, track_index: i32, segment_index: i32) {
    let ti = track_index as usize;
    let si = segment_index as usize;

    let element = with_history_manager(|state| {
        state
            .tracks_manager
            .get(ti)
            .and_then(|track| {
                if let Track::Text(text_track) = track {
                    text_track.track.segments.get(si).cloned()
                } else {
                    None
                }
            })
            .and_then(|segment| segment.text_element.clone())
    });

    if let Some(text_element) = element {
        let ui_element: UITextElement = text_element.into();
        global_store!(ui).set_video_editor_current_text_element(ui_element);
        global_store!(ui).set_video_editor_is_editing_text_segment(true);
    } else {
        crate::toast_warn!(ui, tr("Failed to load text element"));
    }
}

fn video_editor_update_text_element(ui: &AppWindow, element: UITextElement) {
    let selected = get_selected_segment_indices(ui);
    let (ti, si) = match selected.last() {
        Some((track_idx, segment_idx)) => (*track_idx, *segment_idx),
        None => {
            crate::toast_warn!(ui, tr("No text segment selected"));
            return;
        }
    };

    let is_text_track = with_history_manager(|state| {
        state
            .tracks_manager
            .get(ti)
            .map(|track| matches!(track, Track::Text(_)))
    });

    if !is_text_track.unwrap_or(false) {
        crate::toast_warn!(ui, tr("Selected track is not a text track"));
        return;
    }

    let text_element: TextElement = element.clone().into();

    let existing_keyframe_tracks = with_history_manager(|state| {
        state.tracks_manager.get(ti).and_then(|track| {
            if let Track::Text(text_track) = track {
                text_track.track.segments.get(si).and_then(|seg| {
                    seg.text_element
                        .as_ref()
                        .map(|elem| elem.keyframe_tracks.clone())
                })
            } else {
                None
            }
        })
    });

    let mut final_element = text_element;
    if let Some(kf_tracks) = existing_keyframe_tracks {
        final_element.keyframe_tracks = kf_tracks;
    }

    let result = with_history_manager(|state| {
        if let Some(track) = state.tracks_manager.get_mut(ti) {
            track.modify_segment(si, |segment| {
                segment.text_element = Some(final_element.clone());
            })
        } else {
            Err(video_editor::Error::IndexOutOfBounds(
                ti,
                state.tracks_manager.len(),
            ))
        }
    });

    match result {
        Ok(_) => {
            global_store!(ui).set_video_editor_current_text_element(element.clone());
            sync_and_refresh_simple(ui);

            let config = TextStyleConfig::from(&element);
            tokio::spawn(async move {
                save_text_style_config(config).await;
            });
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_update_text_position(ui: &AppWindow, x: f32, y: f32) {
    let current_element = global_store!(ui).get_video_editor_current_text_element();
    let mut updated_element = current_element;
    updated_element.position_x = x;
    updated_element.position_y = y;

    global_store!(ui).set_video_editor_current_text_element(updated_element.clone());
    video_editor_update_text_element(ui, updated_element);
}

fn video_editor_add_text_keyframe(
    ui: &AppWindow,
    property: slint::SharedString,
    time_ms: i32,
    value: UIKeyframeValue,
) {
    let selected = get_selected_segment_indices(ui);
    let (ti, si) = match selected.first() {
        Some((track_idx, segment_idx)) => (*track_idx, *segment_idx),
        None => {
            crate::toast_warn!(ui, tr("No text segment selected"));
            return;
        }
    };

    let prop_name = property.to_string();
    let keyframe_value: KeyframeValue = value.into();

    let segment_timeline_offset_ms: i64 = with_history_manager(|state| {
        state.tracks_manager.get(ti).and_then(|track| {
            if let Track::Text(text_track) = track {
                text_track
                    .track
                    .segments
                    .get(si)
                    .map(|seg| seg.timeline_offset.as_millis() as i64)
            } else {
                None
            }
        })
    })
    .unwrap_or(0);

    let relative_time_ms = time_ms as i64 - segment_timeline_offset_ms;

    let result = with_history_manager(|state| {
        state.history_manager.execute(
            &mut state.tracks_manager,
            Box::new(AddTextKeyframeCommand::new(
                ti,
                si,
                prop_name,
                relative_time_ms,
                keyframe_value,
            )),
        )
    });

    match result {
        Ok(_) => {
            sync_and_refresh_simple(ui);
            global_ve_filter!(ui)
                .set_toggle_keyframe_flag(!global_ve_filter!(ui).get_toggle_keyframe_flag());
        }
        Err(e) => crate::toast_warn!(ui, e.to_string()),
    }
}

fn video_editor_remove_text_keyframe(ui: &AppWindow, property: SharedString, time_ms: i32) {
    let selected = get_selected_segment_indices(ui);
    let (ti, si) = match selected.first() {
        Some((track_idx, segment_idx)) => (*track_idx, *segment_idx),
        None => return,
    };

    let prop_name = property.to_string();

    let segment_timeline_offset_ms: i64 = with_history_manager(|state| {
        state.tracks_manager.get(ti).and_then(|track| {
            if let Track::Text(text_track) = track {
                text_track
                    .track
                    .segments
                    .get(si)
                    .map(|seg| seg.timeline_offset.as_millis() as i64)
            } else {
                None
            }
        })
    })
    .unwrap_or(0);

    let relative_time_ms = time_ms as i64 - segment_timeline_offset_ms;

    let result = with_history_manager(|state| {
        state.history_manager.execute(
            &mut state.tracks_manager,
            Box::new(RemoveTextKeyframeCommand::new(
                ti,
                si,
                prop_name,
                relative_time_ms,
            )),
        )
    });

    if let Ok(_) = result {
        sync_and_refresh_simple(ui);
        global_ve_filter!(ui)
            .set_toggle_keyframe_flag(!global_ve_filter!(ui).get_toggle_keyframe_flag());
        crate::toast_success!(ui, tr("Removed keyframe"));
    }
}

fn video_editor_move_text_keyframe(
    ui: &AppWindow,
    property: SharedString,
    old_time_ms: i32,
    new_time_ms: i32,
) {
    let selected = get_selected_segment_indices(ui);
    let (ti, si) = match selected.first() {
        Some((track_idx, segment_idx)) => (*track_idx, *segment_idx),
        None => return,
    };

    let prop_name = property.to_string();

    let segment_timeline_offset_ms: i64 = with_history_manager(|state| {
        state.tracks_manager.get(ti).and_then(|track| {
            if let Track::Text(text_track) = track {
                text_track
                    .track
                    .segments
                    .get(si)
                    .map(|seg| seg.timeline_offset.as_millis() as i64)
            } else {
                None
            }
        })
    })
    .unwrap_or(0);

    let relative_old_time_ms = old_time_ms as i64 - segment_timeline_offset_ms;
    let relative_new_time_ms = new_time_ms as i64 - segment_timeline_offset_ms;

    let result = with_history_manager(|state| {
        state.history_manager.execute(
            &mut state.tracks_manager,
            Box::new(MoveTextKeyframeCommand::new(
                ti,
                si,
                prop_name,
                relative_old_time_ms,
                relative_new_time_ms,
            )),
        )
    });

    if let Ok(_) = result {
        sync_and_refresh_simple(ui);
        global_ve_filter!(ui)
            .set_toggle_keyframe_flag(!global_ve_filter!(ui).get_toggle_keyframe_flag());
    }
}

fn video_editor_update_text_keyframe_value(
    ui: &AppWindow,
    property: SharedString,
    time_ms: i32,
    value: UIKeyframeValue,
) {
    let selected = get_selected_segment_indices(ui);
    let (ti, si) = match selected.first() {
        Some((track_idx, segment_idx)) => (*track_idx, *segment_idx),
        None => return,
    };

    let prop_name = property.to_string();
    let keyframe_value: KeyframeValue = value.into();

    let segment_timeline_offset_ms: i64 = with_history_manager(|state| {
        state.tracks_manager.get(ti).and_then(|track| {
            if let Track::Text(text_track) = track {
                text_track
                    .track
                    .segments
                    .get(si)
                    .map(|seg| seg.timeline_offset.as_millis() as i64)
            } else {
                None
            }
        })
    })
    .unwrap_or(0);

    let relative_time_ms = time_ms as i64 - segment_timeline_offset_ms;

    let result = with_history_manager(|state| {
        state.history_manager.execute(
            &mut state.tracks_manager,
            Box::new(UpdateTextKeyframeValueCommand::new(
                ti,
                si,
                prop_name,
                relative_time_ms,
                keyframe_value,
            )),
        )
    });

    if let Ok(_) = result {
        sync_and_refresh_simple(ui);
    }
}

fn video_editor_get_text_keyframes(
    _ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
    property: slint::SharedString,
) -> ModelRc<UIKeyframe> {
    let ti = track_index as usize;
    let si = segment_index as usize;
    let prop_name = property.as_str();

    let keyframes = with_history_manager(|state| {
        state
            .tracks_manager
            .get(ti)
            .and_then(|track| {
                if let Track::Text(text_track) = track {
                    text_track.track.segments.get(si).cloned()
                } else {
                    None
                }
            })
            .and_then(|segment| segment.text_element.clone())
            .and_then(|element| element.keyframe_tracks.get_track(prop_name).cloned())
            .map(|track| {
                track
                    .keyframes
                    .iter()
                    .map(|k| k.clone().into())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });

    ModelRc::new(VecModel::from_slice(&keyframes))
}

fn video_editor_text_property_has_keyframe_at_playhead(
    ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
    property: SharedString,
    _flag: bool,
) -> bool {
    let ti = track_index as usize;
    let si = segment_index as usize;
    let prop_name = property.as_str();
    let timeline_offset = global_store!(ui).get_video_editor_timeline_offset();

    let (segment_timeline_offset_ms, element) = with_history_manager(|state| {
        state.tracks_manager.get(ti).and_then(|track| {
            if let Track::Text(text_track) = track {
                text_track.track.segments.get(si).map(|seg| {
                    (
                        seg.timeline_offset.as_millis() as i64,
                        seg.text_element.clone(),
                    )
                })
            } else {
                None
            }
        })
    })
    .unwrap_or((0, None));

    let relative_timeline_offset = timeline_offset as i64 - segment_timeline_offset_ms;

    element
        .and_then(|elem| {
            elem.keyframe_tracks.get_track(prop_name).map(|track| {
                track
                    .keyframes
                    .iter()
                    .any(|k| k.time_ms == relative_timeline_offset)
            })
        })
        .unwrap_or(false)
}

fn video_editor_get_text_property_tracks(
    _ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
) -> ModelRc<UIPropertyTrack> {
    let ti = track_index as usize;
    let si = segment_index as usize;

    let tracks: Vec<UIPropertyTrack> = with_history_manager(|state| {
        state
            .tracks_manager
            .get(ti)
            .and_then(|track| {
                if let Track::Text(text_track) = track {
                    text_track.track.segments.get(si).cloned()
                } else {
                    None
                }
            })
            .and_then(|segment| segment.text_element.clone())
            .map(|element| {
                element
                    .keyframe_tracks
                    .tracks
                    .iter()
                    .filter(|t| t.has_keyframes())
                    .map(|t| t.clone().into())
                    .collect()
            })
    })
    .unwrap_or_default();

    ModelRc::new(VecModel::from_slice(&tracks))
}

fn video_editor_get_text_element_at_playhead(
    ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
) -> UITextElement {
    let ti = track_index as usize;
    let si = segment_index as usize;
    let timeline_offset = global_store!(ui).get_video_editor_timeline_offset();

    let result = with_history_manager(|state| {
        state.tracks_manager.get(ti).and_then(|track| {
            if let Track::Text(text_track) = track {
                text_track.track.segments.get(si).map(|seg| {
                    (
                        seg.timeline_offset.as_millis() as i64,
                        seg.duration.as_millis() as i64,
                        seg.text_element.clone(),
                    )
                })
            } else {
                None
            }
        })
    });

    let (segment_timeline_offset_ms, segment_duration_ms, element) = result.unwrap_or((0, 0, None));

    if let Some(elem) = element {
        let relative_time_ms = (timeline_offset as i64 - segment_timeline_offset_ms).max(0);
        if relative_time_ms > segment_duration_ms {
            return global_store!(ui).get_video_editor_current_text_element();
        }

        let position = elem
            .get_value_at_time("position", relative_time_ms)
            .and_then(|v| v.as_float2())
            .unwrap_or(elem.position);

        let rotation = elem
            .get_value_at_time("rotation", relative_time_ms)
            .and_then(|v| v.as_float())
            .unwrap_or(elem.rotation);

        let opacity = elem
            .get_value_at_time("opacity", relative_time_ms)
            .and_then(|v| v.as_float())
            .unwrap_or(elem.opacity);

        let base_element: UITextElement = elem.into();
        UITextElement {
            position_x: position.0,
            position_y: position.1,
            rotation: rotation,
            opacity: opacity,
            ..base_element
        }
    } else {
        global_store!(ui).get_video_editor_current_text_element()
    }
}

fn video_editor_get_draw_circle_at_playhead(
    ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
) -> UIDrawCircleDetail {
    let ti = track_index as usize;
    let si = segment_index as usize;
    let timeline_offset = global_store!(ui).get_video_editor_timeline_offset();

    let result = with_history_manager(|state| {
        state.tracks_manager.get(ti).and_then(|track| {
            match track {
                Track::Video(video_track) => {
                    video_track.track.segments.get(si).map(|seg| {
                        (
                            seg.timeline_offset.as_millis() as i64,
                            seg.duration.as_millis() as i64,
                            seg.video_filters
                                .iter()
                                .find(|f| f.inner.name() == DrawCircleFilter::NAME)
                                .and_then(|f| f.inner.as_any().downcast_ref::<DrawCircleFilter>())
                                .cloned(),
                            true, // is_video_track
                        )
                    })
                }
                Track::Image(image_track) => {
                    image_track.track.segments.get(si).map(|seg| {
                        (
                            seg.timeline_offset.as_millis() as i64,
                            seg.duration.as_millis() as i64,
                            seg.image_filters
                                .iter()
                                .find(|f| f.inner.name() == DrawCircleFilter::NAME)
                                .and_then(|f| f.inner.as_any().downcast_ref::<DrawCircleFilter>())
                                .cloned(),
                            false, // is_image_track
                        )
                    })
                }
                _ => None,
            }
        })
    });

    let (segment_timeline_offset_ms, segment_duration_ms, filter, _) =
        result.unwrap_or((0, 0, None, false));

    if let Some(f) = filter {
        let relative_time_ms = (timeline_offset as i64 - segment_timeline_offset_ms).max(0);
        if relative_time_ms > segment_duration_ms {
            return DrawCircleFilter::default().into();
        }

        let values = f.get_values_at_time(relative_time_ms);

        let (fill_r, fill_g, fill_b, fill_a) = f
            .fill_color
            .map(|c| (c.0 as i32, c.1 as i32, c.2 as i32, c.3 as i32))
            .unwrap_or((0, 0, 0, 0));

        let (border_r, border_g, border_b, border_a) = f
            .border_color
            .map(|c| (c.0 as i32, c.1 as i32, c.2 as i32, c.3 as i32))
            .unwrap_or((0, 0, 0, 0));

        UIDrawCircleDetail {
            center_x: values.center_x,
            center_y: values.center_y,
            radius: values.radius as i32,
            fill_color_r: fill_r,
            fill_color_g: fill_g,
            fill_color_b: fill_b,
            fill_color_a: fill_a,
            border_color_r: border_r,
            border_color_g: border_g,
            border_color_b: border_b,
            border_color_a: border_a,
            border_width: values.border_width as i32,
        }
    } else {
        DrawCircleFilter::default().into()
    }
}

fn video_editor_exit_text_editing(ui: &AppWindow) {
    global_store!(ui).set_video_editor_is_editing_text_segment(false);
}

async fn save_text_style_config(config: TextStyleConfig) {
    let data = serde_json::to_string(&config).expect("serialize text style config failed");
    if sqldb::entry::insert(VIDEO_EDITOR_TABLE, TEXT_STYLE_CONFIG_ID, &data)
        .await
        .is_err()
    {
        if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, TEXT_STYLE_CONFIG_ID, &data).await
        {
            log::warn!("Failed to save text style config: {:?}", e);
        }
    }
}

fn load_text_style_config_on_startup(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_text_style_config()
            .await
            .unwrap_or_else(|| TextStyleConfig::default());
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let element = UITextElement::from(&config);
            global_store!(ui).set_video_editor_current_text_element(element);
        });
    });
}

async fn load_text_style_config() -> Option<TextStyleConfig> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, TEXT_STYLE_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => Some(TextStyleConfig::default()),
    }
}

fn video_editor_show_preset_text_style_panel(ui: &AppWindow) {
    global_store!(ui).set_is_show_preset_text_style_panel(true);
}

fn video_editor_show_preset_text_style_new_lineinput(ui: &AppWindow) {
    global_store!(ui).set_is_show_preset_text_style_new_lineinput(true);
}

fn video_editor_create_preset_text_style(ui: &AppWindow, name: SharedString) {
    if name.is_empty() {
        crate::toast_warn!(ui, tr("Name cannot be empty"));
        return;
    }

    let current_element = global_store!(ui).get_video_editor_current_text_element();
    let style_data = UIPresetTextStyle {
        name,
        style: current_element,
    };

    store_preset_text_styles!(ui).push(style_data);
    let config = collect_preset_text_styles_from_ui(ui);
    save_preset_text_styles_to_db(ui.as_weak(), config);
    global_store!(ui).set_is_show_preset_text_style_new_lineinput(false);
    crate::toast_success!(ui, tr("Preset text style created"));
}

fn video_editor_remove_preset_text_style(ui: &AppWindow, index: i32) {
    let idx = index as usize;
    if idx < store_preset_text_styles!(ui).row_count() {
        store_preset_text_styles!(ui).remove(idx);
        let config = collect_preset_text_styles_from_ui(ui);
        save_preset_text_styles_to_db(ui.as_weak(), config);
        crate::toast_success!(ui, tr("Preset text style removed"));
    }
}

fn video_editor_apply_preset_text_style(ui: &AppWindow, style: UITextElement) {
    global_store!(ui).set_video_editor_current_text_element(style.clone());
    video_editor_update_text_element(ui, style);
    global_store!(ui).set_is_show_preset_text_style_panel(false);
}

fn load_preset_text_styles_from_db(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, PRESET_TEXT_STYLES_ID).await {
            Ok(item) => {
                serde_json::from_str::<PresetTextStyleConfig>(&item.data).unwrap_or_default()
            }
            Err(_) => {
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, PRESET_TEXT_STYLES_ID, "{}").await;
                PresetTextStyleConfig::default()
            }
        };

        let ui_styles: Vec<UIPresetTextStyle> =
            config.styles.into_iter().map(|s| s.into()).collect();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            store_preset_text_styles!(ui).set_vec(ui_styles);
        });
    });
}

fn collect_preset_text_styles_from_ui(ui: &AppWindow) -> PresetTextStyleConfig {
    let data: Vec<PresetTextStyleData> = store_preset_text_styles!(ui)
        .iter()
        .map(|s| s.into())
        .collect();

    PresetTextStyleConfig { styles: data }
}

pub fn save_preset_text_styles_to_db(ui: Weak<AppWindow>, config: PresetTextStyleConfig) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).unwrap_or_default();
        if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, PRESET_TEXT_STYLES_ID, &data).await
        {
            toast::async_toast_warn(
                ui,
                format!("{}. {e}", crate::logic::tr::tr("update entry failed")),
            );
        }
    });
}

fn video_editor_is_video_segment(_ui: &AppWindow, track_index: i32, segment_index: i32) -> bool {
    if track_index < 0 || segment_index < 0 {
        return false;
    }

    with_history_manager(|state| {
        let track_idx = track_index as usize;
        state
            .tracks_manager
            .get(track_idx)
            .map_or(false, |track| matches!(track, Track::Video(_)))
    })
}

fn video_editor_is_audio_segment(_ui: &AppWindow, track_index: i32, segment_index: i32) -> bool {
    if track_index < 0 || segment_index < 0 {
        return false;
    }

    with_history_manager(|state| {
        let track_idx = track_index as usize;
        let seg_idx = segment_index as usize;
        state
            .tracks_manager
            .get(track_idx)
            .map_or(false, |track| match track {
                Track::Audio(_) => true,
                Track::Video(_) => track
                    .get_segment(seg_idx)
                    .map_or(false, |seg| !seg.metadata.audios.is_empty()),
                _ => false,
            })
    })
}

fn video_editor_is_subtitle_segment(_ui: &AppWindow, track_index: i32, segment_index: i32) -> bool {
    if track_index < 0 || segment_index < 0 {
        return false;
    }

    with_history_manager(|state| {
        let track_idx = track_index as usize;
        state
            .tracks_manager
            .get(track_idx)
            .map_or(false, |track| matches!(track, Track::Subtitle(_)))
    })
}
