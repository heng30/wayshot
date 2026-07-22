use crate::{
    db::{OnlineSearchImageConfigData, VIDEO_EDITOR_TABLE},
    global_logic, global_store,
    logic::{
        recorder::picker_directory, toast, tr::tr, video_editor::playlist::import_file_to_playlist,
        video_editor::project::ONLINE_SEARCH_IMAGE_CONFIG_ID,
    },
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, OnlineSearchImageParam as UIOnlineSearchImageParam,
        OnlineSearchImageResultItem as UIOnlineSearchImageResultItem,
        OnlineSearchImageSetting as UIOnlineSearchImageSetting,
        OnlineSearchImageSourceEntry as UIOnlineSearchImageSourceEntry,
    },
};
use imagedl_rs::{ImageClient, ImageInfo, SearchResult, SourceRegistry};
use once_cell::sync::Lazy;
use slint::{
    ComponentHandle, Image, Model, ModelRc, SharedPixelBuffer, SharedString, VecModel, Weak,
};
use std::{collections::HashSet, path::PathBuf};
use tokio::{sync::Mutex, task::JoinHandle};

static STATE: Lazy<Mutex<OnlineSearchState>> = Lazy::new(|| {
    Mutex::new(OnlineSearchState {
        search_handle: None,
    })
});

struct OnlineSearchState {
    search_handle: Option<JoinHandle<()>>,
}

macro_rules! store_online_search_results {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_online_search_image_results()
            .as_any()
            .downcast_ref::<VecModel<UIOnlineSearchImageResultItem>>()
            .expect("We know we set a VecModel<UIOnlineSearchImageResultItem> earlier")
    };
}

macro_rules! store_video_editor_sources {
    ($sources:expr) => {
        $sources
            .as_any()
            .downcast_ref::<VecModel<UIOnlineSearchImageSourceEntry>>()
            .expect("We know we set a VecModel<UIOnlineSearchImageSourceEntry> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_online_search_image, ui, param);
    logic_cb!(video_editor_online_search_image_cancel, ui);
    logic_cb!(video_editor_online_search_image_download, ui, index);
    logic_cb!(video_editor_online_search_image_update_config, ui, config);
    logic_cb!(
        video_editor_online_search_image_update_source,
        ui,
        index,
        source
    );
    logic_cb!(video_editor_online_search_image_refresh_sources_status, ui);
    logic_cb!(
        video_editor_online_search_image_refresh_source_status,
        ui,
        index
    );
    logic_cb!(video_editor_online_search_image_select_all_sources, ui);
    logic_cb!(video_editor_online_search_image_deselect_all_sources, ui);
    logic_cb!(video_editor_online_search_image_choose_save_dir, ui);
    logic_cb!(video_editor_online_search_image_config_is_valid, ui);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let registry = SourceRegistry::with_builtin_sources();
        let builtin_names: Vec<String> = registry
            .source_names()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let builtin_set: HashSet<&str> = builtin_names.iter().map(|s| s.as_str()).collect();

        let db_config = load_config_from_db().await;
        let mut download_dir = String::new();
        let mut http_proxy_url = String::new();
        let mut socks5_proxy_url = String::new();
        let mut search_limits: i32 = 25;
        let mut entries: Vec<UIOnlineSearchImageSourceEntry> = Vec::new();

        if let Some(config) = db_config {
            download_dir = config.download_dir;
            http_proxy_url = config.http_proxy_url;
            socks5_proxy_url = config.socks5_proxy_url;
            search_limits = if config.search_limits > 0 {
                config.search_limits
            } else {
                25
            };

            for entry in config.sources {
                if builtin_set.contains(entry.name.as_str()) {
                    entries.push(UIOnlineSearchImageSourceEntry {
                        name: SharedString::from(entry.name.as_str()),
                        enabled: entry.enabled,
                        is_testing: false,
                        can_access: entry.can_access,
                        proxy_type: SharedString::from(entry.proxy_type.as_str()),
                    });
                } else {
                    log::warn!(
                        "[OnlineSearch] Removing illegal source from db: {}",
                        entry.name
                    );
                }
            }
        }

        let db_source_names: HashSet<String> = entries.iter().map(|e| e.name.to_string()).collect();
        for name in &builtin_names {
            if !db_source_names.contains(name.as_str()) {
                entries.push(UIOnlineSearchImageSourceEntry {
                    name: SharedString::from(name.as_str()),
                    enabled: false,
                    is_testing: false,
                    can_access: false,
                    proxy_type: SharedString::from("None"),
                });
            }
        }

        let any_enabled = entries.iter().any(|e| e.enabled);
        if !any_enabled {
            for entry in &mut entries {
                if entry.name == "bing" || entry.name == "baidu" {
                    entry.enabled = true;
                }
            }
        }

        sort_sources(&mut entries);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_online_search_image_setting(
                UIOnlineSearchImageSetting {
                    id: SharedString::from(ONLINE_SEARCH_IMAGE_CONFIG_ID),
                    download_dir: SharedString::from(download_dir),
                    http_proxy_url: SharedString::from(http_proxy_url),
                    socks5_proxy_url: SharedString::from(socks5_proxy_url),
                    search_limits,
                    sources: ModelRc::new(VecModel::from(entries)),
                },
            );
        });
    });
}

