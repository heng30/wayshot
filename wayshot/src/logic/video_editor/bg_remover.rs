use crate::{
    db::{BgRemoverConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        downloader::{downloader_cancel, downloader_start},
        recorder::picker_directory,
        share_screen::picker_file,
        toast,
        tr::tr,
        video_editor::{
            img_animation::IMAGE_EXTENSIONS, playlist::import_file_to_playlist,
            project::BG_REMOVER_CONFIG_ID,
        },
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, Downloader as UIDownloader, DownloaderState as UIDownloaderState,
        VideoEditorBgRemoverConfig as UIBgRemoverConfig,
    },
};
use anyhow::{Context, Result, bail};
use background_remover::BackgroundRemover;
use downloader::DownloadState;
use slint::{ComponentHandle, Model as SlintModel, SharedString, VecModel};
use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

#[macro_export]
macro_rules! store_video_editor_bg_remover_models_downloader {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_bg_remover_models_downloader()
            .as_any()
            .downcast_ref::<VecModel<UIDownloader>>()
            .expect("We know we set a VecModel<UIDownloader> earlier for video editor bg remover models")
    };
}

static PROCESS_ID: AtomicU32 = AtomicU32::new(1);
static IMPORTED_IMAGE_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_bg_remover_import_image, ui);
    logic_cb!(video_editor_bg_remover_process_image, ui);
    logic_cb!(video_editor_bg_remover_process_cancel, ui);
    logic_cb!(video_editor_bg_remover_export_to_playlist, ui);
    logic_cb!(video_editor_bg_remover_remove_all_images, ui);
    logic_cb!(video_editor_bg_remover_choose_export_dir, ui);
    logic_cb!(video_editor_bg_remover_choose_model_path, ui, index);
    logic_cb!(video_editor_bg_remover_update_config, ui, config);
    logic_cb!(video_editor_bg_remover_model_start_download, ui, index, url);
    logic_cb!(
        video_editor_bg_remover_model_cancel_download,
        ui,
        index,
        url
    );
    logic_cb_pure!(video_editor_bg_remover_setting_is_valid, ui);
}

fn inner_init(ui: &AppWindow) {
    let downloaders = background_remover::Model::all_models()
        .into_iter()
        .map(|m| UIDownloader {
            url: m.download_url().to_string().into(),
            filename: m.to_filename().to_string().into(),
            state: UIDownloaderState::UnStart,
            progress: 0.0,
        })
        .collect::<Vec<_>>();
    store_video_editor_bg_remover_models_downloader!(ui).set_vec(downloaders);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_bg_remover_config()
            .await
            .unwrap_or_else(|| BgRemoverConfigData {
                id: BG_REMOVER_CONFIG_ID.to_string(),
                ..Default::default()
            });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_bg_remover_config(config.into());
        });
    });
}

fn save_bg_remover_config(config: BgRemoverConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize bg remover config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, BG_REMOVER_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, BG_REMOVER_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save bg remover config: {:?}", e);
            }
        }
    });
}

async fn load_bg_remover_config() -> Option<BgRemoverConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, BG_REMOVER_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn video_editor_bg_remover_import_image(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Select image"),
            &tr("Image Files"),
            &IMAGE_EXTENSIONS,
        ) else {
            return;
        };

        *IMPORTED_IMAGE_PATH.lock().unwrap() = Some(filepath.clone());

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if let Ok(img) = image::open(&filepath) {
                let rgba_img = img.to_rgba8();
                let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    rgba_img.as_raw(),
                    rgba_img.width(),
                    rgba_img.height(),
                );
                global_store!(ui)
                    .set_video_editor_bg_remover_image(slint::Image::from_rgba8(buffer));
                global_store!(ui)
                    .set_video_editor_bg_remover_processed_image(slint::Image::default());
            }
        });
    });
}

