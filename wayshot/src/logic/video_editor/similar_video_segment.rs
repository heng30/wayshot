use crate::{
    db::{SimilarVideoSegmentConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        recorder::picker_directory,
        share_screen::picker_file,
        toast::async_toast_warn,
        tr::tr,
        video_editor::{
            img_animation::IMAGE_EXTENSIONS, playlist::picker_files,
            project::SIMILAR_VIDEO_SEGMENT_CONFIG_ID,
        },
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, VideoEditorSimilarVideoSegmentConfig as UISimilarVideoSegmentConfig,
        VideoEditorSimilarVideoSegmentItem as UISimilarVideoSegmentItem,
    },
};
use similar_video_segment::{
    CancellationToken, ExportProgress, MatchResult, ScanProgress, SimilarVideoConfig,
    export_segments, scan_videos,
};
use slint::{ComponentHandle, Model as SlintModel, SharedString, VecModel};
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "avi", "mkv", "webm", "flv", "ts", "m2ts", "wmv", "asf", "3gp", "ogv", "m4v",
];

static PROCESS_INC_INDEX: AtomicU32 = AtomicU32::new(0);
static CANCEL_TOKEN: Mutex<Option<CancellationToken>> = Mutex::new(None);
static SCAN_RESULTS: Mutex<Vec<MatchResult>> = Mutex::new(Vec::new());

#[macro_export]
macro_rules! store_similar_video_segment_items {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_similar_video_segment_items()
            .as_any()
            .downcast_ref::<VecModel<UISimilarVideoSegmentItem>>()
            .expect("VecModel<UISimilarVideoSegmentItem> for similar video segment")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_similar_video_segment_import_file, ui);
    logic_cb!(video_editor_similar_video_segment_import_folder, ui);
    logic_cb!(video_editor_similar_video_segment_choose_query_image, ui);
    logic_cb!(video_editor_similar_video_segment_process, ui);
    logic_cb!(video_editor_similar_video_segment_cancel, ui);
    logic_cb!(video_editor_similar_video_segment_remove_all, ui);
    logic_cb!(video_editor_similar_video_segment_remove_item, ui, index);
    logic_cb!(video_editor_similar_video_segment_choose_export_dir, ui);
    logic_cb!(video_editor_similar_video_segment_update_config, ui, config);
    logic_cb_pure!(video_editor_similar_video_segment_setting_is_valid, ui);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_config()
            .await
            .unwrap_or_else(|| SimilarVideoSegmentConfigData {
                ..Default::default()
            });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_similar_video_segment_config(config.into());
        });
    });
}

fn save_config(config: SimilarVideoSegmentConfigData) {
    tokio::spawn(async move {
        let data =
            serde_json::to_string(&config).expect("serialize similar video segment config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, SIMILAR_VIDEO_SEGMENT_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, SIMILAR_VIDEO_SEGMENT_CONFIG_ID, &data)
                    .await
            {
                log::warn!("Failed to save similar video segment config: {:?}", e);
            }
        }
    });
}

async fn load_config() -> Option<SimilarVideoSegmentConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, SIMILAR_VIDEO_SEGMENT_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn video_editor_similar_video_segment_import_file(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(file_paths) = picker_files(
            ui_weak.clone(),
            &tr("Select video"),
            &tr("Video Files"),
            &VIDEO_EXTENSIONS,
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            for filepath in file_paths {
                let filename = filepath
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let item = UISimilarVideoSegmentItem {
                    path: filepath.to_string_lossy().to_string().into(),
                    filename: filename.into(),
                    status: 0,
                    error_msg: SharedString::default(),
                    match_count: 0,
                };
                store_similar_video_segment_items!(ui).push(item);
            }
        });
    });
}

fn video_editor_similar_video_segment_import_folder(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(
            ui_weak.clone(),
            &crate::logic::tr::tr("Choose video folder"),
        ) else {
            return;
        };

        let mut video_files: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dirpath) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type()
                    && file_type.is_file()
                {
                    let path = entry.path();
                    if let Some(ext) = path.extension().and_then(|e| e.to_str())
                        && VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str())
                    {
                        video_files.push(path);
                    }
                }
            }
        }

        video_files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        if video_files.is_empty() {
            async_toast_warn(ui_weak, tr("No video files found in folder"));
            return;
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            for path in video_files {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let item = UISimilarVideoSegmentItem {
                    path: path.to_string_lossy().to_string().into(),
                    filename: filename.into(),
                    status: 0,
                    error_msg: SharedString::default(),
                    match_count: 0,
                };
                store_similar_video_segment_items!(ui).push(item);
            }
        });
    });
}