async fn load_config_from_db() -> Option<OnlineSearchImageConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, ONLINE_SEARCH_IMAGE_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn save_config_to_db(config: OnlineSearchImageConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize online search config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, ONLINE_SEARCH_IMAGE_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, ONLINE_SEARCH_IMAGE_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save online search config: {:?}", e);
            }
        }
    });
}

fn sort_sources(entries: &mut [UIOnlineSearchImageSourceEntry]) {
    entries.sort_by(|a, b| match (a.enabled, b.enabled) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a
            .name
            .to_string()
            .to_lowercase()
            .cmp(&b.name.to_string().to_lowercase()),
    });
}

fn sort_sources_in_store(ui: &AppWindow) {
    let setting = global_store!(ui).get_video_editor_online_search_image_setting();
    let mut entries: Vec<UIOnlineSearchImageSourceEntry> = setting.sources.iter().collect();

    sort_sources(&mut entries);

    global_store!(ui).set_video_editor_online_search_image_setting(UIOnlineSearchImageSetting {
        id: setting.id,
        download_dir: setting.download_dir,
        http_proxy_url: setting.http_proxy_url,
        socks5_proxy_url: setting.socks5_proxy_url,
        search_limits: setting.search_limits,
        sources: ModelRc::new(VecModel::from(entries)),
    });
}

fn video_editor_online_search_image(ui: &AppWindow, param: UIOnlineSearchImageParam) {
    let keyword = param.keyword.to_string();
    if keyword.is_empty() {
        return;
    }

    global_store!(ui).set_video_editor_online_search_image_is_searching(true);
    global_store!(ui)
        .set_video_editor_online_search_image_results(ModelRc::new(VecModel::from_slice(&[])));

    let ui_weak = ui.as_weak();
    let keyword: SharedString = param.keyword.clone();

    let setting = global_store!(ui).get_video_editor_online_search_image_setting();
    let download_dir = setting.download_dir.to_string();
    let http_proxy_url = setting.http_proxy_url.to_string();
    let socks5_proxy_url = setting.socks5_proxy_url.to_string();
    let search_limits = if setting.search_limits > 0 {
        setting.search_limits as usize
    } else {
        25
    };

    let source_configs: Vec<(String, String)> = setting
        .sources
        .iter()
        .filter(|e| e.enabled)
        .map(|e| (e.name.to_string(), e.proxy_type.to_string()))
        .collect();

    tokio::spawn(async move {
        let mut state = STATE.lock().await;
        if let Some(handle) = state.search_handle.take() {
            handle.abort();
        }
        drop(state);

        let handle = tokio::spawn(async move {
            do_search(
                ui_weak,
                keyword.to_string(),
                download_dir,
                http_proxy_url,
                socks5_proxy_url,
                search_limits,
                source_configs,
            )
            .await;
        });

        let mut state = STATE.lock().await;
        state.search_handle = Some(handle);
    });
}

