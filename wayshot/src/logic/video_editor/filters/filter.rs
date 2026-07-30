use crate::{
    db::VIDEO_EDITOR_TABLE,
    global_store, global_ve_filter,
    logic::{
        toast,
        tr::tr,
        video_editor::{
            command::{
                refresh_preview, sync_and_refresh, sync_manager_to_ui, with_history_manager,
            },
            common_type::{
                MarkedFiltersConfig, PresetFilter, PresetFiltersConfig, PresetSubtitleStyleConfig,
                SegmentFilterData,
            },
            conversion::{
                audio_filter_to_json_detail, image_filter_to_json_detail,
                subtitle_filter_to_json_detail, track_to_filter_type, video_filter_to_json_detail,
            },
            project::{MARKED_FILTERS_ID, PRESET_FILTERS_ID, PRESET_SUBTITLE_STYLES_ID},
            segment::{refresh_affected_segments, segment_contains_filter},
            track::get_selected_segment_indices,
        },
    },
    slint_generatedAppWindow::{
        AppWindow, FilterEntry as UIFilterEntry, FilterType as UIFilterType,
        PresetFilter as UIPresetFilter, SegmentFilter as UISegmentFilter,
    },
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::{collections::HashSet, sync::atomic::Ordering, time::Duration};
use video_editor::{
    commands::{
        BatchCommand, Command,
        filter::{
            AddFilterCommand, FilterType, MoveFilterCommand, RemoveFilterCommand,
            ToggleFilterCommand,
        },
        segment::SetPlaybackSpeedCommand,
    },
    filters::{
        audio::{
            AudioSpeedFilter, CompressorFilter, CopyChannelFilter, DenoiseFilter,
            FadeInFilter as AudioFadeInFilter, FadeOutFilter as AudioFadeOutFilter, GainFilter,
            LimiterFilter, MuteFilter, NoiseGateFilter, NormalizeFilter, VoiceChangerFilter,
            all_filter_names as all_audio_filter_names,
        },
        subtitle::{
            all_filter_names as all_subtitle_filter_names,
            style::{
                BackgroundColorFilter, BorderRadiusFilter, FontPathFilter, FontSizeFilter,
                MarginHorizontalFilter, MarginVerticalFilter, OutlineColorFilter,
                OutlineWidthFilter, PaddingFilter, PrimaryColorFilter,
            },
        },
        traits::{AudioFilter, ImageFilterWrapper, SubtitleFilter, VideoFilter},
        video::{
            BackgroundFilter, BorderFilter, BreathingFilter, ChromaKeyFilter, CircleMaskFilter,
            CropFilter, DeviceFrameFilter, DirectionalBlurFilter, DrawCircleFilter,
            DrawRectangleFilter, EdgeDetectFilter, FadeInFilter as ImageFadeInFilter,
            FadeOutFilter as ImageFadeOutFilter, FisheyeFilter, FlipFilter, FlyInFilter,
            FocusFilter, FrameExtractFilter, GaussianBlurFilter, GenieFilter, GrainFilter,
            GrayscaleFilter, GridFilter, HSLAdjustFilter, LightingFilter, LinearMaskFilter,
            Live2dFilter, LocalMagnifyFilter, MagnifierFilter, MirrorMaskFilter, MosaicFilter,
            OldFilmFilter, OpacityFilter, PageFlipFilter, RectangleMaskFilter, ShadowFilter,
            SharpenFilter, SketchFilter, SlideFilter, SpeedFilter, SplitFilter,
            TextHighlightFilter, TransformFilter, VignetteFilter, WaveFilter, WipeFilter,
            ZoomFilter, all_filter_names as all_image_filter_names,
            all_filter_names as all_video_filter_names,
        },
    },
    tracks::Track,
};

struct SpeedFilterInfo {
    track_idx: usize,
    seg_idx: usize,
    playback_speed: f32,
    duration: Duration,
    filter_enabled: bool,
    filter_speed: f32,
}

#[macro_export]
macro_rules! from_filter_json {
    ($func_name:ident, $filter_type:ty, $ui_type:ty) => {
        fn $func_name(_ui: &AppWindow, json: SharedString) -> $ui_type {
            match serde_json::from_str::<$filter_type>(json.as_str()) {
                Ok(filter) => filter.into(),
                Err(_) => <$filter_type>::default().into(),
            }
        }
    };
}

#[macro_export]
macro_rules! ve_filter_cb {
    ($callback_name:ident, $ui:expr, $($arg:ident),*) => {
        {{
            let ui_weak = $ui.as_weak();
            paste::paste! {
                crate::global_ve_filter!($ui)
                    .[<on_ $callback_name>](move |$($arg),*| {
                        $callback_name(&ui_weak.unwrap(), $($arg),*)
                    });
            }
        }}
    };
    ($callback_name:ident, $ui:expr) => {
        {{
            let ui_weak = $ui.as_weak();
            paste::paste! {
                crate::global_ve_filter!($ui)
                    .[<on_ $callback_name>](move || {
                        $callback_name(&ui_weak.unwrap())
                    });
            }
        }}
    };
}

#[macro_export]
macro_rules! ve_filter_cache_preset_filters {
    ($ui:expr) => {
        crate::global_ve_filter!($ui)
            .get_cache_preset_filters()
            .as_any()
            .downcast_ref::<VecModel<UIPresetFilter>>()
            .expect("We know we set a VecModel<UIPresetFilter> earlier")
    };
}

#[macro_export]
macro_rules! ve_filter_preset_filters {
    ($ui:expr, $type:ident) => {
        paste::paste! {
            crate::global_ve_filter!($ui)
                .[<get_ $type _preset_filters>]()
                .as_any()
                .downcast_ref::<VecModel<UIPresetFilter>>()
                .expect("We know we set a VecModel<UIPresetFilter>")
        }
    };
}

macro_rules! filter_default_match {
    ($filter_name:expr, $($filter_type:ty), *) => {
        match $filter_name {
            $(
                <$filter_type>::NAME => Some(Box::new(<$filter_type>::default())),
            )*
            _ => None,
        }
    };
}

macro_rules! filter_from_json_match {
    ($filter_name:expr, $detail:expr, $($filter_type:ty), *) => {
        match $filter_name {
            $(
                <$filter_type>::NAME => {
                    let filter = serde_json::from_str::<$filter_type>($detail)
                        .unwrap_or_else(|_| <$filter_type>::default());
                    Some(Box::new(filter))
                }
            )*
            _ => None,
        }
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    ve_filter_cb!(up_filter, ui, index);
    ve_filter_cb!(down_filter, ui, index);
    ve_filter_cb!(move_to_top_filter, ui, index);
    ve_filter_cb!(move_to_bottom_filter, ui, index);
    ve_filter_cb!(move_filter_by_drag, ui, from_index, to_index);
    ve_filter_cb!(
        move_preset_filter_by_drag,
        ui,
        filter_type_index,
        from_index,
        to_index
    );
    ve_filter_cb!(remove_filter, ui, index);
    ve_filter_cb!(toggle_filter, ui, index);
    ve_filter_cb!(copy_filter, ui, index);
    ve_filter_cb!(cut_filter, ui, index);
    ve_filter_cb!(paste_filter, ui);
    ve_filter_cb!(add_filter, ui, filter_name);
    ve_filter_cb!(create_preset_filter, ui, name);
    ve_filter_cb!(add_preset_filter, ui, filter);
    ve_filter_cb!(remove_preset_filter, ui, index, filter_type);
    ve_filter_cb!(preset_filter_up, ui, index, filter_type);
    ve_filter_cb!(preset_filter_down, ui, index, filter_type);
    ve_filter_cb!(preset_filter_move_to_top, ui, index, filter_type);
    ve_filter_cb!(preset_filter_move_to_bottom, ui, index, filter_type);
    ve_filter_cb!(preset_filter_rename, ui, filter_type_index, index, name);
    ve_filter_cb!(add_cache_preset_filter, ui, filter);
    ve_filter_cb!(remove_cache_preset_filter, ui, filter);
    ve_filter_cb!(toggle_mark_filter, ui, filter);
    ve_filter_cb!(refresh_filter_list, ui);
    ve_filter_cb!(get_new_filter_index, ui, filter_type);
    ve_filter_cb!(search_filter, ui, text, filter_type);
    ve_filter_cb!(search_preset_filter, ui, text, filter_type);
}

fn inner_init(ui: &AppWindow) {
    load_preset_filters_from_db(ui);
    load_marked_filters_from_db(ui);
}

pub fn up_filter(ui: &AppWindow, index: i32) {
    let merged_index = index as usize;
    let selected_segments = get_selected_segment_indices(ui);

    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return;
    };

    let Some((filter_type, _)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        return;
    };

    let mut batch_command = BatchCommand::new("Move filter up".to_string());

    for (track_idx, seg_idx) in &selected_segments {
        let Some((ft, fi)) = get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
        else {
            continue;
        };

        if ft != filter_type {
            continue;
        }

        if fi == 0 {
            continue;
        }

        let command = MoveFilterCommand::new(*track_idx, *seg_idx, ft, fi, fi - 1);
        batch_command.add_command(Box::new(command));
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            global_ve_filter!(ui).set_selected_filter_index(-1);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to move filter"), e)),
    }
}

pub fn down_filter(ui: &AppWindow, index: i32) {
    let merged_index = index as usize;
    let selected_segments = get_selected_segment_indices(ui);

    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return;
    };

    let Some((filter_type, _, _)) =
        get_filter_type_local_index_and_count(*track_idx, *seg_idx, merged_index)
    else {
        return;
    };

    let mut batch_command = BatchCommand::new("Move filter down".to_string());

    for (track_idx, seg_idx) in &selected_segments {
        let Some((ft, fi, count)) =
            get_filter_type_local_index_and_count(*track_idx, *seg_idx, merged_index)
        else {
            continue;
        };

        if ft != filter_type {
            continue;
        }

        if fi + 1 >= count {
            continue;
        }

        let command = MoveFilterCommand::new(*track_idx, *seg_idx, ft, fi, fi + 1);
        batch_command.add_command(Box::new(command));
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            global_ve_filter!(ui).set_selected_filter_index(-1);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to move filter"), e)),
    }
}

