use crate::{
    global_store, global_ve_filter,
    logic::{
        tr::tr,
        video_editor::{
            command::{sync_and_refresh, with_history_manager},
            conversion::video_filter_to_json_detail,
            filters::filter::get_filter_type_and_local_index,
            track::get_selected_segment_indices,
        },
    },
    slint_generatedAppWindow::{
        AnimatableProperty as UIAnimatableProperty, AppWindow, Keyframe as UIKeyframe,
        KeyframeValue as UIKeyframeValue, PropertyTrack as UIPropertyTrack,
    },
    ve_filter_cb,
};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use video_editor::{
    commands::filter::{
        AddKeyframeCommand, FilterType, MoveKeyframeCommand, RemoveKeyframeCommand,
        UpdateKeyframeValueCommand,
    },
    filters::{
        audio::{CompressorFilter, GainFilter, VoiceChangerFilter},
        interpolation::{get_color_at_time, get_float_at_time, get_float2_at_time},
        keyframe::{Keyframe, KeyframeTracks, KeyframeValue},
        video::{
            BorderFilter, ChromaKeyFilter, CircleMaskFilter, CropFilter, DirectionalBlurFilter,
            DrawCircleFilter, DrawRectangleFilter, EdgeDetectFilter, FisheyeFilter, FocusFilter,
            GaussianBlurFilter, GrainFilter, GrayscaleFilter, GridFilter, HSLAdjustFilter,
            LinearMaskFilter, Live2dFilter, LocalMagnifyFilter, MagnifierFilter, MirrorMaskFilter,
            MosaicFilter, OldFilmFilter, OpacityFilter, RectangleMaskFilter, ShadowFilter,
            SharpenFilter, SketchFilter, TransformFilter, VignetteFilter, WaveFilter,
        },
    },
};

macro_rules! animatable_properties_match {
    ($name:expr, $($filter_type:ty), *) => {
        match $name {
            $(
                <$filter_type>::NAME => <$filter_type>::animatable_properties()
                    .iter()
                    .map(|p| p.clone().into())
                    .collect(),
            )*
            _ => vec![],
        }
    };
}

macro_rules! filter_detail_at_time_match {
    ($name:expr, $wrapper:expr, $time_ms:expr, $fallback:expr, $($filter_type:ty, $getter:ident), *) => {
        match $name {
            $(
                <$filter_type>::NAME => {
                    if let Some(f) = $wrapper.inner.as_any().downcast_ref::<$filter_type>() {
                        $getter(f, $time_ms)
                    } else {
                        $fallback
                    }
                }
            )*
            _ => $fallback,
        }
    };
}

macro_rules! audio_filter_detail_at_time_match {
    ($name:expr, $wrapper:expr, $time_ms:expr, $($filter_type:ty, $getter:ident), *) => {
        match $name {
            $(
                <$filter_type>::NAME => {
                    if let Some(f) = $wrapper.inner.as_any().downcast_ref::<$filter_type>() {
                        $getter(f, $time_ms)
                    } else {
                        serde_json::to_string(&<$filter_type>::default())
                            .unwrap_or_default()
                            .into()
                    }
                }
            )*
            _ => SharedString::new(),
        }
    };
}

macro_rules! with_segment_filter {
    ($segment:expr, $filter_type:expr, $local_index:expr, |$wrapper:ident| $body:expr) => {
        match $filter_type {
            FilterType::Video => $segment
                .video_filters
                .get($local_index)
                .map(|$wrapper| $body),
            FilterType::Image => $segment
                .image_filters
                .get($local_index)
                .map(|$wrapper| $body),
            FilterType::Audio => $segment
                .audio_filters
                .get($local_index)
                .map(|$wrapper| $body),
            _ => None,
        }
    };
}

macro_rules! with_segment_filter_and_then {
    ($segment:expr, $filter_type:expr, $local_index:expr, |$wrapper:ident| $body:expr) => {
        match $filter_type {
            FilterType::Video => $segment
                .video_filters
                .get($local_index)
                .and_then(|$wrapper| $body),
            FilterType::Image => $segment
                .image_filters
                .get($local_index)
                .and_then(|$wrapper| $body),
            FilterType::Audio => $segment
                .audio_filters
                .get($local_index)
                .and_then(|$wrapper| $body),
            _ => None,
        }
    };
}

pub fn init(ui: &AppWindow) {
    ve_filter_cb!(get_animatable_properties, ui, filter_name);
    ve_filter_cb!(get_property_tracks, ui, filter_index);
    ve_filter_cb!(
        get_segment_filter_keyframes,
        ui,
        track_index,
        segment_index,
        filter_index
    );
    ve_filter_cb!(
        property_has_keyframe_at_playhead,
        ui,
        filter_index,
        property_name,
        _flag
    );
    ve_filter_cb!(
        toggle_keyframe_at_playhead,
        ui,
        filter_index,
        property_name,
        value
    );
    ve_filter_cb!(
        add_keyframe,
        ui,
        filter_index,
        property_name,
        time_ms,
        value
    );
    ve_filter_cb!(remove_keyframe, ui, filter_index, property_name, time_ms);
    ve_filter_cb!(remove_keyframe_at_index, ui, filter_index, keyframe_index);
    ve_filter_cb!(
        move_keyframe,
        ui,
        filter_index,
        property_name,
        old_time_ms,
        new_time_ms
    );
    ve_filter_cb!(
        move_keyframe_at_index,
        ui,
        filter_index,
        keyframe_index,
        new_time_ms
    );
    ve_filter_cb!(
        move_keyframe_to_playhead,
        ui,
        filter_index,
        property_name,
        old_time_ms
    );
    ve_filter_cb!(
        update_keyframe_value,
        ui,
        filter_index,
        property_name,
        time_ms,
        value
    );
    ve_filter_cb!(get_filter_detail_at_time, ui, filter_index, time_ms);
}

fn get_animatable_properties(
    _ui: &AppWindow,
    filter_name: SharedString,
) -> ModelRc<UIAnimatableProperty> {
    let name = filter_name.to_string().to_lowercase();

    let properties: Vec<UIAnimatableProperty> = animatable_properties_match!(
        name.as_str(),
        // video/image filters
        TransformFilter,
        OpacityFilter,
        MosaicFilter,
        ChromaKeyFilter,
        CropFilter,
        VignetteFilter,
        LinearMaskFilter,
        CircleMaskFilter,
        MirrorMaskFilter,
        RectangleMaskFilter,
        DrawCircleFilter,
        DrawRectangleFilter,
        HSLAdjustFilter,
        LocalMagnifyFilter,
        MagnifierFilter,
        GaussianBlurFilter,
        DirectionalBlurFilter,
        SharpenFilter,
        EdgeDetectFilter,
        GrainFilter,
        GridFilter,
        GrayscaleFilter,
        FisheyeFilter,
        FocusFilter,
        OldFilmFilter,
        SketchFilter,
        WaveFilter,
        BorderFilter,
        ShadowFilter,
        Live2dFilter,
        // audio filters
        GainFilter,
        CompressorFilter,
        VoiceChangerFilter
    );

    ModelRc::new(VecModel::from_slice(&properties))
}

