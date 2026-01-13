use crate::{
    logic::{recorder::picker_directory, toast, tr::tr},
    slint_generatedAppWindow::AppWindow,
};
use downloader::{DownloadState, Downloader};
use once_cell::sync::Lazy;
use slint::{ComponentHandle, SharedString};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

static DOWNLOADER_CACHE: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn init(_ui: &AppWindow) {}

pub fn downloader_start(
    ui: &AppWindow,
    url: SharedString,
    filename: SharedString,
    progress_cb: impl FnMut(&AppWindow, u64, u64, f32) + 'static + Send + Clone,
    mut enter_cb: impl FnMut(&AppWindow, PathBuf) + 'static + Send,
    mut exit_cb: impl FnMut(&AppWindow, downloader::Result<DownloadState>) + 'static + Send,
) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(save_dir) = picker_directory(ui_weak.clone(), &tr("Choose model"), &filename)
        else {
            return;
        };

        let save_path = save_dir.join(&filename);

        let save_path_clone = save_path.clone();
        _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
            enter_cb(&ui, save_path_clone);
        });

        let ui_weak_clone = ui_weak.clone();
        let downloader = Downloader::new(url.to_string(), save_path.clone());

        DOWNLOADER_CACHE
            .lock()
            .unwrap()
            .insert(url.to_string(), downloader.cancel_sig());

        let result = downloader
            .start(move |downloaded: u64, total: u64, progress: f32| {
                let mut cb = progress_cb.clone();
                _ = ui_weak_clone.clone().upgrade_in_event_loop(move |ui| {
                    cb(&ui, downloaded, total, progress);
                });
            })
            .await;

        match result {
            Ok(DownloadState::Cancelled) => {
                toast::async_toast_info(
                    ui_weak.clone(),
                    format!("Download `{}` was cancelled!", save_path.display()),
                );
            }
            Ok(DownloadState::Incompleted) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("Download `{}` was incompleted!", save_path.display()),
                );
            }
            Ok(DownloadState::Finsished) => {
                toast::async_toast_success(
                    ui_weak.clone(),
                    format!("Download `{}` completed successfully!", save_path.display()),
                );
            }
            Err(ref e) => {
                let err_msg = e.to_string();
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("Download `{}` failed! {err_msg}", save_path.display()),
                );
            }
        }

        DOWNLOADER_CACHE.lock().unwrap().remove(url.as_str());

        _ = ui_weak.clone().upgrade_in_event_loop(move |ui| {
            exit_cb(&ui, result);
        });
    });
}

pub fn downloader_cancel(
    ui: &AppWindow,
    url: SharedString,
    mut cb: impl FnMut(&AppWindow) + 'static + Send,
) {
    if let Some(cancel_sig) = DOWNLOADER_CACHE.lock().unwrap().remove(url.as_str()) {
        cancel_sig.store(true, Ordering::Relaxed);
        cb(&ui);
    }
}