fn move_to_top_filter(ui: &AppWindow, index: i32) {
    let merged_index = index as usize;
    let selected_segments = get_selected_segment_indices(ui);

    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return;
    };

    let Some((filter_type, local_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        return;
    };

    if local_index == 0 {
        return;
    }

    let mut batch_command = BatchCommand::new("Move filter to top".to_string());

    for (track_idx, seg_idx) in &selected_segments {
        let Some((ft, fi)) = get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
        else {
            continue;
        };

        if ft != filter_type {
            continue;
        }

        if fi == 0 {
            continue;
        }

        let command = MoveFilterCommand::new(*track_idx, *seg_idx, ft, fi, 0);
        batch_command.add_command(Box::new(command));
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            global_ve_filter!(ui).set_selected_filter_index(-1);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to move filter"), e)),
    }
}

fn move_to_bottom_filter(ui: &AppWindow, index: i32) {
    let merged_index = index as usize;
    let selected_segments = get_selected_segment_indices(ui);

    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return;
    };

    let Some((filter_type, local_index, count)) =
        get_filter_type_local_index_and_count(*track_idx, *seg_idx, merged_index)
    else {
        return;
    };

    if local_index + 1 >= count {
        return;
    }

    let mut batch_command = BatchCommand::new("Move filter to bottom".to_string());

    for (track_idx, seg_idx) in &selected_segments {
        let Some((ft, fi, cnt)) =
            get_filter_type_local_index_and_count(*track_idx, *seg_idx, merged_index)
        else {
            continue;
        };

        if ft != filter_type {
            continue;
        }

        if fi + 1 >= cnt {
            continue;
        }

        let command = MoveFilterCommand::new(*track_idx, *seg_idx, ft, fi, cnt - 1);
        batch_command.add_command(Box::new(command));
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            global_ve_filter!(ui).set_selected_filter_index(-1);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to move filter"), e)),
    }
}

pub fn move_filter_by_drag(ui: &AppWindow, from_index: i32, to_index: i32) {
    if from_index == to_index {
        return;
    }

    let from_merged = from_index as usize;
    let to_merged = to_index as usize;
    let selected_segments = get_selected_segment_indices(ui);

    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return;
    };

    let Some((filter_type, _from_local)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, from_merged)
    else {
        return;
    };

    // Type validation: target must be in the same type group
    let Some((filter_type_to, _to_local)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, to_merged)
    else {
        return;
    };

    if filter_type != filter_type_to {
        return;
    }

    let mut batch_command = BatchCommand::new("Move filter by drag".to_string());

    for (track_idx, seg_idx) in &selected_segments {
        let Some((ft, fi)) = get_filter_type_and_local_index(*track_idx, *seg_idx, from_merged)
        else {
            continue;
        };

        if ft != filter_type {
            continue;
        }

        let Some((ft_to, ti)) = get_filter_type_and_local_index(*track_idx, *seg_idx, to_merged)
        else {
            continue;
        };

        if ft_to != filter_type {
            continue;
        }

        let command = MoveFilterCommand::new(*track_idx, *seg_idx, ft, fi, ti);
        batch_command.add_command(Box::new(command));
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            global_ve_filter!(ui).set_selected_filter_index(-1);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to move filter"), e)),
    }
}

pub fn move_preset_filter_by_drag(
    ui: &AppWindow,
    filter_type_index: i32,
    from_index: i32,
    to_index: i32,
) {
    if from_index == to_index {
        return;
    }

    let filter_type = global_ve_filter!(ui).invoke_filter_type_from_int(filter_type_index);
    let from_idx = from_index as usize;
    let to_idx = to_index as usize;

    let should_save = with_preset_filters(ui, filter_type, |filters| {
        if from_idx >= filters.row_count() || to_idx >= filters.row_count() {
            return false;
        }
        let item = filters.remove(from_idx);
        filters.insert(to_idx, item);
        true
    });

    if should_save {
        let config = collect_preset_filters_from_ui(ui);
        save_preset_filters_to_db(ui.as_weak(), config);
    }
}