async fn do_search(
    ui_weak: Weak<AppWindow>,
    keyword: String,
    download_dir: String,
    http_proxy_url: String,
    socks5_proxy_url: String,
    search_limits: usize,
    source_configs: Vec<(String, String)>,
) {
    if source_configs.is_empty() {
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_online_search_image_is_searching(false);
            crate::toast_warn!(ui, tr("Please enable at least one source in settings"));
        });
        return;
    }

    let mut handles = Vec::new();
    for (source_name, proxy_type) in source_configs {
        let proxy = match proxy_type.as_str() {
            "Http" if !http_proxy_url.is_empty() => Some(http_proxy_url.clone()),
            "Socks5" if !socks5_proxy_url.is_empty() => Some(socks5_proxy_url.clone()),
            _ => None,
        };
        let client = match build_client(&download_dir, proxy.as_deref(), search_limits) {
            Some(c) => c,
            None => continue,
        };
        let http_client = client.http().clone();
        let keyword = keyword.clone();
        let source_name_clone = source_name.clone();
        let ui_weak = ui_weak.clone();

        handles.push(tokio::spawn(async move {
            let source_refs = [source_name_clone.as_str()];
            let results = client.search(&keyword, &source_refs).await;
            let images: Vec<ImageInfo> = match results.get(&source_name_clone) {
                Some(SearchResult::Ok(imgs)) => imgs.iter().take(search_limits).cloned().collect(),
                Some(SearchResult::Err(err)) => {
                    log::warn!(
                        "[OnlineSearch] Source '{}' failed: {}",
                        source_name_clone,
                        err
                    );
                    Vec::new()
                }
                None => Vec::new(),
            };

            if images.is_empty() {
                return;
            }

            let images_clone = images.clone();
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                for info in images_clone {
                    store_online_search_results!(ui).push(info.into());
                }
                let count = store_online_search_results!(ui).row_count() as i32;
                global_store!(ui).set_video_editor_online_search_image_result_count(count);
                global_store!(ui).set_video_editor_online_search_image_is_searching(false);
            });

            // Load thumbnails for this source's results
            let mut loaded: Vec<(String, Vec<u8>, u32, u32)> = Vec::new();
            let mut failed: Vec<String> = Vec::new();

            for info in &images {
                let thumb_url = match info.candidate_download_urls.last() {
                    Some(url) => url.clone(),
                    None => {
                        failed.push(info.identifier.clone());
                        continue;
                    }
                };

                match http_client
                    .get_bytes(&thumb_url, reqwest::header::HeaderMap::new())
                    .await
                {
                    Ok(bytes) => {
                        if let Some((pixels, w, h)) = decode_image_bytes(&bytes) {
                            loaded.push((info.identifier.clone(), pixels, w, h));
                        } else {
                            failed.push(info.identifier.clone());
                        }
                    }
                    Err(_) => failed.push(info.identifier.clone()),
                }

                if loaded.len() >= 5 {
                    let batch = std::mem::take(&mut loaded);
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        update_thumbnails_by_identifier(&ui, batch);
                    });
                }
            }

            if !loaded.is_empty() || !failed.is_empty() {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    update_thumbnails_by_identifier(&ui, loaded);
                    remove_failed_items_by_identifier(&ui, failed);
                });
            }
        }));
    }

    for handle in handles {
        if let Err(e) = handle.await {
            log::warn!("[OnlineSearch] Search task failed: {}", e);
        }
    }
}

fn decode_image_bytes(data: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(data).ok()?;
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    Some((rgba.into_raw(), width, height))
}

fn extract_thumbnail_pixels(thumbnail: &Image) -> Option<(Vec<u8>, u32, u32)> {
    let buffer = thumbnail.clone().to_rgba8()?;
    let w = buffer.width();
    let h = buffer.height();
    if w == 0 || h == 0 {
        return None;
    }
    Some((buffer.as_bytes().to_vec(), w, h))
}

fn update_thumbnails_by_identifier(ui: &AppWindow, thumbnails: Vec<(String, Vec<u8>, u32, u32)>) {
    for (identifier, pixels, w, h) in thumbnails {
        for (i, item) in store_online_search_results!(ui).iter().enumerate() {
            if item.identifier == identifier {
                let buffer =
                    SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&pixels, w, h);
                let mut item = item;
                item.thumbnail = Image::from_rgba8(buffer);
                _ = store_online_search_results!(ui).set_row_data(i, item);
                break;
            }
        }
    }
}

fn remove_failed_items_by_identifier(ui: &AppWindow, failed_identifiers: Vec<String>) {
    if failed_identifiers.is_empty() {
        return;
    }

    let mut indices_to_remove: Vec<usize> = Vec::new();
    for (i, item) in store_online_search_results!(ui).iter().enumerate() {
        if failed_identifiers.contains(&item.identifier.to_string()) {
            indices_to_remove.push(i);
        }
    }

    indices_to_remove.sort_unstable_by(|a, b| b.cmp(a));
    for idx in indices_to_remove {
        store_online_search_results!(ui).remove(idx);
    }

    let count = store_online_search_results!(ui).row_count() as i32;
    global_store!(ui).set_video_editor_online_search_image_result_count(count);
}

