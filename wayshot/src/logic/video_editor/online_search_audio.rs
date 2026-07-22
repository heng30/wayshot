use crate::{
    db::{OnlineSearchAudioConfigData, VIDEO_EDITOR_TABLE},
    global_logic, global_store,
    logic::{
        recorder::picker_directory,
        toast,
        tr::tr,
        video_editor::project::ONLINE_SEARCH_AUDIO_CONFIG_ID,
        video_editor::{item_preview::show_preview_item, playlist::import_file_to_playlist},
    },
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, MediaType as UIMediaType, OnlineSearchAudioParam as UIOnlineSearchAudioParam,
        OnlineSearchAudioResultItem as UIOnlineSearchAudioResultItem,
        OnlineSearchAudioSetting as UIOnlineSearchAudioSetting,
        OnlineSearchAudioSourceEntry as UIOnlineSearchAudioSourceEntry,
        VideoEditorPlaylistItem as UIVideoEditorPlaylistItem,
    },
};
use lru::LruCache;
use musicdl_rs::{
    DownloadContent, DownloadedSongInfo, MusicClient, SearchResult, SongInfo, SourceRegistry,
};
use once_cell::sync::Lazy;
use slint::{
    ComponentHandle, Image, Model, ModelRc, SharedPixelBuffer, SharedString, VecModel, Weak,
};
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    num::NonZeroUsize,
    path::PathBuf,
};
use tokio::{sync::Mutex, task::JoinHandle};

const CACHE_MAX_ENTRIES: usize = 20;

static STATE: Lazy<Mutex<OnlineSearchAudioState>> = Lazy::new(|| {
    Mutex::new(OnlineSearchAudioState {
        search_handle: None,
        cache: LruCache::new(NonZeroUsize::new(CACHE_MAX_ENTRIES).unwrap()),
        lyrics: HashMap::new(),
    })
});

struct OnlineSearchAudioState {
    search_handle: Option<JoinHandle<()>>,
    cache: LruCache<String, DownloadedSongInfo>,
    lyrics: HashMap<String, String>,
}

macro_rules! store_online_search_audio_results {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_online_search_audio_results()
            .as_any()
            .downcast_ref::<VecModel<UIOnlineSearchAudioResultItem>>()
            .expect("We know we set a VecModel<UIOnlineSearchAudioResultItem> earlier")
    };
}

macro_rules! store_video_editor_audio_sources {
    ($sources:expr) => {
        $sources
            .as_any()
            .downcast_ref::<VecModel<UIOnlineSearchAudioSourceEntry>>()
            .expect("We know we set a VecModel<UIOnlineSearchAudioSourceEntry> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_online_search_audio, ui, param);
    logic_cb!(video_editor_online_search_audio_cancel, ui);
    logic_cb!(video_editor_online_search_audio_preview, ui, index);
    logic_cb!(video_editor_online_search_audio_download, ui, index);
    logic_cb!(video_editor_online_search_audio_update_config, ui, config);
    logic_cb!(
        video_editor_online_search_audio_update_source,
        ui,
        index,
        source
    );
    logic_cb!(video_editor_online_search_audio_refresh_sources_status, ui);
    logic_cb!(
        video_editor_online_search_audio_refresh_source_status,
        ui,
        index
    );
    logic_cb!(video_editor_online_search_audio_select_all_sources, ui);
    logic_cb!(video_editor_online_search_audio_deselect_all_sources, ui);
    logic_cb!(video_editor_online_search_audio_choose_save_dir, ui);
    logic_cb!(video_editor_online_search_audio_config_is_valid, ui);
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
        let mut search_limits: i32 = 10;
        let mut entries: Vec<UIOnlineSearchAudioSourceEntry> = Vec::new();

        if let Some(config) = db_config {
            download_dir = config.download_dir;
            search_limits = if config.search_limits > 0 {
                config.search_limits
            } else {
                10
            };

            for entry in config.sources {
                if builtin_set.contains(entry.name.as_str()) {
                    entries.push(UIOnlineSearchAudioSourceEntry {
                        name: SharedString::from(entry.name.as_str()),
                        enabled: entry.enabled,
                        is_testing: false,
                        can_access: entry.can_access,
                        proxy_type: SharedString::from(if entry.proxy_type.is_empty() {
                            "None"
                        } else {
                            entry.proxy_type.as_str()
                        }),
                    });
                } else {
                    log::warn!(
                        "[OnlineSearchAudio] Removing illegal source from db: {}",
                        entry.name
                    );
                }
            }
        }

        let db_source_names: HashSet<String> = entries.iter().map(|e| e.name.to_string()).collect();
        for name in &builtin_names {
            if !db_source_names.contains(name.as_str()) {
                entries.push(UIOnlineSearchAudioSourceEntry {
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
                if entry.name == "netease" || entry.name == "kugou" {
                    entry.enabled = true;
                }
            }
        }

        sort_sources(&mut entries);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_online_search_audio_setting(
                UIOnlineSearchAudioSetting {
                    id: SharedString::from(ONLINE_SEARCH_AUDIO_CONFIG_ID),
                    download_dir: SharedString::from(download_dir),
                    search_limits,
                    sources: ModelRc::new(VecModel::from(entries)),
                },
            );
        });
    });
}

