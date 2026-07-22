use super::{
    command::with_history_manager, common_type::VideoEditorPreviewConfig,
    conversion::rgba_image_to_slint_image, filters::refresh_selected_filter_detail_at_playhead,
    project::db_update_preview_config, track::get_selected_segment_indices,
};
use crate::{
    global_store, global_ve_filter,
    logic::tr::tr,
    logic::video_editor::conversion::layer_frame_to_ui,
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, Resolution as UIResolution, VideoEditorLayerImage as UIVideoEditorLayerImage,
        VideoEditorNewProjectConfig as UIVideoEditorNewProjectConfig,
        VideoEditorPreviewConfig as UIVideoEditorPreviewConfig,
        VideoEditorVolume as UIVideoEditorVolume, VideoPreviewSize as UIVideoPreviewSize,
    },
};
use slint::{ComponentHandle, Model, SharedString, VecModel, Weak};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};
use video_editor::{
    preview::PreviewRenderer,
    tracks::{track::Track, unified_mixer::UnifiedMixerConfig},
};

static PENDING_SEEK_ID: AtomicU32 = AtomicU32::new(0);
static PLAYBACK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static CURRENT_RENDERER: Mutex<Option<PreviewRenderer>> = Mutex::new(None);

#[macro_export]
macro_rules! store_video_editor_preview_layer_images {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_preview_layer_images()
            .as_any()
            .downcast_ref::<VecModel<UIVideoEditorLayerImage>>()
            .expect("We know we set a VecModel<UIVideoEditorLayerImage> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_preview_show_dialog, ui);
    logic_cb!(video_editor_preview_update_config, ui, config);
    logic_cb!(video_editor_preview_jump_previous_segment, ui);
    logic_cb!(video_editor_preview_jump_next_segment, ui);
    logic_cb!(video_editor_preview_jump_to_segment_start, ui);
    logic_cb!(video_editor_preview_jump_to_segment_end, ui);
    logic_cb!(video_editor_preview_rewind, ui);
    logic_cb!(video_editor_preview_fast_forward, ui);
    logic_cb!(video_editor_preview_toggle, ui);
    logic_cb!(video_editor_preview_change_volume, ui, value);
    logic_cb!(video_editor_preview_seek, ui, position_ms);
    logic_cb!(video_editor_timeline_seek, ui, position_ms);
    logic_cb!(
        video_editor_calc_timeline_offset,
        ui,
        index,
        total_index,
        _flag
    );
    logic_cb!(video_editor_preview_size, ui, config, _flag, _flag2);
    logic_cb!(selected_segment_under_playhead, ui, timeline_offset, _flag);
}

fn inner_init(ui: &AppWindow) {
    store_video_editor_preview_layer_images!(ui).set_vec(vec![]);
}

fn video_editor_preview_show_dialog(ui: &AppWindow) {
    global_store!(ui).set_video_editor_is_show_preview_setting_dialog(true);
}

fn video_editor_preview_update_config(ui: &AppWindow, config: UIVideoEditorPreviewConfig) {
    global_store!(ui).set_video_editor_new_project_config(UIVideoEditorNewProjectConfig {
        preview_config: config.clone(),
        ..global_store!(ui).get_video_editor_new_project_config()
    });
    global_store!(ui).set_video_editor_is_show_preview_setting_dialog(false);
    global_store!(ui).set_video_editor_is_unsaved(true);

    let preview_config: VideoEditorPreviewConfig = config.into();
    db_update_preview_config(ui.as_weak(), preview_config);
}

fn video_editor_preview_jump_previous_segment(ui: &AppWindow) {
    let current_offset = global_store!(ui).get_video_editor_timeline_offset();
    let current_position = Duration::from_millis(current_offset as u64);

    let prev_segment_start = with_history_manager(|state| {
        let mut prev_start = None;

        'out: for track in state.tracks_manager.iter() {
            for segment in track.segments().iter().rev() {
                if segment.timeline_offset < current_position {
                    prev_start = Some(segment.timeline_offset);
                    break 'out;
                }
            }
        }

        prev_start
    });

    if let Some(offset) = prev_segment_start {
        seek_to_position(ui, offset, true);
        update_edit_mode_for_seek(ui, offset);
        crate::toast_info!(ui, tr("Jump to previous segment"));
    } else {
        crate::toast_info!(ui, tr("Already at start"));
    }
}

