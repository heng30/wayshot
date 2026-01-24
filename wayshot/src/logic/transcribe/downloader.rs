use crate::{
    global_store,
    logic::{
        downloader::{downloader_cancel, downloader_start},
        share_screen::picker_file,
        tr::tr,
    },
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, Downloader as UIDownloader, DownloaderState as UIDownloaderState,
    },
};
use downloader::DownloadState;
use fun_ast_nano::Model as FunAstNanoModel;
use slint::{ComponentHandle, Model, SharedString, VecModel};
use std::path::PathBuf;

#[macro_export]
macro_rules! store_transcribe_models_dowloader {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_transcribe_models_dowloader()
            .as_any()
            .downcast_ref::<VecModel<UIDownloader>>()
            .expect("We know we set a VecModel<UIDownloader> earlier for transcribe models")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(&ui);

    logic_cb!(transcribe_choose_model_path, ui, index);
    logic_cb!(transcribe_model_start_download, ui, index, url);
    logic_cb!(transcribe_model_cancel_download, ui, index, url);
}

pub fn inner_init(ui: &AppWindow) {
    let downloaders = FunAstNanoModel::all_models()
        .into_iter()
        .map(|m| UIDownloader {
            url: m.download_url().to_string().into(),
            filename: m.to_filename().to_string().into(),
            state: UIDownloaderState::UnStart,
            progress: 0.0,
        })
        .collect::<Vec<_>>();

    store_transcribe_models_dowloader!(ui).set_vec(downloaders);
}

fn transcribe_choose_model_path(ui: &AppWindow, index: i32) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Choose model or tokenizer"),
            &tr("fun ast model or tokenizer"),
            &["pt", "json"],
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let filepath = filepath.to_string_lossy().to_string().into();
            let mut setting = global_store!(ui).get_transcribe_setting_cache();
            match index {
                0 => setting.model_path = filepath,
                1 => setting.model_tokenizer_path = filepath,
                _ => log::warn!("Unexcepted trancribe model index = {index}"),
            }

            global_store!(ui).set_transcribe_setting_cache(setting);
        });
    });
}

fn transcribe_model_start_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;
    let filename = FunAstNanoModel::all_models()[index].to_filename().into();

    downloader_start(
        ui,
        url,
        filename,
        move |ui: &AppWindow, _downloaded: u64, _total: u64, progress: f32| {
            if let Some(mut item) = store_transcribe_models_dowloader!(ui).row_data(index) {
                item.progress = progress;
                store_transcribe_models_dowloader!(ui).set_row_data(index, item);
            }
        },
        move |ui: &AppWindow, filepath: PathBuf| {
            if let Some(mut item) = store_transcribe_models_dowloader!(ui).row_data(index) {
                item.state = UIDownloaderState::Downloading;
                store_transcribe_models_dowloader!(ui).set_row_data(index, item);
            }

            let filepath = filepath.to_string_lossy().to_string().into();
            let mut setting = global_store!(ui).get_transcribe_setting_cache();
            match index {
                0 => setting.model_path = filepath,
                1 => setting.model_tokenizer_path = filepath,
                _ => log::warn!("Unexcepted trancribe model index = {index}"),
            }
            global_store!(ui).set_transcribe_setting_cache(setting);
        },
        move |ui: &AppWindow, result: downloader::Result<downloader::DownloadState>| {
            if let Some(mut item) = store_transcribe_models_dowloader!(ui).row_data(index) {
                match result {
                    Ok(DownloadState::Cancelled) => item.state = UIDownloaderState::Cancelled,
                    Ok(DownloadState::Incompleted) => item.state = UIDownloaderState::Failed,
                    Ok(DownloadState::Finsished) => item.state = UIDownloaderState::Finished,
                    Err(_) => item.state = UIDownloaderState::Failed,
                }
                store_transcribe_models_dowloader!(ui).set_row_data(index, item);
            }
        },
    );
}

fn transcribe_model_cancel_download(ui: &AppWindow, index: i32, url: SharedString) {
    let index = index as usize;

    downloader_cancel(ui, url, move |ui: &AppWindow| {
        if let Some(mut item) = store_transcribe_models_dowloader!(ui).row_data(index) {
            item.state = UIDownloaderState::Cancelled;
            store_transcribe_models_dowloader!(ui).set_row_data(index, item);
        }
    });
}