fn get_property_tracks(ui: &AppWindow, filter_index: i32) -> ModelRc<UIPropertyTrack> {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        return ModelRc::new(VecModel::default());
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();
    let merged_index = filter_index as usize;

    let Some((actual_filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        return ModelRc::new(VecModel::default());
    };

    if actual_filter_type != FilterType::Video
        && actual_filter_type != FilterType::Image
        && actual_filter_type != FilterType::Audio
    {
        return ModelRc::new(VecModel::default());
    }

    let tracks: Vec<UIPropertyTrack> = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;

        with_segment_filter!(segment, actual_filter_type, local_index, |wrapper| {
            let filter_tracks = wrapper.inner.get_keyframe_tracks();
            filter_tracks
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

fn get_segment_filter_keyframes(
    _ui: &AppWindow,
    track_index: i32,
    segment_index: i32,
    filter_index: i32,
) -> ModelRc<UIKeyframe> {
    let merged_index = filter_index as usize;

    let Some((actual_filter_type, local_index)) =
        get_filter_type_and_local_index(track_index as usize, segment_index as usize, merged_index)
    else {
        return ModelRc::new(VecModel::default());
    };

    // Video, Image, and Audio filters support keyframes
    if actual_filter_type != FilterType::Video
        && actual_filter_type != FilterType::Image
        && actual_filter_type != FilterType::Audio
    {
        return ModelRc::new(VecModel::default());
    }

    let keyframes: Vec<UIKeyframe> = with_history_manager(|state| {
        let track = state.tracks_manager.get(track_index as usize)?;
        let segment = track.get_segment(segment_index as usize).ok()?;

        with_segment_filter!(segment, actual_filter_type, local_index, |wrapper| {
            let filter_tracks = wrapper.inner.get_keyframe_tracks();
            filter_tracks
                .tracks
                .iter()
                .flat_map(|t| t.keyframes.iter().map(|k| k.clone().into()))
                .collect()
        })
    })
    .unwrap_or_default();

    ModelRc::new(VecModel::from_slice(&keyframes))
}

fn property_has_keyframe_at_playhead(
    ui: &AppWindow,
    filter_index: i32,
    property_name: SharedString,
    _flag: bool,
) -> bool {
    let playhead_time_ms = global_store!(ui).get_video_editor_timeline_offset();
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        return false;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();

    // 获取 segment 的 timeline_offset，计算相对于 segment 开头的时间
    let segment_timeline_offset_ms: i32 = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;
        Some(segment.timeline_offset.as_millis() as i32)
    })
    .unwrap_or(0);

    let relative_time_ms = playhead_time_ms - segment_timeline_offset_ms;
    let merged_index = filter_index as usize;

    let Some((actual_filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        return false;
    };

    if actual_filter_type != FilterType::Video
        && actual_filter_type != FilterType::Image
        && actual_filter_type != FilterType::Audio
    {
        return false;
    }

    with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;

        with_segment_filter!(segment, actual_filter_type, local_index, |wrapper| {
            let filter_tracks = wrapper.inner.get_keyframe_tracks();
            filter_tracks
                .tracks
                .iter()
                .find(|t| t.property_name == property_name.to_string())
                .map(|t| {
                    t.keyframes
                        .iter()
                        .any(|k| k.time_ms == relative_time_ms as i64)
                })
                .unwrap_or(false)
        })
    })
    .unwrap_or(false)
}

fn toggle_keyframe_at_playhead(
    ui: &AppWindow,
    filter_index: i32,
    property_name: SharedString,
    value: UIKeyframeValue,
) {
    let playhead_time_ms = global_store!(ui).get_video_editor_timeline_offset();
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        return;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();

    // 获取 segment 的 timeline_offset，计算相对于 segment 开头的时间
    let segment_timeline_offset_ms: i32 = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;
        Some(segment.timeline_offset.as_millis() as i32)
    })
    .unwrap_or(0);

    let relative_time_ms = playhead_time_ms - segment_timeline_offset_ms;
    let has_keyframe =
        property_has_keyframe_at_playhead(ui, filter_index, property_name.clone(), true);

    if has_keyframe {
        remove_keyframe(ui, filter_index, property_name, relative_time_ms);
    } else {
        add_keyframe(ui, filter_index, property_name, relative_time_ms, value);
    }
}

fn add_keyframe(
    ui: &AppWindow,
    filter_index: i32,
    property_name: SharedString,
    time_ms: i32,
    value: UIKeyframeValue,
) {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        log::warn!("No segments selected for keyframe operation");
        return;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();
    let merged_index = filter_index as usize;

    let Some((actual_filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        log::warn!("No filter found at merged index {}", merged_index);
        return;
    };

    if actual_filter_type != FilterType::Video
        && actual_filter_type != FilterType::Image
        && actual_filter_type != FilterType::Audio
    {
        log::warn!(
            "Filter type {:?} doesn't support keyframes",
            actual_filter_type
        );
        return;
    }

    let keyframe_value: KeyframeValue = value.into();

    let command = AddKeyframeCommand::new(
        *track_idx,
        *seg_idx,
        local_index,
        property_name.to_string(),
        time_ms as i64,
        keyframe_value,
        actual_filter_type,
    );

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            refresh_selected_filter_detail_at_playhead(ui);
            global_ve_filter!(ui)
                .set_toggle_keyframe_flag(!global_ve_filter!(ui).get_toggle_keyframe_flag());
            crate::toast_success!(ui, tr("Added keyframe"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to add keyframe"), e)),
    }
}

fn remove_keyframe(ui: &AppWindow, filter_index: i32, property_name: SharedString, time_ms: i32) {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        log::warn!("No segments selected for keyframe operation");
        return;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();
    let merged_index = filter_index as usize;

    let Some((actual_filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        log::warn!("No filter found at merged index {}", merged_index);
        return;
    };

    if actual_filter_type != FilterType::Video
        && actual_filter_type != FilterType::Image
        && actual_filter_type != FilterType::Audio
    {
        return;
    }

    let time_ms_i64 = time_ms as i64;
    let keyframe_opt = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;

        with_segment_filter_and_then!(segment, actual_filter_type, local_index, |wrapper| {
            let tracks = wrapper.inner.get_keyframe_tracks();
            tracks
                .get_track(&property_name.to_string())
                .and_then(|prop_track| {
                    prop_track
                        .keyframes
                        .iter()
                        .find(|kf| kf.time_ms == time_ms_i64)
                        .cloned()
                })
        })
    });

    let Some(keyframe) = keyframe_opt else {
        crate::toast_warn!(ui, format!("{} {}ms", tr("Keyframe not found at"), time_ms));
        return;
    };

    let command = RemoveKeyframeCommand::new(
        *track_idx,
        *seg_idx,
        local_index,
        property_name.to_string(),
        keyframe,
        actual_filter_type,
    );

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            refresh_selected_filter_detail_at_playhead(ui);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
            global_ve_filter!(ui)
                .set_toggle_keyframe_flag(!global_ve_filter!(ui).get_toggle_keyframe_flag());
            crate::toast_success!(ui, tr("Removed keyframe"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove keyframe"), e)),
    }
}

fn remove_keyframe_at_index(ui: &AppWindow, filter_index: i32, keyframe_index: i32) {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        log::warn!("No segments selected for keyframe operation");
        return;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();
    let merged_index = filter_index as usize;

    let Some((actual_filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        log::warn!("No filter found at merged index {}", merged_index);
        return;
    };

    if actual_filter_type != FilterType::Video
        && actual_filter_type != FilterType::Image
        && actual_filter_type != FilterType::Audio
    {
        return;
    }

    // Get the filter's keyframe tracks and find the keyframe at the flattened index
    let Some((property_name, keyframe)) = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;

        with_segment_filter!(segment, actual_filter_type, local_index, |wrapper| {
            find_keyframe_at_flattened_index(
                &wrapper.inner.get_keyframe_tracks(),
                keyframe_index as usize,
            )
        })
    })
    .unwrap_or(None) else {
        crate::toast_warn!(
            ui,
            format!("{} {}", tr("Keyframe index not found"), keyframe_index)
        );
        return;
    };

    let command = RemoveKeyframeCommand::new(
        *track_idx,
        *seg_idx,
        local_index,
        property_name,
        keyframe,
        actual_filter_type,
    );

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            refresh_selected_filter_detail_at_playhead(ui);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
            global_ve_filter!(ui)
                .set_toggle_keyframe_flag(!global_ve_filter!(ui).get_toggle_keyframe_flag());
            crate::toast_success!(ui, tr("Removed keyframe"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove keyframe"), e)),
    }
}

fn find_keyframe_at_flattened_index(
    tracks: &KeyframeTracks,
    keyframe_index: usize,
) -> Option<(String, Keyframe)> {
    let mut cumulative = 0;
    for track in tracks.tracks.iter().filter(|t| t.has_keyframes()) {
        let track_len = track.keyframes.len();
        if keyframe_index < cumulative + track_len {
            let local_idx = keyframe_index - cumulative;
            let kf = &track.keyframes[local_idx];
            return Some((track.property_name.clone(), kf.clone()));
        }
        cumulative += track_len;
    }
    None
}

fn move_keyframe(
    ui: &AppWindow,
    filter_index: i32,
    property_name: SharedString,
    old_time_ms: i32,
    new_time_ms: i32,
) {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        log::warn!("No segments selected for keyframe operation");
        return;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();
    let merged_index = filter_index as usize;

    let Some((actual_filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        log::warn!("No filter found at merged index {}", merged_index);
        return;
    };

    if actual_filter_type != FilterType::Video
        && actual_filter_type != FilterType::Image
        && actual_filter_type != FilterType::Audio
    {
        return;
    }

    let command = MoveKeyframeCommand::new(
        *track_idx,
        *seg_idx,
        local_index,
        property_name.to_string(),
        old_time_ms as i64,
        new_time_ms as i64,
        actual_filter_type,
    );

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            refresh_selected_filter_detail_at_playhead(ui);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to move keyframe"), e)),
    }
}

fn move_keyframe_to_playhead(
    ui: &AppWindow,
    filter_index: i32,
    property_name: SharedString,
    old_time_ms: i32,
) {
    let playhead_time_ms = global_store!(ui).get_video_editor_timeline_offset();
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        log::warn!("No segments selected for keyframe operation");
        return;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();
    let (segment_timeline_offset_ms, segment_duration_ms) = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;
        Some((
            segment.timeline_offset.as_millis() as i32,
            segment.duration.as_millis() as i32,
        ))
    })
    .unwrap_or((0, 0));

    let new_time_ms = playhead_time_ms - segment_timeline_offset_ms;
    let clamped_new_time_ms = new_time_ms.clamp(0, segment_duration_ms);
    if clamped_new_time_ms == old_time_ms {
        log::info!("Keyframe already at playhead position, skipping move");
        return;
    }

    move_keyframe(
        ui,
        filter_index,
        property_name,
        old_time_ms,
        clamped_new_time_ms,
    );
}