fn collect_speed_filter_info(
    selected_segments: &[(usize, usize)],
    merged_index: usize,
    filter_type: FilterType,
) -> Vec<SpeedFilterInfo> {
    selected_segments
        .iter()
        .filter_map(|(t_idx, s_idx)| {
            let Some((ft, fi)) = get_filter_type_and_local_index(*t_idx, *s_idx, merged_index)
            else {
                return None;
            };

            if ft != filter_type {
                return None;
            }

            with_history_manager(|state| {
                let track = state.tracks_manager.get(*t_idx)?;
                let segment = track.get_segment(*s_idx).ok()?;

                let info: Option<(bool, f32)> = match ft {
                    FilterType::Video => segment.video_filters.get(fi).and_then(|f| {
                        if f.inner.name() == SpeedFilter::NAME {
                            let filter_speed = f
                                .inner
                                .as_any()
                                .downcast_ref::<SpeedFilter>()
                                .map(|sf| sf.speed)
                                .unwrap_or(1.0);
                            Some((f.enabled(), filter_speed))
                        } else {
                            None
                        }
                    }),
                    FilterType::Audio => segment.audio_filters.get(fi).and_then(|f| {
                        if f.inner.name() == AudioSpeedFilter::NAME {
                            let filter_speed = f
                                .inner
                                .as_any()
                                .downcast_ref::<AudioSpeedFilter>()
                                .map(|sf| sf.speed)
                                .unwrap_or(1.0);
                            Some((f.enabled.load(Ordering::Relaxed), filter_speed))
                        } else {
                            None
                        }
                    }),
                    FilterType::Image => segment.image_filters.get(fi).and_then(|f| {
                        if f.inner.name() == SpeedFilter::NAME {
                            let filter_speed = f
                                .inner
                                .as_any()
                                .downcast_ref::<SpeedFilter>()
                                .map(|sf| sf.speed)
                                .unwrap_or(1.0);
                            Some((f.enabled(), filter_speed))
                        } else {
                            None
                        }
                    }),
                    FilterType::Subtitle => None,
                };

                info.map(|(enabled, filter_speed)| SpeedFilterInfo {
                    track_idx: *t_idx,
                    seg_idx: *s_idx,
                    playback_speed: segment.playback_speed,
                    duration: segment.duration,
                    filter_enabled: enabled,
                    filter_speed,
                })
            })
        })
        .collect()
}

pub fn remove_filter(ui: &AppWindow, index: i32) {
    let merged_index = index as usize;
    let selected_segments = get_selected_segment_indices(ui);

    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return;
    };

    let Some((filter_type, _)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        return;
    };

    let speed_info = collect_speed_filter_info(&selected_segments, merged_index, filter_type);

    let mut batch_command = BatchCommand::new("Remove filter".to_string());

    for (track_idx, seg_idx) in &selected_segments {
        let Some((ft, fi)) = get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
        else {
            continue;
        };

        if ft != filter_type {
            continue;
        }

        let command = match ft {
            FilterType::Video => Box::new(RemoveFilterCommand::new_video(*track_idx, *seg_idx, fi)),
            FilterType::Audio => Box::new(RemoveFilterCommand::new_audio(*track_idx, *seg_idx, fi)),
            FilterType::Subtitle => {
                Box::new(RemoveFilterCommand::new_subtitle(*track_idx, *seg_idx, fi))
            }
            FilterType::Image => Box::new(RemoveFilterCommand::new_image(*track_idx, *seg_idx, fi)),
        };
        batch_command.add_command(command);
    }

    for info in speed_info {
        batch_command.add_command(Box::new(SetPlaybackSpeedCommand::new(
            info.track_idx,
            info.seg_idx,
            1.0,
            info.playback_speed,
            info.duration,
        )));
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            global_ve_filter!(ui).set_selected_filter_index(-1);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
            crate::toast_success!(ui, tr("Removed filter"));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove filter"), e)),
    }
}

fn toggle_filter(ui: &AppWindow, index: i32) {
    let merged_index = index as usize;
    let selected_segments = get_selected_segment_indices(ui);

    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return;
    };

    let Some((filter_type, _)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        return;
    };

    let speed_info = collect_speed_filter_info(&selected_segments, merged_index, filter_type);

    let mut batch_command = BatchCommand::new("Toggle filter".to_string());

    for (track_idx, seg_idx) in &selected_segments {
        let Some((ft, fi)) = get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
        else {
            continue;
        };

        if ft != filter_type {
            continue;
        }

        let command = ToggleFilterCommand::new(*track_idx, *seg_idx, ft, fi);
        batch_command.add_command(Box::new(command));
    }

    for info in speed_info {
        if info.filter_enabled {
            batch_command.add_command(Box::new(SetPlaybackSpeedCommand::new(
                info.track_idx,
                info.seg_idx,
                1.0,
                info.playback_speed,
                info.duration,
            )));
        } else {
            batch_command.add_command(Box::new(SetPlaybackSpeedCommand::new(
                info.track_idx,
                info.seg_idx,
                info.filter_speed,
                info.playback_speed,
                info.duration,
            )));
        }
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to toggle filter"), e)),
    }
}

pub fn copy_filter(ui: &AppWindow, index: i32) {
    let merged_index = index as usize;
    let selected_segments = get_selected_segment_indices(ui);
    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return;
    };

    let Some((filter_type, filter_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        return;
    };

    let filter_data: Option<(bool, String, String)> = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;

        match filter_type {
            FilterType::Video => segment.video_filters.get(filter_index).map(|f| {
                let detail = video_filter_to_json_detail(&f.inner);
                (f.enabled(), f.inner.name().to_string(), detail)
            }),
            FilterType::Audio => segment.audio_filters.get(filter_index).map(|f| {
                let detail = audio_filter_to_json_detail(&f.inner);
                (f.enabled(), f.inner.name().to_string(), detail)
            }),
            FilterType::Subtitle => segment.subtitle_filters.get(filter_index).map(|f| {
                let detail = subtitle_filter_to_json_detail(&f.inner);
                (f.enabled(), f.inner.name().to_string(), detail)
            }),
            FilterType::Image => segment.image_filters.get(filter_index).map(|f| {
                let detail = image_filter_to_json_detail(&f.inner);
                let name = f.inner.name().to_string();
                (f.enabled(), name, detail)
            }),
        }
    });

    if let Some((enabled, name, detail)) = filter_data {
        let segment_filter = UISegmentFilter {
            ty: filter_type.into(),
            enabled,
            name: SharedString::from(name.clone()),
            detail: SharedString::from(detail),
        };
        global_ve_filter!(ui).set_cache_copied_filter(segment_filter);
        crate::toast_success!(ui, format!("{}: {}", tr("Copied filter"), name));
    }
}

pub fn cut_filter(ui: &AppWindow, index: i32) {
    let merged_index = index as usize;
    let selected_segments = get_selected_segment_indices(ui);
    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return;
    };

    let Some((filter_type, filter_index)) =
        get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
    else {
        return;
    };

    let filter_data: Option<(bool, String, String)> = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;

        match filter_type {
            FilterType::Video => segment.video_filters.get(filter_index).map(|f| {
                let detail = video_filter_to_json_detail(&f.inner);
                (f.enabled(), f.inner.name().to_string(), detail)
            }),
            FilterType::Audio => segment.audio_filters.get(filter_index).map(|f| {
                let detail = audio_filter_to_json_detail(&f.inner);
                (f.enabled(), f.inner.name().to_string(), detail)
            }),
            FilterType::Subtitle => segment.subtitle_filters.get(filter_index).map(|f| {
                let detail = subtitle_filter_to_json_detail(&f.inner);
                (f.enabled(), f.inner.name().to_string(), detail)
            }),
            FilterType::Image => segment.image_filters.get(filter_index).map(|f| {
                let detail = image_filter_to_json_detail(&f.inner);
                (f.enabled(), f.inner.name().to_string(), detail)
            }),
        }
    });

    let filter_name = match filter_data {
        Some((enabled, name, detail)) => {
            let segment_filter = UISegmentFilter {
                ty: filter_type.into(),
                enabled,
                name: SharedString::from(name.clone()),
                detail: SharedString::from(detail),
            };
            global_ve_filter!(ui).set_cache_copied_filter(segment_filter);
            name
        }
        None => return,
    };

    let mut batch_command = BatchCommand::new(format!("Cut filter '{}'", filter_name));

    for (track_idx, seg_idx) in &selected_segments {
        let Some((ft, fi)) = get_filter_type_and_local_index(*track_idx, *seg_idx, merged_index)
        else {
            continue;
        };

        if ft != filter_type {
            continue;
        }

        let command = match ft {
            FilterType::Video => Box::new(RemoveFilterCommand::new_video(*track_idx, *seg_idx, fi)),
            FilterType::Audio => Box::new(RemoveFilterCommand::new_audio(*track_idx, *seg_idx, fi)),
            FilterType::Subtitle => {
                Box::new(RemoveFilterCommand::new_subtitle(*track_idx, *seg_idx, fi))
            }
            FilterType::Image => Box::new(RemoveFilterCommand::new_image(*track_idx, *seg_idx, fi)),
        };
        batch_command.add_command(command);
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            global_ve_filter!(ui).set_selected_filter_index(-1);
            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
            crate::toast_success!(ui, format!("{}: {}", tr("Cut filter"), filter_name));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to cut filter"), e)),
    }
}