fn video_editor_bg_remover_process_image(ui: &AppWindow) {
    let model_path = match get_and_check_model_setting(ui) {
        Ok(path) => path,
        Err(e) => {
            global_store!(ui).set_video_editor_is_show_bg_remover_setting_dialog(true);
            crate::toast_warn!(ui, format!("{e}"));
            return;
        }
    };

    let Some(filepath) = IMPORTED_IMAGE_PATH.lock().unwrap().clone() else {
        crate::toast_warn!(ui, tr("No image imported"));
        return;
    };

    let ui_weak = ui.as_weak();
    let model_index = global_store!(ui)
        .get_video_editor_bg_remover_config()
        .selected_model_index;
    global_store!(ui).set_video_editor_bg_remover_processed_image(slint::Image::default());
    global_store!(ui).set_video_editor_bg_remover_is_processing(true);
    global_store!(ui).set_video_editor_bg_remover_progress(0.0);

    let process_id = PROCESS_ID.fetch_add(1, Ordering::Relaxed) + 1;

    tokio::spawn(async move {
        let model = if model_index == 0 {
            background_remover::Model::Modnet
        } else {
            background_remover::Model::Rmbg14
        };

        let mut remover = match BackgroundRemover::new(model, &model_path) {
            Ok(r) => r,
            Err(e) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    crate::toast_warn!(ui, format!("{}: {e}", tr("Failed to load model")));
                    global_store!(ui).set_video_editor_bg_remover_is_processing(false);
                });
                return;
            }
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_bg_remover_progress(0.3);
        });

        match process_image(&mut remover, &filepath) {
            Ok(processed_img) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if !global_store!(ui).get_video_editor_bg_remover_is_processing() {
                        return;
                    }

                    if PROCESS_ID.load(Ordering::Relaxed) != process_id {
                        return;
                    }

                    global_store!(ui).set_video_editor_bg_remover_progress(0.8);

                    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                        processed_img.as_raw(),
                        processed_img.width(),
                        processed_img.height(),
                    );
                    global_store!(ui).set_video_editor_bg_remover_is_processing(false);
                    global_store!(ui).set_video_editor_bg_remover_progress(1.0);
                    global_store!(ui).set_video_editor_bg_remover_processed_image(
                        slint::Image::from_rgba8(buffer),
                    );
                });
            }
            Err(e) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if PROCESS_ID.load(Ordering::Relaxed) != process_id {
                        return;
                    }

                    if global_store!(ui).get_video_editor_bg_remover_is_processing() {
                        global_store!(ui).set_video_editor_bg_remover_is_processing(false);
                        crate::toast_warn!(ui, format!("{}: {e}", tr("Failed to process image")));
                    }
                });
            }
        }
    });
}

fn process_image(remover: &mut BackgroundRemover, img_path: &PathBuf) -> Result<image::RgbaImage> {
    let img = image::open(img_path).context("Failed to open image")?;
    let rgb_img = img.to_rgb8();

    let result = remover
        .remove(&rgb_img)
        .context("Failed to remove background")?;

    Ok(result)
}

fn video_editor_bg_remover_export_to_playlist(ui: &AppWindow) {
    let processed_image = global_store!(ui).get_video_editor_bg_remover_processed_image();
    let Some(buffer) = processed_image.to_rgba8() else {
        crate::toast_warn!(ui, tr("No processed image to export"));
        return;
    };

    let Some(filepath) = IMPORTED_IMAGE_PATH.lock().unwrap().clone() else {
        crate::toast_warn!(ui, tr("No original image path stored"));
        return;
    };

    let filename = filepath
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("output");
    let config = global_store!(ui).get_video_editor_bg_remover_config();
    let export_dir = config.export_dir.to_string();
    let output_path = PathBuf::from(export_dir).join(format!("{}_nobg.png", filename));

    let Some(rgba_img): Option<image::RgbaImage> =
        image::ImageBuffer::from_raw(buffer.width(), buffer.height(), buffer.as_bytes().to_vec())
    else {
        crate::toast_warn!(ui, tr("Failed to create RgbaImage from buffer"));
        return;
    };

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        if let Err(e) = rgba_img.save(&output_path) {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}: {}", tr("Failed to save image"), e),
            );
            return;
        }

        import_file_to_playlist(ui_weak.clone(), output_path.clone(), None).await;
        toast::async_toast_success(
            ui_weak,
            format!("{} {}", tr("Exported to"), output_path.display()),
        );
    });
}

fn video_editor_bg_remover_remove_all_images(ui: &AppWindow) {
    PROCESS_ID.fetch_add(1, Ordering::Relaxed);
    global_store!(ui).set_video_editor_bg_remover_is_processing(false);
    global_store!(ui).set_video_editor_bg_remover_image(slint::Image::default());
    global_store!(ui).set_video_editor_bg_remover_processed_image(slint::Image::default());
    *IMPORTED_IMAGE_PATH.lock().unwrap() = None;
}

fn video_editor_bg_remover_process_cancel(ui: &AppWindow) {
    PROCESS_ID.fetch_add(1, Ordering::Relaxed);
    global_store!(ui).set_video_editor_bg_remover_is_processing(false);
    global_store!(ui).set_video_editor_bg_remover_progress(0.0);
}