fn video_editor_preview_jump_next_segment(ui: &AppWindow) {
    let current_offset = global_store!(ui).get_video_editor_timeline_offset();
    let current_position = Duration::from_millis(current_offset as u64);

    let next_segment_start = with_history_manager(|state| {
        let mut next_start = None;

        'out: for track in state.tracks_manager.iter() {
            for segment in track.segments() {
                if segment.timeline_offset > current_position {
                    next_start = Some(segment.timeline_offset);
                    break 'out;
                }
            }
        }

        next_start
    });

    if let Some(offset) = next_segment_start {
        seek_to_position(ui, offset, true);
        update_edit_mode_for_seek(ui, offset);
        crate::toast_info!(ui, tr("Jump to next segment"));
    } else {
        crate::toast_info!(ui, tr("No next segment"));
    }
}

fn video_editor_preview_jump_to_segment_start(ui: &AppWindow) {
    let edited_track_index = global_store!(ui).get_video_editor_current_edited_track_index();
    if edited_track_index < 0 {
        return;
    }

    let current_offset = global_store!(ui).get_video_editor_timeline_offset();
    let current_position = Duration::from_millis(current_offset as u64);

    let segment_start = with_history_manager(|state| {
        if let Some(track) = state.tracks_manager.iter().nth(edited_track_index as usize) {
            for segment in track.segments().iter() {
                let seg_start = segment.timeline_offset;
                let seg_end = seg_start + segment.duration;
                if current_position >= seg_start && current_position < seg_end {
                    return Some(seg_start);
                }
            }
        }
        None
    });

    if let Some(offset) = segment_start {
        seek_to_position(ui, offset, true);
        update_edit_mode_for_seek(ui, offset);
    }
}

fn video_editor_preview_jump_to_segment_end(ui: &AppWindow) {
    let edited_track_index = global_store!(ui).get_video_editor_current_edited_track_index();
    if edited_track_index < 0 {
        return;
    }

    let current_offset = global_store!(ui).get_video_editor_timeline_offset();
    let current_position = Duration::from_millis(current_offset as u64);

    let segment_end = with_history_manager(|state| {
        if let Some(track) = state.tracks_manager.iter().nth(edited_track_index as usize) {
            for segment in track.segments().iter() {
                let seg_start = segment.timeline_offset;
                let seg_end = seg_start + segment.duration;
                if current_position >= seg_start && current_position < seg_end {
                    return Some(seg_end);
                }
            }
        }
        None
    });

    if let Some(offset) = segment_end {
        seek_to_position(ui, offset, true);
        update_edit_mode_for_seek(ui, offset);
    }
}

fn video_editor_preview_rewind(ui: &AppWindow) {
    let position_ms = global_store!(ui).get_video_editor_timeline_offset();
    let new_position =
        Duration::from_millis(position_ms as u64).saturating_sub(Duration::from_secs(5));
    seek_to_position(ui, new_position, true);
    update_edit_mode_for_seek(ui, new_position);
}

fn video_editor_preview_fast_forward(ui: &AppWindow) {
    let position_ms = global_store!(ui).get_video_editor_timeline_offset();
    let new_position =
        Duration::from_millis(position_ms as u64).saturating_add(Duration::from_secs(5));
    seek_to_position(ui, new_position, true);
    update_edit_mode_for_seek(ui, new_position);
}