async fn load_config_from_db() -> Option<OnlineSearchAudioConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, ONLINE_SEARCH_AUDIO_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn save_config_to_db(config: OnlineSearchAudioConfigData) {
    tokio::spawn(async move {
        let data =
            serde_json::to_string(&config).expect("serialize online search audio config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, ONLINE_SEARCH_AUDIO_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, ONLINE_SEARCH_AUDIO_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save online search audio config: {:?}", e);
            }
        }
    });
}

fn find_source_proxy_type(setting: &UIOnlineSearchAudioSetting, source_name: &str) -> String {
    for entry in setting.sources.iter() {
        if entry.name == source_name {
            return entry.proxy_type.to_string();
        }
    }
    "None".to_string()
}

fn proxy_for_type(
    http_proxy_url: &str,
    socks5_proxy_url: &str,
    proxy_type: &str,
) -> Option<String> {
    match proxy_type {
        "Http" => {
            if http_proxy_url.is_empty() {
                None
            } else {
                Some(http_proxy_url.to_string())
            }
        }
        "Socks5" => {
            if socks5_proxy_url.is_empty() {
                None
            } else {
                Some(socks5_proxy_url.to_string())
            }
        }
        _ => None,
    }
}

fn sort_sources(entries: &mut [UIOnlineSearchAudioSourceEntry]) {
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
    let setting = global_store!(ui).get_video_editor_online_search_audio_setting();
    let mut entries: Vec<UIOnlineSearchAudioSourceEntry> = setting.sources.iter().collect();

    sort_sources(&mut entries);

    global_store!(ui).set_video_editor_online_search_audio_setting(UIOnlineSearchAudioSetting {
        id: setting.id,
        download_dir: setting.download_dir,
        search_limits: setting.search_limits,
        sources: ModelRc::new(VecModel::from(entries)),
    });
}

