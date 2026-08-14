use super::{
    bookmark::sync_bookmarks_to_ui,
    common_type::{
        VideoEditorPreferenceConfig, VideoEditorPreferenceMcpConfig, VideoEditorPreviewConfig,
        VideoEditorUIState,
    },
    library::refresh_library_ui,
    media_list_common::{MediaListContext, refresh_ui},
    segment::{refresh_affected_segments, refresh_all_segment_audio_samples},
};
use crate::{
    config,
    db::VIDEO_EDITOR_TABLE,
    global_logic, global_store, global_util,
    logic::{
        share_screen::picker_file,
        toast,
        tr::tr,
        video_editor::{export::picker_save_file, mcp},
    },
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, ChapterSummaryEntry as UIChapterSummaryEntry,
        ChapterSummaryProgressType as UIChapterSummaryProgressType,
        VideoEditorNewProjectConfig as UIVideoEditorNewProjectConfig,
        VideoEditorPlaylistItem as UIVideoEditorPlaylistItem,
        VideoEditorPreferenceCacheConfig as UIVideoEditorPreferenceCacheConfig,
        VideoEditorPreferenceMcpConfig as UIVideoEditorPreferenceMcpConfig,
        VideoEditorPreferenceTrackConfig as UIVideoEditorPreferenceTrackConfig,
        VideoEditorRecentEntry as UIVideoEditorRecentEntry,
        VideoEditorRecoveryInfo as UIVideoEditorRecoveryInfo,
        VideoEditorTracksManager as UIVideoEditorTracksManager,
    },
    toast_success, toast_warn,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use video_editor::{
    commands::{AffectedSegments, HistoryManager},
    export::progress::CancellationToken,
    media::MediaList,
    metadata::Metadata,
    preview::clear_global_audio_display_cache,
    project::{
        AutoSaveConfig, AutoSaveHandle, AutoSaveManager, BookmarkData, ChapterSummaryData,
        ProjectFile,
        autosave::{
            RecoveryInfo, check_for_recovery, cleanup_recovery_file, cleanup_temp_files_by_path,
            restore_from_recovery,
        },
        project::{ManagerData, ProjectPreviewConfig},
        recent::RecentFilesManager,
    },
    tracks::{
        audio_track::AudioTrack,
        clear_global_cache,
        image_track::ImageTrack,
        manager::Manager,
        segment::Segment,
        set_global_cache_max_frames,
        subtitle_track::SubtitleTrack,
        track::{InnerTrack, Track},
        video_track::VideoTrack,
    },
};

pub const PROJECT_EXT: &str = "wayshot";
pub const RECENT_ID: &str = "video_editor_recent";
pub const UI_STATE_ID: &str = "video_editor_ui_state";
pub const LIBRARY_ID: &str = "video_editor_library";
pub const TTS_MARKED_VOICE_ID: &str = "video_editor_tts_marked_voices";
pub const TTS_IMPORTED_VOICE_ID: &str = "video_editor_tts_imported_voices";
pub const PREVIEW_CONFIG_ID: &str = "video_editor_preview_config";
pub const PREFERENCE_CONFIG_ID: &str = "video_editor_preference_config";
pub const EXPORT_VIDEO_CONFIG_ID: &str = "video_editor_export_video_config";
pub const EXPORT_AUDIO_CONFIG_ID: &str = "video_editor_export_audio_config";
pub const RECORD_AUDIO_CONFIG_ID: &str = "video_editor_record_audio_config";
pub const PRESET_FILTERS_ID: &str = "video_editor_preset_filters";
pub const MARKED_FILTERS_ID: &str = "video_editor_marked_filters";
pub const SUBTITLE_STYLE_ID: &str = "video_editor_subtitle_style";
pub const PRESET_SUBTITLE_STYLES_ID: &str = "video_editor_preset_subtitle_styles";
pub const TIMER_STYLE_DEFAULT_ID: &str = "timer_style_default";
pub const PRESET_TEXT_STYLES_ID: &str = "video_editor_preset_text_styles";
pub const TEXT_STYLE_CONFIG_ID: &str = "text_style_default";
pub const TTS_CONFIG_ID: &str = "tts_config_default";
pub const CODE_IMAGE_CONFIG_ID: &str = "code_image_config_default";
pub const PURE_COLOR_IMAGE_CONFIG_ID: &str = "pure_color_image_config";
pub const BG_ANIMATION_CONFIG_ID: &str = "bg_animation";
pub const BG_REMOVER_CONFIG_ID: &str = "bg_remover_config_default";
pub const MUSICGEN_CONFIG_ID: &str = "musicgen_config_default";
pub const DEWATERMARK_CONFIG_ID: &str = "dewatermark_config_default";
pub const SUBTITLE_REMOVER_CONFIG_ID: &str = "subtitle_remover_config_default";
pub const CUTOUT_CONFIG_ID: &str = "cutout_config_default";
pub const CLEAR_VISION_CONFIG_ID: &str = "clear_vision_config_default";
pub const STEM_SPLITTER_CONFIG_ID: &str = "stem_splitter_config_default";
pub const DEEP_FILTER_CONFIG_ID: &str = "deep_filter_config_default";
pub const SPEAKERS_CONFIG_ID: &str = "speakers_config_default";
pub const SIMILAR_VIDEO_SEGMENT_CONFIG_ID: &str = "similar_video_segment_config_default";
pub const DEDUP_PHOTOS_CONFIG_ID: &str = "dedup_photos_config_default";
pub const OCR_CONFIG_ID: &str = "ocr_config_default";
pub const TRANSCRIBE_CONFIG_ID: &str = "transcribe_config_default";
pub const SMART_MIX_CONFIG_ID: &str = "smart_mix_config_default";
pub const SUBTITLE_TRANSLATE_CONFIG_ID: &str = "subtitle_translate_config_default";
pub const IMG_ANIMATION_CONFIG_ID: &str = "img_animation";
pub const ONLINE_SEARCH_IMAGE_CONFIG_ID: &str = "online_search_image_config_default";
pub const ONLINE_SEARCH_AUDIO_CONFIG_ID: &str = "online_search_audio_config_default";
pub const GLOBAL_FILTER_CONFIG_ID: &str = "global_filter_config";
pub const SCENE_DETECT_CONFIG_ID: &str = "scene_detect_config";
pub const GLOBAL_MEMO_ID: &str = "video_editor_global_memo";
pub static PROJECT_STATE: Mutex<Option<ProjectState>> = Mutex::new(None);
static BACKUP_CANCELLATION_TOKEN: Mutex<Option<CancellationToken>> = Mutex::new(None);

