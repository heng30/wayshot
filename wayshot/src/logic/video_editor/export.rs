use super::project::{EXPORT_AUDIO_CONFIG_ID, EXPORT_VIDEO_CONFIG_ID, PROJECT_STATE};
use crate::{
    config,
    db::VIDEO_EDITOR_TABLE,
    global_logic, global_store,
    logic::{toast, tr::tr},
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, MediaType as UIMediaType, SubtitleType as UISubtitleType,
        VideoEditorExportAudioConfig as UIVideoEditorExportAudioConfig,
        VideoEditorExportQueueItem as UIVideoEditorExportQueueItem,
        VideoEditorExportVideoConfig as UIVideoEditorExportVideoConfig,
    },
};
use slint::{ComponentHandle, Model, VecModel, Weak};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use video_editor::export::progress::CancellationToken;
use video_editor::export::{
    AudioExportConfig, AudioExporter, Mp4ExportConfig, Mp4Exporter, SubtitleExportConfig,
    SubtitleExporter,
};

use super::common_type::{VideoEditorExportAudioConfig, VideoEditorExportVideoConfig};

static NEXT_TASK_ID: AtomicUsize = AtomicUsize::new(0);
static CANCELLATION_TOKENS: LazyLock<Mutex<HashMap<usize, CancellationToken>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[macro_export]
macro_rules! store_video_editor_export_queue {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_export_queue()
            .as_any()
            .downcast_ref::<VecModel<UIVideoEditorExportQueueItem>>()
            .expect("We know we set a VecModel<UIVideoEditorExportQueueItem> earlier")
    };
}

fn db_update_export_video_config(ui: Weak<AppWindow>, config: VideoEditorExportVideoConfig) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("Failed to serialize export video config");
        if let Err(e) =
            sqldb::entry::update(VIDEO_EDITOR_TABLE, EXPORT_VIDEO_CONFIG_ID, &data).await
        {
            toast::async_toast_warn(ui, format!("{}. {e}", tr("update entry failed")));
        }
    });
}

fn db_update_export_audio_config(ui: Weak<AppWindow>, config: VideoEditorExportAudioConfig) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("Failed to serialize export audio config");
        if let Err(e) =
            sqldb::entry::update(VIDEO_EDITOR_TABLE, EXPORT_AUDIO_CONFIG_ID, &data).await
        {
            toast::async_toast_warn(ui, format!("{}. {e}", tr("update entry failed")));
        }
    });
}

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_export_video, ui, config);
    logic_cb!(video_editor_export_audio, ui, config);
    logic_cb!(video_editor_export_subtitle, ui, ty);
    logic_cb!(video_editor_export_queue_cancel, ui, index);
    logic_cb!(video_editor_export_queue_remove, ui, index);

    // Restore export video config
    let ui_weak_for_export_video = ui.as_weak();
    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, EXPORT_VIDEO_CONFIG_ID).await {
            Ok(setting) => serde_json::from_str::<VideoEditorExportVideoConfig>(&setting.data)
                .unwrap_or_default(),
            _ => {
                let config = VideoEditorExportVideoConfig::default();
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, EXPORT_VIDEO_CONFIG_ID, "{}").await;
                config
            }
        };

        _ = ui_weak_for_export_video.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_export_video_config(config.into());
        });
    });

    // Restore export audio config
    let ui_weak_for_export_audio = ui.as_weak();
    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, EXPORT_AUDIO_CONFIG_ID).await {
            Ok(setting) => serde_json::from_str::<VideoEditorExportAudioConfig>(&setting.data)
                .unwrap_or_default(),
            _ => {
                let config = VideoEditorExportAudioConfig::default();
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, EXPORT_AUDIO_CONFIG_ID, "{}").await;
                config
            }
        };

        _ = ui_weak_for_export_audio.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_export_audio_config(config.into());
        });
    });
}