fn video_editor_online_search_audio(ui: &AppWindow, param: UIOnlineSearchAudioParam) {
    let keyword = param.keyword.to_string();
    if keyword.is_empty() {
        return;
    }

    global_store!(ui).set_video_editor_online_search_audio_is_searching(true);
    global_store!(ui)
        .set_video_editor_online_search_audio_results(ModelRc::new(VecModel::from_slice(&[])));

    let ui_weak = ui.as_weak();

    let audio_setting = global_store!(ui).get_video_editor_online_search_audio_setting();
    let download_dir = audio_setting.download_dir.to_string();
    let search_limits = if audio_setting.search_limits > 0 {
        audio_setting.search_limits as usize
    } else {
        10
    };

    let http_proxy_url = global_store!(ui)
        .get_video_editor_online_search_image_setting()
        .http_proxy_url
        .to_string();
    let socks5_proxy_url = global_store!(ui)
        .get_video_editor_online_search_image_setting()
        .socks5_proxy_url
        .to_string();

    let source_configs: Vec<(String, String)> = audio_setting
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
                keyword,
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
            global_store!(ui).set_video_editor_online_search_audio_is_searching(false);
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
        let client =
            match build_client(&download_dir, proxy.as_deref(), search_limits, false, false) {
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
            let songs: Vec<SongInfo> = match results.get(&source_name_clone) {
                Some(SearchResult::Ok(s)) => s.iter().take(search_limits).cloned().collect(),
                Some(SearchResult::Err(err)) => {
                    log::warn!(
                        "[OnlineSearchAudio] Source '{}' failed: {}",
                        source_name_clone,
                        err
                    );
                    Vec::new()
                }
                None => Vec::new(),
            };

            if songs.is_empty() {
                return;
            }

            // Push results to UI immediately
            let songs_clone = songs.clone();

            // Save lyrics from search results for later download
            {
                let mut state = STATE.lock().await;
                for info in &songs {
                    if let Some(ref lyric) = info.lyric
                        && !lyric.is_empty()
                    {
                        state.lyrics.insert(info.identifier.clone(), lyric.clone());
                    }
                }
            }

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                for info in songs_clone {
                    store_online_search_audio_results!(ui).push(info.into());
                }
                let count = store_online_search_audio_results!(ui).row_count() as i32;
                global_store!(ui).set_video_editor_online_search_audio_result_count(count);
                global_store!(ui).set_video_editor_online_search_audio_is_searching(false);
            });

            // Load cover thumbnails for this source's results
            let mut loaded: Vec<(String, Vec<u8>, u32, u32)> = Vec::new();
            let mut failed: Vec<String> = Vec::new();

            for info in &songs {
                let cover_url = match &info.cover_url {
                    Some(url) if !url.is_empty() => url.clone(),
                    _ => {
                        failed.push(info.identifier.clone());
                        continue;
                    }
                };

                match http_client
                    .get_bytes(&cover_url, reqwest::header::HeaderMap::new())
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

                if loaded.len() >= 3 {
                    let batch = std::mem::take(&mut loaded);
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        update_audio_thumbnails_by_identifier(&ui, batch);
                    });
                }
            }

            if !loaded.is_empty() || !failed.is_empty() {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    update_audio_thumbnails_by_identifier(&ui, loaded);
                    remove_failed_audio_items_by_identifier(&ui, failed);
                });
            }
        }));
    }

    for handle in handles {
        if let Err(e) = handle.await {
            log::warn!("[OnlineSearchAudio] Search task failed: {}", e);
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

fn update_audio_thumbnails_by_identifier(
    ui: &AppWindow,
    thumbnails: Vec<(String, Vec<u8>, u32, u32)>,
) {
    for (identifier, pixels, w, h) in thumbnails {
        for (i, item) in store_online_search_audio_results!(ui).iter().enumerate() {
            if item.identifier == identifier {
                let buffer =
                    SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&pixels, w, h);
                let mut item = item;
                item.thumbnail = Image::from_rgba8(buffer);
                _ = store_online_search_audio_results!(ui).set_row_data(i, item);
                break;
            }
        }
    }
}

fn remove_failed_audio_items_by_identifier(ui: &AppWindow, failed_identifiers: Vec<String>) {
    if failed_identifiers.is_empty() {
        return;
    }

    let mut indices_to_remove: Vec<usize> = Vec::new();
    for (i, item) in store_online_search_audio_results!(ui).iter().enumerate() {
        if failed_identifiers.contains(&item.identifier.to_string()) {
            indices_to_remove.push(i);
        }
    }

    indices_to_remove.sort_unstable_by(|a, b| b.cmp(a));
    for idx in indices_to_remove {
        store_online_search_audio_results!(ui).remove(idx);
    }

    let count = store_online_search_audio_results!(ui).row_count() as i32;
    global_store!(ui).set_video_editor_online_search_audio_result_count(count);
}

fn build_client(
    download_dir: &str,
    proxy: Option<&str>,
    search_limits: usize,
    download_lrc: bool,
    download_cover: bool,
) -> Option<MusicClient> {
    let mut builder = MusicClient::builder()
        .with_builtin_sources()
        .search_limits(search_limits)
        .work_dir(PathBuf::from(download_dir))
        .download_content(DownloadContent {
            audio: true,
            cover: download_cover,
            lyric: download_lrc,
        });

    if let Some(proxy_url) = proxy {
        builder = builder.proxy(proxy_url);
    }

    builder.build().ok()
}

fn video_editor_online_search_audio_cancel(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let mut state = STATE.lock().await;
        if let Some(handle) = state.search_handle.take() {
            handle.abort();
        }
        drop(state);

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            crate::global_store!(ui).set_video_editor_online_search_audio_is_searching(false);
        });
    });
}