crate::db_add!(VIDEO_EDITOR_TABLE, VideoEditorUIState);
crate::db_update!(VIDEO_EDITOR_TABLE, VideoEditorUIState);
crate::db_remove!(VIDEO_EDITOR_TABLE);

#[derive(Derivative)]
#[derivative(Default)]
pub struct ProjectState {
    pub current_project_path: Option<PathBuf>,

    #[derivative(Default(value = "Some(RecentFilesManager::new(10))"))]
    pub recent_files_manager: Option<RecentFilesManager>,

    #[derivative(Default(value = "HistoryManager::new()"))]
    pub history_manager: HistoryManager,

    #[derivative(Default(value = "Manager::new()"))]
    pub tracks_manager: Manager,

    #[derivative(Default(value = "create_playlist_with_cache()"))]
    pub playlist: MediaList,

    #[derivative(Default(value = "create_library_with_cache()"))]
    pub library: MediaList,

    pub chapter_summary: Vec<ChapterSummaryData>,
    pub bookmarks: Vec<BookmarkData>,
    pub memo: String,

    pub autosave_manager: Option<AutoSaveManager>,
    pub autosave_handle: Option<AutoSaveHandle>,
}

#[macro_export]
macro_rules! store_video_editor_recent {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_recent_entries()
            .as_any()
            .downcast_ref::<VecModel<UIVideoEditorRecentEntry>>()
            .expect("We know we set a VecModel<UIVideoEditorRecentEntry> earlier")
    };
}

#[macro_export]
macro_rules! store_video_editor_playlist {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_playlist()
            .as_any()
            .downcast_ref::<VecModel<UIVideoEditorPlaylistItem>>()
            .expect("We know we set a VecModel<UIVideoEditorPlaylistItem> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_update_ui_state, ui);
    logic_cb!(video_editor_new_project, ui);
    logic_cb!(video_editor_init_recent, ui);
    logic_cb!(video_editor_show_recent_dialog, ui);
    logic_cb!(video_editor_open_recent_project, ui, index);
    logic_cb!(video_editor_show_metadata_dialog, ui);
    logic_cb!(video_editor_open_project, ui);
    logic_cb!(video_editor_save_project, ui);
    logic_cb!(video_editor_save_as_project, ui);
    logic_cb!(video_editor_backup_project, ui);
    logic_cb!(video_editor_cancel_backup_project, ui);
    logic_cb!(video_editor_close_project, ui);
    logic_cb!(video_editor_close_project_with_save, ui);
    logic_cb!(video_editor_close_project_without_save, ui);
    logic_cb!(video_editor_quit, ui);
    logic_cb!(video_editor_show_preference_setting_dialog, ui);
    logic_cb!(video_editor_update_preference_track_config, ui, setting);
    logic_cb!(video_editor_update_preference_cache_config, ui, setting);
    logic_cb!(video_editor_update_preference_mcp_config, ui, setting);
    logic_cb!(video_editor_recover_from_autosave, ui);
    logic_cb!(video_editor_ignore_recovery, ui);
    logic_cb!(video_editor_cancel_recovery, ui);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let ui_weak_for_preview = ui.as_weak();
    let ui_weak_for_preference = ui.as_weak();

    *PROJECT_STATE.lock().unwrap() = Some(ProjectState::default());
    global_store!(ui).set_video_editor_tracks_manager(UIVideoEditorTracksManager::default());
    global_store!(ui).set_video_editor_new_project_config(UIVideoEditorNewProjectConfig {
        name: SharedString::default(),
        preview_config: ProjectPreviewConfig::default().into(),
    });

    tokio::spawn(async move {
        let entry = if let Ok(setting) = sqldb::entry::select(VIDEO_EDITOR_TABLE, UI_STATE_ID).await
            && let Ok(entry) = serde_json::from_str::<VideoEditorUIState>(&setting.data)
        {
            entry
        } else {
            let entry = VideoEditorUIState::default();
            db_remove(ui_weak.clone(), UI_STATE_ID);
            db_add(ui_weak.clone(), entry.clone());

            entry
        };

        let ui_entry = entry.into();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_ui_state(ui_entry);
        });
    });

    tokio::spawn(async move {
        let recent_manager = match sqldb::entry::select(VIDEO_EDITOR_TABLE, RECENT_ID).await {
            Ok(setting) => serde_json::from_str::<RecentFilesManager>(&setting.data)
                .unwrap_or_else(|_| RecentFilesManager::new(10)),
            _ => {
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, RECENT_ID, "{}").await;
                RecentFilesManager::new(10)
            }
        };

        if let Some(ref mut s) = *PROJECT_STATE.lock().unwrap() {
            s.recent_files_manager = Some(recent_manager);
        }
    });

    tokio::spawn(async move {
        let preview_config =
            match sqldb::entry::select(VIDEO_EDITOR_TABLE, PREVIEW_CONFIG_ID).await {
                Ok(setting) => serde_json::from_str::<VideoEditorPreviewConfig>(&setting.data)
                    .unwrap_or_default(),
                _ => {
                    let config = VideoEditorPreviewConfig::default();
                    _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, PREVIEW_CONFIG_ID, "{}").await;
                    config
                }
            };

        _ = ui_weak_for_preview.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_new_project_config(UIVideoEditorNewProjectConfig {
                preview_config: preview_config.into(),
                ..global_store!(ui).get_video_editor_new_project_config()
            });
        });
    });

    tokio::spawn(async move {
        let preference_config =
            match sqldb::entry::select(VIDEO_EDITOR_TABLE, PREFERENCE_CONFIG_ID).await {
                Ok(setting) => serde_json::from_str::<VideoEditorPreferenceConfig>(&setting.data)
                    .unwrap_or_default(),
                _ => {
                    let config = VideoEditorPreferenceConfig::default();
                    _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, PREFERENCE_CONFIG_ID, "{}").await;
                    config
                }
            };

        set_global_cache_max_frames(preference_config.cache.max_frames.max(10) as usize);

        _ = ui_weak_for_preference.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_preference_config(preference_config.into());
            mcp::start_from_config(&ui);
        });
    });
}

