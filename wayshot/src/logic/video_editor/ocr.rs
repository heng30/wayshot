use super::project::OCR_CONFIG_ID;
use crate::{
    db::{OcrConfigData, VIDEO_EDITOR_TABLE},
    global_logic, global_store,
    logic::{
        recorder::picker_directory,
        share_screen::picker_file,
        toast::{self},
        tr::tr,
        video_editor::img_animation::IMAGE_EXTENSIONS,
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, Downloader as UIDownloader, DownloaderState as UIDownloaderState,
        OcrTaskMode as UIOcrTaskMode, OcrTextBlock as UIOcrTextBlock,
        VideoEditorOCRConfig as UIOcrConfig,
    },
};
use anyhow::Result;
use paddle_ocr_rs::{OcrResult, OcrTask};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};

static OCR_MODEL: OnceLock<Mutex<Option<paddle_ocr_rs::PaddleOCR>>> = OnceLock::new();

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_ocr_import_image, ui);
    logic_cb!(video_editor_ocr_preview_screenshot, ui);
    logic_cb!(video_editor_ocr_process_image, ui);
    logic_cb!(video_editor_ocr_copy_text, ui);
    logic_cb!(video_editor_ocr_copy_json, ui);
    logic_cb!(video_editor_ocr_clear, ui);
    logic_cb!(video_editor_ocr_choose_model_dir, ui);
    logic_cb!(video_editor_ocr_update_config, ui, config);
    logic_cb!(video_editor_ocr_model_start_download, ui, url);
    logic_cb_pure!(video_editor_ocr_setting_is_valid, ui);
}

fn inner_init(ui: &AppWindow) {
    let downloader = UIDownloader {
        url: "https://huggingface.co/PaddlePaddle/PaddleOCR-VL-1.5"
            .to_string()
            .into(),
        filename: "PaddleOCR-VL-1.5".to_string().into(),
        state: UIDownloaderState::UnStart,
        progress: 0.0,
    };
    global_store!(ui)
        .set_video_editor_ocr_models_downloader(ModelRc::new(VecModel::from_slice(&[downloader])));

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_ocr_config().await.unwrap_or_else(|| OcrConfigData {
            id: OCR_CONFIG_ID.to_string(),
            ..Default::default()
        });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_ocr_config(config.into());
        });
    });

    OCR_MODEL
        .set(Mutex::new(None))
        .ok()
        .expect("OCR_MODEL already initialized");
}

fn video_editor_ocr_import_image(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Choose image for OCR"),
            "image.png",
            &IMAGE_EXTENSIONS,
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if let Ok(img) = image::open(&filepath) {
                let rgba_img = img.to_rgba8();
                let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    rgba_img.as_raw(),
                    rgba_img.width(),
                    rgba_img.height(),
                );
                global_store!(ui).set_video_editor_ocr_image(slint::Image::from_rgba8(buffer));
            }
        });
    });
}

fn video_editor_ocr_preview_screenshot(ui: &AppWindow) {
    let preview = global_store!(ui).get_video_editor_preview_image();
    if preview.to_rgba8().is_none() {
        crate::toast_warn!(ui, tr("No preview image available"));
        return;
    }
    global_store!(ui).set_video_editor_ocr_image(preview);
}