fn video_editor_similar_video_segment_choose_query_image(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Select query image"),
            &tr("Image Files"),
            &IMAGE_EXTENSIONS,
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_similar_video_segment_query_image_path(
                filepath.to_string_lossy().to_string().into(),
            );
        });
    });
}

fn video_editor_similar_video_segment_process(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_similar_video_segment_config();
    let query_image_path = global_store!(ui)
        .get_video_editor_similar_video_segment_query_image_path()
        .to_string();

    if query_image_path.is_empty() {
        crate::toast_warn!(ui, tr("Please select a query image"));
        return;
    }

    if !Path::new(&query_image_path).exists() {
        crate::toast_warn!(ui, tr("Query image file not found"));
        return;
    }

    let export_dir = config.export_dir.to_string();
    if export_dir.is_empty() {
        let ui_weak = ui.as_weak();
        tokio::spawn(async move {
            let Some(dirpath) = picker_directory(
                ui_weak.clone(),
                &crate::logic::tr::tr("Choose export directory"),
            ) else {
                return;
            };

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                let mut config = global_store!(ui).get_video_editor_similar_video_segment_config();
                config.export_dir = dirpath.to_string_lossy().to_string().into();
                global_store!(ui).set_video_editor_similar_video_segment_config(config.clone());
                save_config(config.into());
                start_processing(&ui);
            });
        });
        return;
    }

    start_processing(ui);
}

fn start_processing(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_similar_video_segment_config();
    let query_image_path = global_store!(ui)
        .get_video_editor_similar_video_segment_query_image_path()
        .to_string();
    let export_dir = config.export_dir.to_string();

    if query_image_path.is_empty() || export_dir.is_empty() {
        return;
    }

    let total_items = store_similar_video_segment_items!(ui).row_count();
    if total_items == 0 {
        crate::toast_warn!(ui, tr("No video files to process"));
        return;
    }

    let video_paths: Vec<PathBuf> = (0..total_items)
        .filter_map(|i| {
            store_similar_video_segment_items!(ui)
                .row_data(i)
                .map(|item| PathBuf::from(item.path.to_string()))
        })
        .collect();

    let keep_audio = config.keep_audio;
    let similar_config = SimilarVideoConfig {
        sample_interval: config.sample_interval.max(1) as u32,
        similarity_threshold: config.similarity_threshold,
        merge_gap_duration: Duration::from_secs(5),
        before_duration: Duration::from_secs_f32(config.before_duration_secs.max(0.0)),
        after_duration: Duration::from_secs_f32(config.after_duration_secs.max(0.0)),
        output_dir: PathBuf::from(&export_dir),
    };

    let cancel_token = CancellationToken::new();
    *CANCEL_TOKEN.lock().unwrap() = Some(cancel_token.clone());

    let inc_index = PROCESS_INC_INDEX.fetch_add(1, Ordering::Relaxed) + 1;

    global_store!(ui).set_video_editor_similar_video_segment_is_progressing(true);
    global_store!(ui).set_video_editor_similar_video_segment_current_index(0);

    // Reset all items to pending status
    for i in 0..total_items {
        if let Some(mut item) = store_similar_video_segment_items!(ui).row_data(i) {
            item.status = 0;
            item.error_msg = SharedString::default();
            item.match_count = 0;
            store_similar_video_segment_items!(ui).set_row_data(i, item);
        }
    }

    let ui_weak = ui.as_weak();
    tokio::task::spawn_blocking(move || {
        let query_path = PathBuf::from(&query_image_path);
        let mut all_matches: Vec<MatchResult> = Vec::new();

        for (idx, video_path) in video_paths.iter().enumerate() {
            if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                return;
            }

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(mut item) = store_similar_video_segment_items!(ui).row_data(idx) {
                    item.status = 1;
                    store_similar_video_segment_items!(ui).set_row_data(idx, item);
                }
            });

            if cancel_token.is_cancelled() {
                break;
            }

            let scan_result = scan_videos(
                &query_path,
                &[video_path.clone()],
                &similar_config,
                Some(cancel_token.clone()),
                |_progress: ScanProgress| {},
            );

            match scan_result {
                Ok(matches) => {
                    let match_count = matches.len() as i32;
                    all_matches.extend(matches.clone());

                    if !matches.is_empty() {
                        _ = export_segments(
                            &matches,
                            &similar_config,
                            Some(cancel_token.clone()),
                            |_progress: ExportProgress| {},
                            keep_audio,
                        );
                    }

                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                            return;
                        }
                        if let Some(mut item) = store_similar_video_segment_items!(ui).row_data(idx)
                        {
                            item.status = 2;
                            item.match_count = match_count;
                            store_similar_video_segment_items!(ui).set_row_data(idx, item);
                        }
                        global_store!(ui)
                            .set_video_editor_similar_video_segment_current_index((idx + 1) as i32);
                    });
                }
                Err(similar_video_segment::Error::Cancelled) => break,
                Err(e) => {
                    let error_msg = format!("{}", e);
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                            return;
                        }
                        if let Some(mut item) = store_similar_video_segment_items!(ui).row_data(idx)
                        {
                            item.status = 3;
                            item.error_msg = error_msg.into();
                            store_similar_video_segment_items!(ui).set_row_data(idx, item);
                        }
                        global_store!(ui)
                            .set_video_editor_similar_video_segment_current_index((idx + 1) as i32);
                    });
                }
            }
        }

        *SCAN_RESULTS.lock().unwrap() = all_matches;

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                return;
            }
            global_store!(ui).set_video_editor_similar_video_segment_is_progressing(false);
            global_store!(ui).set_video_editor_similar_video_segment_current_index(0);
            *CANCEL_TOKEN.lock().unwrap() = None;
        });
    });
}