fn video_editor_preview_toggle(ui: &AppWindow) {
    let thread_id = PLAYBACK_THREAD_ID.fetch_add(1, Ordering::SeqCst) + 1;
    let is_previewing = global_store!(ui).get_video_editor_is_previewing();

    if is_previewing {
        stop_playback();
        global_store!(ui).set_video_editor_is_previewing(false);

        let position_ms = global_store!(ui).get_video_editor_timeline_offset();
        let position = Duration::from_millis(position_ms as u64);
        update_edit_mode_for_seek(ui, position);
    } else {
        let ui_state = global_store!(ui).get_video_editor_ui_state();
        let volume = ui_state.preview_volume.clamp(0.0, 100.0) / 100.0;
        let position_ms = global_store!(ui).get_video_editor_timeline_offset();
        let position = Duration::from_millis(position_ms as u64);

        if let Some(mut renderer) = create_renderer(ui, position, volume) {
            if let Err(e) = renderer.play() {
                crate::toast_warn!(ui, format!("{}: {:?}", tr("Failed to start playback"), e));
                return;
            }

            {
                let mut guard = CURRENT_RENDERER.lock().unwrap();
                *guard = Some(renderer);
            }

            global_store!(ui).set_video_editor_is_previewing(true);
            start_playback_update_thread(ui.as_weak(), thread_id);
        }
    }
}

fn video_editor_preview_change_volume(_ui: &AppWindow, value: f32) {
    let value = value.clamp(0.0, 100.0) / 100.0;
    let mut guard = CURRENT_RENDERER.lock().unwrap();
    if let Some(ref mut renderer) = *guard {
        renderer.set_volume(value);
    }
}

fn video_editor_preview_seek(ui: &AppWindow, position_ms: i32) {
    let position = Duration::from_millis(position_ms as u64);
    seek_to_position(ui, position, false);
    update_edit_mode_for_seek(ui, position);
}

fn video_editor_timeline_seek(ui: &AppWindow, position_ms: i32) {
    let position = Duration::from_millis(position_ms as u64);
    seek_to_position(ui, position, true);
    update_edit_mode_for_seek(ui, position);
}

fn update_edit_mode_for_seek(ui: &AppWindow, position: Duration) {
    let selected_filter = global_ve_filter!(ui).get_selected_filter();

    if !global_ve_filter!(ui).invoke_is_edit_mode_filter(selected_filter.name) {
        global_ve_filter!(ui).set_is_in_edit_mode(false);
        return;
    }

    let edited_track_index = global_store!(ui).get_video_editor_current_edited_track_index();
    if edited_track_index >= 0 {
        let supports_edit_mode = with_history_manager(|state| {
            state
                .tracks_manager
                .iter()
                .nth(edited_track_index as usize)
                .map_or(false, |track| {
                    matches!(track, Track::Video(_) | Track::Audio(_) | Track::Image(_))
                })
        });
        if !supports_edit_mode {
            global_ve_filter!(ui).set_is_in_edit_mode(false);
            return;
        }
    }

    let edited_segment_at_position = if edited_track_index >= 0 {
        with_history_manager(|state| {
            if let Some(track) = state.tracks_manager.iter().nth(edited_track_index as usize) {
                for (seg_idx, segment) in track.segments().iter().enumerate() {
                    let seg_start = segment.timeline_offset;
                    let seg_end = seg_start + segment.duration;
                    if position >= seg_start && position < seg_end {
                        return Some((edited_track_index as usize, seg_idx));
                    }
                }
            }
            None
        })
    } else {
        None
    };

    // Check if the edited segment is selected
    let is_edit_mode = if let Some((track_idx, seg_idx)) = edited_segment_at_position {
        let selected_segments = get_selected_segment_indices(ui);
        selected_segments.contains(&(track_idx, seg_idx))
    } else {
        false // Edited track has no segment at this position
    };

    global_ve_filter!(ui).set_is_in_edit_mode(is_edit_mode);
}

fn video_editor_calc_timeline_offset(
    _ui: &AppWindow,
    index: i32,
    total_index: f32,
    _flag: bool,
) -> SharedString {
    if total_index <= 0.0 {
        return "00:00:00,000".into();
    }

    let duration_ms =
        with_history_manager(|state| state.tracks_manager.duration.as_millis() as i32);

    if duration_ms <= 0 {
        return "00:00:00,000".into();
    }

    let time_ms = duration_ms as f32 * index as f32 / total_index as f32;
    video_utils::subtitle::ms_to_srt_timestamp(time_ms as u64).into()
}