fn video_editor_ocr_process_image(ui: &AppWindow) {
    let model_dir = match get_and_check_model_setting(ui) {
        Ok(item) => item,
        Err(e) => {
            global_store!(ui).set_video_editor_is_show_ocr_dialog(true);
            crate::toast_warn!(ui, format!("{e}"));
            return;
        }
    };

    let ocr_image = global_store!(ui).get_video_editor_ocr_image();
    let Some(img) = slint_image_to_dynamic(ocr_image) else {
        crate::toast_warn!(ui, tr("No image imported"));
        return;
    };

    let config = global_store!(ui).get_video_editor_ocr_config();
    let task_mode = config.task_mode;
    let timeout_secs = config.timeout.max(300) as u64;

    global_store!(ui).set_video_editor_ocr_text("".into());
    global_store!(ui).set_video_editor_ocr_blocks(ModelRc::new(VecModel::from_slice(&[])));
    global_store!(ui).set_video_editor_ocr_is_processing(true);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        if let Err(e) = get_or_load_model(&model_dir).await {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}: {e}", tr("Failed to load OCR model")),
            );
            async_set_processing(ui_weak.clone(), false);
            return;
        }

        let result: Result<OcrResult, paddle_ocr_rs::Error> = match tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            tokio::task::spawn_blocking(move || {
                log::info!(
                    "OCR: starting inference, img size: {}x{}",
                    img.width(),
                    img.height()
                );

                let model_store = OCR_MODEL.get().unwrap();
                let mut guard = model_store.lock().unwrap();

                match guard.as_mut() {
                    Some(model) => match task_mode {
                        UIOcrTaskMode::Spotting => {
                            model.ocr_with_task_inner(img, OcrTask::Spotting).map(|r| r)
                        }
                        UIOcrTaskMode::Text => {
                            model.ocr_with_task_inner(img, OcrTask::Text).map(|r| r)
                        }
                    },
                    None => Err(paddle_ocr_rs::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Model not loaded",
                    ))),
                }
            }),
        )
        .await
        {
            Ok(Ok(inner)) => inner,
            Ok(Err(join_err)) => Err(paddle_ocr_rs::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Task join error: {join_err}"),
            ))),
            Err(_) => {
                log::warn!("OCR: inference timed out after {timeout_secs}s");
                toast::async_toast_warn(ui_weak.clone(), tr("OCR timed out, try a smaller image"));
                async_set_processing(ui_weak, false);
                return;
            }
        };

        log::info!("{result:?}");

        match result {
            Ok(ocr_result) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_video_editor_ocr_text(ocr_result.text.into());
                    let ui_blocks: Vec<UIOcrTextBlock> = ocr_result
                        .blocks
                        .iter()
                        .map(|b| {
                            let (x1, y1, x2, y2) = b.bbox.map_or((0, 0, 0, 0), |bb| {
                                (bb.x1 as i32, bb.y1 as i32, bb.x2 as i32, bb.y2 as i32)
                            });
                            UIOcrTextBlock {
                                text: b.text.clone().into(),
                                x1,
                                y1,
                                x2,
                                y2,
                            }
                        })
                        .collect();
                    global_store!(ui)
                        .set_video_editor_ocr_blocks(ModelRc::new(VecModel::from(ui_blocks)));
                    global_store!(ui).set_video_editor_ocr_is_processing(false);
                    crate::toast_success!(ui, tr("OCR completed"));
                });
            }
            Err(e) => {
                log::warn!("OCR failed: {e}");
                toast::async_toast_warn(ui_weak.clone(), format!("{}: {e}", tr("OCR failed")));
                async_set_processing(ui_weak, false);
            }
        }
    });
}

fn video_editor_ocr_copy_text(ui: &AppWindow) {
    let text = global_store!(ui).get_video_editor_ocr_text();
    if !text.is_empty() {
        global_logic!(ui).invoke_copy_to_clipboard(text.to_string().into());
        crate::toast_success!(ui, tr("Text copied"));
    }
}

fn video_editor_ocr_copy_json(ui: &AppWindow) {
    let blocks = global_store!(ui).get_video_editor_ocr_blocks();
    let text = global_store!(ui).get_video_editor_ocr_text();

    if text.is_empty() {
        return;
    }

    let json_blocks: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let bbox = if b.x1 == 0 && b.y1 == 0 && b.x2 == 0 && b.y2 == 0 {
                serde_json::Value::Null
            } else {
                serde_json::json!({
                    "x1": b.x1,
                    "y1": b.y1,
                    "x2": b.x2,
                    "y2": b.y2,
                })
            };
            serde_json::json!({
                "text": b.text.to_string(),
                "bbox": bbox,
            })
        })
        .collect();

    let json_value = serde_json::json!({
        "text": text.to_string(),
        "blocks": json_blocks,
    });

    let json_str = serde_json::to_string_pretty(&json_value).unwrap_or_default();
    global_logic!(ui).invoke_copy_to_clipboard(json_str.into());
    crate::toast_success!(ui, tr("JSON copied"));
}