fn video_editor_similar_video_segment_cancel(ui: &AppWindow) {
    PROCESS_INC_INDEX.fetch_add(1, Ordering::Relaxed);

    if let Some(token) = CANCEL_TOKEN.lock().unwrap().as_ref() {
        token.cancel();
    }

    global_store!(ui).set_video_editor_similar_video_segment_is_progressing(false);
    global_store!(ui).set_video_editor_similar_video_segment_current_index(0);
}

fn video_editor_similar_video_segment_remove_all(ui: &AppWindow) {
    PROCESS_INC_INDEX.fetch_add(1, Ordering::Relaxed);
    *CANCEL_TOKEN.lock().unwrap() = None;
    *SCAN_RESULTS.lock().unwrap() = Vec::new();

    global_store!(ui).set_video_editor_similar_video_segment_is_progressing(false);
    global_store!(ui).set_video_editor_similar_video_segment_current_index(0);
    store_similar_video_segment_items!(ui).set_vec(vec![]);
}

fn video_editor_similar_video_segment_remove_item(ui: &AppWindow, index: i32) {
    let idx = index as usize;
    if idx < store_similar_video_segment_items!(ui).row_count() {
        store_similar_video_segment_items!(ui).remove(idx);
    }
}

fn video_editor_similar_video_segment_choose_export_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(
            ui_weak.clone(),
            &crate::logic::tr::tr("Choose export directory"),
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_similar_video_segment_config();
            config.export_dir = dirpath.to_string_lossy().to_string().into();
            global_store!(ui).set_video_editor_similar_video_segment_config(config.clone());
            save_config(config.into());
        });
    });
}

fn video_editor_similar_video_segment_update_config(
    ui: &AppWindow,
    config: UISimilarVideoSegmentConfig,
) {
    global_store!(ui).set_video_editor_similar_video_segment_config(config.clone());
    save_config(config.into());
}

fn video_editor_similar_video_segment_setting_is_valid(ui: &AppWindow) -> bool {
    let config = global_store!(ui).get_video_editor_similar_video_segment_config();
    let export_dir = config.export_dir.to_string();
    !export_dir.is_empty() && PathBuf::from(&export_dir).exists()
}