fn video_editor_update_ui_state(ui: &AppWindow) {
    let ui_state = global_store!(ui).get_video_editor_ui_state();
    let state: VideoEditorUIState = ui_state.into();
    db_update(ui.as_weak(), state);
}

fn video_editor_new_project(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let ui_preview_config = global_store!(ui)
        .get_video_editor_new_project_config()
        .preview_config;
    let preview_config: ProjectPreviewConfig = ui_preview_config.clone().into();

    tokio::spawn(async move {
        let Some(mut path) = picker_save_file(
            ui_weak.clone(),
            &tr("New Project"),
            &tr("Wayshot Project"),
            &[],
            &format!("untitled_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
        ) else {
            return;
        };

        if path.extension().is_none() {
            path.set_extension(PROJECT_EXT);
        }

        let project_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        let (manager, playlist, chapter_summary, bookmarks, memo) = {
            let mut state = PROJECT_STATE.lock().unwrap();
            if state.is_none() {
                *state = Some(ProjectState::default());
            }

            if let Some(ref mut state) = *state {
                state.current_project_path = Some(path.clone());
                (
                    state.tracks_manager.clone(),
                    state.playlist.clone(),
                    state.chapter_summary.clone(),
                    state.bookmarks.clone(),
                    state.memo.clone(),
                )
            } else {
                unreachable!("Already init PROJECT_STATE");
            }
        };

        let manager_data = ManagerData::new(manager)
            .with_preview_config(preview_config.clone())
            .with_playlist(playlist)
            .with_chapter_summary(chapter_summary)
            .with_bookmarks(bookmarks)
            .with_memo(memo);

        match video_editor::project::save_project(&manager_data, &path) {
            Ok(()) => {
                add_to_recent(path.clone());
                setup_autosave(Some(&path));

                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui)
                        .set_video_editor_tracks_manager(UIVideoEditorTracksManager::default());
                    global_store!(ui).set_video_editor_is_unsaved(false);
                    global_store!(ui).set_video_editor_chapter_summary_entries(ModelRc::new(
                        VecModel::default(),
                    ));
                    global_store!(ui).set_video_editor_chapter_summary_progress_type(
                        UIChapterSummaryProgressType::None,
                    );
                    global_store!(ui).set_video_editor_bookmarks(ModelRc::new(VecModel::default()));
                    global_store!(ui).set_video_editor_project_memo_text(SharedString::default());
                    global_store!(ui).set_video_editor_new_project_config(
                        UIVideoEditorNewProjectConfig {
                            name: project_name.clone().into(),
                            preview_config: ui_preview_config,
                        },
                    );
                    crate::toast_success!(
                        ui,
                        format!("{} {}", tr("New project created"), project_name)
                    );
                });
            }
            Err(e) => toast::async_toast_warn(
                ui_weak,
                format!("{}: {}", tr("Failed to create project"), e),
            ),
        }
    });
}

fn video_editor_init_recent(ui: &AppWindow) {
    let state = PROJECT_STATE.lock().unwrap();
    if let Some(ref s) = *state
        && let Some(ref manager) = s.recent_files_manager
    {
        let ui_entries: Vec<UIVideoEditorRecentEntry> = manager
            .get_existing_files()
            .into_iter()
            .map(|rf| rf.clone().into())
            .collect();

        store_video_editor_recent!(ui).set_vec(ui_entries);
    }
}

fn video_editor_show_recent_dialog(ui: &AppWindow) {
    global_store!(ui).set_video_editor_is_show_recent_dialog(true);
}

fn video_editor_open_recent_project(ui: &AppWindow, index: i32) {
    let ui_weak = ui.as_weak();
    global_store!(ui).set_video_editor_is_show_recent_dialog(false);
    let entry = store_video_editor_recent!(ui)
        .row_data(index as usize)
        .unwrap();

    let name = entry.name.clone();
    let path = PathBuf::from(entry.path.as_str());

    if check_and_show_recovery_dialog(ui_weak.clone(), &path) {
        return;
    }

    tokio::spawn(async move {
        async_open_project_path(ui_weak, name, path);
    });
}

fn video_editor_show_metadata_dialog(ui: &AppWindow) {
    global_store!(ui).set_video_editor_is_show_metadata_dialog(true);
}

fn video_editor_open_project(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(path) = picker_file(
            ui_weak.clone(),
            &tr("Choose project file"),
            &tr("Wayshot Project"),
            &[PROJECT_EXT],
        ) else {
            return;
        };

        let project_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        if check_and_show_recovery_dialog(ui_weak.clone(), &path) {
            return;
        }

        async_open_project_path(ui_weak, project_name.into(), path);
    });
}

fn video_editor_save_project(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let preview_config = global_store!(ui)
        .get_video_editor_new_project_config()
        .preview_config
        .into();

    tokio::spawn(async move {
        let project_path = {
            let state = PROJECT_STATE.lock().unwrap();
            if let Some(ref s) = *state {
                s.current_project_path.clone()
            } else {
                None
            }
        };

        if let Some(path) = project_path {
            async_save_project_to_path(ui_weak, preview_config, path);
        } else {
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                video_editor_save_as_project(&ui);
            });
        }
    });
}