fn video_editor_online_search_audio_preview(ui: &AppWindow, index: i32) {
    let idx = index as usize;
    let ui_weak = ui.as_weak();

    let store = crate::global_store!(ui);
    let model = store.get_video_editor_online_search_audio_results();
    let item = match model
        .as_any()
        .downcast_ref::<VecModel<UIOnlineSearchAudioResultItem>>()
    {
        Some(vec_model) => vec_model.row_data(idx),
        None => None,
    };

    let Some(item) = item else {
        crate::toast_warn!(ui, tr("Invalid item index"));
        return;
    };

    if item.is_downloading {
        return;
    }

    crate::toast_info!(ui, tr("Loading audio, please wait..."));

    let source_name = item.source.to_string();
    let download_url = item.download_url.to_string();
    let identifier = item.identifier.to_string();
    let song_name = item.song_name.to_string();
    let singers = item.singers.to_string();
    let ext = item.ext.to_string();
    let cover_url = item.cover_url.to_string();

    let audio_setting = store.get_video_editor_online_search_audio_setting();
    let download_dir = audio_setting.download_dir.to_string();
    let search_limits = if audio_setting.search_limits > 0 {
        audio_setting.search_limits as usize
    } else {
        10
    };

    let image_setting = store.get_video_editor_online_search_image_setting();
    let http_proxy_url = image_setting.http_proxy_url.to_string();
    let socks5_proxy_url = image_setting.socks5_proxy_url.to_string();
    let proxy = proxy_for_type(
        &http_proxy_url,
        &socks5_proxy_url,
        &find_source_proxy_type(&audio_setting, &source_name),
    );

    let download_lrc = global_store!(ui)
        .get_video_editor_online_search_audio_param()
        .download_lrc;
    let download_cover = global_store!(ui)
        .get_video_editor_online_search_audio_param()
        .download_cover;

    tokio::spawn(async move {
        // Check cache first
        let cached = {
            let mut state = STATE.lock().await;
            state.cache.get(&identifier).cloned()
        };

        let dl_info = if let Some(cached_info) = cached {
            cached_info
        } else {
            // Cache miss: set downloading state before network request
            let idx = idx;
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                if let Some(mut item) = store_online_search_audio_results!(ui).row_data(idx) {
                    item.is_downloading = true;
                    store_online_search_audio_results!(ui).set_row_data(idx, item);
                }
            });
            let client = match build_client(
                &download_dir,
                proxy.as_deref(),
                search_limits,
                download_lrc,
                download_cover,
            ) {
                Some(c) => c,
                None => {
                    toast::async_toast_warn(
                        ui_weak.clone(),
                        tr("Failed to create download client"),
                    );
                    mark_download_failed(&ui_weak, idx);
                    return;
                }
            };

            let mut song_info = SongInfo::new(&source_name, &identifier);
            song_info.song_name = Some(song_name.clone());
            song_info.singers = Some(singers.clone());
            song_info.ext = Some(ext.clone());
            song_info.download_url = Some(download_url.clone());
            song_info.download_url_status.ok = true;
            if !cover_url.is_empty() {
                song_info.cover_url = Some(cover_url.clone());
            }
            // Restore lyric from search results
            if let Some(lyric) = STATE.lock().await.lyrics.get(&identifier) {
                song_info.lyric = Some(lyric.clone());
            }

            match client.download(&source_name, &[song_info]).await {
                Ok(downloaded) => match downloaded.into_iter().next() {
                    Some(info) => {
                        // Cache in memory (LRU)
                        let mut state = STATE.lock().await;
                        state.cache.put(identifier.clone(), info.clone());
                        info
                    }
                    None => {
                        toast::async_toast_warn(ui_weak.clone(), tr("Download returned no data"));
                        mark_download_failed(&ui_weak, idx);
                        return;
                    }
                },
                Err(e) => {
                    log::warn!("[OnlineSearchAudio] Preview download failed: {}", e);
                    toast::async_toast_warn(ui_weak.clone(), tr("All download attempts failed"));
                    mark_download_failed(&ui_weak, idx);
                    return;
                }
            }
        };

        // Write to temp file for preview (not user's download dir)
        let file_ext = dl_info.format.extension();
        let mut temp_file = match tempfile::Builder::new()
            .prefix(&identifier)
            .suffix(&format!(".{}", file_ext))
            .tempfile()
        {
            Ok(f) => f,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {}", tr("Failed to create temp file"), e),
                );
                mark_download_failed(&ui_weak, idx);
                return;
            }
        };
        if let Err(e) = temp_file.write_all(&dl_info.data) {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}: {}", tr("Failed to write preview file"), e),
            );
            mark_download_failed(&ui_weak, idx);
            return;
        }
        // Persist the temp file so the path remains valid for the audio player
        let temp_path = match temp_file.keep() {
            Ok((_, path)) => path,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {}", tr("Failed to persist temp file"), e),
                );
                mark_download_failed(&ui_weak, idx);
                return;
            }
        };

        mark_preview_complete(&ui_weak, idx);

        // Set preview item and trigger load (dialog will be shown after audio is loaded)
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let display_name = if singers.is_empty() {
                song_name.clone()
            } else {
                format!("{} - {}", singers, song_name)
            };
            global_store!(ui).set_video_editor_preview_item(UIVideoEditorPlaylistItem {
                file_path: SharedString::from(temp_path.to_string_lossy().to_string()),
                name: SharedString::from(display_name),
                media_type: UIMediaType::Audio,
                duration: SharedString::default(),
                file_size: SharedString::default(),
                thumbnail: Image::default(),
                is_selected: false,
                is_marked: false,
                is_folder: false,
                folder_id: SharedString::default(),
                item_id: SharedString::default(),
                folder_source_path: SharedString::default(),
            });
            // Don't show dialog here — load_audio_preview will show it after loading
            show_preview_item(&ui);
        });
    });
}

