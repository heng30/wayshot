use crate::{
    db::{DedupPhotosConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        downloader::{downloader_cancel, downloader_start},
        recorder::picker_directory,
        share_screen::picker_file,
        tr::tr,
        video_editor::project::DEDUP_PHOTOS_CONFIG_ID,
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, Downloader as UIDownloader, DownloaderState as UIDownloaderState,
        VideoEditorDedupPhotosConfig as UIDedupPhotosConfig,
        VideoEditorDedupPhotosItem as UIDedupPhotosItem,
    },
};
use dedup_photos::{
    CancellationToken, DedupOptions, DedupReason, KeepStrategy, ProgressEvent, SemanticConfig,
    dedup_directory_with,
};
use downloader::DownloadState;
use slint::{ComponentHandle, Model as SlintModel, SharedString, VecModel};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

static PROCESS_INC_INDEX: AtomicU32 = AtomicU32::new(0);
static CANCEL_TOKEN: Mutex<Option<CancellationToken>> = Mutex::new(None);

#[macro_export]
macro_rules! store_dedup_photos_items {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_dedup_photos_items()
            .as_any()
            .downcast_ref::<VecModel<UIDedupPhotosItem>>()
            .expect("VecModel<UIDedupPhotosItem> for dedup photos")
    };
}

#[macro_export]
macro_rules! store_dedup_photos_models_downloader {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_dedup_photos_models_downloader()
            .as_any()
            .downcast_ref::<VecModel<UIDownloader>>()
            .expect("We know we set a VecModel<UIDownloader> earlier for video editor dedup photos models")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_dedup_photos_choose_scan_dir, ui);
    logic_cb!(video_editor_dedup_photos_process, ui);
    logic_cb!(video_editor_dedup_photos_cancel, ui);
    logic_cb!(video_editor_dedup_photos_remove_all, ui);
    logic_cb!(video_editor_dedup_photos_remove_item, ui, index);
    logic_cb!(video_editor_dedup_photos_choose_model_path, ui);
    logic_cb!(video_editor_dedup_photos_update_config, ui, config);
    logic_cb!(
        video_editor_dedup_photos_model_start_download,
        ui,
        index,
        url
    );
    logic_cb!(
        video_editor_dedup_photos_model_cancel_download,
        ui,
        index,
        url
    );
    logic_cb_pure!(video_editor_dedup_photos_setting_is_valid, ui);
}

fn inner_init(ui: &AppWindow) {
    let downloaders = vec![UIDownloader {
        url: dedup_photos::semantic::CLIP_MODEL_URL.into(),
        filename: dedup_photos::semantic::CLIP_MODEL_FILE.into(),
        state: UIDownloaderState::UnStart,
        progress: 0.0,
    }];
    store_dedup_photos_models_downloader!(ui).set_vec(downloaders);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_config()
            .await
            .unwrap_or_else(|| DedupPhotosConfigData {
                id: DEDUP_PHOTOS_CONFIG_ID.to_string(),
                ..Default::default()
            });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_dedup_photos_config(config.into());
        });
    });
}

fn save_config(config: DedupPhotosConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize dedup photos config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, DEDUP_PHOTOS_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, DEDUP_PHOTOS_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save dedup photos config: {:?}", e);
            }
        }
    });
}

async fn load_config() -> Option<DedupPhotosConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, DEDUP_PHOTOS_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn reason_text(reason: &DedupReason) -> String {
    match reason {
        DedupReason::Exact => tr("Exact duplicate").to_string(),
        DedupReason::Perceptual { hamming_distance } => format!(
            "{} (Hamming distance {}/{})",
            tr("Perceptual duplicate"),
            hamming_distance,
            dedup_photos::hash::DHASH_BITS
        ),
        DedupReason::Semantic { cosine_similarity } => format!(
            "{} (cosine similarity {:.3})",
            tr("Semantic duplicate"),
            cosine_similarity
        ),
    }
}

fn scan_image_files(root: &Path) -> Vec<PathBuf> {
    dedup_photos::scan::collect_files(root, false, &["duplicate".to_string()], None)
        .into_iter()
        .map(|f| f.path)
        .collect()
}

fn fill_items_from_dir(ui: &AppWindow, dir: &Path) {
    let files = scan_image_files(dir);
    let items = files
        .into_iter()
        .map(|path| {
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            UIDedupPhotosItem {
                path: path.to_string_lossy().to_string().into(),
                filename: filename.into(),
                status: 0,
                error_msg: SharedString::default(),
                result_msg: SharedString::default(),
            }
        })
        .collect::<Vec<_>>();
    store_dedup_photos_items!(ui).set_vec(items);
}

fn video_editor_dedup_photos_choose_scan_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(ui_weak.clone(), &tr("Choose scan directory")) else {
            return;
        };

        let scan_dir = dirpath.to_string_lossy().to_string();
        let dir_for_fill = dirpath.clone();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_dedup_photos_config();
            config.scan_dir = scan_dir.into();
            global_store!(ui).set_video_editor_dedup_photos_config(config.clone());
            save_config(config.into());
            fill_items_from_dir(&ui, &dir_for_fill);
        });
    });
}