fn video_editor_save_as_project(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let project_config = global_store!(ui).get_video_editor_new_project_config();
    let preview_config = project_config.preview_config.into();

    let project_name = if project_config.name.is_empty() {
        format!("untitled_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"))
    } else {
        project_config.name.into()
    };

    tokio::spawn(async move {
        let Some(mut path) = picker_save_file(
            ui_weak.clone(),
            &tr("Save project as"),
            &tr("Wayshot Project"),
            &[],
            &project_name,
        ) else {
            return;
        };

        if path.extension().is_none() {
            path.set_extension(PROJECT_EXT);
        }

        let new_project_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        async_save_project_to_path(ui_weak.clone(), preview_config, path.clone());

        {
            let mut state = PROJECT_STATE.lock().unwrap();
            if let Some(ref mut s) = *state {
                s.current_project_path = Some(path.clone());
            }
        }

        add_to_recent(path);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let config = global_store!(ui).get_video_editor_new_project_config();
            global_store!(ui).set_video_editor_new_project_config(UIVideoEditorNewProjectConfig {
                name: new_project_name.into(),
                preview_config: config.preview_config,
            });
            global_store!(ui).set_video_editor_is_unsaved(false);

            if global_store!(ui).get_video_editor_is_in_quit_state() {
                global_util!(ui).invoke_close_window();
            }
        });
    });
}

fn video_editor_backup_project(ui: &AppWindow) {
    let preview_config_val: ProjectPreviewConfig = global_store!(ui)
        .get_video_editor_new_project_config()
        .preview_config
        .into();

    let (project_path, project_name, manager, chapter_summary, bookmarks, memo) = {
        let state = PROJECT_STATE.lock().unwrap();
        if let Some(ref s) = *state {
            let path = s.current_project_path.clone();
            let name = path
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string();
            (
                path,
                name,
                s.tracks_manager.clone(),
                s.chapter_summary.clone(),
                s.bookmarks.clone(),
                s.memo.clone(),
            )
        } else {
            toast_warn!(ui, tr("No project is currently open"));
            return;
        }
    };

    let Some(current_path) = project_path else {
        toast_warn!(ui, tr("No project is currently open"));
        return;
    };

    global_store!(ui).set_video_editor_is_show_backup_progress_dialog(true);
    global_store!(ui).set_video_editor_backup_progress(0.0);
    global_store!(ui).set_video_editor_backup_is_canceled(false);

    let cancellation_token = CancellationToken::new();
    *BACKUP_CANCELLATION_TOKEN.lock().unwrap() = Some(cancellation_token.clone());

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(mut backup_path) = picker_save_file(
            ui_weak.clone(),
            &tr("Backup Project"),
            &tr("Backup Folder"),
            &[],
            &format!(
                "{}_backup_{}",
                project_name,
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            ),
        ) else {
            _ = ui_weak.upgrade_in_event_loop(|ui| {
                global_store!(ui).set_video_editor_is_show_backup_progress_dialog(false);
            });
            return;
        };

        if backup_path.extension().is_some() {
            backup_path.set_extension("");
        }

        let files_to_copy = collect_segment_media_files(&manager);
        let total_files = files_to_copy.len();

        if total_files == 0 {
            _ = ui_weak.upgrade_in_event_loop(|ui| {
                global_store!(ui).set_video_editor_is_show_backup_progress_dialog(false);
                toast_warn!(ui, tr("No media files to backup"));
            });
            return;
        }

        if let Err(e) = tokio::fs::create_dir_all(&backup_path).await {
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_video_editor_is_show_backup_progress_dialog(false);
                toast_warn!(
                    ui,
                    format!("{}: {e}", tr("Failed to create backup directory"))
                );
            });
            return;
        }

        // Map: original absolute path -> relative filename in backup dir (top-level only)
        let mut path_mappings: HashMap<PathBuf, PathBuf> = HashMap::new();
        // Track used filenames to avoid collisions
        let mut used_names: HashSet<String> = HashSet::new();

        for (index, file_path) in files_to_copy.iter().enumerate() {
            if cancellation_token.is_cancelled() {
                _ = tokio::fs::remove_dir_all(&backup_path).await;
                toast::async_toast_info(ui_weak, tr("Backup cancelled"));
                return;
            }

            let progress = (index as f32 + 0.5) / (total_files as f32);
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_video_editor_backup_progress(progress);
            });

            // Generate unique filename at top level
            let dest_filename = unique_top_level_filename(&file_path, &used_names);
            used_names.insert(dest_filename.clone());
            let dest_path = backup_path.join(&dest_filename);

            if let Err(e) = tokio::fs::copy(&file_path, &dest_path).await {
                log::warn!("Failed to copy file {}: {e}", file_path.display());
            }

            path_mappings.insert(file_path.clone(), PathBuf::from(&dest_filename));
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_backup_progress(0.95);
        });

        let backup_project_path = backup_path.join(current_path.file_name().unwrap_or_default());
        let modified_project_file =
            create_project_file_with_relative_paths(&manager, &path_mappings, preview_config_val)
                .with_chapter_summary(chapter_summary)
                .with_bookmarks(bookmarks)
                .with_memo(memo);

        match video_editor::project::save_project(&modified_project_file, &backup_project_path) {
            Ok(()) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_video_editor_backup_progress(1.0);
                    toast_success!(
                        ui,
                        format!("{} {}", tr("Backup completed"), backup_path.display())
                    );
                });
            }
            Err(e) => toast::async_toast_warn(
                ui_weak,
                format!("{}: {e}", tr("Failed to save backup project")),
            ),
        }
    });
}

fn video_editor_cancel_backup_project(ui: &AppWindow) {
    global_store!(ui).set_video_editor_backup_is_canceled(true);

    if let Some(token) = BACKUP_CANCELLATION_TOKEN.lock().unwrap().take() {
        token.cancel();
    }
}

