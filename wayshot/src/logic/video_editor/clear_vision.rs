use crate::{
    db::{ClearVisionConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        downloader::{downloader_cancel, downloader_start},
        recorder::picker_directory,
        share_screen::picker_file,
        toast::{self, async_toast_warn},
        tr::tr,
        video_editor::{
            img_animation::IMAGE_EXTENSIONS, playlist::import_file_to_playlist,
            project::CLEAR_VISION_CONFIG_ID,
        },
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, Downloader as UIDownloader, DownloaderState as UIDownloaderState,
        VideoEditorClearVisionConfig as UIClearVisionConfig,
    },
};
use anyhow::{Context, Result, bail};
use clear_vision::{Model, load_session, process};
use deconvolution::{
    iterative::{
        Ictm, Landweber, RichardsonLucy, RichardsonLucyTv, TikhonovMiller, VanCittert,
        damped_richardson_lucy_with, ictm_with, landweber_with, richardson_lucy_tv_with,
        richardson_lucy_with, tikhonov_miller_with, van_cittert_with,
    },
    optimization::{Fista, Ista, fista_with, ista_with},
    psf::basic::{box2d, defocus, disk, gaussian2d, motion_linear, oriented_gaussian},
    psf::microscopy::{astigmatic, double_helix, lorentz2d},
    spectral::{InverseFilter, Wiener, inverse_filter_with, wiener_with},
};
use downloader::DownloadState;
use image::GenericImageView;
use slint::{ComponentHandle, Model as SlintModel, SharedString, VecModel};
use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

static IMPORTED_IMAGE_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
static PROCESS_CANCELLED: AtomicBool = AtomicBool::new(false);
static INC_INDEX: AtomicU32 = AtomicU32::new(0);

#[macro_export]
macro_rules! store_video_editor_clear_vision_models_downloader {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_clear_vision_models_downloader()
            .as_any()
            .downcast_ref::<VecModel<UIDownloader>>()
            .expect("We know we set a VecModel<UIDownloader> earlier for video editor clear vision models")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_clear_vision_import_image, ui);
    logic_cb!(video_editor_clear_vision_process_image, ui, active_tab);
    logic_cb!(video_editor_clear_vision_export_to_playlist, ui);
    logic_cb!(video_editor_clear_vision_remove_all_images, ui);
    logic_cb!(video_editor_clear_vision_choose_export_dir, ui);
    logic_cb!(video_editor_clear_vision_choose_model_path, ui, index);
    logic_cb!(video_editor_clear_vision_update_config, ui, config);
    logic_cb!(
        video_editor_clear_vision_model_start_download,
        ui,
        index,
        url
    );
    logic_cb!(
        video_editor_clear_vision_model_cancel_download,
        ui,
        index,
        url
    );
    logic_cb_pure!(video_editor_clear_vision_setting_is_valid, ui, active_tab);
    logic_cb!(video_editor_clear_vision_process_cancel, ui);
}

fn inner_init(ui: &AppWindow) {
    let downloaders = Model::all_models()
        .into_iter()
        .map(|m| UIDownloader {
            url: m.download_url().to_string().into(),
            filename: m.to_filename().to_string().into(),
            state: UIDownloaderState::UnStart,
            progress: 0.0,
        })
        .collect::<Vec<_>>();
    store_video_editor_clear_vision_models_downloader!(ui).set_vec(downloaders);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_clear_vision_config()
            .await
            .unwrap_or_else(|| ClearVisionConfigData {
                id: CLEAR_VISION_CONFIG_ID.to_string(),
                ..Default::default()
            });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_clear_vision_config(config.into());
        });
    });
}

fn save_clear_vision_config(config: ClearVisionConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize clear vision config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, CLEAR_VISION_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, CLEAR_VISION_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save clear vision config: {:?}", e);
            }
        }
    });
}

async fn load_clear_vision_config() -> Option<ClearVisionConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, CLEAR_VISION_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn video_editor_clear_vision_import_image(ui: &AppWindow) {
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
                let (w, h) = img.dimensions();
                let rgba_img = img.to_rgba8();
                let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    rgba_img.as_raw(),
                    w,
                    h,
                );
                global_store!(ui)
                    .set_video_editor_clear_vision_image(slint::Image::from_rgba8(buffer));
                global_store!(ui)
                    .set_video_editor_clear_vision_processed_image(slint::Image::default());
            }
        });
    });
}