fn video_editor_dedup_photos_process(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_dedup_photos_config();
    let scan_dir = config.scan_dir.to_string();

    if scan_dir.is_empty() || !Path::new(&scan_dir).is_dir() {
        crate::toast_warn!(ui, tr("Please select a valid directory"));
        return;
    }

    let semantic_enabled = config.semantic_enabled;
    let model_path = config.model_path.to_string();
    if semantic_enabled && (model_path.is_empty() || !Path::new(&model_path).exists()) {
        crate::toast_warn!(ui, tr("CLIP model file not found"));
        return;
    }

    let total_items = store_dedup_photos_items!(ui).row_count();
    if total_items == 0 {
        crate::toast_warn!(ui, tr("No files to process"));
        return;
    }

    let options = DedupOptions {
        threshold: config.threshold.max(1) as u32,
        semantic: if semantic_enabled {
            Some(SemanticConfig {
                model_path: PathBuf::from(&model_path),
                threshold: config.semantic_threshold,
            })
        } else {
            None
        },
        keep: match config.keep_strategy {
            1 => KeepStrategy::Newest,
            2 => KeepStrategy::Oldest,
            _ => KeepStrategy::Largest,
        },
        all_files: config.all_files,
        duplicate_dir_name: if config.duplicate_dir_name.is_empty() {
            "duplicate".to_string()
        } else {
            config.duplicate_dir_name.to_string()
        },
    };

    let cancel_token = CancellationToken::new();
    *CANCEL_TOKEN.lock().unwrap() = Some(cancel_token.clone());

    let inc_index = PROCESS_INC_INDEX.fetch_add(1, Ordering::Relaxed) + 1;

    global_store!(ui).set_video_editor_dedup_photos_is_progressing(true);
    global_store!(ui).set_video_editor_dedup_photos_progress(0.0);
    global_store!(ui).set_video_editor_dedup_photos_current_index(0);

    for i in 0..total_items {
        if let Some(mut item) = store_dedup_photos_items!(ui).row_data(i) {
            item.status = 1;
            item.error_msg = SharedString::default();
            item.result_msg = SharedString::default();
            store_dedup_photos_items!(ui).set_row_data(i, item);
        }
    }

    let ui_weak = ui.as_weak();
    tokio::task::spawn_blocking(move || {
        let root = PathBuf::from(&scan_dir);
        let total_stages = if semantic_enabled { 6.0 } else { 5.0 };
        let finished_stages = AtomicU32::new(0);

        let progress_weak = ui_weak.clone();
        let progress = move |event: ProgressEvent| {
            let value = match event {
                ProgressEvent::StageStarted { .. } => {
                    finished_stages.load(Ordering::Relaxed) as f32 / total_stages
                }
                ProgressEvent::ItemDone { done, total, .. } => {
                    let base = finished_stages.load(Ordering::Relaxed) as f32;
                    if total > 0 {
                        (base + done as f32 / total as f32) / total_stages
                    } else {
                        (base + 1.0) / total_stages
                    }
                }
                ProgressEvent::StageFinished { .. } => {
                    (finished_stages.fetch_add(1, Ordering::Relaxed) + 1) as f32 / total_stages
                }
            };

            _ = progress_weak.upgrade_in_event_loop(move |ui| {
                if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                    return;
                }
                global_store!(ui).set_video_editor_dedup_photos_progress(value);
            });
        };

        let result = dedup_directory_with(&root, &options, Some(&progress), Some(&cancel_token));

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if inc_index != PROCESS_INC_INDEX.load(Ordering::Relaxed) {
                return;
            }

            match &result {
                Ok(res) => {
                    let mut moved: HashMap<String, String> = HashMap::new();
                    for d in &res.duplicates {
                        moved.insert(d.path.to_string_lossy().to_string(), reason_text(&d.reason));
                    }

                    let count = store_dedup_photos_items!(ui).row_count();
                    let mut items = Vec::with_capacity(count);
                    for i in 0..count {
                        if let Some(mut item) = store_dedup_photos_items!(ui).row_data(i) {
                            item.status = 2;
                            if let Some(reason) = moved.get(&item.path.to_string()) {
                                item.result_msg = reason.clone().into();
                            } else {
                                item.result_msg = tr("kept").into();
                            }
                            items.push(item);
                        }
                    }

                    let kept_text = tr("kept");
                    items.sort_by(|a, b| {
                        let rank = |item: &UIDedupPhotosItem| -> (u8, u8) {
                            let status_rank = match item.status {
                                3 => 0,
                                2 => 1,
                                _ => 2,
                            };
                            let kept_rank =
                                if item.status == 2 && item.result_msg.as_str() == kept_text {
                                    0
                                } else {
                                    1
                                };
                            (status_rank, kept_rank)
                        };
                        rank(a).cmp(&rank(b))
                    });
                    store_dedup_photos_items!(ui).set_vec(items);

                    global_store!(ui).set_video_editor_dedup_photos_progress(1.0);

                    let summary = &res.summary;
                    if summary.moved_files > 0 {
                        crate::toast_success!(
                            ui,
                            format!(
                                "{}: {} {}, {} {}",
                                tr("Photo dedup finished"),
                                summary.moved_files,
                                tr("file(s) moved"),
                                summary.groups,
                                tr("group(s) found")
                            )
                        );
                    } else {
                        crate::toast_success!(ui, tr("No duplicates found"));
                    }
                }
                Err(dedup_photos::DedupError::Cancelled) => {
                    crate::toast_warn!(ui, tr("Operation cancelled"));
                }
                Err(e) => {
                    crate::toast_warn!(ui, format!("{}: {}", tr("Photo dedup failed"), e));
                }
            }

            global_store!(ui).set_video_editor_dedup_photos_is_progressing(false);
            global_store!(ui).set_video_editor_dedup_photos_current_index(0);
            *CANCEL_TOKEN.lock().unwrap() = None;
        });
    });
}