fn video_editor_preview_size(
    _ui: &AppWindow,
    config: UIVideoEditorPreviewConfig,
    _flag: bool,
    _flag2: i32,
) -> UIVideoPreviewSize {
    if config.resolution == UIResolution::Original {
        let size = with_history_manager(|state| {
            for track in state.tracks_manager.iter() {
                if let Track::Video(video_track) = track
                    && let Some(video_meta) = video_track.track.metadata.videos.first()
                {
                    return (video_meta.width as i32, video_meta.height as i32).into();
                }
            }

            (1920, 1080).into()
        });
        return size;
    }

    let resolution: Option<(u32, u32)> = config.resolution.into();
    resolution
        .map(|(w, h)| (w as i32, h as i32))
        .unwrap_or((1920, 1080))
        .into()
}

fn selected_segment_under_playhead(ui: &AppWindow, timeline_offset: i32, _flag: bool) -> bool {
    let selected_segments = get_selected_segment_indices(ui);
    if selected_segments.is_empty() {
        return false;
    }

    let position = Duration::from_millis(timeline_offset as u64);

    with_history_manager(|state| {
        for (track_idx, seg_idx) in selected_segments.iter() {
            if let Some(track) = state.tracks_manager.iter().nth(*track_idx)
                && let Some(segment) = track.segments().iter().nth(*seg_idx)
            {
                let seg_start = segment.timeline_offset;
                let seg_end = seg_start + segment.duration;
                if position >= seg_start && position < seg_end {
                    return true;
                }
            }
        }
        false
    })
}

fn create_renderer(ui: &AppWindow, position: Duration, volume: f32) -> Option<PreviewRenderer> {
    let manager = with_history_manager(|state| Arc::new(state.tracks_manager.clone()));
    let mut mixer_config: UnifiedMixerConfig = global_store!(ui)
        .get_video_editor_new_project_config()
        .preview_config
        .into();

    let cache_config = global_store!(ui).get_video_editor_preference_config().cache;
    mixer_config.max_cache_duration =
        Duration::from_secs(cache_config.max_cache_duration.max(3) as u64);

    let mut renderer = PreviewRenderer::new(manager, mixer_config.into());
    renderer.set_volume(volume);

    if let Err(e) = renderer.seek(position) {
        log::error!("Failed to seek to position: {:?}", e);
    }

    Some(renderer)
}

fn stop_playback() {
    let mut guard = CURRENT_RENDERER.lock().unwrap();
    if let Some(ref mut renderer) = *guard {
        renderer.stop();
    }
    *guard = None;
}

pub fn seek_to_position(ui: &AppWindow, position: Duration, need_refresh: bool) {
    let was_previewing = global_store!(ui).get_video_editor_is_previewing();
    if was_previewing {
        global_store!(ui).set_video_editor_is_previewing(false);
    }

    stop_playback();

    global_store!(ui).set_video_editor_timeline_offset(position.as_millis() as i32);

    let ui_weak = ui.as_weak();
    let seek_id = PENDING_SEEK_ID.fetch_add(1, Ordering::SeqCst) + 1;
    let thread_id = PLAYBACK_THREAD_ID.fetch_add(1, Ordering::SeqCst) + 1;
    let manager = with_history_manager(|state| Arc::new(state.tracks_manager.clone()));
    let mut mixer_config: UnifiedMixerConfig = global_store!(ui)
        .get_video_editor_new_project_config()
        .preview_config
        .into();

    let cache_config = global_store!(ui).get_video_editor_preference_config().cache;
    mixer_config.max_cache_duration =
        Duration::from_secs(cache_config.max_cache_duration.max(3) as u64);

    let ui_state = global_store!(ui).get_video_editor_ui_state();
    let volume = ui_state.preview_volume.clamp(0.0, 100.0) / 100.0;

    tokio::task::spawn_blocking(move || {
        let mut renderer = PreviewRenderer::new(manager, mixer_config.into());
        renderer.set_volume(volume);

        if let Err(e) = renderer.seek(position) {
            log::error!("Failed to seek: {:?}", e);
            return;
        }

        if PENDING_SEEK_ID.load(Ordering::SeqCst) != seek_id {
            return;
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if PENDING_SEEK_ID.load(Ordering::SeqCst) != seek_id {
                return;
            }

            // 获取字幕和各个图层图片，保存到Store中，用于UI编辑
            if let Some(layer_frames) = renderer.current_layerframe() {
                let img = rgba_image_to_slint_image(&layer_frames.composited_image);
                global_store!(ui).set_video_editor_preview_image(img);

                let layers: Vec<_> = layer_frames
                    .layers
                    .iter()
                    .filter_map(|layer| layer_frame_to_ui(layer))
                    .collect();
                store_video_editor_preview_layer_images!(ui).set_vec(layers);
            } else {
                global_store!(ui).set_video_editor_preview_image(Default::default());
                store_video_editor_preview_layer_images!(ui).set_vec(vec![]);
            }

            let mut guard = CURRENT_RENDERER.lock().unwrap();
            *guard = Some(renderer);

            if was_previewing && let Some(ref mut renderer) = *guard {
                match renderer.play() {
                    Err(e) => log::error!("Failed to resume playback: {:?}", e),
                    _ => {
                        global_store!(ui).set_video_editor_is_previewing(true);
                        start_playback_update_thread(ui.as_weak(), thread_id);
                    }
                }
            }

            if need_refresh {
                refresh_selected_filter_detail_at_playhead(&ui);
            }
        });
    });
}