fn move_keyframe_at_index(
    ui: &AppWindow,
    filter_index: i32,
    keyframe_index: i32,
    new_time_ms: i32,
) {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        log::warn!("No segments selected for keyframe operation");
        return;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();
    let merged_index = filter_index as usize;

    let Some((actual_filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        log::warn!("No filter found at merged index {}", merged_index);
        return;
    };

    if actual_filter_type != FilterType::Video
        && actual_filter_type != FilterType::Image
        && actual_filter_type != FilterType::Audio
    {
        return;
    }

    let Some((property_name, keyframe)) = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;

        with_segment_filter!(segment, actual_filter_type, local_index, |wrapper| {
            find_keyframe_at_flattened_index(
                &wrapper.inner.get_keyframe_tracks(),
                keyframe_index as usize,
            )
        })
    })
    .unwrap_or(None) else {
        crate::toast_warn!(
            ui,
            format!("{} {}", tr("Keyframe index not found"), keyframe_index)
        );
        return;
    };

    let old_time_ms = keyframe.time_ms;
    if old_time_ms == new_time_ms as i64 {
        return;
    }

    let command = MoveKeyframeCommand::new(
        *track_idx,
        *seg_idx,
        local_index,
        property_name,
        old_time_ms,
        new_time_ms as i64,
        actual_filter_type,
    );

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            refresh_selected_filter_detail_at_playhead(ui);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to move keyframe"), e)),
    }
}

fn update_keyframe_value(
    ui: &AppWindow,
    filter_index: i32,
    property_name: SharedString,
    time_ms: i32,
    value: UIKeyframeValue,
) {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        log::warn!("No segments selected for keyframe operation");
        return;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();
    let merged_index = filter_index as usize;

    let Some((actual_filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        log::warn!("No filter found at merged index {}", merged_index);
        return;
    };

    if actual_filter_type != FilterType::Video
        && actual_filter_type != FilterType::Image
        && actual_filter_type != FilterType::Audio
    {
        return;
    }

    let keyframe_value: KeyframeValue = value.into();

    let command = UpdateKeyframeValueCommand::new(
        *track_idx,
        *seg_idx,
        local_index,
        property_name.to_string(),
        time_ms as i64,
        keyframe_value,
        actual_filter_type,
    );

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            refresh_selected_filter_detail_at_playhead(ui);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
        }
        Err(e) => crate::toast_warn!(
            ui,
            format!("{}: {}", tr("Failed to update keyframe value"), e)
        ),
    }
}