pub fn paste_filter(ui: &AppWindow) {
    let cached_filter = global_ve_filter!(ui).get_cache_copied_filter();
    let filter_entry = cached_filter.clone().into();
    let filter_name = cached_filter.name.to_string();

    if filter_name.is_empty() {
        crate::toast_warn!(ui, tr("No filter copied to paste"));
        return;
    }

    let selected_segments = get_selected_segment_indices(ui);
    let Some((track_idx, _seg_idx)) = selected_segments.last() else {
        return;
    };

    let track_type: Option<FilterType> = with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        Some(track_to_filter_type(&track))
    });

    let Some(track_type) = track_type else {
        return;
    };

    let copied_type: FilterType = cached_filter.ty.into();

    // Validate compatibility between copied filter type and track type
    if !is_filter_compatible(&copied_type, &track_type) {
        crate::toast_warn!(ui, tr("Cannot paste filter to this track type"));
        return;
    }

    let paste_type = copied_type;
    let filter_detail = cached_filter.detail.to_string();
    let enabled = cached_filter.enabled;

    let mut batch_command = BatchCommand::new(format!("Paste filter '{}'", filter_name));

    for (track_idx, seg_idx) in &selected_segments {
        let has_filter = segment_contains_filter(*track_idx, *seg_idx, &filter_entry);
        if matches!(has_filter, Some(true)) {
            continue;
        }

        if let Some(cmd) = create_filter_command_with_detail(
            *track_idx,
            *seg_idx,
            paste_type.clone(),
            &filter_name,
            &filter_detail,
            enabled,
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
                sync_manager_to_ui(ui);
                refresh_affected_segments(ui, execute_result.affected_segments);
                refresh_preview(ui);

                global_ve_filter!(ui).set_selected_filter_index(-1);
                global_ve_filter!(ui).invoke_refresh_filter_list();
                crate::toast_success!(ui, format!("{}: {}", tr("Pasted filter"), filter_name));
            }
            Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to paste filter"), e)),
        }
    } else {
        crate::toast_warn!(
            ui,
            format!(
                "{}: {} {filter_name}",
                tr("Failed to paste filter"),
                tr("All selected segments have contained filter")
            )
        );
    }
}

pub fn refresh_filter_list(ui: &AppWindow) {
    global_store!(ui).set_video_editor_segment_filter_flag(
        !global_store!(ui).get_video_editor_segment_filter_flag(),
    );
}

fn get_new_filter_index(ui: &AppWindow, filter_type: UIFilterType) -> i32 {
    let selected_segments = get_selected_segment_indices(ui);

    let Some((track_idx, seg_idx)) = selected_segments.last() else {
        return -1;
    };

    with_history_manager(|state| {
        let track = state.tracks_manager.get(*track_idx)?;
        let segment = track.get_segment(*seg_idx).ok()?;

        let video_count = segment.video_filters.len();
        let audio_count = segment.audio_filters.len();
        let subtitle_count = segment.subtitle_filters.len();
        let image_count = segment.image_filters.len();

        let internal_type: FilterType = filter_type.into();
        let index = match internal_type {
            FilterType::Video => video_count.saturating_sub(1),
            FilterType::Audio => (video_count + audio_count).saturating_sub(1),
            FilterType::Subtitle => (video_count + audio_count + subtitle_count).saturating_sub(1),
            FilterType::Image => {
                (video_count + audio_count + subtitle_count + image_count).saturating_sub(1)
            }
        };

        Some(index as i32)
    })
    .unwrap_or(-1)
}

fn is_filter_compatible(filter_type: &FilterType, track_type: &FilterType) -> bool {
    match (filter_type, track_type) {
        (FilterType::Video, FilterType::Video) => true,
        (FilterType::Video, FilterType::Image) => true,
        (FilterType::Image, FilterType::Image) => true,
        (FilterType::Image, FilterType::Video) => true,
        (FilterType::Audio, FilterType::Audio) => true,
        (FilterType::Audio, FilterType::Video) => true,
        (FilterType::Subtitle, FilterType::Subtitle) => true,
        _ => false,
    }
}

fn add_filter(ui: &AppWindow, entry: UIFilterEntry) {
    let filter_name = entry.name.to_string();
    let filter_type: FilterType = entry.ty.into();
    let selected_segments = get_selected_segment_indices(ui);

    if selected_segments.is_empty() {
        return;
    }

    let mut batch_command = BatchCommand::new(format!("Add {} filter", filter_name));

    for (track_idx, seg_idx) in &selected_segments {
        let command =
            create_filter_command(*track_idx, *seg_idx, filter_type.clone(), &filter_name);
        if let Some(cmd) = command {
            batch_command.add_command(cmd);
        }
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );
            crate::toast_success!(
                ui,
                format!("{} {} {}", tr("Added"), filter_name, tr("filter"))
            );
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to add filter"), e)),
    }
}

fn create_preset_filter(ui: &AppWindow, name: SharedString) {
    if ve_filter_cache_preset_filters!(ui).row_count() == 0 {
        crate::toast_warn!(ui, tr("No filters selected for preset"));
        return;
    }

    let all_filters: Vec<UISegmentFilter> = ve_filter_cache_preset_filters!(ui)
        .iter()
        .map(|preset| preset.filters.iter().collect::<Vec<_>>())
        .flatten()
        .collect();

    let preset_filter = UIPresetFilter {
        name,
        filters: ModelRc::from(VecModel::from_slice(&all_filters)),
    };

    let filter_type = get_filter_type_for_preset(ui);
    let filter_type = if filter_type == FilterType::Video {
        if all_filters.iter().all(|f| f.ty == UIFilterType::Audio) {
            FilterType::Audio
        } else {
            FilterType::Video
        }
    } else {
        filter_type
    };

    match filter_type {
        FilterType::Video => ve_filter_preset_filters!(ui, video).push(preset_filter),
        FilterType::Audio => ve_filter_preset_filters!(ui, audio).push(preset_filter),
        FilterType::Subtitle => ve_filter_preset_filters!(ui, subtitle).push(preset_filter),
        FilterType::Image => ve_filter_preset_filters!(ui, image).push(preset_filter),
    }

    let config = collect_preset_filters_from_ui(ui);
    save_preset_filters_to_db(ui.as_weak(), config);

    ve_filter_cache_preset_filters!(ui).set_vec(vec![]);

    crate::toast_success!(ui, tr("Preset filter created"));
}