fn video_editor_bg_remover_choose_export_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(
            ui_weak.clone(),
            &crate::logic::tr::tr("Choose export directory"),
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_bg_remover_config();
            config.export_dir = dirpath.to_string_lossy().to_string().into();
            global_store!(ui).set_video_editor_bg_remover_config(config.clone());
            save_bg_remover_config(config.into());
        });
    });
}

fn video_editor_bg_remover_choose_model_path(ui: &AppWindow, index: i32) {
    let ui_weak = ui.as_weak();
    let title = if index == 0 {
        tr("Choose modnet model")
    } else {
        tr("Choose rmbg model")
    };

    tokio::spawn(async move {
        let Some(filepath) = picker_file(ui_weak.clone(), &title, &tr("ONNX Model"), &["onnx"])
        else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let filepath_str = filepath.to_string_lossy().to_string().into();
            let mut config = global_store!(ui).get_video_editor_bg_remover_config();
            match index {
                0 => config.modnet_path = filepath_str,
                1 => config.rmbg_path = filepath_str,
                _ => log::warn!("Unexpected bg remover model index = {index}"),
            }
            global_store!(ui).set_video_editor_bg_remover_config(config.clone());
            save_bg_remover_config(config.into());
        });
    });
}

fn video_editor_bg_remover_model_start_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;
    let filename = background_remover::Model::all_models()[index]
        .to_filename()
        .into();

    downloader_start(
        ui,
        url,
        filename,
        move |ui: &AppWindow, _downloaded: u64, _total: u64, progress: f32| {
            if let Some(mut item) =
                store_video_editor_bg_remover_models_downloader!(ui).row_data(index)
            {
                item.progress = progress;
                store_video_editor_bg_remover_models_downloader!(ui).set_row_data(index, item);
            }
        },
        move |ui: &AppWindow, filepath: PathBuf| {
            if let Some(mut item) =
                store_video_editor_bg_remover_models_downloader!(ui).row_data(index)
            {
                item.state = UIDownloaderState::Downloading;
                store_video_editor_bg_remover_models_downloader!(ui).set_row_data(index, item);
            }

            let filepath_str = filepath.to_string_lossy().to_string().into();
            let mut config = global_store!(ui).get_video_editor_bg_remover_config();
            match index {
                0 => config.modnet_path = filepath_str,
                1 => config.rmbg_path = filepath_str,
                _ => log::warn!("Unexpected bg remover model index = {index}"),
            }
            global_store!(ui).set_video_editor_bg_remover_config(config.clone());
            save_bg_remover_config(config.into());
        },
        move |ui: &AppWindow, result: downloader::Result<downloader::DownloadState>| {
            if let Some(mut item) =
                store_video_editor_bg_remover_models_downloader!(ui).row_data(index)
            {
                match result {
                    Ok(DownloadState::Cancelled) => item.state = UIDownloaderState::Cancelled,
                    Ok(DownloadState::Incompleted) => item.state = UIDownloaderState::Failed,
                    Ok(DownloadState::Finsished) => item.state = UIDownloaderState::Finished,
                    Err(_) => item.state = UIDownloaderState::Failed,
                }
                store_video_editor_bg_remover_models_downloader!(ui).set_row_data(index, item);
            }
        },
    );
}

fn video_editor_bg_remover_model_cancel_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;

    downloader_cancel(ui, url, move |ui: &AppWindow| {
        if let Some(mut item) = store_video_editor_bg_remover_models_downloader!(ui).row_data(index)
        {
            item.state = UIDownloaderState::Cancelled;
            store_video_editor_bg_remover_models_downloader!(ui).set_row_data(index, item);
        }
    });
}

fn video_editor_bg_remover_setting_is_valid(ui: &AppWindow) -> bool {
    get_and_check_model_setting(ui).is_ok()
}

fn video_editor_bg_remover_update_config(ui: &AppWindow, config: UIBgRemoverConfig) {
    global_store!(ui).set_video_editor_bg_remover_config(config.clone());
    save_bg_remover_config(config.into());
}

fn get_and_check_model_setting(ui: &AppWindow) -> Result<PathBuf> {
    let config = global_store!(ui).get_video_editor_bg_remover_config();
    let model_path = if config.selected_model_index == 0 {
        config.modnet_path.to_string()
    } else {
        config.rmbg_path.to_string()
    };

    if model_path.is_empty() {
        bail!(tr("Please select a model file").to_string());
    }

    let path = PathBuf::from(&model_path);
    if !path.exists() {
        bail!(tr("Model file not found").to_string());
    }

    Ok(path)
}