fn get_opacity_filter_detail_at_time(f: &OpacityFilter, time_ms: i32) -> SharedString {
    let opacity = f
        .keyframe_tracks
        .get_track("opacity")
        .map(|t| get_float_at_time(t, time_ms as i64, f.opacity))
        .unwrap_or(f.opacity);

    let interpolated = OpacityFilter::new(opacity.clamp(0.0, 1.0));

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_mosaic_filter_detail_at_time(f: &MosaicFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let (left, top) = tracks
        .get_track("region")
        .map(|t| get_float2_at_time(t, time_ms as i64, f.left, f.top))
        .unwrap_or((f.left, f.top));

    let (width, height) = tracks
        .get_track("size")
        .map(|t| get_float2_at_time(t, time_ms as i64, f.width, f.height))
        .unwrap_or((f.width, f.height));

    let interpolated = MosaicFilter::new(
        left.clamp(0.0, 1.0),
        top.clamp(0.0, 1.0),
        width.clamp(0.0, 1.0),
        height.clamp(0.0, 1.0),
        f.block_size,
    );

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_chroma_key_filter_detail_at_time(f: &ChromaKeyFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let similarity = tracks
        .get_track("similarity")
        .map(|t| get_float_at_time(t, time_ms as i64, f.similarity))
        .unwrap_or(f.similarity);

    let softness = tracks
        .get_track("softness")
        .map(|t| get_float_at_time(t, time_ms as i64, f.softness))
        .unwrap_or(f.softness);

    let feather = tracks
        .get_track("feather")
        .map(|t| get_float_at_time(t, time_ms as i64, f.feather))
        .unwrap_or(f.feather);

    let spill_reduction = tracks
        .get_track("spill_reduction")
        .map(|t| get_float_at_time(t, time_ms as i64, f.spill_reduction))
        .unwrap_or(f.spill_reduction);

    let interpolated = ChromaKeyFilter::new(
        f.target_color,
        similarity.clamp(0.0, 1.0),
        softness.clamp(0.0, 1.0),
        feather.clamp(0.0, 1.0),
        spill_reduction.clamp(0.0, 1.0),
    );

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_crop_filter_detail_at_time(f: &CropFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let (left, top) = tracks
        .get_track("region")
        .map(|t| get_float2_at_time(t, time_ms as i64, f.left, f.top))
        .unwrap_or((f.left, f.top));

    let (width, height) = tracks
        .get_track("size")
        .map(|t| get_float2_at_time(t, time_ms as i64, f.width, f.height))
        .unwrap_or((f.width, f.height));

    let interpolated = CropFilter::new(
        left.clamp(0.0, 1.0),
        top.clamp(0.0, 1.0),
        width.clamp(0.0, 1.0),
        height.clamp(0.0, 1.0),
        f.shape,
    );

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_gain_filter_detail_at_time(f: &GainFilter, time_ms: i32) -> SharedString {
    let amplitude = f
        .keyframe_tracks
        .get_track("amplitude")
        .map(|t| get_float_at_time(t, time_ms as i64, f.amplitude))
        .unwrap_or(f.amplitude);

    let interpolated = GainFilter::from_db(20.0 * amplitude.log10());

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_compressor_filter_detail_at_time(f: &CompressorFilter, time_ms: i32) -> SharedString {
    let makeup_gain = f
        .keyframe_tracks
        .get_track("makeup_gain")
        .map(|t| get_float_at_time(t, time_ms as i64, f.makeup_gain))
        .unwrap_or(f.makeup_gain);

    let interpolated = CompressorFilter::new(
        f.threshold,
        f.ratio,
        f.attack,
        f.release,
        makeup_gain.clamp(0.0, 20.0),
    );

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_voice_changer_filter_detail_at_time(f: &VoiceChangerFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let pitch_semitones = tracks
        .get_track("pitch_semitones")
        .map(|t| get_float_at_time(t, time_ms as i64, f.pitch_semitones))
        .unwrap_or(f.pitch_semitones);

    let formant_semitones = tracks
        .get_track("formant_semitones")
        .map(|t| get_float_at_time(t, time_ms as i64, f.formant_semitones))
        .unwrap_or(f.formant_semitones);

    let interpolated = VoiceChangerFilter::default()
        .with_pitch_semitones(pitch_semitones.clamp(-12.0, 12.0))
        .with_formant_semitones(formant_semitones.clamp(-6.0, 6.0));

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_transform_filter_detail_at_time(f: &TransformFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let zoom = tracks
        .get_track("zoom_level")
        .map(|t| get_float_at_time(t, time_ms as i64, f.zoom_level))
        .unwrap_or(f.zoom_level);

    let (center_x, center_y) = tracks
        .get_track("center_percent")
        .map(|t| get_float2_at_time(t, time_ms as i64, f.center_x_percent, f.center_y_percent))
        .unwrap_or((f.center_x_percent, f.center_y_percent));

    let rotation = tracks
        .get_track("rotation")
        .map(|t| get_float_at_time(t, time_ms as i64, f.rotation.to_degrees()).to_radians())
        .unwrap_or(f.rotation);

    let interpolated = TransformFilter::new(zoom.clamp(0.01, 10.0), center_x, center_y, rotation);

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_draw_rectangle_filter_detail_at_time(f: &DrawRectangleFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    // Get interpolated position (x, y)
    let (x, y) = tracks
        .get_track("position")
        .map(|t| get_float2_at_time(t, time_ms as i64, f.x, f.y))
        .unwrap_or((f.x, f.y));

    // Get interpolated size (width, height)
    let (width, height) = tracks
        .get_track("size")
        .map(|t| get_float2_at_time(t, time_ms as i64, f.width, f.height))
        .unwrap_or((f.width, f.height));

    // Get interpolated corner_radius
    let corner_radius = tracks
        .get_track("corner_radius")
        .map(|t| get_float_at_time(t, time_ms as i64, f.corner_radius as f32) as u32)
        .unwrap_or(f.corner_radius);

    // Get interpolated border_width
    let border_width = tracks
        .get_track("border_width")
        .map(|t| get_float_at_time(t, time_ms as i64, f.border_width as f32) as u32)
        .unwrap_or(f.border_width);

    // Interpolate fill_color from keyframes
    let fill_color = tracks
        .get_track("fill_color")
        .filter(|t| t.has_keyframes())
        .map(|t| get_color_at_time(t, time_ms as i64, (0, 0, 0, 255)))
        .or(f.fill_color);

    // Interpolate border_color from keyframes
    let border_color = tracks
        .get_track("border_color")
        .filter(|t| t.has_keyframes())
        .map(|t| get_color_at_time(t, time_ms as i64, (255, 255, 255, 255)))
        .or(f.border_color);

    let interpolated = DrawRectangleFilter::new(x, y, width, height)
        .with_fill_color(fill_color)
        .with_border_color(border_color)
        .with_border_width(border_width)
        .with_corner_radius(corner_radius);

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_draw_circle_filter_detail_at_time(f: &DrawCircleFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let (center_x, center_y) = tracks
        .get_track("center")
        .map(|t| get_float2_at_time(t, time_ms as i64, f.center_x, f.center_y))
        .unwrap_or((f.center_x, f.center_y));

    let radius = tracks
        .get_track("radius")
        .map(|t| get_float_at_time(t, time_ms as i64, f.radius as f32) as u32)
        .unwrap_or(f.radius);

    let border_width = tracks
        .get_track("border_width")
        .map(|t| get_float_at_time(t, time_ms as i64, f.border_width as f32) as u32)
        .unwrap_or(f.border_width);

    // Interpolate fill_color from keyframes
    let fill_color = tracks
        .get_track("fill_color")
        .map(|t| get_color_at_time(t, time_ms as i64, (0, 0, 0, 255)))
        .or(f.fill_color);

    // Interpolate border_color from keyframes
    let border_color = tracks
        .get_track("border_color")
        .map(|t| get_color_at_time(t, time_ms as i64, (255, 255, 255, 255)))
        .or(f.border_color);

    let interpolated = DrawCircleFilter::new(center_x, center_y, radius)
        .with_fill_color(fill_color)
        .with_border_color(border_color)
        .with_border_width(border_width);

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_vignette_filter_detail_at_time(f: &VignetteFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let intensity = tracks
        .get_track("intensity")
        .map(|t| get_float_at_time(t, time_ms as i64, f.intensity))
        .unwrap_or(f.intensity);

    let inner_radius = tracks
        .get_track("inner_radius")
        .map(|t| get_float_at_time(t, time_ms as i64, f.inner_radius))
        .unwrap_or(f.inner_radius);

    let outer_radius = tracks
        .get_track("outer_radius")
        .map(|t| get_float_at_time(t, time_ms as i64, f.outer_radius))
        .unwrap_or(f.outer_radius);

    let center_x = tracks
        .get_track("center_x")
        .map(|t| get_float_at_time(t, time_ms as i64, f.center_x))
        .unwrap_or(f.center_x);

    let center_y = tracks
        .get_track("center_y")
        .map(|t| get_float_at_time(t, time_ms as i64, f.center_y))
        .unwrap_or(f.center_y);

    let interpolated = VignetteFilter {
        intensity: intensity.clamp(0.0, 1.0),
        inner_radius: inner_radius.clamp(0.0, 1.0),
        outer_radius: outer_radius.clamp(0.0, 1.0).max(inner_radius),
        center_x: center_x.clamp(0.0, 1.0),
        center_y: center_y.clamp(0.0, 1.0),
        aspect: f.aspect,
        keyframe_tracks: KeyframeTracks::default(),
    };

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_linear_mask_filter_detail_at_time(f: &LinearMaskFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let center_x = tracks
        .get_track("center_x")
        .map(|t| get_float_at_time(t, time_ms as i64, f.center_x))
        .unwrap_or(f.center_x);

    let center_y = tracks
        .get_track("center_y")
        .map(|t| get_float_at_time(t, time_ms as i64, f.center_y))
        .unwrap_or(f.center_y);

    let rotation = tracks
        .get_track("rotation")
        .map(|t| get_float_at_time(t, time_ms as i64, f.rotation))
        .unwrap_or(f.rotation);

    let feather = tracks
        .get_track("feather")
        .map(|t| get_float_at_time(t, time_ms as i64, f.feather))
        .unwrap_or(f.feather);

    let opacity = tracks
        .get_track("opacity")
        .map(|t| get_float_at_time(t, time_ms as i64, f.opacity))
        .unwrap_or(f.opacity);

    let interpolated = LinearMaskFilter::default()
        .with_center_x(center_x.clamp(0.0, 1.0))
        .with_center_y(center_y.clamp(0.0, 1.0))
        .with_rotation(rotation.clamp(0.0, 360.0))
        .with_feather(feather.clamp(0.0, 1.0))
        .with_opacity(opacity.clamp(0.0, 1.0))
        .with_flip(f.flip);

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_circle_mask_filter_detail_at_time(f: &CircleMaskFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let center_x = tracks
        .get_track("center_x")
        .map(|t| get_float_at_time(t, time_ms as i64, f.center_x))
        .unwrap_or(f.center_x);

    let center_y = tracks
        .get_track("center_y")
        .map(|t| get_float_at_time(t, time_ms as i64, f.center_y))
        .unwrap_or(f.center_y);

    let feather = tracks
        .get_track("feather")
        .map(|t| get_float_at_time(t, time_ms as i64, f.feather))
        .unwrap_or(f.feather);

    let opacity = tracks
        .get_track("opacity")
        .map(|t| get_float_at_time(t, time_ms as i64, f.opacity))
        .unwrap_or(f.opacity);

    let radius = tracks
        .get_track("radius")
        .map(|t| get_float_at_time(t, time_ms as i64, f.radius as f32))
        .unwrap_or(f.radius as f32);

    let interpolated = CircleMaskFilter::default()
        .with_center_x(center_x.clamp(0.0, 1.0))
        .with_center_y(center_y.clamp(0.0, 1.0))
        .with_feather(feather.clamp(0.0, 1.0))
        .with_opacity(opacity.clamp(0.0, 1.0))
        .with_radius(radius.clamp(0.0, 5000.0) as u32)
        .with_flip(f.flip);

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_mirror_mask_filter_detail_at_time(f: &MirrorMaskFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let center_x = tracks
        .get_track("center_x")
        .map(|t| get_float_at_time(t, time_ms as i64, f.center_x))
        .unwrap_or(f.center_x);

    let center_y = tracks
        .get_track("center_y")
        .map(|t| get_float_at_time(t, time_ms as i64, f.center_y))
        .unwrap_or(f.center_y);

    let rotation = tracks
        .get_track("rotation")
        .map(|t| get_float_at_time(t, time_ms as i64, f.rotation))
        .unwrap_or(f.rotation);

    let feather = tracks
        .get_track("feather")
        .map(|t| get_float_at_time(t, time_ms as i64, f.feather))
        .unwrap_or(f.feather);

    let opacity = tracks
        .get_track("opacity")
        .map(|t| get_float_at_time(t, time_ms as i64, f.opacity))
        .unwrap_or(f.opacity);

    let width = tracks
        .get_track("width")
        .map(|t| get_float_at_time(t, time_ms as i64, f.width))
        .unwrap_or(f.width);

    let interpolated = MirrorMaskFilter::default()
        .with_center_x(center_x.clamp(0.0, 1.0))
        .with_center_y(center_y.clamp(0.0, 1.0))
        .with_rotation(rotation.clamp(0.0, 360.0))
        .with_feather(feather.clamp(0.0, 1.0))
        .with_opacity(opacity.clamp(0.0, 1.0))
        .with_width(width.clamp(0.0, 1.0))
        .with_flip(f.flip);

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_rectangle_mask_filter_detail_at_time(f: &RectangleMaskFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let center_x = tracks
        .get_track("center_x")
        .map(|t| get_float_at_time(t, time_ms as i64, f.center_x))
        .unwrap_or(f.center_x);

    let center_y = tracks
        .get_track("center_y")
        .map(|t| get_float_at_time(t, time_ms as i64, f.center_y))
        .unwrap_or(f.center_y);

    let rotation = tracks
        .get_track("rotation")
        .map(|t| get_float_at_time(t, time_ms as i64, f.rotation))
        .unwrap_or(f.rotation);

    let feather = tracks
        .get_track("feather")
        .map(|t| get_float_at_time(t, time_ms as i64, f.feather))
        .unwrap_or(f.feather);

    let opacity = tracks
        .get_track("opacity")
        .map(|t| get_float_at_time(t, time_ms as i64, f.opacity))
        .unwrap_or(f.opacity);

    let width = tracks
        .get_track("width")
        .map(|t| get_float_at_time(t, time_ms as i64, f.width))
        .unwrap_or(f.width);

    let height = tracks
        .get_track("height")
        .map(|t| get_float_at_time(t, time_ms as i64, f.height))
        .unwrap_or(f.height);

    let interpolated = RectangleMaskFilter::default()
        .with_center_x(center_x.clamp(0.0, 1.0))
        .with_center_y(center_y.clamp(0.0, 1.0))
        .with_rotation(rotation.clamp(0.0, 360.0))
        .with_feather(feather.clamp(0.0, 1.0))
        .with_opacity(opacity.clamp(0.0, 1.0))
        .with_width(width.clamp(0.0, 1.0))
        .with_height(height.clamp(0.0, 1.0))
        .with_flip(f.flip);

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_local_magnify_filter_detail_at_time(f: &LocalMagnifyFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let (center_x, center_y) = tracks
        .get_track("center")
        .map(|t| get_float2_at_time(t, time_ms as i64, f.center_x, f.center_y))
        .unwrap_or((f.center_x, f.center_y));

    let selection_radius = tracks
        .get_track("selection_radius")
        .map(|t| get_float_at_time(t, time_ms as i64, f.selection_radius as f32) as u32)
        .unwrap_or(f.selection_radius);

    let scale = tracks
        .get_track("scale")
        .map(|t| get_float_at_time(t, time_ms as i64, f.scale))
        .unwrap_or(f.scale);

    let border_width = tracks
        .get_track("border_width")
        .map(|t| get_float_at_time(t, time_ms as i64, f.border_width as f32) as u32)
        .unwrap_or(f.border_width);

    let border_color = tracks
        .get_track("border_color")
        .filter(|t| t.has_keyframes())
        .map(|t| get_color_at_time(t, time_ms as i64, (255, 255, 255, 255)))
        .or(f.border_color);

    let interpolated = LocalMagnifyFilter::new(center_x, center_y, selection_radius, scale)
        .with_border_color(border_color)
        .with_border_width(border_width);

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_magnifier_filter_detail_at_time(f: &MagnifierFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let (center_x, center_y) = tracks
        .get_track("center")
        .map(|t| get_float2_at_time(t, time_ms as i64, f.center_x, f.center_y))
        .unwrap_or((f.center_x, f.center_y));

    let radius = tracks
        .get_track("radius")
        .map(|t| get_float_at_time(t, time_ms as i64, f.radius as f32) as u32)
        .unwrap_or(f.radius);

    let scale = tracks
        .get_track("scale")
        .map(|t| get_float_at_time(t, time_ms as i64, f.scale))
        .unwrap_or(f.scale);

    let border_width = tracks
        .get_track("border_width")
        .map(|t| get_float_at_time(t, time_ms as i64, f.border_width as f32) as u32)
        .unwrap_or(f.border_width);

    let border_color = tracks
        .get_track("border_color")
        .filter(|t| t.has_keyframes())
        .map(|t| get_color_at_time(t, time_ms as i64, (255, 255, 255, 255)))
        .or(f.border_color);

    let interpolated = MagnifierFilter::new(center_x, center_y, radius, scale)
        .with_border_color(border_color)
        .with_border_width(border_width);

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_gaussian_blur_filter_detail_at_time(f: &GaussianBlurFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let radius = tracks
        .get_track("radius")
        .map(|t| get_float_at_time(t, time_ms as i64, f.radius))
        .unwrap_or(f.radius);

    let sigma = tracks
        .get_track("sigma")
        .map(|t| get_float_at_time(t, time_ms as i64, f.sigma))
        .unwrap_or(f.sigma);

    let (left, top) = tracks
        .get_track("region")
        .map(|track| get_float2_at_time(track, time_ms as i64, f.left, f.top))
        .unwrap_or((f.left, f.top));

    let (width, height) = tracks
        .get_track("size")
        .map(|track| get_float2_at_time(track, time_ms as i64, f.width, f.height))
        .unwrap_or((f.width, f.height));

    let interpolated = GaussianBlurFilter::new(radius.clamp(0.0, 50.0))
        .with_sigma(sigma.clamp(0.1, 20.0))
        .with_left(left.clamp(0.0, 1.0))
        .with_top(top.clamp(0.0, 1.0))
        .with_width(width.clamp(0.0, 1.0))
        .with_height(height.clamp(0.0, 1.0))
        .with_keyframe_tracks(KeyframeTracks::default());

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_directional_blur_filter_detail_at_time(
    f: &DirectionalBlurFilter,
    time_ms: i32,
) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let angle = tracks
        .get_track("angle")
        .map(|t| get_float_at_time(t, time_ms as i64, f.angle))
        .unwrap_or(f.angle);

    let length = tracks
        .get_track("length")
        .map(|t| get_float_at_time(t, time_ms as i64, f.length))
        .unwrap_or(f.length);

    let spread = tracks
        .get_track("spread")
        .map(|t| get_float_at_time(t, time_ms as i64, f.spread))
        .unwrap_or(f.spread);

    let interpolated = DirectionalBlurFilter::new(angle % 360.0, length.clamp(0.0, 100.0))
        .with_spread(spread.clamp(0.0, 1.0))
        .with_keyframe_tracks(KeyframeTracks::default());

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_sharpen_filter_detail_at_time(f: &SharpenFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let strength = tracks
        .get_track("strength")
        .map(|t| get_float_at_time(t, time_ms as i64, f.strength))
        .unwrap_or(f.strength);

    let radius = tracks
        .get_track("radius")
        .map(|t| get_float_at_time(t, time_ms as i64, f.radius))
        .unwrap_or(f.radius);

    let interpolated = SharpenFilter::new(strength.clamp(0.0, 5.0))
        .with_radius(radius.clamp(0.0, 10.0))
        .with_keyframe_tracks(KeyframeTracks::default());

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_edge_detect_filter_detail_at_time(f: &EdgeDetectFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let threshold = tracks
        .get_track("threshold")
        .map(|t| get_float_at_time(t, time_ms as i64, f.threshold))
        .unwrap_or(f.threshold);

    let strength = tracks
        .get_track("strength")
        .map(|t| get_float_at_time(t, time_ms as i64, f.strength))
        .unwrap_or(f.strength);

    let interpolated = EdgeDetectFilter::new(threshold.clamp(0.0, 255.0), strength.clamp(0.0, 2.0))
        .with_invert(f.invert)
        .with_edge_color(f.edge_color)
        .with_background_color(f.background_color)
        .with_keyframe_tracks(KeyframeTracks::default());

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_grain_filter_detail_at_time(f: &GrainFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let intensity = tracks
        .get_track("intensity")
        .map(|t| get_float_at_time(t, time_ms as i64, f.intensity))
        .unwrap_or(f.intensity);

    let grain_size = tracks
        .get_track("grain_size")
        .map(|t| get_float_at_time(t, time_ms as i64, f.grain_size))
        .unwrap_or(f.grain_size);

    let roughness = tracks
        .get_track("roughness")
        .map(|t| get_float_at_time(t, time_ms as i64, f.roughness))
        .unwrap_or(f.roughness);

    let interpolated = GrainFilter::new(intensity.clamp(0.0, 1.0))
        .with_grain_size(grain_size.clamp(1.0, 10.0))
        .with_colored(f.colored)
        .with_roughness(roughness.clamp(0.0, 1.0))
        .with_seed(f.seed)
        .with_keyframe_tracks(KeyframeTracks::default());

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_fisheye_filter_detail_at_time(f: &FisheyeFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;
    let time_ms_i64 = time_ms as i64;

    let center_x = tracks
        .get_track("center_x")
        .map(|t| get_float_at_time(t, time_ms_i64, f.center_x))
        .unwrap_or(f.center_x);

    let center_y = tracks
        .get_track("center_y")
        .map(|t| get_float_at_time(t, time_ms_i64, f.center_y))
        .unwrap_or(f.center_y);

    let strength = tracks
        .get_track("strength")
        .map(|t| get_float_at_time(t, time_ms_i64, f.strength))
        .unwrap_or(f.strength);

    let radius = tracks
        .get_track("radius")
        .map(|t| get_float_at_time(t, time_ms_i64, f.radius as f32) as u32)
        .unwrap_or(f.radius);

    let interpolated = FisheyeFilter::new(
        center_x.clamp(0.0, 1.0),
        center_y.clamp(0.0, 1.0),
        strength.clamp(-1.0, 2.0),
        radius,
    );

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_focus_filter_detail_at_time(f: &FocusFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;
    let time_ms_i64 = time_ms as i64;

    let center_x = tracks
        .get_track("center_x")
        .map(|t| get_float_at_time(t, time_ms_i64, f.center_x))
        .unwrap_or(f.center_x);

    let center_y = tracks
        .get_track("center_y")
        .map(|t| get_float_at_time(t, time_ms_i64, f.center_y))
        .unwrap_or(f.center_y);

    let focus_radius = tracks
        .get_track("focus_radius")
        .map(|t| get_float_at_time(t, time_ms_i64, f.focus_radius as f32) as u32)
        .unwrap_or(f.focus_radius);

    let feather = tracks
        .get_track("feather")
        .map(|t| get_float_at_time(t, time_ms_i64, f.feather as f32) as u32)
        .unwrap_or(f.feather);

    let blur_radius = tracks
        .get_track("blur_radius")
        .map(|t| get_float_at_time(t, time_ms_i64, f.blur_radius as f32) as u32)
        .unwrap_or(f.blur_radius);

    let aperture_blades = tracks
        .get_track("aperture_blades")
        .map(|t| {
            get_float_at_time(t, time_ms_i64, f.aperture_blades as f32).clamp(3.0, 12.0) as u32
        })
        .unwrap_or(f.aperture_blades);

    let highlight_boost = tracks
        .get_track("highlight_boost")
        .map(|t| get_float_at_time(t, time_ms_i64, f.highlight_boost))
        .unwrap_or(f.highlight_boost);

    let interpolated = FocusFilter::new(
        center_x.clamp(0.0, 1.0),
        center_y.clamp(0.0, 1.0),
        focus_radius,
        blur_radius,
    )
    .with_feather(feather)
    .with_aperture_blades(aperture_blades)
    .with_highlight_boost(highlight_boost.clamp(0.0, 2.0));

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_grayscale_filter_detail_at_time(f: &GrayscaleFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;
    let time_ms_i64 = time_ms as i64;

    let intensity = tracks
        .get_track("intensity")
        .map(|t| get_float_at_time(t, time_ms_i64, f.intensity))
        .unwrap_or(f.intensity);

    let contrast = tracks
        .get_track("contrast")
        .map(|t| get_float_at_time(t, time_ms_i64, f.contrast))
        .unwrap_or(f.contrast);

    let interpolated = GrayscaleFilter::new(intensity.clamp(0.0, 1.0))
        .with_contrast(contrast.clamp(-1.0, 1.0))
        .with_luminance_standard(f.luminance_standard)
        .with_keyframe_tracks(KeyframeTracks::default());

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_old_film_filter_detail_at_time(f: &OldFilmFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;
    let time_ms_i64 = time_ms as i64;

    let scratch_intensity = tracks
        .get_track("scratch_intensity")
        .map(|t| get_float_at_time(t, time_ms_i64, f.scratch_intensity))
        .unwrap_or(f.scratch_intensity);

    let dust_intensity = tracks
        .get_track("dust_intensity")
        .map(|t| get_float_at_time(t, time_ms_i64, f.dust_intensity))
        .unwrap_or(f.dust_intensity);

    let flicker_intensity = tracks
        .get_track("flicker_intensity")
        .map(|t| get_float_at_time(t, time_ms_i64, f.flicker_intensity))
        .unwrap_or(f.flicker_intensity);

    let flicker_speed = tracks
        .get_track("flicker_speed")
        .map(|t| get_float_at_time(t, time_ms_i64, f.flicker_speed))
        .unwrap_or(f.flicker_speed);

    let vertical_lines_intensity = tracks
        .get_track("vertical_lines_intensity")
        .map(|t| get_float_at_time(t, time_ms_i64, f.vertical_lines_intensity))
        .unwrap_or(f.vertical_lines_intensity);

    let jitter_intensity = tracks
        .get_track("jitter_intensity")
        .map(|t| get_float_at_time(t, time_ms_i64, f.jitter_intensity))
        .unwrap_or(f.jitter_intensity);

    let sepia_intensity = tracks
        .get_track("sepia_intensity")
        .map(|t| get_float_at_time(t, time_ms_i64, f.sepia_intensity))
        .unwrap_or(f.sepia_intensity);

    let interpolated = OldFilmFilter::default()
        .with_seed(f.seed)
        .with_scratch_intensity(scratch_intensity.clamp(0.0, 1.0))
        .with_scratch_count(f.scratch_count)
        .with_scratch_width(f.scratch_width)
        .with_dust_intensity(dust_intensity.clamp(0.0, 1.0))
        .with_dust_count(f.dust_count)
        .with_dust_size_max(f.dust_size_max)
        .with_flicker_intensity(flicker_intensity.clamp(0.0, 0.3))
        .with_flicker_speed(flicker_speed.clamp(1.0, 10.0))
        .with_vertical_lines_intensity(vertical_lines_intensity.clamp(0.0, 1.0))
        .with_vertical_lines_count(f.vertical_lines_count)
        .with_jitter_intensity(jitter_intensity.clamp(0.0, 10.0))
        .with_sepia_intensity(sepia_intensity.clamp(0.0, 1.0))
        .with_keyframe_tracks(KeyframeTracks::default());

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_hsl_adjust_filter_detail_at_time(f: &HSLAdjustFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;

    let hue_shift = tracks
        .get_track("hue_shift")
        .map(|t| get_float_at_time(t, time_ms as i64, f.hue_shift))
        .unwrap_or(f.hue_shift);

    let saturation = tracks
        .get_track("saturation")
        .map(|t| get_float_at_time(t, time_ms as i64, f.saturation))
        .unwrap_or(f.saturation);

    let lightness = tracks
        .get_track("lightness")
        .map(|t| get_float_at_time(t, time_ms as i64, f.lightness))
        .unwrap_or(f.lightness);

    let interpolated = HSLAdjustFilter::new(
        hue_shift.clamp(-180.0, 180.0),
        saturation.clamp(-1.0, 1.0),
        lightness.clamp(-1.0, 1.0),
    )
    .with_preserve_luminance_option(f.preserve_luminance, f.luminance_standard)
    .with_keyframe_tracks(KeyframeTracks::default());

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_sketch_filter_detail_at_time(f: &SketchFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;
    let time_ms_i64 = time_ms as i64;

    let line_intensity = tracks
        .get_track("line_intensity")
        .map(|t| get_float_at_time(t, time_ms_i64, f.line_intensity))
        .unwrap_or(f.line_intensity);

    let line_width = tracks
        .get_track("line_width")
        .map(|t| get_float_at_time(t, time_ms_i64, f.line_width))
        .unwrap_or(f.line_width);

    let detail_level = tracks
        .get_track("detail_level")
        .map(|t| get_float_at_time(t, time_ms_i64, f.detail_level))
        .unwrap_or(f.detail_level);

    let interpolated =
        SketchFilter::new(line_intensity.clamp(0.0, 1.0), line_width.clamp(1.0, 10.0))
            .with_paper_color(f.paper_color)
            .with_pencil_color(f.pencil_color)
            .with_detail_level(detail_level.clamp(0.0, 1.0))
            .with_keyframe_tracks(KeyframeTracks::default());

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_wave_filter_detail_at_time(f: &WaveFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;
    let time_ms_i64 = time_ms as i64;

    let amplitude = tracks
        .get_track("amplitude")
        .map(|t| get_float_at_time(t, time_ms_i64, f.amplitude))
        .unwrap_or(f.amplitude);

    let frequency = tracks
        .get_track("frequency")
        .map(|t| get_float_at_time(t, time_ms_i64, f.frequency))
        .unwrap_or(f.frequency);

    let speed = tracks
        .get_track("speed")
        .map(|t| get_float_at_time(t, time_ms_i64, f.speed))
        .unwrap_or(f.speed);

    let center_x = tracks
        .get_track("center_x")
        .map(|t| get_float_at_time(t, time_ms_i64, f.center_x))
        .unwrap_or(f.center_x);

    let center_y = tracks
        .get_track("center_y")
        .map(|t| get_float_at_time(t, time_ms_i64, f.center_y))
        .unwrap_or(f.center_y);

    let interpolated = WaveFilter::new(
        amplitude.clamp(0.0, 100.0),
        frequency.clamp(0.1, 10.0),
        f.wave_type,
    )
    .with_speed(speed.clamp(0.0, 10.0))
    .with_phase(f.phase)
    .with_center_x(center_x.clamp(0.0, 1.0))
    .with_center_y(center_y.clamp(0.0, 1.0))
    .with_keyframe_tracks(KeyframeTracks::default());

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_border_filter_detail_at_time(f: &BorderFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;
    let time_ms_i64 = time_ms as i64;

    let size = tracks
        .get_track("size")
        .map(|t| get_float_at_time(t, time_ms_i64, f.size as f32) as u32)
        .unwrap_or(f.size);

    let corner_radius = tracks
        .get_track("corner_radius")
        .map(|t| get_float_at_time(t, time_ms_i64, f.corner_radius as f32) as u32)
        .unwrap_or(f.corner_radius);

    let color: [u8; 4] = tracks
        .get_track("color")
        .filter(|t| t.has_keyframes())
        .map(|t| {
            let (r, g, b, a) = get_color_at_time(t, time_ms_i64, (0, 0, 0, 255));
            [r, g, b, a]
        })
        .unwrap_or(f.color);

    let interpolated = BorderFilter::new(size, color, corner_radius);

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_shadow_filter_detail_at_time(f: &ShadowFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;
    let time_ms_i64 = time_ms as i64;

    let color: [u8; 4] = tracks
        .get_track("color")
        .filter(|t| t.has_keyframes())
        .map(|t| {
            let (r, g, b, a) = get_color_at_time(t, time_ms_i64, (0, 0, 0, 255));
            [r, g, b, a]
        })
        .unwrap_or(f.color);

    let opacity = tracks
        .get_track("opacity")
        .map(|t| get_float_at_time(t, time_ms_i64, f.opacity))
        .unwrap_or(f.opacity);

    let size = tracks
        .get_track("size")
        .map(|t| get_float_at_time(t, time_ms_i64, f.size))
        .unwrap_or(f.size);

    let blur = tracks
        .get_track("blur")
        .map(|t| get_float_at_time(t, time_ms_i64, f.blur))
        .unwrap_or(f.blur);

    let angle = tracks
        .get_track("angle")
        .map(|t| get_float_at_time(t, time_ms_i64, f.angle))
        .unwrap_or(f.angle);

    let distance = tracks
        .get_track("distance")
        .map(|t| get_float_at_time(t, time_ms_i64, f.distance))
        .unwrap_or(f.distance);

    let interpolated = ShadowFilter::new(
        color,
        opacity.clamp(0.0, 1.0),
        blur.clamp(0.0, 100.0),
        angle % 360.0,
        distance.clamp(0.0, 200.0),
    )
    .with_size(size.clamp(0.0, 1.0));

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}
fn get_grid_filter_detail_at_time(f: &GridFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;
    let time_ms_i64 = time_ms as i64;

    let line_size = tracks
        .get_track("line_size")
        .map(|t| get_float_at_time(t, time_ms_i64, f.line_size as f32) as u32)
        .unwrap_or(f.line_size);

    let line_color: [u8; 4] = tracks
        .get_track("line_color")
        .filter(|t| t.has_keyframes())
        .map(|t| {
            let (r, g, b, a) = get_color_at_time(t, time_ms_i64, (255, 255, 255, 255));
            [r, g, b, a]
        })
        .unwrap_or(f.line_color);

    let interpolated = GridFilter {
        rows: f.rows,
        columns: f.columns,
        line_color,
        line_size,
        keyframe_tracks: KeyframeTracks::default(),
    };

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}

fn get_filter_detail_at_time(ui: &AppWindow, filter_index: i32, time_ms: i32) -> SharedString {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        return SharedString::new();
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();
    let merged_index = filter_index as usize;

    let Some((actual_filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        return SharedString::new();
    };

    if actual_filter_type != FilterType::Video
        && actual_filter_type != FilterType::Image
        && actual_filter_type != FilterType::Audio
    {
        return SharedString::new();
    }

    with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;

        match actual_filter_type {
            FilterType::Video | FilterType::Image => {
                let filters = if actual_filter_type == FilterType::Video {
                    &segment.video_filters
                } else {
                    &segment.image_filters
                };

                filters.get(local_index).map(|wrapper| {
                    let name = wrapper.inner.name().to_lowercase();
                    filter_detail_at_time_match!(
                        name.as_str(),
                        wrapper,
                        time_ms,
                        video_filter_to_json_detail(&wrapper.inner).into(),
                        TransformFilter,
                        get_transform_filter_detail_at_time,
                        OpacityFilter,
                        get_opacity_filter_detail_at_time,
                        MosaicFilter,
                        get_mosaic_filter_detail_at_time,
                        ChromaKeyFilter,
                        get_chroma_key_filter_detail_at_time,
                        CropFilter,
                        get_crop_filter_detail_at_time,
                        HSLAdjustFilter,
                        get_hsl_adjust_filter_detail_at_time,
                        DrawCircleFilter,
                        get_draw_circle_filter_detail_at_time,
                        DrawRectangleFilter,
                        get_draw_rectangle_filter_detail_at_time,
                        VignetteFilter,
                        get_vignette_filter_detail_at_time,
                        LinearMaskFilter,
                        get_linear_mask_filter_detail_at_time,
                        CircleMaskFilter,
                        get_circle_mask_filter_detail_at_time,
                        MirrorMaskFilter,
                        get_mirror_mask_filter_detail_at_time,
                        RectangleMaskFilter,
                        get_rectangle_mask_filter_detail_at_time,
                        LocalMagnifyFilter,
                        get_local_magnify_filter_detail_at_time,
                        MagnifierFilter,
                        get_magnifier_filter_detail_at_time,
                        GaussianBlurFilter,
                        get_gaussian_blur_filter_detail_at_time,
                        DirectionalBlurFilter,
                        get_directional_blur_filter_detail_at_time,
                        SharpenFilter,
                        get_sharpen_filter_detail_at_time,
                        EdgeDetectFilter,
                        get_edge_detect_filter_detail_at_time,
                        GrainFilter,
                        get_grain_filter_detail_at_time,
                        GridFilter,
                        get_grid_filter_detail_at_time,
                        GrayscaleFilter,
                        get_grayscale_filter_detail_at_time,
                        FisheyeFilter,
                        get_fisheye_filter_detail_at_time,
                        FocusFilter,
                        get_focus_filter_detail_at_time,
                        OldFilmFilter,
                        get_old_film_filter_detail_at_time,
                        SketchFilter,
                        get_sketch_filter_detail_at_time,
                        WaveFilter,
                        get_wave_filter_detail_at_time,
                        BorderFilter,
                        get_border_filter_detail_at_time,
                        ShadowFilter,
                        get_shadow_filter_detail_at_time,
                        Live2dFilter,
                        get_live_2d_filter_detail_at_time
                    )
                })
            }
            FilterType::Audio => segment.audio_filters.get(local_index).map(|wrapper| {
                let name = wrapper.inner.name().to_lowercase();
                audio_filter_detail_at_time_match!(
                    name.as_str(),
                    wrapper,
                    time_ms,
                    GainFilter,
                    get_gain_filter_detail_at_time,
                    CompressorFilter,
                    get_compressor_filter_detail_at_time,
                    VoiceChangerFilter,
                    get_voice_changer_filter_detail_at_time
                )
            }),
            _ => Some(SharedString::new()),
        }
    })
    .unwrap_or_else(|| SharedString::new())
}

pub fn check_filter_has_keyframes(ui: &AppWindow, filter_index: i32) -> bool {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        return false;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();
    let merged_index = filter_index as usize;

    let Some((actual_filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        return false;
    };

    if actual_filter_type != FilterType::Video
        && actual_filter_type != FilterType::Image
        && actual_filter_type != FilterType::Audio
    {
        return false;
    }

    with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;

        match actual_filter_type {
            FilterType::Video => segment
                .video_filters
                .get(local_index)
                .map(|wrapper| wrapper.inner.get_keyframe_tracks().has_keyframes()),
            FilterType::Image => segment
                .image_filters
                .get(local_index)
                .map(|wrapper| wrapper.inner.get_keyframe_tracks().has_keyframes()),
            FilterType::Audio => segment
                .audio_filters
                .get(local_index)
                .map(|wrapper| wrapper.inner.get_keyframe_tracks().has_keyframes()),
            _ => Some(false),
        }
    })
    .unwrap_or(false)
}

pub fn refresh_selected_filter_detail_at_playhead(ui: &AppWindow) {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        return;
    }

    let selected_filter_index = global_ve_filter!(ui).get_selected_filter_index();
    if selected_filter_index < 0 {
        return;
    }

    if !check_filter_has_keyframes(ui, selected_filter_index) {
        return;
    }

    let (track_idx, seg_idx) = selected_segments.last().unwrap();
    let segment_timeline_offset_ms: i32 = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;
        Some(segment.timeline_offset.as_millis() as i32)
    })
    .unwrap_or(0);

    let playhead_time_ms = global_store!(ui).get_video_editor_timeline_offset();
    let relative_time_ms = playhead_time_ms - segment_timeline_offset_ms;
    let interpolated_detail =
        get_filter_detail_at_time(ui, selected_filter_index, relative_time_ms);

    if !interpolated_detail.is_empty() {
        let mut selected_filter = global_ve_filter!(ui).get_selected_filter();
        selected_filter.detail = interpolated_detail;
        global_ve_filter!(ui).set_selected_filter(selected_filter);
        global_ve_filter!(ui)
            .set_toggle_keyframe_flag(!global_ve_filter!(ui).get_toggle_keyframe_flag());
    }
}

fn get_live_2d_filter_detail_at_time(f: &Live2dFilter, time_ms: i32) -> SharedString {
    let tracks = &f.keyframe_tracks;
    let time_ms_i64 = time_ms as i64;

    let model_view_fill = tracks
        .get_track("model_view_fill")
        .map(|t| get_float_at_time(t, time_ms_i64, f.model_view_fill))
        .unwrap_or(f.model_view_fill);

    let interpolated = Live2dFilter {
        model_dir: f.model_dir.clone(),
        motion_index: f.motion_index,
        expression_index: f.expression_index,
        model_view_fill,
        background: f.background,
        keyframe_tracks: KeyframeTracks::default(),
    };

    serde_json::to_string(&interpolated)
        .unwrap_or_default()
        .into()
}