fn add_preset_filter(ui: &AppWindow, filter: UIPresetFilter) {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        return;
    }

    let mut batch_command = BatchCommand::new(format!("Add preset filter: {}", filter.name));

    for segment_filter in filter.filters.iter() {
        let filter_name = segment_filter.name.to_string();
        let filter_detail = segment_filter.detail.to_string();
        let enabled = segment_filter.enabled;
        let filter_ty: UIFilterType = segment_filter.ty;

        for (track_idx, seg_idx) in &selected_segments {
            let command = create_filter_command_with_detail(
                *track_idx,
                *seg_idx,
                filter_ty.into(),
                &filter_name,
                &filter_detail,
                enabled,
            );
            if let Some(cmd) = command {
                batch_command.add_command(cmd);
            }
        }
    }

    let result = with_history_manager(|state| {
        state
            .history_manager
            .execute(&mut state.tracks_manager, Box::new(batch_command))
    });

    match result {
        Ok(execute_result) => {
            sync_and_refresh(ui, execute_result.affected_segments, Some(true));

            global_store!(ui).set_video_editor_segment_filter_flag(
                !global_store!(ui).get_video_editor_segment_filter_flag(),
            );

            crate::toast_success!(ui, format!("{} {}", tr("Added preset filter"), filter.name));
        }
        Err(e) => crate::toast_warn!(ui, format!("{}: {}", tr("Failed to add preset filter"), e)),
    }
}

fn add_cache_preset_filter(ui: &AppWindow, filter: UISegmentFilter) {
    let preset = UIPresetFilter {
        name: SharedString::new(), // 不要名称，只是临时存储，而且不会在界面上展示
        filters: ModelRc::from(VecModel::from_slice(&vec![filter])),
    };

    ve_filter_cache_preset_filters!(ui).push(preset);
}

fn remove_cache_preset_filter(ui: &AppWindow, filter: UISegmentFilter) {
    let new_cache: Vec<UIPresetFilter> = ve_filter_cache_preset_filters!(ui)
        .iter()
        .filter(|preset| {
            preset
                .filters
                .iter()
                .all(|f| f.name != filter.name || f.detail != filter.detail)
        })
        .collect();

    ve_filter_cache_preset_filters!(ui).set_vec(new_cache);
}

pub fn remove_preset_filter(ui: &AppWindow, index: i32, filter_type: UIFilterType) {
    let idx = index as usize;

    match filter_type {
        UIFilterType::Video => {
            if idx < ve_filter_preset_filters!(ui, video).row_count() {
                ve_filter_preset_filters!(ui, video).remove(idx);
            }
        }
        UIFilterType::Audio => {
            if idx < ve_filter_preset_filters!(ui, audio).row_count() {
                ve_filter_preset_filters!(ui, audio).remove(idx);
            }
        }
        UIFilterType::Subtitle => {
            if idx < ve_filter_preset_filters!(ui, subtitle).row_count() {
                ve_filter_preset_filters!(ui, subtitle).remove(idx);
            }
        }
        UIFilterType::Image => {
            if idx < ve_filter_preset_filters!(ui, image).row_count() {
                ve_filter_preset_filters!(ui, image).remove(idx);
            }
        }
    }

    let config = collect_preset_filters_from_ui(ui);
    save_preset_filters_to_db(ui.as_weak(), config);

    crate::toast_success!(ui, tr("Preset filter removed"));
}

pub fn preset_filter_up(ui: &AppWindow, index: i32, filter_type: UIFilterType) {
    let idx = index as usize;
    if idx == 0 {
        return;
    }

    with_preset_filters(ui, filter_type, |filters| {
        if idx < filters.row_count() {
            let a = filters.row_data(idx).unwrap();
            let b = filters.row_data(idx - 1).unwrap();
            filters.set_row_data(idx, b);
            filters.set_row_data(idx - 1, a);
        }
    });

    let config = collect_preset_filters_from_ui(ui);
    save_preset_filters_to_db(ui.as_weak(), config);
}

pub fn preset_filter_down(ui: &AppWindow, index: i32, filter_type: UIFilterType) {
    let idx = index as usize;

    let should_save = with_preset_filters(ui, filter_type, |filters| {
        let count = filters.row_count();
        if idx >= count - 1 {
            return false;
        }
        let a = filters.row_data(idx).unwrap();
        let b = filters.row_data(idx + 1).unwrap();
        filters.set_row_data(idx, b);
        filters.set_row_data(idx + 1, a);
        true
    });

    if should_save {
        let config = collect_preset_filters_from_ui(ui);
        save_preset_filters_to_db(ui.as_weak(), config);
    }
}

pub fn preset_filter_move_to_top(ui: &AppWindow, index: i32, filter_type: UIFilterType) {
    let idx = index as usize;
    if idx == 0 {
        return;
    }

    let should_save = with_preset_filters(ui, filter_type, |filters| {
        if idx >= filters.row_count() {
            return false;
        }
        let item = filters.remove(idx);
        filters.insert(0, item);
        true
    });

    if should_save {
        let config = collect_preset_filters_from_ui(ui);
        save_preset_filters_to_db(ui.as_weak(), config);
    }
}

pub fn preset_filter_move_to_bottom(ui: &AppWindow, index: i32, filter_type: UIFilterType) {
    let idx = index as usize;

    let should_save = with_preset_filters(ui, filter_type, |filters| {
        if idx >= filters.row_count() - 1 {
            return false;
        }
        let item = filters.remove(idx);
        filters.push(item);
        true
    });

    if should_save {
        let config = collect_preset_filters_from_ui(ui);
        save_preset_filters_to_db(ui.as_weak(), config);
    }
}

fn preset_filter_rename(ui: &AppWindow, filter_type_index: i32, index: i32, name: SharedString) {
    let filter_type = global_ve_filter!(ui).invoke_filter_type_from_int(filter_type_index);
    let idx = index as usize;

    let should_save = with_preset_filters(ui, filter_type, |filters| {
        if idx >= filters.row_count() {
            return false;
        }
        let mut item = filters.row_data(idx).unwrap();
        item.name = name;
        filters.set_row_data(idx, item);
        true
    });

    if should_save {
        let config = collect_preset_filters_from_ui(ui);
        save_preset_filters_to_db(ui.as_weak(), config);
    }
}

fn toggle_mark_filter(ui: &AppWindow, filter: UIFilterEntry) {
    let filter_name = filter.name.to_string();
    let filter_type: UIFilterType = filter.ty;

    let filters = match filter_type {
        UIFilterType::Video => global_ve_filter!(ui).get_video_filters(),
        UIFilterType::Audio => global_ve_filter!(ui).get_audio_filters(),
        UIFilterType::Subtitle => global_ve_filter!(ui).get_subtitle_filters(),
        UIFilterType::Image => global_ve_filter!(ui).get_image_filters(),
    };

    let mut updated_filters: Vec<UIFilterEntry> = filters
        .iter()
        .map(|f| {
            if f.name == filter_name {
                UIFilterEntry {
                    ty: f.ty,
                    name: f.name.clone(),
                    is_marked: !f.is_marked,
                }
            } else {
                f
            }
        })
        .collect();

    updated_filters.sort_by(|a, b| b.is_marked.cmp(&a.is_marked));

    match filter_type {
        UIFilterType::Video => global_ve_filter!(ui)
            .set_video_filters(ModelRc::new(VecModel::from_slice(&updated_filters))),
        UIFilterType::Audio => global_ve_filter!(ui)
            .set_audio_filters(ModelRc::new(VecModel::from_slice(&updated_filters))),
        UIFilterType::Subtitle => global_ve_filter!(ui)
            .set_subtitle_filters(ModelRc::new(VecModel::from_slice(&updated_filters))),
        UIFilterType::Image => global_ve_filter!(ui)
            .set_image_filters(ModelRc::new(VecModel::from_slice(&updated_filters))),
    }

    save_marked_filters_to_db(ui.as_weak(), collect_marked_filters_from_ui(ui));
}