fn video_editor_export_video(ui: &AppWindow, config: UIVideoEditorExportVideoConfig) {
    let ui_weak = ui.as_weak();
    let config_for_save: VideoEditorExportVideoConfig = config.clone().into();
    db_update_export_video_config(ui.as_weak(), config_for_save);

    let default_name = {
        let state_guard = PROJECT_STATE.lock().unwrap();
        let state = state_guard.as_ref().unwrap();
        state
            .current_project_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("video")
            .to_string()
    };

    let fps = global_logic!(ui).invoke_fps_to_int(config.fps) as u32;
    let channels = global_logic!(ui).invoke_audio_channels_to_int(config.channels) as u16;
    let sample_rate = global_logic!(ui).invoke_audio_sample_rate_to_int(config.sample_rate) as u32;
    let low_memory_mode = config.low_memory_mode;

    let resolution: Option<(u32, u32)> = config.resolution.into();
    let (width, height) = match resolution {
        Some((w, h)) => (Some(w), Some(h)),
        None => (None, None),
    };

    tokio::spawn(async move {
        let output_path = match picker_save_file(
            ui_weak.clone(),
            &tr("Export Video"),
            &tr("MP4 Video"),
            &["mp4"],
            &format!("{}.mp4", default_name),
        ) {
            Some(path) => path,
            _ => return,
        };

        let file_name = output_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("video.mp4")
            .to_string();

        let manager = {
            let state_guard = PROJECT_STATE.lock().unwrap();
            let state = state_guard.as_ref().unwrap();
            state.tracks_manager.clone()
        };

        let task_id = next_export_task_id();
        let cancellation_token = CancellationToken::new();

        let export_config = Mp4ExportConfig::default()
            .with_output_path(output_path.clone())
            .with_width(width)
            .with_height(height)
            .with_fps(Some(fps))
            .with_audio_channels(Some(channels))
            .with_audio_sample_rate(Some(sample_rate))
            .with_burn_subtitles(true)
            .with_low_memory_mode(low_memory_mode)
            .with_cancellation_token(Some(cancellation_token.clone()));

        let task = UIVideoEditorExportQueueItem {
            id: format!("Video [{}]", task_id).into(),
            name: file_name.into(),
            media_type: UIMediaType::Video,
            progress: 0.0,
            is_cancelled: false,
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            store_video_editor_export_queue!(ui).push(task);
        });

        CANCELLATION_TOKENS
            .lock()
            .unwrap()
            .insert(task_id, cancellation_token.clone());

        let ui_weak_for_progress = ui_weak.clone();
        let result = tokio::task::spawn_blocking(move || {
            let exporter = Mp4Exporter::new(manager, export_config);
            exporter.export_with_progress(move |progress| {
                update_export_task_progress(&ui_weak_for_progress, task_id, progress.progress());
            })
        })
        .await;

        CANCELLATION_TOKENS.lock().unwrap().remove(&task_id);

        match result {
            Ok(Ok(_)) => {
                update_export_task_progress(&ui_weak, task_id, 1.0);
                toast::async_toast_success(ui_weak, tr("Video export completed"));
            }
            Ok(Err(e)) => {
                toast::async_toast_warn(ui_weak, format!("{}: {}", tr("Video export failed"), e))
            }
            Err(_) => toast::async_toast_warn(ui_weak, tr("Video export was cancelled")),
        }
    });
}