fn video_editor_online_search_audio_download(ui: &AppWindow, index: i32) {
    let idx = index as usize;
    let ui_weak = ui.as_weak();

    let store = crate::global_store!(ui);
    let model = store.get_video_editor_online_search_audio_results();
    let item = match model
        .as_any()
        .downcast_ref::<slint::VecModel<UIOnlineSearchAudioResultItem>>()
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
    let download_url = item.download_url.to_string();
    let identifier = item.identifier.to_string();
    let song_name = item.song_name.to_string();
    let singers = item.singers.to_string();
    let ext = item.ext.to_string();
    let cover_url = item.cover_url.to_string();

    let audio_setting = store.get_video_editor_online_search_audio_setting();
    let download_dir = audio_setting.download_dir.to_string();
    let search_limits = if audio_setting.search_limits > 0 {
        audio_setting.search_limits as usize
    } else {
        10
    };

    let image_setting = store.get_video_editor_online_search_image_setting();
    let http_proxy_url = image_setting.http_proxy_url.to_string();
    let socks5_proxy_url = image_setting.socks5_proxy_url.to_string();
    let proxy = proxy_for_type(
        &http_proxy_url,
        &socks5_proxy_url,
        &find_source_proxy_type(&audio_setting, &source_name),
    );

    let download_lrc = global_store!(ui)
        .get_video_editor_online_search_audio_param()
        .download_lrc;
    let download_cover = global_store!(ui)
        .get_video_editor_online_search_audio_param()
        .download_cover;

    item.is_downloading = true;
    if let Some(vec_model) = model
        .as_any()
        .downcast_ref::<VecModel<UIOnlineSearchAudioResultItem>>()
    {
        _ = vec_model.set_row_data(idx, item);
    }

    crate::toast_info!(ui, tr("Downloading, please wait..."));

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

        // Check cache, but re-download if cached data is missing requested content
        let cached = {
            let mut state = STATE.lock().await;
            state.cache.get(&identifier).cloned()
        };

        let cache_usable = cached.as_ref().map_or(false, |info| {
            let lrc_ok = !download_lrc || info.lyric_data.as_ref().map_or(false, |d| !d.is_empty());
            let cover_ok =
                !download_cover || info.cover_data.as_ref().map_or(false, |d| !d.is_empty());
            lrc_ok && cover_ok
        });

        let dl_info = if cache_usable {
            cached.unwrap()
        } else {
            let client = match build_client(
                &download_dir,
                proxy.as_deref(),
                search_limits,
                download_lrc,
                download_cover,
            ) {
                Some(c) => c,
                None => {
                    toast::async_toast_warn(
                        ui_weak.clone(),
                        tr("Failed to create download client"),
                    );
                    mark_download_failed(&ui_weak, idx);
                    return;
                }
            };

            let mut song_info = SongInfo::new(&source_name, &identifier);
            song_info.song_name = Some(song_name.clone());
            song_info.singers = Some(singers.clone());
            song_info.ext = Some(ext.clone());
            song_info.download_url = Some(download_url.clone());
            song_info.download_url_status.ok = true;
            if !cover_url.is_empty() {
                song_info.cover_url = Some(cover_url.clone());
            }

            if let Some(lyric) = STATE.lock().await.lyrics.get(&identifier) {
                song_info.lyric = Some(lyric.clone());
            }

            match client.download(&source_name, &[song_info]).await {
                Ok(downloaded) => match downloaded.into_iter().next() {
                    Some(info) => {
                        let mut state = STATE.lock().await;
                        state.cache.put(identifier.clone(), info.clone());
                        info
                    }
                    None => {
                        toast::async_toast_warn(ui_weak.clone(), tr("Download returned no data"));
                        mark_download_failed(&ui_weak, idx);
                        return;
                    }
                },
                Err(e) => {
                    log::warn!("[OnlineSearchAudio] Download failed: {}", e);
                    toast::async_toast_warn(ui_weak.clone(), tr("All download attempts failed"));
                    mark_download_failed(&ui_weak, idx);
                    return;
                }
            }
        };

        let file_ext = dl_info.format.extension();
        let filename = sanitize_filename(&song_name, &singers, file_ext);
        let save_path = PathBuf::from(&download_dir).join(&filename);

        if std::fs::write(&save_path, &dl_info.data).is_ok() {
            import_file_to_playlist(ui_weak.clone(), save_path, None).await;

            // Save lyrics if available
            if let Some(ref lyric_data) = dl_info.lyric_data
                && !lyric_data.is_empty()
            {
                let lrc_filename = sanitize_filename(&song_name, &singers, "lrc");
                let lrc_save_path = PathBuf::from(&download_dir).join(&lrc_filename);
                if let Err(e) = std::fs::write(&lrc_save_path, lyric_data) {
                    log::warn!("[OnlineSearchAudio] Failed to save lyrics: {}", e);
                } else {
                    import_file_to_playlist(ui_weak.clone(), lrc_save_path, None).await;
                }
            } else if download_lrc {
                toast::async_toast_warn(ui_weak.clone(), tr("No lyrics available for this song"));
            }

            // Save cover if available
            if let Some(ref cover_data) = dl_info.cover_data
                && !cover_data.is_empty()
            {
                let cover_ext_from_url = cover_url
                    .rsplit('.')
                    .next()
                    .filter(|e| {
                        ["jpg", "jpeg", "png", "gif", "webp", "bmp"]
                            .contains(&e.to_lowercase().as_str())
                    })
                    .unwrap_or("jpg");
                let cover_ext = infer::get(cover_data)
                    .map(|t| t.extension())
                    .unwrap_or(cover_ext_from_url);

                let cover_filename = sanitize_filename(&song_name, &singers, cover_ext);
                let cover_save_path = PathBuf::from(&download_dir).join(&cover_filename);
                if let Err(e) = std::fs::write(&cover_save_path, cover_data) {
                    log::warn!("[OnlineSearchAudio] Failed to save cover: {}", e);
                } else {
                    import_file_to_playlist(ui_weak.clone(), cover_save_path, None).await;
                }
            } else if download_cover {
                toast::async_toast_warn(ui_weak.clone(), tr("No cover available for this song"));
            }

            mark_download_success(&ui_weak, idx);
        } else {
            toast::async_toast_warn(ui_weak.clone(), tr("Failed to save downloaded file"));
            mark_download_failed(&ui_weak, idx);
        }
    });
}