fn search_filter(ui: &AppWindow, text: SharedString, filter_type: UIFilterType) {
    let search_text = text.to_string().to_lowercase();
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, MARKED_FILTERS_ID).await {
            Ok(item) => serde_json::from_str::<MarkedFiltersConfig>(&item.data).unwrap_or_default(),
            Err(_) => MarkedFiltersConfig::default(),
        };

        let (all_names, marked_names): (&[&str], &[String]) = match filter_type {
            UIFilterType::Video => (all_video_filter_names(), &config.video),
            UIFilterType::Audio => (all_audio_filter_names(), &config.audio),
            UIFilterType::Subtitle => (all_subtitle_filter_names(), &config.subtitle),
            UIFilterType::Image => (all_image_filter_names(), &config.image),
        };

        let marked_set: HashSet<&str> = marked_names.iter().map(|s| s.as_str()).collect();

        let filtered: Vec<UIFilterEntry> = if search_text.is_empty() {
            all_names
                .iter()
                .map(|name| UIFilterEntry {
                    ty: filter_type,
                    name: SharedString::from(*name),
                    is_marked: marked_set.contains(*name),
                })
                .collect()
        } else {
            all_names
                .iter()
                .filter(|name| {
                    let name_lower = name.to_lowercase();
                    let translated = tr(name);
                    let translated_lower = translated.to_lowercase();
                    name_lower.contains(&search_text) || translated_lower.contains(&search_text)
                })
                .map(|name| UIFilterEntry {
                    ty: filter_type,
                    name: SharedString::from(*name),
                    is_marked: marked_set.contains(*name),
                })
                .collect()
        };

        let mut sorted = filtered;
        sorted.sort_by(|a, b| b.is_marked.cmp(&a.is_marked));

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let set_filters: fn(&AppWindow, ModelRc<UIFilterEntry>) = match filter_type {
                UIFilterType::Video => |ui, f| global_ve_filter!(ui).set_video_filters(f),
                UIFilterType::Audio => |ui, f| global_ve_filter!(ui).set_audio_filters(f),
                UIFilterType::Subtitle => |ui, f| global_ve_filter!(ui).set_subtitle_filters(f),
                UIFilterType::Image => |ui, f| global_ve_filter!(ui).set_image_filters(f),
            };
            set_filters(&ui, ModelRc::new(VecModel::from_slice(&sorted)));
        });
    });
}

fn search_preset_filter(ui: &AppWindow, text: SharedString, filter_type: UIFilterType) {
    let search_text = text.to_string().to_lowercase();
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, PRESET_FILTERS_ID).await {
            Ok(item) => serde_json::from_str::<PresetFiltersConfig>(&item.data).unwrap_or_default(),
            Err(_) => PresetFiltersConfig::default(),
        };

        let db_presets = match filter_type {
            UIFilterType::Video => config.video,
            UIFilterType::Audio => config.audio,
            UIFilterType::Subtitle => config.subtitle,
            UIFilterType::Image => config.image,
        };

        let filtered: Vec<PresetFilter> = if search_text.is_empty() {
            db_presets
        } else {
            db_presets
                .into_iter()
                .filter(|preset| preset.name.to_lowercase().contains(&search_text))
                .collect()
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let ui_presets: Vec<UIPresetFilter> = filtered
                .into_iter()
                .filter_map(|preset| {
                    let filters: Vec<UISegmentFilter> =
                        match serde_json::from_str::<Vec<SegmentFilterData>>(&preset.filters) {
                            Ok(data) => data
                                .into_iter()
                                .map(|f| UISegmentFilter {
                                    ty: filter_type,
                                    enabled: f.enabled,
                                    name: f.name.into(),
                                    detail: f.detail.into(),
                                })
                                .collect(),
                            Err(_) => return None,
                        };
                    Some(UIPresetFilter {
                        name: preset.name.into(),
                        filters: ModelRc::from(VecModel::from_slice(&filters)),
                    })
                })
                .collect();

            match filter_type {
                UIFilterType::Video => ve_filter_preset_filters!(ui, video).set_vec(ui_presets),
                UIFilterType::Audio => ve_filter_preset_filters!(ui, audio).set_vec(ui_presets),
                UIFilterType::Subtitle => {
                    ve_filter_preset_filters!(ui, subtitle).set_vec(ui_presets)
                }
                UIFilterType::Image => ve_filter_preset_filters!(ui, image).set_vec(ui_presets),
            }
        });
    });
}

pub fn create_filter_command(
    track_index: usize,
    segment_index: usize,
    filter_type: FilterType,
    filter_name: &str,
) -> Option<Box<dyn Command>> {
    match filter_type {
        FilterType::Video => {
            let filter = create_video_filter_by_name(filter_name)?;
            Some(Box::new(AddFilterCommand::new_video(
                track_index,
                segment_index,
                filter,
            )))
        }
        FilterType::Audio => {
            let filter = create_audio_filter_by_name(filter_name)?;
            Some(Box::new(AddFilterCommand::new_audio(
                track_index,
                segment_index,
                filter,
            )))
        }
        FilterType::Subtitle => {
            let filter = create_subtitle_filter_by_name(filter_name)?;
            Some(Box::new(AddFilterCommand::new_subtitle(
                track_index,
                segment_index,
                filter,
            )))
        }
        FilterType::Image => {
            let filter = create_image_filter_by_name(filter_name)?;
            Some(Box::new(AddFilterCommand::new_image(
                track_index,
                segment_index,
                filter,
            )))
        }
    }
}

pub fn create_filter_command_with_detail(
    track_index: usize,
    segment_index: usize,
    filter_type: FilterType,
    filter_name: &str,
    filter_detail: &str,
    enabled: bool,
) -> Option<Box<dyn Command>> {
    match filter_type {
        FilterType::Video => {
            let filter = create_video_filter_by_name_with_detail(filter_name, filter_detail)?;
            Some(Box::new(AddFilterCommand::new_video(
                track_index,
                segment_index,
                filter,
            )))
        }
        FilterType::Audio => {
            let filter = create_audio_filter_by_name_with_detail(filter_name, filter_detail)?;
            Some(Box::new(AddFilterCommand::new_audio(
                track_index,
                segment_index,
                filter,
            )))
        }
        FilterType::Subtitle => {
            let filter = create_subtitle_filter_by_name_with_detail(filter_name, filter_detail)?;
            Some(Box::new(AddFilterCommand::new_subtitle(
                track_index,
                segment_index,
                filter,
            )))
        }
        FilterType::Image => {
            let filter =
                create_image_filter_by_name_with_detail(filter_name, filter_detail, enabled)?;
            Some(Box::new(AddFilterCommand::new_image(
                track_index,
                segment_index,
                filter,
            )))
        }
    }
}