/// Collect all media files referenced by segments (deduplicated by absolute path).
/// Only files that exist on disk are included.
fn collect_segment_media_files(manager: &Manager) -> Vec<PathBuf> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut result: Vec<PathBuf> = Vec::new();

    let collect = |path: &Path, seen: &mut HashSet<PathBuf>, result: &mut Vec<PathBuf>| {
        if path.exists() && seen.insert(path.to_path_buf()) {
            result.push(path.to_path_buf());
        }
    };

    for track in manager.iter() {
        match track {
            Track::Video(inner) => {
                for segment in &inner.track.segments {
                    collect(&segment.metadata.path, &mut seen, &mut result);
                }
            }
            Track::Audio(inner) => {
                for segment in &inner.track.segments {
                    collect(&segment.metadata.path, &mut seen, &mut result);
                }
            }
            Track::Subtitle(inner) => {
                for segment in &inner.track.segments {
                    collect(&segment.metadata.path, &mut seen, &mut result);
                }
            }
            Track::Image(inner) => {
                for segment in &inner.track.segments {
                    collect(&segment.metadata.path, &mut seen, &mut result);
                }
            }
            Track::Text(_) => {}
        }
    }

    result
}

/// Generate a unique filename for the backup top-level directory.
/// If the original name is already used, append a counter suffix.
fn unique_top_level_filename(original_path: &Path, used_names: &HashSet<String>) -> String {
    let original_name = original_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    if !used_names.contains(&original_name) {
        return original_name;
    }

    let stem = original_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let extension = original_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    for counter in 1..1000 {
        let new_name = format!("{}_{}{}", stem, counter, extension);
        if !used_names.contains(&new_name) {
            return new_name;
        }
    }

    format!("{}_{}{}", stem, uuid::Uuid::new_v4(), extension)
}

fn create_project_file_with_relative_paths(
    manager: &Manager,
    path_mappings: &HashMap<PathBuf, PathBuf>,
    preview_config: ProjectPreviewConfig,
) -> ManagerData {
    let modified_tracks: Vec<Track> = manager
        .iter()
        .map(|track| match track {
            Track::Video(inner) => {
                let modified_segments = modify_segment_paths(&inner.track.segments, path_mappings);
                let modified_metadata = modify_metadata_path(&inner.track.metadata, path_mappings);
                Track::Video(Arc::new(VideoTrack {
                    name: inner.name.clone(),
                    hiding: inner.hiding,
                    muted: inner.muted,
                    locked: inner.locked,
                    track: InnerTrack::new(
                        modified_metadata,
                        inner.track.duration,
                        modified_segments,
                    ),
                }))
            }
            Track::Audio(inner) => {
                let modified_segments = modify_segment_paths(&inner.track.segments, path_mappings);
                let modified_metadata = modify_metadata_path(&inner.track.metadata, path_mappings);
                Track::Audio(Arc::new(AudioTrack {
                    name: inner.name.clone(),
                    hiding: inner.hiding,
                    locked: inner.locked,
                    track: InnerTrack::new(
                        modified_metadata,
                        inner.track.duration,
                        modified_segments,
                    ),
                }))
            }
            Track::Subtitle(inner) => {
                let modified_segments = modify_segment_paths(&inner.track.segments, path_mappings);
                let modified_metadata = modify_metadata_path(&inner.track.metadata, path_mappings);
                Track::Subtitle(Arc::new(SubtitleTrack {
                    name: inner.name.clone(),
                    hiding: inner.hiding,
                    locked: inner.locked,
                    track: InnerTrack::new(
                        modified_metadata,
                        inner.track.duration,
                        modified_segments,
                    ),
                }))
            }
            Track::Image(inner) => {
                let modified_segments = modify_segment_paths(&inner.track.segments, path_mappings);
                let modified_metadata = modify_metadata_path(&inner.track.metadata, path_mappings);
                Track::Image(Arc::new(ImageTrack {
                    name: inner.name.clone(),
                    hiding: inner.hiding,
                    locked: inner.locked,
                    track: InnerTrack::new(
                        modified_metadata,
                        inner.track.duration,
                        modified_segments,
                    ),
                }))
            }
            Track::Text(inner) => Track::Text(inner.clone()),
        })
        .collect();

    let mut modified_manager = Manager::new();
    modified_manager.tracks = modified_tracks;
    modified_manager.duration = manager.duration;

    ManagerData::new(modified_manager)
        .with_preview_config(preview_config)
        .with_is_backup(true)
}

fn modify_metadata_path(
    metadata: &Arc<Metadata>,
    path_mappings: &HashMap<PathBuf, PathBuf>,
) -> Arc<Metadata> {
    let relative_path = path_mappings
        .get(&metadata.path)
        .cloned()
        .unwrap_or_else(|| metadata.path.clone());

    if relative_path == metadata.path {
        return metadata.clone();
    }

    let mut modified = metadata.as_ref().clone();
    modified.path = relative_path;
    Arc::new(modified)
}

fn modify_segment_paths(
    segments: &[Arc<Segment>],
    path_mappings: &HashMap<PathBuf, PathBuf>,
) -> Vec<Arc<Segment>> {
    segments
        .iter()
        .map(|seg| {
            let relative_path = path_mappings
                .get(&seg.metadata.path)
                .cloned()
                .unwrap_or_else(|| seg.metadata.path.clone());

            let mut modified_metadata = seg.metadata.as_ref().clone();
            modified_metadata.path = relative_path;

            let mut modified_segment = seg.as_ref().clone();
            modified_segment.metadata = Arc::new(modified_metadata);
            Arc::new(modified_segment)
        })
        .collect()
}

fn clear_ui_state(ui: &AppWindow) {
    global_store!(ui).set_video_editor_tracks_manager(UIVideoEditorTracksManager::default());
    global_store!(ui).set_video_editor_preview_image(slint::Image::default());
    global_store!(ui).set_video_editor_is_unsaved(false);
    global_store!(ui).set_video_editor_is_previewing(false);
    global_store!(ui).set_video_editor_timeline_offset(0);
    global_store!(ui).set_video_editor_chapter_summary_entries(ModelRc::new(VecModel::default()));
    global_store!(ui)
        .set_video_editor_chapter_summary_progress_type(UIChapterSummaryProgressType::None);
    global_store!(ui).set_video_editor_bookmarks(ModelRc::new(VecModel::default()));
    global_store!(ui).set_video_editor_project_memo_text(SharedString::default());
    global_store!(ui).set_video_editor_new_project_config(UIVideoEditorNewProjectConfig {
        name: SharedString::default(),
        ..global_store!(ui).get_video_editor_new_project_config()
    });
    global_logic!(ui).invoke_video_editor_clear_all_selected_state();
    store_video_editor_playlist!(ui).set_vec(vec![]);
    crate::logic::video_editor::filters::global::clear_ui_state(ui);
}