fn sanitize_filename(song_name: &str, singers: &str, ext: &str) -> String {
    let raw = format!("{} - {}.{}", singers, song_name, ext);
    raw.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn mark_download_failed(ui_weak: &Weak<AppWindow>, idx: usize) {
    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        if let Some(mut item) = store_online_search_audio_results!(ui).row_data(idx) {
            item.is_downloading = false;
            store_online_search_audio_results!(ui).set_row_data(idx, item);
        }
    });
}

fn mark_preview_complete(ui_weak: &Weak<AppWindow>, idx: usize) {
    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        if let Some(mut item) = store_online_search_audio_results!(ui).row_data(idx) {
            item.is_downloading = false;
            store_online_search_audio_results!(ui).set_row_data(idx, item);
        }
    });
}

fn mark_download_success(ui_weak: &Weak<AppWindow>, idx: usize) {
    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        if let Some(mut item) = store_online_search_audio_results!(ui).row_data(idx) {
            item.is_downloading = false;
            item.is_downloaded = true;
            store_online_search_audio_results!(ui).set_row_data(idx, item);
        }
    });
}

fn video_editor_online_search_audio_update_config(
    ui: &AppWindow,
    config: UIOnlineSearchAudioSetting,
) {
    global_store!(ui).set_video_editor_online_search_audio_setting(config.clone());
    save_config_to_db(config.into());
}