fn video_editor_clear_vision_process_image(ui: &AppWindow, active_tab: i32) {
    PROCESS_CANCELLED.store(false, Ordering::SeqCst);
    global_store!(ui).set_video_editor_clear_vision_progress(0.0);
    global_store!(ui).set_video_editor_clear_vision_is_processing(true);
    global_store!(ui).set_video_editor_clear_vision_processed_image(slint::Image::default());
    let inc_index = INC_INDEX.fetch_add(1, Ordering::Relaxed) + 1;

    if active_tab == 0 {
        video_editor_swinir_process_image(ui, inc_index);
    } else {
        video_editor_deconv_process_image(ui, inc_index);
    }
}

fn video_editor_swinir_process_image(ui: &AppWindow, inc_index: u32) {
    let model_path = match get_and_check_model_setting(ui) {
        Ok(path) => path,
        Err(e) => {
            global_store!(ui).set_video_editor_is_show_clear_vision_setting_dialog(true);
            crate::toast_warn!(ui, format!("{e}"));
            return;
        }
    };

    let Some(filepath) = IMPORTED_IMAGE_PATH.lock().unwrap().clone() else {
        crate::toast_warn!(ui, tr("No image imported"));
        return;
    };

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let mut session = match load_session(&model_path) {
            Ok(s) => s,
            Err(e) => {
                async_toast_warn(ui_weak, format!("{}: {e}", tr("Failed to load model")));
                return;
            }
        };

        match process_image(&filepath, &mut session, ui_weak.clone()) {
            Ok(processed_img) => {
                let rgba_img = processed_img.to_rgba8();
                let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    rgba_img.as_raw(),
                    rgba_img.width(),
                    rgba_img.height(),
                );
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if !global_store!(ui).get_video_editor_clear_vision_is_processing()
                        || inc_index != INC_INDEX.load(Ordering::Relaxed)
                    {
                        return;
                    }

                    global_store!(ui).set_video_editor_clear_vision_progress(1.0);
                    global_store!(ui).set_video_editor_clear_vision_is_processing(false);
                    global_store!(ui).set_video_editor_clear_vision_processed_image(
                        slint::Image::from_rgba8(buffer),
                    );
                });
            }
            Err(e) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if !global_store!(ui).get_video_editor_clear_vision_is_processing()
                        || inc_index != INC_INDEX.load(Ordering::Relaxed)
                    {
                        return;
                    }

                    global_store!(ui).set_video_editor_clear_vision_is_processing(false);
                    crate::toast_warn!(ui, format!("{}: {e}", tr("Failed to process image")));
                });
            }
        }
    });
}

fn process_image(
    img_path: &PathBuf,
    session: &mut clear_vision::ort::session::Session,
    ui_weak: slint::Weak<AppWindow>,
) -> Result<image::DynamicImage> {
    let img = image::open(img_path).context("Failed to open image")?;
    let result = process(
        &img,
        session,
        move |p: f32| {
            _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_video_editor_clear_vision_progress(p);
            });
        },
        || PROCESS_CANCELLED.load(Ordering::SeqCst),
    )
    .context("Failed to enhance image")?;
    Ok(result)
}

fn video_editor_clear_vision_export_to_playlist(ui: &AppWindow) {
    let processed_image = global_store!(ui).get_video_editor_clear_vision_processed_image();
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
    let config = global_store!(ui).get_video_editor_clear_vision_config();
    let export_dir = config.export_dir.to_string();
    let output_path = PathBuf::from(export_dir).join(format!("{}_hd.png", filename));

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

fn video_editor_clear_vision_remove_all_images(ui: &AppWindow) {
    INC_INDEX.fetch_add(1, Ordering::Relaxed);
    PROCESS_CANCELLED.store(true, Ordering::SeqCst);
    global_store!(ui).set_video_editor_clear_vision_is_processing(false);
    global_store!(ui).set_video_editor_clear_vision_image(slint::Image::default());
    global_store!(ui).set_video_editor_clear_vision_processed_image(slint::Image::default());
    *IMPORTED_IMAGE_PATH.lock().unwrap() = None;
}

fn video_editor_clear_vision_process_cancel(ui: &AppWindow) {
    INC_INDEX.fetch_add(1, Ordering::Relaxed);
    PROCESS_CANCELLED.store(true, Ordering::SeqCst);
    global_store!(ui).set_video_editor_clear_vision_progress(0.0);
    global_store!(ui).set_video_editor_clear_vision_is_processing(false);
}

fn video_editor_clear_vision_choose_export_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(dirpath) = picker_directory(
            ui_weak.clone(),
            &crate::logic::tr::tr("Choose export directory"),
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_clear_vision_config();
            config.export_dir = dirpath.to_string_lossy().to_string().into();
            global_store!(ui).set_video_editor_clear_vision_config(config.clone());
            save_clear_vision_config(config.into());
        });
    });
}