fn build_client(
    download_dir: &str,
    proxy: Option<&str>,
    search_limits: usize,
) -> Option<ImageClient> {
    let mut builder = ImageClient::builder()
        .with_builtin_sources()
        .search_limits(search_limits)
        .work_dir(PathBuf::from(download_dir));

    if let Some(proxy_url) = proxy {
        builder = builder.proxy(proxy_url);
    }

    builder.build().ok()
}

fn proxy_for_type(setting: &UIOnlineSearchImageSetting, proxy_type: &str) -> Option<String> {
    match proxy_type {
        "Http" => {
            let url = setting.http_proxy_url.to_string();
            if url.is_empty() { None } else { Some(url) }
        }
        "Socks5" => {
            let url = setting.socks5_proxy_url.to_string();
            if url.is_empty() { None } else { Some(url) }
        }
        _ => None,
    }
}

fn video_editor_online_search_image_cancel(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let mut state = STATE.lock().await;
        if let Some(handle) = state.search_handle.take() {
            handle.abort();
        }
        drop(state);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            crate::global_store!(ui).set_video_editor_online_search_image_is_searching(false);
        });
    });
}

fn video_editor_online_search_image_download(ui: &AppWindow, index: i32) {
    let idx = index as usize;
    let ui_weak = ui.as_weak();

    let store = crate::global_store!(ui);
    let model = store.get_video_editor_online_search_image_results();
    let item = match model
        .as_any()
        .downcast_ref::<slint::VecModel<UIOnlineSearchImageResultItem>>()
    {
        Some(vec_model) => vec_model.row_data(idx),
        None => None,
    };

    let Some(mut item) = item else {
        crate::toast_warn!(ui, tr("Invalid item index"));
        return;
    };

    if item.is_downloaded || item.is_downloading {
        return;
    }

    let source_name = item.source.to_string();
    let download_urls: Vec<String> = item.download_urls.iter().map(|u| u.to_string()).collect();
    let thumbnail_pixels = extract_thumbnail_pixels(&item.thumbnail);
    let setting = store.get_video_editor_online_search_image_setting();
    let download_dir = setting.download_dir.to_string();
    let proxy_type = setting
        .sources
        .iter()
        .find(|e| e.name == source_name)
        .map(|e| e.proxy_type.to_string())
        .unwrap_or_default();
    let proxy = proxy_for_type(&setting, &proxy_type);

    item.is_downloading = true;
    if let Some(vec_model) = model
        .as_any()
        .downcast_ref::<slint::VecModel<UIOnlineSearchImageResultItem>>()
    {
        let _ = vec_model.set_row_data(idx, item);
    }

    tokio::spawn(async move {
        let download_dir_path = PathBuf::from(&download_dir);

        if let Err(e) = std::fs::create_dir_all(&download_dir_path) {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}: {}", tr("Failed to create download directory"), e),
            );
            mark_download_failed(&ui_weak, idx);
            return;
        }

        let client = match build_client(&download_dir, proxy.as_deref(), 25) {
            Some(c) => c,
            None => {
                toast::async_toast_warn(ui_weak.clone(), tr("Failed to create download client"));
                mark_download_failed(&ui_weak, idx);
                return;
            }
        };

        let mut saved = false;
        for url in &download_urls {
            let fake_info = ImageInfo::new(&source_name, vec![url.clone()]);
            match client.download(&source_name, &[fake_info]).await {
                Ok(downloaded) => {
                    if let Some(dl_info) = downloaded.first() {
                        let ext = dl_info.format.extension();
                        let filename =
                            format!("{}.{}", chrono::Local::now().format("%Y%m%d%H%M%S%3f"), ext);
                        let save_path = PathBuf::from(&download_dir).join(&filename);

                        if std::fs::write(&save_path, &dl_info.data).is_ok() {
                            import_file_to_playlist(ui_weak.clone(), save_path, None).await;
                            mark_download_success(&ui_weak, idx);
                            saved = true;
                            break;
                        }
                    }
                }
                Err(e) => {
                    log::warn!("[OnlineSearch] Download attempt failed for {}: {}", url, e);
                }
            }
        }

        if !saved {
            if let Some((pixels, w, h)) = thumbnail_pixels {
                let save_path = PathBuf::from(&download_dir).join(format!(
                    "{}.png",
                    chrono::Local::now().format("%Y%m%d%H%M%S%3f")
                ));
                if let Some(img_buf) = image::RgbaImage::from_raw(w, h, pixels) {
                    if img_buf
                        .save_with_format(&save_path, image::ImageFormat::Png)
                        .is_ok()
                    {
                        import_file_to_playlist(ui_weak.clone(), save_path, None).await;
                        mark_download_success(&ui_weak, idx);
                        return;
                    }
                }
            }
            toast::async_toast_warn(ui_weak.clone(), tr("All download attempts failed"));
            mark_download_failed(&ui_weak, idx);
        }
    });
}