fn video_editor_export_audio(ui: &AppWindow, config: UIVideoEditorExportAudioConfig) {
    let ui_weak = ui.as_weak();
    let config_for_save: VideoEditorExportAudioConfig = config.clone().into();
    db_update_export_audio_config(ui.as_weak(), config_for_save);

    let default_name = {
        let state_guard = PROJECT_STATE.lock().unwrap();
        let state = state_guard.as_ref().unwrap();
        state
            .current_project_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("audio")
            .to_string()
    };

    let ext = global_logic!(ui)
        .invoke_audio_format_to_str(config.format)
        .to_ascii_lowercase();
    let format = config.format.into();
    let channels = global_logic!(ui).invoke_audio_channels_to_int(config.channels) as u16;
    let sample_rate = global_logic!(ui).invoke_audio_sample_rate_to_int(config.sample_rate) as u32;

    tokio::spawn(async move {
        let output_path = match picker_save_file(
            ui_weak.clone(),
            &tr("Export Audio"),
            &tr("Audio File"),
            &[&ext],
            &format!("{}.{}", default_name, ext),
        ) {
            Some(path) => path,
            _ => return,
        };

        let file_name = output_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("audio.mp3")
            .to_string();

        let manager = {
            let state_guard = PROJECT_STATE.lock().unwrap();
            let state = state_guard.as_ref().unwrap();
            Arc::new(state.tracks_manager.clone())
        };

        let task_id = next_export_task_id();
        let cancellation_token = CancellationToken::new();

        let export_config = AudioExportConfig::default()
            .with_output_path(output_path.clone())
            .with_format(format)
            .with_channels(Some(channels))
            .with_sample_rate(Some(sample_rate))
            .with_cancellation_token(Some(cancellation_token.clone()));

        let task = UIVideoEditorExportQueueItem {
            id: format!("Audio [{}]", task_id).into(),
            name: file_name.into(),
            media_type: UIMediaType::Audio,
            progress: 0.0,
            is_cancelled: false,
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            store_video_editor_export_queue!(ui).push(task);
        });

        CANCELLATION_TOKENS
            .lock()
            .unwrap()
            .insert(task_id, cancellation_token.clone());

        let ui_weak_for_progress = ui_weak.clone();
        let result = tokio::task::spawn_blocking(move || {
            let exporter = AudioExporter::new(manager, export_config);
            exporter.export_with_progress(move |progress| {
                update_export_task_progress(&ui_weak_for_progress, task_id, progress.progress());
            })
        })
        .await;

        CANCELLATION_TOKENS.lock().unwrap().remove(&task_id);

        match result {
            Ok(Ok(_)) => {
                update_export_task_progress(&ui_weak, task_id, 1.0);
                toast::async_toast_success(ui_weak, tr("Audio export completed"));
            }
            Ok(Err(e)) => {
                toast::async_toast_warn(ui_weak, format!("{}: {}", tr("Audio export failed"), e))
            }
            Err(_) => toast::async_toast_warn(ui_weak, tr("Audio export was cancelled")),
        }
    });
}

fn video_editor_export_subtitle(ui: &AppWindow, ty: UISubtitleType) {
    let ui_weak = ui.as_weak();

    let default_name = {
        let state_guard = PROJECT_STATE.lock().unwrap();
        let state = state_guard.as_ref().unwrap();
        state
            .current_project_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("subtitle")
            .to_string()
    };

    let format = ty.into();
    let ext = global_logic!(ui).invoke_subtitle_to_str(ty);

    tokio::spawn(async move {
        let output_path = match picker_save_file(
            ui_weak.clone(),
            &tr("Export Subtitle"),
            &tr("Subtitle File"),
            &[&ext],
            &format!("{}.{}", default_name, ext),
        ) {
            Some(path) => path,
            _ => return,
        };

        let file_name = output_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("subtitle.srt")
            .to_string();

        let manager = {
            let state_guard = PROJECT_STATE.lock().unwrap();
            let state = state_guard.as_ref().unwrap();
            Arc::new(state.tracks_manager.clone())
        };

        let task_id = next_export_task_id();
        let cancellation_token = CancellationToken::new();

        let export_config = SubtitleExportConfig::default()
            .with_output_base_path(output_path.clone())
            .with_format(format)
            .with_cancellation_token(Some(cancellation_token.clone()));

        let task = UIVideoEditorExportQueueItem {
            name: file_name.into(),
            id: format!("Subtitle [{}]", task_id).into(),
            media_type: UIMediaType::Subtitle,
            progress: 0.0,
            is_cancelled: false,
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            store_video_editor_export_queue!(ui).push(task);
        });

        CANCELLATION_TOKENS
            .lock()
            .unwrap()
            .insert(task_id, cancellation_token.clone());

        let ui_weak_for_progress = ui_weak.clone();
        let result = tokio::task::spawn_blocking(move || {
            let exporter = SubtitleExporter::new(manager, export_config);
            update_export_task_progress(&ui_weak_for_progress, task_id, 0.5);
            let result = exporter.export_all_tracks();
            update_export_task_progress(&ui_weak_for_progress, task_id, 1.0);
            result
        })
        .await;

        CANCELLATION_TOKENS.lock().unwrap().remove(&task_id);

        match result {
            Ok(Ok(_)) => toast::async_toast_success(ui_weak, tr("Subtitle export completed")),
            Ok(Err(e)) => {
                toast::async_toast_warn(ui_weak, format!("{}: {}", tr("Subtitle export failed"), e))
            }
            Err(_) => toast::async_toast_warn(ui_weak, tr("Subtitle export was cancelled")),
        }
    });
}