fn video_editor_ocr_clear(ui: &AppWindow) {
    global_store!(ui).set_video_editor_ocr_image(slint::Image::default());
    global_store!(ui).set_video_editor_ocr_text("".into());
    global_store!(ui).set_video_editor_ocr_blocks(ModelRc::new(VecModel::from_slice(&[])));

    if !global_store!(ui).get_video_editor_ocr_is_processing() {
        *OCR_MODEL.get().unwrap().lock().unwrap() = None;
    }
}

fn video_editor_ocr_update_config(ui: &AppWindow, config: UIOcrConfig) {
    global_store!(ui).set_video_editor_ocr_config(config.clone());
    save_ocr_config(config.into());
}

fn video_editor_ocr_choose_model_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(dir) = picker_directory(ui_weak.clone(), &tr("Choose model directory")) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut setting = global_store!(ui).get_video_editor_ocr_config();
            setting.model_dir = dir.to_string_lossy().to_string().into();
            global_logic!(ui).invoke_video_editor_ocr_update_config(setting);
        });
    });
}

fn video_editor_ocr_model_start_download(ui: &AppWindow, url: SharedString) {
    let ui_weak = ui.as_weak();

    let model_id = url
        .to_string()
        .strip_prefix("https://huggingface.co/")
        .unwrap_or(&url.to_string())
        .to_string();

    tokio::spawn(async move {
        let Some(base_dir) = picker_directory(ui_weak.clone(), &tr("Choose model save directory"))
        else {
            return;
        };

        let target_dir = base_dir.join("PaddleOCR-VL1.5");
        if !target_dir.exists()
            && let Err(e) = std::fs::create_dir_all(&target_dir)
        {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!(
                    "{}. {}: {}",
                    tr("Failed to create directory"),
                    tr("Reason"),
                    e
                ),
            );
            return;
        }

        let model_id_clone = model_id.clone();
        let target_dir_clone = target_dir.clone();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let downloaders = global_store!(ui).get_video_editor_ocr_models_downloader();
            if let Some(mut item) = downloaders.row_data(0) {
                item.state = UIDownloaderState::Downloading;
                downloaders.set_row_data(0, item);
            }
        });

        tokio::task::spawn_blocking(move || {
            let api = hf_hub::HFClient::builder()
                .build_sync()
                .expect("Failed to build hf_hub API");

            let (owner, name) = hf_hub::split_id(&model_id_clone);
            let repo = api.model(owner, name);

            let files_to_download = [
                "config.json",
                "preprocessor_config.json",
                "tokenizer.json",
                "model.safetensors",
            ];

            let mut downloaded_count = 0;

            for filename in files_to_download {
                match repo.download_file().filename(filename).send() {
                    Ok(cache_path) => {
                        let target_path = target_dir_clone.join(filename);
                        if let Err(e) = std::fs::copy(&cache_path, &target_path) {
                            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                                let downloaders =
                                    global_store!(ui).get_video_editor_ocr_models_downloader();
                                if let Some(mut item) = downloaders.row_data(0) {
                                    item.state = UIDownloaderState::Failed;
                                    downloaders.set_row_data(0, item);
                                }

                                crate::toast_warn!(
                                    ui,
                                    format!(
                                        "{} {}: {} {}",
                                        tr("Failed to copy"),
                                        filename,
                                        tr("to target"),
                                        e
                                    )
                                );
                            });
                            return;
                        }

                        downloaded_count += 1;
                        let progress = downloaded_count as f32 / files_to_download.len() as f32;

                        _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            let downloaders =
                                global_store!(ui).get_video_editor_ocr_models_downloader();
                            if let Some(mut item) = downloaders.row_data(0) {
                                item.progress = progress;
                                downloaders.set_row_data(0, item);
                            }
                        });
                    }
                    Err(e) => {
                        log::warn!("Failed to download {}: {}", filename, e);
                        _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            let downloaders =
                                global_store!(ui).get_video_editor_ocr_models_downloader();
                            if let Some(mut item) = downloaders.row_data(0) {
                                item.state = UIDownloaderState::Failed;
                                downloaders.set_row_data(0, item);
                            }
                            crate::toast_warn!(
                                ui,
                                format!("{}: {}", tr("Failed to download model"), e)
                            );
                        });
                        return;
                    }
                }
            }

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                let downloaders = global_store!(ui).get_video_editor_ocr_models_downloader();
                if let Some(mut item) = downloaders.row_data(0) {
                    item.state = UIDownloaderState::Finished;
                    item.progress = 1.0;
                    downloaders.set_row_data(0, item);
                }

                let mut setting = global_store!(ui).get_video_editor_ocr_config();
                setting.model_dir = target_dir_clone.to_string_lossy().to_string().into();
                global_logic!(ui).invoke_video_editor_ocr_update_config(setting);
                crate::toast_success!(ui, tr("PaddleOCR model downloaded successfully"));
            });
        });
    });
}