fn mark_download_failed(ui_weak: &Weak<AppWindow>, idx: usize) {
    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        if let Some(mut item) = store_online_search_results!(ui).row_data(idx) {
            item.is_downloading = false;
            store_online_search_results!(ui).set_row_data(idx, item);
        }
    });
}

fn mark_download_success(ui_weak: &Weak<AppWindow>, idx: usize) {
    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        if let Some(mut item) = store_online_search_results!(ui).row_data(idx) {
            item.is_downloading = false;
            item.is_downloaded = true;
            store_online_search_results!(ui).set_row_data(idx, item);
        }
    });
}

fn video_editor_online_search_image_update_config(
    ui: &AppWindow,
    config: UIOnlineSearchImageSetting,
) {
    global_store!(ui).set_video_editor_online_search_image_setting(config.clone());
    save_config_to_db(config.into());
}

fn video_editor_online_search_image_update_source(
    ui: &AppWindow,
    index: i32,
    source: UIOnlineSearchImageSourceEntry,
) {
    let sources = global_store!(ui)
        .get_video_editor_online_search_image_setting()
        .sources;
    store_video_editor_sources!(sources).set_row_data(index as usize, source);

    sort_sources_in_store(ui);

    save_config_to_db(
        global_store!(ui)
            .get_video_editor_online_search_image_setting()
            .into(),
    );
}

fn video_editor_online_search_image_refresh_sources_status(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    let store = crate::global_store!(ui);
    let setting = store.get_video_editor_online_search_image_setting();
    let download_dir = setting.download_dir.to_string();
    let http_proxy_url = setting.http_proxy_url.to_string();
    let socks5_proxy_url = setting.socks5_proxy_url.to_string();
    let search_limits = if setting.search_limits > 0 {
        setting.search_limits as usize
    } else {
        25
    };
    let source_configs: Vec<(String, String)> = setting
        .sources
        .iter()
        .map(|e| (e.name.to_string(), e.proxy_type.to_string()))
        .collect();

    let sources = store.get_video_editor_online_search_image_setting().sources;
    let vec_model = store_video_editor_sources!(sources);
    for (i, mut entry) in vec_model.iter().enumerate() {
        entry.is_testing = true;
        _ = vec_model.set_row_data(i, entry);
    }

    crate::toast_info!(ui, tr("Testing sources, this may take a while..."));

    tokio::spawn(async move {
        let mut handles = Vec::new();
        for (source_name, proxy_type) in source_configs {
            let proxy = match proxy_type.as_str() {
                "Http" if !http_proxy_url.is_empty() => Some(http_proxy_url.clone()),
                "Socks5" if !socks5_proxy_url.is_empty() => Some(socks5_proxy_url.clone()),
                _ => None,
            };
            let client = match build_client(&download_dir, proxy.as_deref(), search_limits) {
                Some(c) => c,
                None => continue,
            };

            let ui_weak = ui_weak.clone();
            handles.push(tokio::spawn(async move {
                let results = client.search("dog", &[source_name.as_str()]).await;
                let is_ok = match results.get(&source_name) {
                    Some(SearchResult::Ok(imgs)) => !imgs.is_empty(),
                    _ => false,
                };

                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let sources = global_store!(&ui)
                        .get_video_editor_online_search_image_setting()
                        .sources;
                    let vec_model = store_video_editor_sources!(sources);

                    for (i, mut entry) in vec_model.iter().enumerate() {
                        if entry.name == source_name {
                            entry.is_testing = false;
                            entry.can_access = is_ok;
                            _ = vec_model.set_row_data(i, entry);
                            break;
                        }
                    }
                });
            }));
        }

        for handle in handles {
            if let Err(e) = handle.await {
                log::warn!("[OnlineSearch] Status check task failed: {}", e);
            }
        }
    });
}