fn create_video_filter_by_name(name: &str) -> Option<Box<dyn VideoFilter>> {
    filter_default_match!(
        name,
        ChromaKeyFilter,
        FlipFilter,
        CropFilter,
        TransformFilter,
        ZoomFilter,
        ImageFadeInFilter,
        ImageFadeOutFilter,
        FlyInFilter,
        SlideFilter,
        WipeFilter,
        OpacityFilter,
        VignetteFilter,
        LinearMaskFilter,
        CircleMaskFilter,
        MirrorMaskFilter,
        RectangleMaskFilter,
        BorderFilter,
        MosaicFilter,
        DrawCircleFilter,
        DrawRectangleFilter,
        BackgroundFilter,
        HSLAdjustFilter,
        SpeedFilter,
        BreathingFilter,
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
        TextHighlightFilter,
        ShadowFilter,
        DeviceFrameFilter,
        GenieFilter,
        PageFlipFilter,
        LightingFilter,
        SplitFilter,
        FrameExtractFilter,
        Live2dFilter
    )
}

fn create_audio_filter_by_name(name: &str) -> Option<Box<dyn AudioFilter>> {
    filter_default_match!(
        name,
        GainFilter,
        NormalizeFilter,
        LimiterFilter,
        NoiseGateFilter,
        CompressorFilter,
        DenoiseFilter,
        MuteFilter,
        CopyChannelFilter,
        AudioFadeInFilter,
        AudioFadeOutFilter,
        VoiceChangerFilter,
        AudioSpeedFilter
    )
}

fn create_subtitle_filter_by_name(name: &str) -> Option<Box<dyn SubtitleFilter>> {
    filter_default_match!(
        name,
        BackgroundColorFilter,
        BorderRadiusFilter,
        FontPathFilter,
        FontSizeFilter,
        MarginHorizontalFilter,
        MarginVerticalFilter,
        OutlineColorFilter,
        OutlineWidthFilter,
        PaddingFilter,
        PrimaryColorFilter
    )
}

fn create_image_filter_by_name(name: &str) -> Option<ImageFilterWrapper> {
    create_video_filter_by_name(name).map(|filter| ImageFilterWrapper::new(true, filter))
}

fn create_video_filter_by_name_with_detail(
    name: &str,
    detail: &str,
) -> Option<Box<dyn VideoFilter>> {
    filter_from_json_match!(
        name,
        detail,
        ChromaKeyFilter,
        FlipFilter,
        CropFilter,
        TransformFilter,
        ZoomFilter,
        ImageFadeInFilter,
        ImageFadeOutFilter,
        FlyInFilter,
        SlideFilter,
        WipeFilter,
        OpacityFilter,
        VignetteFilter,
        LinearMaskFilter,
        CircleMaskFilter,
        MirrorMaskFilter,
        RectangleMaskFilter,
        BorderFilter,
        MosaicFilter,
        DrawCircleFilter,
        DrawRectangleFilter,
        BackgroundFilter,
        HSLAdjustFilter,
        SpeedFilter,
        BreathingFilter,
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
        TextHighlightFilter,
        ShadowFilter,
        DeviceFrameFilter,
        GenieFilter,
        PageFlipFilter,
        LightingFilter,
        SplitFilter,
        FrameExtractFilter,
        Live2dFilter
    )
}

fn create_audio_filter_by_name_with_detail(
    name: &str,
    detail: &str,
) -> Option<Box<dyn AudioFilter>> {
    filter_from_json_match!(
        name,
        detail,
        GainFilter,
        NormalizeFilter,
        LimiterFilter,
        NoiseGateFilter,
        CompressorFilter,
        DenoiseFilter,
        MuteFilter,
        CopyChannelFilter,
        AudioFadeInFilter,
        AudioFadeOutFilter,
        VoiceChangerFilter,
        AudioSpeedFilter
    )
}

fn create_subtitle_filter_by_name_with_detail(
    name: &str,
    detail: &str,
) -> Option<Box<dyn SubtitleFilter>> {
    filter_from_json_match!(
        name,
        detail,
        BackgroundColorFilter,
        BorderRadiusFilter,
        FontPathFilter,
        FontSizeFilter,
        MarginHorizontalFilter,
        MarginVerticalFilter,
        OutlineColorFilter,
        OutlineWidthFilter,
        PaddingFilter,
        PrimaryColorFilter
    )
}

fn create_image_filter_by_name_with_detail(
    name: &str,
    detail: &str,
    enabled: bool,
) -> Option<ImageFilterWrapper> {
    create_video_filter_by_name_with_detail(name, detail)
        .map(|filter| ImageFilterWrapper::new(enabled, filter))
}

// Convert a merged filter index to the filter type and its local index within that type.
// Returns None if the index is out of bounds.
pub fn get_filter_type_and_local_index(
    track_idx: usize,
    seg_idx: usize,
    merged_index: usize,
) -> Option<(FilterType, usize)> {
    get_filter_type_local_index_and_count(track_idx, seg_idx, merged_index)
        .map(|(ft, idx, _)| (ft, idx))
}

fn get_filter_type_local_index_and_count(
    track_idx: usize,
    seg_idx: usize,
    merged_index: usize,
) -> Option<(FilterType, usize, usize)> {
    with_history_manager(|state| {
        let track = state.tracks_manager.get(track_idx)?;
        let segment = track.get_segment(seg_idx).ok()?;

        let video_count = segment.video_filters.len();
        let audio_count = segment.audio_filters.len();
        let subtitle_count = segment.subtitle_filters.len();
        let image_count = segment.image_filters.len();

        if merged_index < video_count {
            Some((FilterType::Video, merged_index, video_count))
        } else if merged_index < video_count + audio_count {
            Some((FilterType::Audio, merged_index - video_count, audio_count))
        } else if merged_index < video_count + audio_count + subtitle_count {
            Some((
                FilterType::Subtitle,
                merged_index - video_count - audio_count,
                subtitle_count,
            ))
        } else if merged_index < video_count + audio_count + subtitle_count + image_count {
            Some((
                FilterType::Image,
                merged_index - video_count - audio_count - subtitle_count,
                image_count,
            ))
        } else {
            None
        }
    })
}

fn get_filter_type_for_preset(ui: &AppWindow) -> FilterType {
    let track_idx = global_store!(ui).get_video_editor_current_edited_track_index();

    if track_idx < 0 {
        return FilterType::Video;
    }

    with_history_manager(|state| {
        let idx = track_idx as usize;
        if idx >= state.tracks_manager.len() {
            return FilterType::Video;
        }

        match state.tracks_manager.get(idx) {
            Some(Track::Video(_)) => FilterType::Video,
            Some(Track::Audio(_)) => FilterType::Audio,
            Some(Track::Subtitle(_)) => FilterType::Subtitle,
            Some(Track::Image(_)) => FilterType::Image,
            Some(Track::Text(_)) | None => FilterType::Video,
        }
    })
}