fn video_editor_online_search_audio_update_source(
    ui: &AppWindow,
    index: i32,
    source: UIOnlineSearchAudioSourceEntry,
) {
    let sources = global_store!(ui)
        .get_video_editor_online_search_audio_setting()
        .sources;
    store_video_editor_audio_sources!(sources).set_row_data(index as usize, source);

    sort_sources_in_store(ui);

    save_config_to_db(
        global_store!(ui)
            .get_video_editor_online_search_audio_setting()
            .into(),
    );
}

fn video_editor_online_search_audio_refresh_sources_status(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    let store = crate::global_store!(ui);
    let audio_setting = store.get_video_editor_online_search_audio_setting();
    let download_dir = audio_setting.download_dir.to_string();
    let search_limits = if audio_setting.search_limits > 0 {
        audio_setting.search_limits as usize
    } else {
        10
    };

    let http_proxy_url = global_store!(ui)
        .get_video_editor_online_search_image_setting()
        .http_proxy_url
        .to_string();
    let socks5_proxy_url = global_store!(ui)
        .get_video_editor_online_search_image_setting()
        .socks5_proxy_url
        .to_string();

    let source_configs: Vec<(String, String)> = audio_setting
        .sources
        .iter()
        .map(|e| (e.name.to_string(), e.proxy_type.to_string()))
        .collect();

    let sources = store.get_video_editor_online_search_audio_setting().sources;
    let vec_model = store_video_editor_audio_sources!(sources);
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
            let client =
                match build_client(&download_dir, proxy.as_deref(), search_limits, false, false) {
                    Some(c) => c,
                    None => continue,
                };

            let ui_weak = ui_weak.clone();
            handles.push(tokio::spawn(async move {
                let results = client.search("test", &[source_name.as_str()]).await;
                let is_ok = match results.get(&source_name) {
                    Some(SearchResult::Ok(songs)) => !songs.is_empty(),
                    _ => false,
                };

                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let sources = global_store!(&ui)
                        .get_video_editor_online_search_audio_setting()
                        .sources;
                    let vec_model = store_video_editor_audio_sources!(sources);

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
                log::warn!("[OnlineSearchAudio] Status check task failed: {}", e);
            }
        }
    });
}