fn video_editor_close_project(ui: &AppWindow) {
    if global_store!(ui).get_video_editor_is_unsaved() {
        global_store!(ui).set_video_editor_is_show_unsave_change_dialog(true);
    } else {
        video_editor_close_project_without_save(ui);
    }
}

fn video_editor_close_project_with_save(ui: &AppWindow) {
    video_editor_save_project(ui);
    clear_global_cache();
    clear_global_audio_display_cache();
    clear_ui_state(ui);

    {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            s.current_project_path = None;
            s.history_manager = HistoryManager::new();
            s.tracks_manager = Manager::new();
            s.playlist = create_playlist_with_cache();
            s.chapter_summary = vec![];
            s.bookmarks = vec![];
            s.memo = String::new();
        }
    }

    refresh_library_ui(ui);
}

fn video_editor_close_project_without_save(ui: &AppWindow) {
    cleanup_autosave_temp_files(None);
    stop_autosave();

    clear_global_cache();
    clear_global_audio_display_cache();
    clear_ui_state(ui);

    {
        // 保留 library 和 recent_files_manager，只重置项目相关的状态
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            s.current_project_path = None;
            s.history_manager = HistoryManager::new();
            s.tracks_manager = Manager::new();
            s.playlist = create_playlist_with_cache();
            s.chapter_summary = vec![];
            s.bookmarks = vec![];
            s.memo = String::new();
        }
    }

    refresh_library_ui(ui);
}

fn video_editor_quit(ui: &AppWindow) {
    global_store!(ui).set_video_editor_is_in_quit_state(true);

    if global_store!(ui).get_video_editor_is_unsaved() {
        global_store!(ui).set_video_editor_is_show_unsave_change_dialog(true);
    } else {
        global_util!(ui).invoke_close_window();
    }
}

fn video_editor_show_preference_setting_dialog(ui: &AppWindow) {
    global_store!(ui).set_video_editor_is_show_preference_setting_dialog(true);
}

fn video_editor_update_preference_track_config(
    ui: &AppWindow,
    setting: UIVideoEditorPreferenceTrackConfig,
) {
    let current = global_store!(ui).get_video_editor_preference_config();
    let old_samples_per_second = current.track.waveform_samples_per_second;
    let config = VideoEditorPreferenceConfig {
        track: setting.clone().into(),
        ..current.into()
    };

    global_store!(ui).set_video_editor_preference_config(config.clone().into());
    db_update_preference_config(ui.as_weak(), config);

    if setting.waveform_samples_per_second != old_samples_per_second {
        refresh_all_segment_audio_samples(ui);
    }

    crate::toast_success!(ui, tr("Save config successfully"));
}

fn video_editor_update_preference_cache_config(
    ui: &AppWindow,
    setting: UIVideoEditorPreferenceCacheConfig,
) {
    let current = global_store!(ui).get_video_editor_preference_config();
    let config = VideoEditorPreferenceConfig {
        cache: setting.into(),
        ..current.into()
    };
    global_store!(ui).set_video_editor_preference_config(config.clone().into());
    db_update_preference_config(ui.as_weak(), config);
    crate::toast_success!(ui, tr("Save config successfully"));
}

fn video_editor_update_preference_mcp_config(
    ui: &AppWindow,
    setting: UIVideoEditorPreferenceMcpConfig,
) {
    let current = global_store!(ui).get_video_editor_preference_config();
    let mcp_setting: VideoEditorPreferenceMcpConfig = setting.into();
    let config = VideoEditorPreferenceConfig {
        mcp: mcp_setting,
        ..current.into()
    };
    global_store!(ui).set_video_editor_preference_config(config.clone().into());
    db_update_preference_config(ui.as_weak(), config);
    crate::toast_success!(ui, tr("Save config successfully"));
}

fn video_editor_recover_from_autosave(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let recovery_info: RecoveryInfo = global_store!(ui).get_video_editor_recovery_info().into();
    let original_path = recovery_info.original_project_path.clone();

    tokio::spawn(async move {
        match restore_from_recovery(&recovery_info) {
            Ok(project_file) => {
                _ = cleanup_recovery_file(&recovery_info);
                async_load_project_from_recovery(ui_weak, project_file, original_path);
            }
            Err(e) => {
                toast::async_toast_warn(ui_weak, format!("{}: {}", tr("Failed to recover"), e))
            }
        }
    });
}

fn video_editor_ignore_recovery(ui: &AppWindow) {
    cancel_or_ignore_recovery(ui, true);
}

fn video_editor_cancel_recovery(ui: &AppWindow) {
    cancel_or_ignore_recovery(ui, false);
}

fn cancel_or_ignore_recovery(ui: &AppWindow, is_ignore: bool) {
    let ui_weak = ui.as_weak();
    let ui_recovery = global_store!(ui).get_video_editor_recovery_info();
    let original_path = ui_recovery.original_project_path.clone();

    if is_ignore {
        _ = cleanup_recovery_file(&ui_recovery.into());
    }

    if !original_path.is_empty() {
        let path = PathBuf::from(original_path.as_str());
        let project_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        tokio::spawn(async move {
            async_open_project_path(ui_weak, project_name.into(), path);
        });
    }
}

pub fn async_open_project_path(
    ui_weak: Weak<AppWindow>,
    project_name: SharedString,
    path: PathBuf,
) {
    match video_editor::project::load_project(&path) {
        Ok(data) => async_load_project(ui_weak, data, Some(path), Some(project_name), false),
        Err(e) => {
            toast::async_toast_warn(ui_weak, format!("{}: {}", tr("Failed to load project"), e))
        }
    }
}