fn video_editor_ocr_setting_is_valid(ui: &AppWindow) -> bool {
    get_and_check_model_setting(ui).is_ok()
}

async fn get_or_load_model(model_dir: &Path) -> Result<()> {
    let model_store = OCR_MODEL.get().unwrap();
    if model_store.lock().unwrap().is_some() {
        return Ok(());
    }

    let model_dir = model_dir.to_string_lossy().to_string();
    let model = tokio::task::spawn_blocking(move || paddle_ocr_rs::PaddleOCR::new(&model_dir))
        .await
        .map_err(|e| anyhow::anyhow!("Model loading task failed: {e}"))??;

    *model_store.lock().unwrap() = Some(model);

    Ok(())
}

fn get_and_check_model_setting(ui: &AppWindow) -> Result<PathBuf> {
    let config = global_store!(ui).get_video_editor_ocr_config();
    let model_dir = PathBuf::from(&config.model_dir);

    if !model_dir.exists() {
        anyhow::bail!("No found model directory {}", model_dir.display());
    }

    let config_json = model_dir.join("config.json");
    let tokenizer_json = model_dir.join("tokenizer.json");
    let model_safetensors = model_dir.join("model.safetensors");
    let preprocessor_config = model_dir.join("preprocessor_config.json");

    if !config_json.exists() {
        anyhow::bail!("Missing config.json in model directory");
    }
    if !tokenizer_json.exists() {
        anyhow::bail!("Missing tokenizer.json in model directory");
    }
    if !model_safetensors.exists() {
        anyhow::bail!("Missing model.safetensors in model directory");
    }
    if !preprocessor_config.exists() {
        anyhow::bail!("Missing preprocessor_config.json in model directory");
    }

    Ok(model_dir)
}

fn async_set_processing(ui_weak: slint::Weak<AppWindow>, processing: bool) {
    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        global_store!(ui).set_video_editor_ocr_is_processing(processing);
    });
}

fn save_ocr_config(config: OcrConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize ocr config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, OCR_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, OCR_CONFIG_ID, &data).await {
                log::warn!("Failed to save OCR config: {:?}", e);
            }
        }
    });
}

async fn load_ocr_config() -> Option<OcrConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, OCR_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn slint_image_to_dynamic(image: slint::Image) -> Option<image::DynamicImage> {
    let pixel_buffer = image.to_rgba8()?;
    let width = pixel_buffer.width() as u32;
    let height = pixel_buffer.height() as u32;
    let pixels: Vec<u8> = pixel_buffer
        .as_slice()
        .iter()
        .flat_map(|p| [p.r, p.g, p.b, p.a])
        .collect();
    let rgba_image = image::RgbaImage::from_raw(width, height, pixels)?;
    Some(image::DynamicImage::ImageRgba8(rgba_image))
}