fn video_editor_online_search_image_refresh_source_status(ui: &AppWindow, index: i32) {
    let ui_weak = ui.as_weak();
    let store = crate::global_store!(ui);
    let setting = store.get_video_editor_online_search_image_setting();
    let download_dir = setting.download_dir.to_string();
    let http_proxy_url = setting.http_proxy_url.to_string();
    let socks5_proxy_url = setting.socks5_proxy_url.to_string();

    let sources = setting.sources;
    let vec_model = store_video_editor_sources!(sources);
    let Some(mut entry) = vec_model.row_data(index as usize) else {
        return;
    };

    let source_name = entry.name.to_string();
    let proxy_type = entry.proxy_type.to_string();
    let proxy = match proxy_type.as_str() {
        "Http" if !http_proxy_url.is_empty() => Some(http_proxy_url.clone()),
        "Socks5" if !socks5_proxy_url.is_empty() => Some(socks5_proxy_url.clone()),
        _ => None,
    };

    entry.is_testing = true;
    _ = vec_model.set_row_data(index as usize, entry);

    tokio::spawn(async move {
        let client = match build_client(&download_dir, proxy.as_deref(), 25) {
            Some(c) => c,
            None => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let sources = global_store!(&ui)
                        .get_video_editor_online_search_image_setting()
                        .sources;
                    let vec_model = store_video_editor_sources!(sources);
                    if let Some(mut entry) = vec_model.row_data(index as usize) {
                        entry.is_testing = false;
                        entry.can_access = false;
                        _ = vec_model.set_row_data(index as usize, entry);
                    }
                });
                return;
            }
        };

        let results = client.search("dog", &[source_name.as_str()]).await;
        let is_ok = match results.get(&source_name) {
            Some(SearchResult::Ok(imgs)) => !imgs.is_empty(),
            _ => false,
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let sources = global_store!(&ui)
                .get_video_editor_online_search_image_setting()
                .sources;
            let vec_model = store_video_editor_sources!(sources);
            if let Some(mut entry) = vec_model.row_data(index as usize) {
                entry.is_testing = false;
                entry.can_access = is_ok;
                _ = vec_model.set_row_data(index as usize, entry);
            }
        });
    });
}

fn video_editor_online_search_image_select_all_sources(ui: &AppWindow) {
    set_select_all_sources(ui, true);
}

fn video_editor_online_search_image_deselect_all_sources(ui: &AppWindow) {
    set_select_all_sources(ui, false);
}

fn set_select_all_sources(ui: &AppWindow, enabled: bool) {
    let sources = global_store!(ui)
        .get_video_editor_online_search_image_setting()
        .sources;
    let vec_model = store_video_editor_sources!(sources);
    for (i, mut entry) in vec_model.iter().enumerate() {
        entry.enabled = enabled;
        _ = vec_model.set_row_data(i, entry);
    }

    sort_sources_in_store(ui);

    save_config_to_db(
        global_store!(ui)
            .get_video_editor_online_search_image_setting()
            .into(),
    );
}

fn video_editor_online_search_image_choose_save_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(dir) = picker_directory(ui_weak.clone(), &tr("Choose save directory")) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut setting = global_store!(ui).get_video_editor_online_search_image_setting();
            setting.download_dir = dir.to_string_lossy().to_string().into();
            global_logic!(ui).invoke_video_editor_online_search_image_update_config(setting);
        });
    });
}

fn video_editor_online_search_image_config_is_valid(ui: &AppWindow) -> bool {
    let setting = global_store!(ui).get_video_editor_online_search_image_setting();

    if !global_logic!(ui).invoke_dir_exist(setting.download_dir) || setting.search_limits <= 0 {
        return false;
    }

    setting.sources.iter().any(|e| e.enabled)
}

impl From<ImageInfo> for UIOnlineSearchImageResultItem {
    fn from(info: ImageInfo) -> Self {
        Self {
            source: SharedString::from(info.source),
            thumbnail_url: SharedString::from(
                info.candidate_download_urls
                    .last()
                    .cloned()
                    .unwrap_or_default(),
            ),
            download_urls: ModelRc::new(VecModel::from(
                info.candidate_download_urls
                    .iter()
                    .map(|u| SharedString::from(u.as_str()))
                    .collect::<Vec<_>>(),
            )),
            description: SharedString::from(info.description),
            identifier: SharedString::from(info.identifier),
            thumbnail: Image::default(),
            is_downloading: false,
            is_downloaded: false,
        }
    }
}