fn video_editor_dedup_photos_cancel(ui: &AppWindow) {
    PROCESS_INC_INDEX.fetch_add(1, Ordering::Relaxed);

    if let Some(token) = CANCEL_TOKEN.lock().unwrap().as_ref() {
        token.cancel();
    }

    global_store!(ui).set_video_editor_dedup_photos_is_progressing(false);
    global_store!(ui).set_video_editor_dedup_photos_current_index(0);
}

fn video_editor_dedup_photos_remove_all(ui: &AppWindow) {
    PROCESS_INC_INDEX.fetch_add(1, Ordering::Relaxed);
    *CANCEL_TOKEN.lock().unwrap() = None;

    global_store!(ui).set_video_editor_dedup_photos_is_progressing(false);
    global_store!(ui).set_video_editor_dedup_photos_current_index(0);
    store_dedup_photos_items!(ui).set_vec(vec![]);
}

fn video_editor_dedup_photos_remove_item(ui: &AppWindow, index: i32) {
    let idx = index as usize;
    if idx < store_dedup_photos_items!(ui).row_count() {
        store_dedup_photos_items!(ui).remove(idx);
    }
}

fn video_editor_dedup_photos_choose_model_path(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Select CLIP model"),
            &tr("ONNX Model Files"),
            &["onnx"],
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_dedup_photos_config();
            config.model_path = filepath.to_string_lossy().to_string().into();
            global_store!(ui).set_video_editor_dedup_photos_config(config.clone());
            save_config(config.into());
        });
    });
}

fn video_editor_dedup_photos_update_config(ui: &AppWindow, config: UIDedupPhotosConfig) {
    global_store!(ui).set_video_editor_dedup_photos_config(config.clone());
    save_config(config.into());
}

fn video_editor_dedup_photos_setting_is_valid(ui: &AppWindow) -> bool {
    let config = global_store!(ui).get_video_editor_dedup_photos_config();
    if config.semantic_enabled {
        let model_path = config.model_path.to_string();
        if model_path.is_empty() || !PathBuf::from(&model_path).exists() {
            return false;
        }
    }
    true
}

fn video_editor_dedup_photos_model_start_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;
    let filename = dedup_photos::semantic::CLIP_MODEL_FILE.into();

    downloader_start(
        ui,
        url,
        filename,
        move |ui: &AppWindow, _downloaded: u64, _total: u64, progress: f32| {
            if let Some(mut item) = store_dedup_photos_models_downloader!(ui).row_data(index) {
                item.progress = progress;
                store_dedup_photos_models_downloader!(ui).set_row_data(index, item);
            }
        },
        move |ui: &AppWindow, filepath: PathBuf| {
            if let Some(mut item) = store_dedup_photos_models_downloader!(ui).row_data(index) {
                item.state = UIDownloaderState::Downloading;
                store_dedup_photos_models_downloader!(ui).set_row_data(index, item);
            }

            let filepath_str = filepath.to_string_lossy().to_string().into();
            let mut config = global_store!(ui).get_video_editor_dedup_photos_config();
            config.model_path = filepath_str;
            global_store!(ui).set_video_editor_dedup_photos_config(config.clone());
            save_config(config.into());
        },
        move |ui: &AppWindow, result: downloader::Result<downloader::DownloadState>| {
            if let Some(mut item) = store_dedup_photos_models_downloader!(ui).row_data(index) {
                match result {
                    Ok(DownloadState::Cancelled) => item.state = UIDownloaderState::Cancelled,
                    Ok(DownloadState::Incompleted) => item.state = UIDownloaderState::Failed,
                    Ok(DownloadState::Finsished) => item.state = UIDownloaderState::Finished,
                    Err(_) => item.state = UIDownloaderState::Failed,
                }
                store_dedup_photos_models_downloader!(ui).set_row_data(index, item);
            }
        },
    );
}

fn video_editor_dedup_photos_model_cancel_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;

    downloader_cancel(ui, url, move |ui: &AppWindow| {
        if let Some(mut item) = store_dedup_photos_models_downloader!(ui).row_data(index) {
            item.state = UIDownloaderState::Cancelled;
            store_dedup_photos_models_downloader!(ui).set_row_data(index, item);
        }
    });
}