fn video_editor_clear_vision_choose_model_path(ui: &AppWindow, index: i32) {
    let ui_weak = ui.as_weak();
    let title = tr("Choose SwinIR model");

    tokio::spawn(async move {
        let _ = index;
        let Some(filepath) = picker_file(ui_weak.clone(), &title, &tr("ONNX Model"), &["onnx"])
        else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let filepath_str = filepath.to_string_lossy().to_string().into();
            let mut config = global_store!(ui).get_video_editor_clear_vision_config();
            config.swinir_path = filepath_str;
            global_store!(ui).set_video_editor_clear_vision_config(config.clone());
            save_clear_vision_config(config.into());
        });
    });
}

fn video_editor_clear_vision_model_start_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;
    let filename = Model::all_models()[index].to_filename().into();

    downloader_start(
        ui,
        url,
        filename,
        move |ui: &AppWindow, _downloaded: u64, _total: u64, progress: f32| {
            if let Some(mut item) =
                store_video_editor_clear_vision_models_downloader!(ui).row_data(index)
            {
                item.progress = progress;
                store_video_editor_clear_vision_models_downloader!(ui).set_row_data(index, item);
            }
        },
        move |ui: &AppWindow, filepath: PathBuf| {
            if let Some(mut item) =
                store_video_editor_clear_vision_models_downloader!(ui).row_data(index)
            {
                item.state = UIDownloaderState::Downloading;
                store_video_editor_clear_vision_models_downloader!(ui).set_row_data(index, item);
            }

            let filepath_str = filepath.to_string_lossy().to_string().into();
            let mut config = global_store!(ui).get_video_editor_clear_vision_config();
            match index {
                0 => config.swinir_path = filepath_str,
                _ => log::warn!("Unexpected clear vision model index = {index}"),
            }
            global_store!(ui).set_video_editor_clear_vision_config(config.clone());
            save_clear_vision_config(config.into());
        },
        move |ui: &AppWindow, result: downloader::Result<downloader::DownloadState>| {
            if let Some(mut item) =
                store_video_editor_clear_vision_models_downloader!(ui).row_data(index)
            {
                match result {
                    Ok(DownloadState::Cancelled) => item.state = UIDownloaderState::Cancelled,
                    Ok(DownloadState::Incompleted) => item.state = UIDownloaderState::Failed,
                    Ok(DownloadState::Finsished) => item.state = UIDownloaderState::Finished,
                    Err(_) => item.state = UIDownloaderState::Failed,
                }
                store_video_editor_clear_vision_models_downloader!(ui).set_row_data(index, item);
            }
        },
    );
}

fn video_editor_clear_vision_model_cancel_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;

    downloader_cancel(ui, url, move |ui: &AppWindow| {
        if let Some(mut item) =
            store_video_editor_clear_vision_models_downloader!(ui).row_data(index)
        {
            item.state = UIDownloaderState::Cancelled;
            store_video_editor_clear_vision_models_downloader!(ui).set_row_data(index, item);
        }
    });
}

fn video_editor_clear_vision_setting_is_valid(ui: &AppWindow, active_tab: i32) -> bool {
    if active_tab == 1 {
        true
    } else {
        get_and_check_model_setting(ui).is_ok()
    }
}

fn video_editor_deconv_process_image(ui: &AppWindow, inc_index: u32) {
    let Some(filepath) = IMPORTED_IMAGE_PATH.lock().unwrap().clone() else {
        crate::toast_warn!(ui, tr("No image imported"));
        return;
    };

    let ui_weak = ui.as_weak();
    let config = global_store!(ui).get_video_editor_clear_vision_config();

    tokio::spawn(async move {
        let result = process_deconv_image(&filepath, &config);
        match result {
            Ok(processed_img) => {
                let (w, h) = processed_img.dimensions();
                let rgba_img = processed_img.to_rgba8();
                let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    rgba_img.as_raw(),
                    w,
                    h,
                );
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if !global_store!(ui).get_video_editor_clear_vision_is_processing()
                        || inc_index != INC_INDEX.load(Ordering::Relaxed)
                    {
                        return;
                    }

                    global_store!(ui).set_video_editor_clear_vision_progress(1.0);
                    global_store!(ui).set_video_editor_clear_vision_is_processing(false);
                    global_store!(ui).set_video_editor_clear_vision_processed_image(
                        slint::Image::from_rgba8(buffer),
                    );
                });
            }
            Err(e) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if !global_store!(ui).get_video_editor_clear_vision_is_processing()
                        || inc_index != INC_INDEX.load(Ordering::Relaxed)
                    {
                        return;
                    }
                    global_store!(ui).set_video_editor_clear_vision_is_processing(false);
                    crate::toast_warn!(ui, format!("{}: {e}", tr("Failed to process image")));
                });
            }
        }
    });
}