fn video_editor_export_queue_cancel(ui: &AppWindow, index: i32) {
    if let Some(mut task) = store_video_editor_export_queue!(ui).row_data(index as usize) {
        if let Some(task_id_num) = extract_task_id(&task.id)
            && let Some(token) = CANCELLATION_TOKENS.lock().unwrap().get(&task_id_num)
        {
            token.cancel();
        }

        task.is_cancelled = true;
        store_video_editor_export_queue!(ui).set_row_data(index as usize, task);
        crate::toast_info!(ui, tr("Export cancelled"));
    }
}

fn video_editor_export_queue_remove(ui: &AppWindow, index: i32) {
    if index >= 0 && (index as usize) < store_video_editor_export_queue!(ui).row_count() {
        if let Some(task) = store_video_editor_export_queue!(ui).row_data(index as usize)
            && let Some(task_id_num) = extract_task_id(&task.id)
            && let Some(token) = CANCELLATION_TOKENS.lock().unwrap().get(&task_id_num)
        {
            token.cancel();
            CANCELLATION_TOKENS.lock().unwrap().remove(&task_id_num);
        }

        store_video_editor_export_queue!(ui).remove(index as usize);
        crate::toast_success!(ui, tr("Export removed from queue"));
    }
}

// Extract task ID from a string like "Video [1]" or "Audio [42]"
fn extract_task_id(id_str: &str) -> Option<usize> {
    let start = id_str.find('[')?;
    let end = id_str.find(']')?;
    let num_str = &id_str[start + 1..end];
    num_str.parse().ok()
}

pub fn picker_save_file(
    ui: Weak<AppWindow>,
    title: &str,
    filter_name: &str,
    filter_extensions: &[&str],
    default_name: &str,
) -> Option<PathBuf> {
    let mut file_dialog = native_dialog::DialogBuilder::file()
        .set_title(title)
        .set_filename(default_name);

    if !filter_extensions.is_empty() {
        file_dialog = file_dialog.add_filter(filter_name, filter_extensions);
    }

    let result = file_dialog.save_single_file().show();

    match result {
        Ok(Some(path)) => {
            if let Some(parent) = path.parent()
                && !parent.exists()
            {
                toast::async_toast_warn(ui, tr("Parent directory does not exist"));
                return None;
            }

            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if !is_filename_valid(filename) {
                toast::async_toast_warn(ui, tr("Invalid filename"));
                return None;
            }
            Some(path)
        }
        Err(e) => {
            toast::async_toast_warn(
                ui,
                format!("{}. {}: {}", tr("Save file failed"), tr("Reason"), e),
            );
            None
        }
        _ => None,
    }
}

pub fn update_export_task_progress(ui: &Weak<AppWindow>, task_id: usize, progress: f32) {
    _ = ui.upgrade_in_event_loop(move |ui| {
        for (index, task) in store_video_editor_export_queue!(ui).iter().enumerate() {
            if task.id.to_string().contains(&format!("[{}]", task_id)) {
                let mut updated = task.clone();
                updated.progress = progress;
                store_video_editor_export_queue!(ui).set_row_data(index, updated);
                break;
            }
        }
    });
}

pub async fn add_export_task(
    ui: &Weak<AppWindow>,
    task_id: usize,
    name: String,
    media_type: UIMediaType,
) {
    let media_type_str = match media_type {
        UIMediaType::Video => "Video",
        UIMediaType::Audio => "Audio",
        UIMediaType::Image => "Image",
        UIMediaType::Subtitle => "Subtitle",
        UIMediaType::Text => "Text",
    };

    let task = UIVideoEditorExportQueueItem {
        id: format!("{} [{}]", media_type_str, task_id).into(),
        name: name.into(),
        media_type,
        progress: 0.0,
        is_cancelled: false,
    };

    _ = ui.upgrade_in_event_loop(move |ui| {
        store_video_editor_export_queue!(ui).push(task);
    });
}

fn is_filename_valid(filename: &str) -> bool {
    if filename.is_empty() {
        return false;
    }

    let cache_dir = config::all().cache_dir;
    let test_path = cache_dir.join(filename);

    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&test_path)
        .map(|_| {
            std::fs::remove_file(&test_path).ok();
            true
        })
        .is_ok()
}

pub fn next_export_task_id() -> usize {
    NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_cancellation_token(task_id: usize, token: CancellationToken) {
    CANCELLATION_TOKENS.lock().unwrap().insert(task_id, token);
}

pub fn remove_cancellation_token(task_id: usize) {
    CANCELLATION_TOKENS.lock().unwrap().remove(&task_id);
}