fn async_load_project_from_recovery(
    ui_weak: Weak<AppWindow>,
    project_file: ProjectFile,
    original_path: Option<PathBuf>,
) {
    match ManagerData::try_from(project_file) {
        Ok(data) => async_load_project(ui_weak, data, original_path, None, true),
        Err(e) => toast::async_toast_warn(
            ui_weak,
            format!("{}: {}", tr("Failed to parse recovery file"), e),
        ),
    };
}

fn async_load_project(
    ui_weak: Weak<AppWindow>,
    manager_data: ManagerData,
    project_path: Option<PathBuf>,
    project_name: Option<SharedString>,
    mark_unsaved: bool,
) {
    let loaded_manager = manager_data.inner.unwrap_or_else(Manager::new);
    let preview_config = manager_data.preview_config;
    let mut playlist = manager_data.playlist;
    let chapter_summary = manager_data.chapter_summary;
    let bookmarks = manager_data.bookmarks;
    let memo = manager_data.memo;

    let cache_dir = config::all().cache_dir.join("playlist_thumbnails");
    _ = std::fs::create_dir_all(&cache_dir);
    if let Ok(pl) = std::mem::replace(&mut playlist, MediaList::new("Default".to_string()))
        .with_cache_configured(cache_dir, 160, 90, Duration::from_secs(86400))
    {
        playlist = pl;
    }

    // Build AffectedSegments to refresh all segments' display cache after loading
    let mut affected = AffectedSegments::new();
    for (track_idx, track) in loaded_manager.iter().enumerate() {
        let segment_count = match track {
            Track::Video(inner) => inner.track.segments.len(),
            Track::Audio(inner) => inner.track.segments.len(),
            Track::Image(inner) => inner.track.segments.len(),
            Track::Subtitle(_) | Track::Text(_) => continue,
        };

        for seg_idx in 0..segment_count {
            affected.add_both_thumbnails(track_idx, seg_idx);
        }
    }

    let (original_library, original_recent_files_manager) = {
        let state = PROJECT_STATE.lock().unwrap();
        if let Some(ref s) = *state {
            (s.library.clone(), s.recent_files_manager.clone())
        } else {
            (
                create_library_with_cache(),
                Some(RecentFilesManager::new(10)),
            )
        }
    };

    let mut state = ProjectState::default();
    state.current_project_path = project_path.clone();
    state.tracks_manager = loaded_manager.clone();
    state.history_manager = HistoryManager::new();
    state.playlist = playlist.clone();
    state.library = original_library;
    state.recent_files_manager = original_recent_files_manager;
    state.chapter_summary = chapter_summary.clone();
    state.bookmarks = bookmarks.clone();
    state.memo = memo.clone();
    *PROJECT_STATE.lock().unwrap() = Some(state);

    if let Some(ref path) = project_path {
        setup_autosave(Some(path));
        add_to_recent(path.clone());
    }

    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        let ui_manager = loaded_manager.into();
        global_store!(ui).set_video_editor_tracks_manager(ui_manager);
        global_store!(ui).set_video_editor_is_unsaved(mark_unsaved);

        if let Some(name) = project_name {
            global_store!(ui).set_video_editor_new_project_config(UIVideoEditorNewProjectConfig {
                name,
                preview_config: preview_config.clone().into(),
            });
        }

        refresh_ui(MediaListContext::Playlist, &ui);

        if chapter_summary.is_empty() {
            global_store!(ui)
                .set_video_editor_chapter_summary_entries(ModelRc::new(VecModel::default()));
            global_store!(ui)
                .set_video_editor_chapter_summary_progress_type(UIChapterSummaryProgressType::None);
        } else {
            let entries: Vec<UIChapterSummaryEntry> =
                chapter_summary.iter().map(|ch| ch.clone().into()).collect();
            global_store!(ui).set_video_editor_chapter_summary_entries(ModelRc::new(
                VecModel::from_slice(&entries),
            ));
            global_store!(ui).set_video_editor_chapter_summary_progress_type(
                UIChapterSummaryProgressType::Finished,
            );
        }

        sync_bookmarks_to_ui(&ui);
        global_store!(ui).set_video_editor_project_memo_text(memo.into());

        match project_path {
            Some(ref path) => {
                let msg = format!(
                    "{}: {}",
                    if mark_unsaved {
                        tr("Project recovered")
                    } else {
                        tr("Project loaded")
                    },
                    path.display()
                );
                crate::toast_success!(ui, msg);
            }
            _ => crate::toast_success!(ui, tr("Project recovered from autosave")),
        }

        crate::logic::video_editor::filters::global::init_from_project(&ui);

        // Refresh all segments' display cache (thumbnails and audio samples)
        refresh_affected_segments(&ui, affected);
    });
}

fn check_and_show_recovery_dialog(ui_weak: Weak<AppWindow>, project_path: &Path) -> bool {
    let autosave_dir = config::all().cache_dir.join("autosaves");

    if let Some(recovery) = check_for_recovery(&autosave_dir, project_path) {
        let ui_recovery: UIVideoEditorRecoveryInfo = recovery.into();
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_recovery_info(ui_recovery);
            global_store!(ui).set_video_editor_is_show_recovery_dialog(true);
        });

        return true;
    }
    false
}