fn start_playback_update_thread(ui_weak: Weak<AppWindow>, thread_id: u32) {
    std::thread::spawn(move || {
        let frame_rate = {
            let guard = CURRENT_RENDERER.lock().unwrap();
            guard
                .as_ref()
                .map(|r| r.frame_rate() as f32)
                .unwrap_or(25.0)
        };

        let frame_duration = Duration::from_secs_f64(1.0 / frame_rate as f64);
        let sleeper = spin_sleep::SpinSleeper::default();
        let start_time = std::time::Instant::now();
        let mut frame_index: u64 = 0;
        let mut last_filter_refresh = std::time::Instant::now();

        loop {
            if PLAYBACK_THREAD_ID.load(Ordering::SeqCst) != thread_id {
                break;
            }

            let (is_playing, position, layer_frames, db_level) = {
                let mut guard = CURRENT_RENDERER.lock().unwrap();
                if let Some(ref mut renderer) = *guard {
                    if let Err(e) = renderer.update() {
                        log::error!("Failed to update renderer: {:?}", e);
                    }

                    (
                        renderer.is_playing(),
                        renderer.position(),
                        renderer.current_layerframe().map(|lf| lf.clone()),
                        renderer.current_db_level(),
                    )
                } else {
                    break;
                }
            };

            if !is_playing {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_video_editor_is_previewing(false);
                    update_edit_mode_for_seek(&ui, position);
                });
                break;
            }

            let should_refresh_filter = last_filter_refresh.elapsed() >= Duration::from_millis(200);
            if should_refresh_filter {
                last_filter_refresh = std::time::Instant::now();
            }

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                if PLAYBACK_THREAD_ID.load(Ordering::SeqCst) != thread_id {
                    return;
                }

                if let Some(layer_frames) = layer_frames {
                    let img = rgba_image_to_slint_image(&layer_frames.composited_image);
                    global_store!(ui).set_video_editor_preview_image(img);

                    let layers: Vec<_> = layer_frames
                        .layers
                        .iter()
                        .filter_map(|layer| layer_frame_to_ui(layer))
                        .collect();
                    store_video_editor_preview_layer_images!(ui).set_vec(layers);
                }

                global_store!(ui).set_video_editor_timeline_offset(position.as_millis() as i32);
                global_store!(ui).set_video_editor_preview_volume(UIVideoEditorVolume {
                    left: db_level.0,
                    right: db_level.1,
                });

                if should_refresh_filter {
                    refresh_selected_filter_detail_at_playhead(&ui);
                }
            });

            frame_index += 1;
            let target_time = start_time + frame_duration * frame_index as u32;
            sleeper.sleep_until(target_time);
        }

        {
            let mut guard = CURRENT_RENDERER.lock().unwrap();
            *guard = None;
        }
    });
}