fn load_preset_filters_from_db(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, PRESET_FILTERS_ID).await {
            Ok(item) => serde_json::from_str::<PresetFiltersConfig>(&item.data).unwrap_or_default(),
            Err(_) => {
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, PRESET_FILTERS_ID, "{}").await;
                PresetFiltersConfig::default()
            }
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            fn load_presets(
                presets: Vec<PresetFilter>,
                ui_filter_type: UIFilterType,
            ) -> Vec<UIPresetFilter> {
                presets
                    .into_iter()
                    .filter_map(|preset| {
                        let filters: Vec<UISegmentFilter> =
                            match serde_json::from_str::<Vec<SegmentFilterData>>(&preset.filters) {
                                Ok(data) => data
                                    .into_iter()
                                    .map(|f| UISegmentFilter {
                                        ty: ui_filter_type,
                                        enabled: f.enabled,
                                        name: f.name.into(),
                                        detail: f.detail.into(),
                                    })
                                    .collect(),
                                Err(_) => return None,
                            };
                        Some(UIPresetFilter {
                            name: preset.name.into(),
                            filters: ModelRc::from(VecModel::from_slice(&filters)),
                        })
                    })
                    .collect()
            }

            let video_presets = load_presets(config.video, UIFilterType::Video);
            let audio_presets = load_presets(config.audio, UIFilterType::Audio);
            let subtitle_presets = load_presets(config.subtitle, UIFilterType::Subtitle);
            let image_presets = load_presets(config.image, UIFilterType::Image);

            ve_filter_preset_filters!(ui, video).set_vec(video_presets);
            ve_filter_preset_filters!(ui, audio).set_vec(audio_presets);
            ve_filter_preset_filters!(ui, subtitle).set_vec(subtitle_presets);
            ve_filter_preset_filters!(ui, image).set_vec(image_presets);
        });
    });
}

fn collect_preset_filters_from_ui(ui: &AppWindow) -> PresetFiltersConfig {
    fn to_db_presets(
        ui_presets: &VecModel<UIPresetFilter>,
        filter_type: &str,
    ) -> Vec<PresetFilter> {
        ui_presets
            .iter()
            .map(|preset| {
                let filters: Vec<SegmentFilterData> = preset
                    .filters
                    .iter()
                    .map(|f| SegmentFilterData {
                        enabled: f.enabled,
                        name: f.name.to_string(),
                        detail: f.detail.to_string(),
                    })
                    .collect();
                PresetFilter {
                    filter_type: filter_type.to_string(),
                    name: preset.name.to_string(),
                    filters: serde_json::to_string(&filters).unwrap_or_default(),
                }
            })
            .collect()
    }

    PresetFiltersConfig {
        video: to_db_presets(ve_filter_preset_filters!(ui, video), "video"),
        audio: to_db_presets(ve_filter_preset_filters!(ui, audio), "audio"),
        subtitle: to_db_presets(ve_filter_preset_filters!(ui, subtitle), "subtitle"),
        image: to_db_presets(ve_filter_preset_filters!(ui, image), "image"),
    }
}

fn save_preset_filters_to_db(ui: slint::Weak<AppWindow>, config: PresetFiltersConfig) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).unwrap_or_default();
        if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, PRESET_FILTERS_ID, &data).await {
            crate::logic::toast::async_toast_warn(
                ui,
                format!("{}. {e}", crate::logic::tr::tr("update entry failed")),
            );
        }
    });
}

fn load_marked_filters_from_db(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, MARKED_FILTERS_ID).await {
            Ok(item) => serde_json::from_str::<MarkedFiltersConfig>(&item.data).unwrap_or_default(),
            Err(_) => {
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, MARKED_FILTERS_ID, "{}").await;
                MarkedFiltersConfig::default()
            }
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            apply_marked_filters_to_ui(&ui, config);
        });
    });
}

fn apply_marked_filters_to_ui(ui: &AppWindow, config: MarkedFiltersConfig) {
    fn apply_marks(filters: Vec<UIFilterEntry>, marked_names: &[String]) -> Vec<UIFilterEntry> {
        let marked_set: HashSet<&str> = marked_names.iter().map(|s| s.as_str()).collect();

        let mut result: Vec<UIFilterEntry> = filters
            .iter()
            .map(|f| UIFilterEntry {
                ty: f.ty,
                name: f.name.clone(),
                is_marked: marked_set.contains(f.name.as_str()),
            })
            .collect();

        result.sort_by(|a, b| b.is_marked.cmp(&a.is_marked));
        result
    }

    let video_filters: Vec<UIFilterEntry> = all_video_filter_names()
        .iter()
        .map(|name| UIFilterEntry {
            ty: UIFilterType::Video,
            name: name.to_string().into(),
            is_marked: false,
        })
        .collect();

    let audio_filters: Vec<UIFilterEntry> = all_audio_filter_names()
        .iter()
        .map(|name| UIFilterEntry {
            ty: UIFilterType::Audio,
            name: name.to_string().into(),
            is_marked: false,
        })
        .collect();

    let subtitle_filters: Vec<UIFilterEntry> = all_subtitle_filter_names()
        .iter()
        .map(|name| UIFilterEntry {
            ty: UIFilterType::Subtitle,
            name: name.to_string().into(),
            is_marked: false,
        })
        .collect();

    let image_filters: Vec<UIFilterEntry> = all_image_filter_names()
        .iter()
        .map(|name| UIFilterEntry {
            ty: UIFilterType::Image,
            name: name.to_string().into(),
            is_marked: false,
        })
        .collect();

    let video_filters = apply_marks(video_filters, &config.video);
    let audio_filters = apply_marks(audio_filters, &config.audio);
    let subtitle_filters = apply_marks(subtitle_filters, &config.subtitle);
    let image_filters = apply_marks(image_filters, &config.image);

    global_ve_filter!(ui).set_video_filters(ModelRc::new(VecModel::from_slice(&video_filters)));
    global_ve_filter!(ui).set_audio_filters(ModelRc::new(VecModel::from_slice(&audio_filters)));
    global_ve_filter!(ui)
        .set_subtitle_filters(ModelRc::new(VecModel::from_slice(&subtitle_filters)));
    global_ve_filter!(ui).set_image_filters(ModelRc::new(VecModel::from_slice(&image_filters)));
}

fn collect_marked_filters_from_ui(ui: &AppWindow) -> MarkedFiltersConfig {
    fn get_marked_names(filters: ModelRc<UIFilterEntry>) -> Vec<String> {
        filters
            .iter()
            .filter(|f| f.is_marked)
            .map(|f| f.name.to_string())
            .collect()
    }

    MarkedFiltersConfig {
        video: get_marked_names(global_ve_filter!(ui).get_video_filters()),
        audio: get_marked_names(global_ve_filter!(ui).get_audio_filters()),
        subtitle: get_marked_names(global_ve_filter!(ui).get_subtitle_filters()),
        image: get_marked_names(global_ve_filter!(ui).get_image_filters()),
    }
}

fn save_marked_filters_to_db(ui: Weak<AppWindow>, config: MarkedFiltersConfig) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).unwrap_or_default();
        if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, MARKED_FILTERS_ID, &data).await {
            toast::async_toast_warn(
                ui,
                format!("{}. {e}", crate::logic::tr::tr("update entry failed")),
            );
        }
    });
}

pub fn save_preset_subtitle_styles_to_db(ui: Weak<AppWindow>, config: PresetSubtitleStyleConfig) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).unwrap_or_default();
        if let Err(e) =
            sqldb::entry::update(VIDEO_EDITOR_TABLE, PRESET_SUBTITLE_STYLES_ID, &data).await
        {
            toast::async_toast_warn(
                ui,
                format!("{}. {e}", crate::logic::tr::tr("update entry failed")),
            );
        }
    });
}

fn with_preset_filters<F, R>(ui: &AppWindow, filter_type: UIFilterType, f: F) -> R
where
    F: FnOnce(&VecModel<UIPresetFilter>) -> R,
{
    match filter_type {
        UIFilterType::Video => f(ve_filter_preset_filters!(ui, video)),
        UIFilterType::Audio => f(ve_filter_preset_filters!(ui, audio)),
        UIFilterType::Subtitle => f(ve_filter_preset_filters!(ui, subtitle)),
        UIFilterType::Image => f(ve_filter_preset_filters!(ui, image)),
    }
}