fn async_save_project_to_path(
    ui_weak: Weak<AppWindow>,
    preview_config: ProjectPreviewConfig,
    path: PathBuf,
) {
    let path_display = path.display().to_string();

    let (manager, playlist, chapter_summary, bookmarks, memo) = {
        let state = PROJECT_STATE.lock().unwrap();
        if let Some(ref s) = *state {
            (
                s.tracks_manager.clone(),
                s.playlist.clone(),
                s.chapter_summary.clone(),
                s.bookmarks.clone(),
                s.memo.clone(),
            )
        } else {
            toast::async_toast_warn(
                ui_weak,
                "Failed to save: Project not initialized".to_string(),
            );
            return;
        }
    };

    let manager_data = ManagerData::new(manager)
        .with_preview_config(preview_config)
        .with_playlist(playlist)
        .with_chapter_summary(chapter_summary)
        .with_bookmarks(bookmarks)
        .with_memo(memo);

    match video_editor::project::save_project(&manager_data, &path) {
        Ok(()) => {
            {
                let mut state = PROJECT_STATE.lock().unwrap();
                if let Some(ref mut s) = *state {
                    s.current_project_path = Some(path.clone());
                }
            }

            reset_autosave_dirty();
            cleanup_autosave_temp_files(Some(&path));

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_video_editor_is_unsaved(false);
                crate::toast_success!(ui, format!("{} {}", tr("Project saved"), path_display));

                if global_store!(ui).get_video_editor_is_in_quit_state() {
                    global_util!(ui).invoke_close_window();
                }
            });
        }
        Err(e) => {
            toast::async_toast_warn(ui_weak, format!("{}: {}", tr("Failed to save project"), e))
        }
    }
}

pub fn add_to_recent(path: PathBuf) {
    let mut state = PROJECT_STATE.lock().unwrap();
    if let Some(ref mut s) = *state
        && let Some(ref mut manager) = s.recent_files_manager
    {
        _ = manager.add_file(path.clone());
        let manager_clone = manager.clone();
        drop(state);

        tokio::spawn(async move {
            let data =
                serde_json::to_string(&manager_clone).expect("Failed to serialize recent files");
            if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, RECENT_ID, &data).await {
                log::warn!("Failed to save recent files to database: {}", e);
            }
        });
    }
}

fn create_playlist_with_cache() -> MediaList {
    let cache_dir = config::all().cache_dir.join("playlist_thumbnails");
    _ = std::fs::create_dir_all(&cache_dir);

    MediaList::new("Default".to_string())
        .with_cache_configured(cache_dir, 160, 90, Duration::from_secs(86400))
        .unwrap_or_else(|_| MediaList::new("Default".to_string()))
}

fn create_library_with_cache() -> MediaList {
    let cache_dir = config::all().cache_dir.join("library_thumbnails");
    _ = std::fs::create_dir_all(&cache_dir);

    MediaList::new(String::new())
        .with_cache_configured(cache_dir, 160, 90, Duration::from_secs(86400))
        .unwrap_or_else(|_| MediaList::new(String::new()))
}

pub fn db_update_preview_config(ui: slint::Weak<AppWindow>, config: VideoEditorPreviewConfig) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("Failed to serialize preview config");
        if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, PREVIEW_CONFIG_ID, &data).await {
            toast::async_toast_warn(
                ui,
                format!("{}. {e}", crate::logic::tr::tr("update entry failed")),
            );
        }
    });
}

fn db_update_preference_config(ui: Weak<AppWindow>, config: VideoEditorPreferenceConfig) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("Failed to serialize preference config");
        if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, PREFERENCE_CONFIG_ID, &data).await
        {
            toast::async_toast_warn(
                ui,
                format!("{}. {e}", crate::logic::tr::tr("update entry failed")),
            );
        }
    });
}

pub fn setup_autosave(project_path: Option<&Path>) {
    let autosave_dir = config::all().cache_dir.join("autosaves");
    let config = AutoSaveConfig::new()
        .with_enabled(true)
        .with_interval(Duration::from_secs(60))
        .with_max_temp_files(5)
        .with_temp_location(autosave_dir);

    match AutoSaveManager::new(config, project_path) {
        Ok(manager) => {
            let handle = manager.start_autosave_thread(|| {
                let state = PROJECT_STATE.lock().unwrap();
                if let Some(ref s) = *state {
                    Some(ProjectFile::from(s))
                } else {
                    None
                }
            });

            let mut state = PROJECT_STATE.lock().unwrap();
            if let Some(ref mut s) = *state {
                s.autosave_manager = Some(manager);
                s.autosave_handle = Some(handle);
            }
        }
        Err(e) => log::warn!("Failed to setup autosave: {}", e),
    }
}

pub fn stop_autosave() {
    let mut state = PROJECT_STATE.lock().unwrap();
    if let Some(ref mut s) = *state {
        s.autosave_handle = None;
        s.autosave_manager = None;
    }
}

pub fn reset_autosave_dirty() {
    let state = PROJECT_STATE.lock().unwrap();
    if let Some(ref s) = *state
        && let Some(ref autosave) = s.autosave_manager
    {
        autosave.update_save_time();
        autosave.clear_dirty();
    }
}

pub fn cleanup_autosave_temp_files(path: Option<&PathBuf>) {
    {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state
            && let Some(ref mut autosave) = s.autosave_manager
        {
            if let Err(e) = autosave.cleanup_temp_files() {
                log::warn!("Failed to cleanup autosave temp files via manager: {}", e);
            }
            return;
        }
    }

    // 如果没有 manager 但有 path，通过路径清理
    if let Some(path) = path {
        let autosave_dir = config::all().cache_dir.join("autosaves");
        if let Err(e) = cleanup_temp_files_by_path(&autosave_dir, path) {
            log::warn!("Failed to cleanup autosave temp files by path: {}", e);
        }
    }
}

impl From<ChapterSummaryData> for UIChapterSummaryEntry {
    fn from(ch: ChapterSummaryData) -> UIChapterSummaryEntry {
        UIChapterSummaryEntry {
            start_ms: ch.start_ms as i32,
            end_ms: ch.end_ms as i32,
            title: ch.title.into(),
        }
    }
}

impl From<&ProjectState> for ProjectFile {
    fn from(state: &ProjectState) -> Self {
        let manager_data = ManagerData::new(state.tracks_manager.clone())
            .with_chapter_summary(state.chapter_summary.clone())
            .with_bookmarks(state.bookmarks.clone())
            .with_memo(state.memo.clone());

        ProjectFile::from(&manager_data)
    }
}