fn video_editor_online_search_audio_refresh_source_status(ui: &AppWindow, index: i32) {
    let ui_weak = ui.as_weak();
    let store = crate::global_store!(ui);
    let audio_setting = store.get_video_editor_online_search_audio_setting();
    let download_dir = audio_setting.download_dir.to_string();

    let source_name = audio_setting
        .sources
        .iter()
        .nth(index as usize)
        .map(|e| e.name.to_string())
        .unwrap_or_default();
    let proxy_type = find_source_proxy_type(&audio_setting, &source_name);

    let image_setting = store.get_video_editor_online_search_image_setting();
    let http_proxy_url = image_setting.http_proxy_url.to_string();
    let socks5_proxy_url = image_setting.socks5_proxy_url.to_string();
    let proxy = proxy_for_type(&http_proxy_url, &socks5_proxy_url, &proxy_type);

    let sources = audio_setting.sources;
    let vec_model = store_video_editor_audio_sources!(sources);
    let Some(mut entry) = vec_model.row_data(index as usize) else {
        return;
    };

    entry.is_testing = true;
    _ = vec_model.set_row_data(index as usize, entry);

    tokio::spawn(async move {
        let client = match build_client(&download_dir, proxy.as_deref(), 10, false, false) {
            Some(c) => c,
            None => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let sources = global_store!(&ui)
                        .get_video_editor_online_search_audio_setting()
                        .sources;
                    let vec_model = store_video_editor_audio_sources!(sources);
                    if let Some(mut entry) = vec_model.row_data(index as usize) {
                        entry.is_testing = false;
                        entry.can_access = false;
                        _ = vec_model.set_row_data(index as usize, entry);
                    }
                });
                return;
            }
        };

        let results = client.search("test", &[source_name.as_str()]).await;
        let is_ok = match results.get(&source_name) {
            Some(SearchResult::Ok(songs)) => !songs.is_empty(),
            _ => false,
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let sources = global_store!(&ui)
                .get_video_editor_online_search_audio_setting()
                .sources;
            let vec_model = store_video_editor_audio_sources!(sources);
            if let Some(mut entry) = vec_model.row_data(index as usize) {
                entry.is_testing = false;
                entry.can_access = is_ok;
                _ = vec_model.set_row_data(index as usize, entry);
            }
        });
    });
}

fn video_editor_online_search_audio_select_all_sources(ui: &AppWindow) {
    set_select_all_sources(ui, true);
}

fn video_editor_online_search_audio_deselect_all_sources(ui: &AppWindow) {
    set_select_all_sources(ui, false);
}

fn set_select_all_sources(ui: &AppWindow, enabled: bool) {
    let sources = global_store!(ui)
        .get_video_editor_online_search_audio_setting()
        .sources;
    let vec_model = store_video_editor_audio_sources!(sources);
    for (i, mut entry) in vec_model.iter().enumerate() {
        entry.enabled = enabled;
        _ = vec_model.set_row_data(i, entry);
    }

    sort_sources_in_store(ui);

    save_config_to_db(
        global_store!(ui)
            .get_video_editor_online_search_audio_setting()
            .into(),
    );
}

fn video_editor_online_search_audio_choose_save_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(dir) = picker_directory(ui_weak.clone(), &tr("Choose save directory")) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut setting = global_store!(ui).get_video_editor_online_search_audio_setting();
            setting.download_dir = dir.to_string_lossy().to_string().into();
            global_logic!(ui).invoke_video_editor_online_search_audio_update_config(setting);
        });
    });
}

fn video_editor_online_search_audio_config_is_valid(ui: &AppWindow) -> bool {
    let setting = global_store!(ui).get_video_editor_online_search_audio_setting();

    if !global_logic!(ui).invoke_dir_exist(setting.download_dir) || setting.search_limits <= 0 {
        return false;
    }

    setting.sources.iter().any(|e| e.enabled)
}

impl From<SongInfo> for UIOnlineSearchAudioResultItem {
    fn from(info: SongInfo) -> Self {
        Self {
            source: SharedString::from(info.source),
            song_name: SharedString::from(info.song_name.unwrap_or_default()),
            singers: SharedString::from(info.singers.unwrap_or_default()),
            album: SharedString::from(info.album.unwrap_or_default()),
            duration: SharedString::from(info.duration.unwrap_or_default()),
            ext: SharedString::from(info.ext.unwrap_or_default()),
            file_size: SharedString::from(info.file_size.unwrap_or_default()),
            bitrate: info.bitrate.unwrap_or(0) as i32,
            cover_url: SharedString::from(info.cover_url.unwrap_or_default()),
            download_url: SharedString::from(info.download_url.unwrap_or_default()),
            identifier: SharedString::from(info.identifier),
            thumbnail: Image::default(),
            is_downloading: false,
            is_downloaded: false,
        }
    }
}