fn process_deconv_image(
    img_path: &PathBuf,
    config: &UIClearVisionConfig,
) -> Result<image::DynamicImage> {
    let img = image::open(img_path).context("Failed to open image")?;

    let psf_type = config.psf_type.to_string();
    let psf = match psf_type.as_str() {
        "motion" => {
            let length = config.motion_length as f32;
            let angle = config.motion_angle as f32;
            motion_linear(length, angle).context("Failed to create motion PSF")?
        }
        "defocus" => {
            let radius = config.defocus_radius as f32;
            defocus(radius).context("Failed to create defocus PSF")?
        }
        "oriented-gaussian" => {
            let sigma_major = if config.sigma_major > 0.0 {
                config.sigma_major as f32
            } else {
                3.0
            };
            let sigma_minor = if config.sigma_minor > 0.0 {
                config.sigma_minor as f32
            } else {
                1.0
            };
            let angle = config.oriented_angle as f32;
            oriented_gaussian((15, 15), sigma_major, sigma_minor, angle)
                .context("Failed to create oriented gaussian PSF")?
        }
        "box" => {
            let w = if config.box_width > 0 {
                config.box_width as usize
            } else {
                5
            };
            let h = if config.box_height > 0 {
                config.box_height as usize
            } else {
                5
            };
            box2d((h, w)).context("Failed to create box PSF")?
        }
        "disk" => {
            let radius = if config.disk_radius > 0 {
                config.disk_radius as f32
            } else {
                5.0
            };
            disk(radius).context("Failed to create disk PSF")?
        }
        "lorentz" => {
            let gamma = if config.lorentz_gamma > 0.0 {
                config.lorentz_gamma as f32
            } else {
                2.0
            };
            lorentz2d((15, 15), gamma).context("Failed to create lorentz PSF")?
        }
        "astigmatic" => {
            let sigma_major = if config.sigma_major > 0.0 {
                config.sigma_major as f32
            } else {
                3.0
            };
            let sigma_minor = if config.sigma_minor > 0.0 {
                config.sigma_minor as f32
            } else {
                1.0
            };
            let angle = config.oriented_angle as f32;
            astigmatic((15, 15), sigma_major, sigma_minor, angle)
                .context("Failed to create astigmatic PSF")?
        }
        "double-helix" => {
            let sigma = if config.sigma > 0.0 {
                config.sigma as f32
            } else {
                2.0
            };
            let lobe_separation = if config.lobe_separation > 0.0 {
                config.lobe_separation as f32
            } else {
                4.0
            };
            let angle = config.oriented_angle as f32;
            double_helix((15, 15), sigma, lobe_separation, angle)
                .context("Failed to create double helix PSF")?
        }
        _ => {
            let sigma = if config.sigma > 0.0 {
                config.sigma as f32
            } else {
                2.15
            };
            gaussian2d((15, 15), sigma).context("Failed to create gaussian PSF")?
        }
    };

    let iterations = if config.iterations > 0 {
        config.iterations as usize
    } else {
        30
    };

    let algorithm = config.algorithm.to_string();
    let restored = match algorithm.as_str() {
        "damped-rl" => {
            let rl_config = RichardsonLucy::new()
                .iterations(iterations)
                .filter_epsilon(1e-3)
                .collect_history(false);
            let (img, _) = damped_richardson_lucy_with(&img, &psf, &rl_config)
                .context("Damped R-L deconvolution failed")?;
            img
        }
        "rl-tv" => {
            let tv_weight = if config.tv_weight > 0.0 {
                config.tv_weight as f32
            } else {
                0.01
            };
            let rl_tv_config = RichardsonLucyTv::new()
                .iterations(iterations)
                .filter_epsilon(1e-3)
                .tv_weight(tv_weight)
                .collect_history(false);
            let (img, _) = richardson_lucy_tv_with(&img, &psf, &rl_tv_config)
                .context("R-L TV deconvolution failed")?;
            img
        }
        "wiener" => {
            let nsr = if config.wiener_nsr > 0.0 {
                config.wiener_nsr as f32
            } else {
                0.01
            };
            let wiener_config = Wiener::new().nsr(nsr);
            wiener_with(&img, &psf, &wiener_config).context("Wiener deconvolution failed")?
        }
        "landweber" => {
            let mut lw_config = Landweber::new()
                .iterations(iterations)
                .collect_history(false);
            if config.relaxation > 0.0 {
                lw_config = lw_config.step_size(Some(config.relaxation as f32));
            }
            let (img, _) =
                landweber_with(&img, &psf, &lw_config).context("Landweber deconvolution failed")?;
            img
        }
        "van-cittert" => {
            let mut vc_config = VanCittert::new()
                .iterations(iterations)
                .collect_history(false);
            if config.relaxation > 0.0 {
                vc_config = vc_config.step_size(Some(config.relaxation as f32));
            }
            let (img, _) = van_cittert_with(&img, &psf, &vc_config)
                .context("Van Cittert deconvolution failed")?;
            img
        }
        "tikhonov-miller" => {
            let lambda = if config.tikhonov_lambda > 0.0 {
                config.tikhonov_lambda as f32
            } else {
                0.01
            };
            let tm_config = TikhonovMiller::new()
                .iterations(iterations)
                .lambda(lambda)
                .collect_history(false);
            let (img, _) = tikhonov_miller_with(&img, &psf, &tm_config)
                .context("Tikhonov-Miller deconvolution failed")?;
            img
        }
        "ictm" => {
            let lambda = if config.tikhonov_lambda > 0.0 {
                config.tikhonov_lambda as f32
            } else {
                0.01
            };
            let ictm_config = Ictm::new()
                .iterations(iterations)
                .lambda(lambda)
                .collect_history(false);
            let (img, _) =
                ictm_with(&img, &psf, &ictm_config).context("ICTM deconvolution failed")?;
            img
        }
        "inverse-filter" => {
            let cutoff = if config.inverse_cutoff > 0.0 {
                config.inverse_cutoff as f32
            } else {
                0.01
            };
            let inv_config = InverseFilter::new().truncation_cutoff(cutoff);
            inverse_filter_with(&img, &psf, &inv_config).context("Inverse filter failed")?
        }
        "ista" => {
            let lambda = if config.ista_lambda > 0.0 {
                config.ista_lambda as f32
            } else {
                0.01
            };
            let mut ista_config = Ista::new()
                .iterations(iterations)
                .lambda(lambda)
                .collect_history(false);
            if config.ista_step_size > 0.0 {
                ista_config = ista_config.step_size(Some(config.ista_step_size as f32));
            }
            let (img, _) =
                ista_with(&img, &psf, &ista_config).context("ISTA deconvolution failed")?;
            img
        }
        "fista" => {
            let lambda = if config.ista_lambda > 0.0 {
                config.ista_lambda as f32
            } else {
                0.01
            };
            let mut fista_config = Fista::new()
                .iterations(iterations)
                .lambda(lambda)
                .collect_history(false);
            if config.ista_step_size > 0.0 {
                fista_config = fista_config.step_size(Some(config.ista_step_size as f32));
            }
            let (img, _) =
                fista_with(&img, &psf, &fista_config).context("FISTA deconvolution failed")?;
            img
        }
        _ => {
            // default: Richardson-Lucy
            let rl_config = RichardsonLucy::new()
                .iterations(iterations)
                .filter_epsilon(1e-3)
                .collect_history(false);
            let (img, _) = richardson_lucy_with(&img, &psf, &rl_config)
                .context("Richardson-Lucy deconvolution failed")?;
            img
        }
    };

    Ok(restored)
}

fn video_editor_clear_vision_update_config(ui: &AppWindow, config: UIClearVisionConfig) {
    global_store!(ui).set_video_editor_clear_vision_config(config.clone());
    save_clear_vision_config(config.into());
}

fn get_and_check_model_setting(ui: &AppWindow) -> Result<PathBuf> {
    let config = global_store!(ui).get_video_editor_clear_vision_config();
    let model_path = config.swinir_path.to_string();

    if model_path.is_empty() {
        bail!(tr("Please select a model file").to_string());
    }

    let path = PathBuf::from(&model_path);
    if !path.exists() {
        bail!(tr("Model file not found").to_string());
    }

    Ok(path)
}
