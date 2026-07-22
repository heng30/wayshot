use crate::{
    config,
    db::{SubtitleTranslateConfigData, VIDEO_EDITOR_TABLE},
    global_store, global_ve_filter,
    logic::{
        toast,
        tr::tr,
        video_editor::{
            command::sync_manager_to_ui, command::with_history_manager,
            common_type::SubtitleStyleConfig,
            filters::subtitle::create_subtitle_style_filters_from_config,
            project::SUBTITLE_TRANSLATE_CONFIG_ID,
        },
    },
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, SubtitleTranslateConfig as UISubtitleTranslateConfig,
        SubtitleTranslateProgressType as UISubtitleTranslateProgressType,
    },
    toast_info, toast_warn,
};
use anyhow::{Result, anyhow};
use bot::{APIConfig, Chat, ChatConfig, StreamTextItem};
use once_cell::sync::Lazy;
use slint::{ComponentHandle, Weak};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};
use video_editor::{
    commands::{filter::AddFilterCommand, subtitle::AddSubtitleCommand, track::AddTrackCommand},
    filters::traits::SubtitleEntry,
    metadata::Metadata,
    tracks::{
        subtitle_track::SubtitleTrack,
        track::{InnerTrack, Track},
    },
};

#[derive(serde::Serialize, Clone)]
struct TranslateInput {
    index: usize,
    text: String,
}

#[derive(serde::Deserialize)]
struct TranslateOutput {
    index: usize,
    translation: String,
}

#[derive(Default)]
struct SubtitleTranslateCache {
    stop_sig: Option<Arc<AtomicBool>>,
    inc_index: u64,
}

static SUBTITLE_TRANSLATE_CACHE: Lazy<Mutex<SubtitleTranslateCache>> =
    Lazy::new(|| Mutex::new(SubtitleTranslateCache::default()));

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_subtitle_translate_start, ui, config);
    logic_cb!(video_editor_subtitle_translate_cancel, ui);
    logic_cb!(video_editor_subtitle_translate_update_config, ui, config);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_config().await.unwrap_or_else(|| {
            let mut config = SubtitleTranslateConfigData::default();
            config.id = SUBTITLE_TRANSLATE_CONFIG_ID.to_string();
            config
        });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_subtitle_translate_config(config.into());
        });
    });
}

fn save_config(config: SubtitleTranslateConfigData) {
    tokio::spawn(async move {
        let data =
            serde_json::to_string(&config).expect("serialize subtitle translate config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, SUBTITLE_TRANSLATE_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, SUBTITLE_TRANSLATE_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save subtitle translate config: {:?}", e);
            }
        }
    });
}

async fn load_config() -> Option<SubtitleTranslateConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, SUBTITLE_TRANSLATE_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn video_editor_subtitle_translate_update_config(
    ui: &AppWindow,
    config: UISubtitleTranslateConfig,
) {
    global_store!(ui).set_video_editor_subtitle_translate_config(config.clone());
    let data: SubtitleTranslateConfigData = config.into();
    save_config(data);
}

fn video_editor_subtitle_translate_start(ui: &AppWindow, ui_config: UISubtitleTranslateConfig) {
    let setting = config::all().ai_model;
    if setting.api_base_url.is_empty()
        || setting.model_name.is_empty()
        || setting.api_key.is_empty()
    {
        toast_info!(ui, tr("Please setup AI model and try again"));
        return;
    }

    if ui_config.prompt.is_empty() {
        toast_info!(ui, tr("Please enter translation prompt"));
        return;
    }

    video_editor_subtitle_translate_update_config(ui, ui_config.clone());

    let prompt = ui_config.prompt.to_string();
    let batch_size = ui_config.batch_size.max(1) as usize;
    let max_retries = ui_config.max_retries.max(0) as usize;

    let (subtitle_entries, source_track_name) = {
        let state = crate::logic::video_editor::project::PROJECT_STATE
            .lock()
            .unwrap();
        let Some(ref s) = *state else {
            toast_warn!(ui, tr("No project opened"));
            return;
        };

        let mut found_entries: Vec<SubtitleEntry> = vec![];
        let mut track_name = "S".to_string();
        for track in &s.tracks_manager.tracks {
            if let Track::Subtitle(st) = track {
                found_entries = st.get_subtitle_entries();
                track_name = st.name.clone();
                break;
            }
        }

        (found_entries, track_name)
    };

    if subtitle_entries.is_empty() {
        toast_warn!(ui, tr("No subtitle track found or subtitle track is empty"));
        return;
    }

    global_store!(ui).set_video_editor_subtitle_translate_progress_type(
        UISubtitleTranslateProgressType::Translating,
    );
    global_store!(ui).set_video_editor_subtitle_translate_progress(0.0);

    let stop_sig = Arc::new(AtomicBool::new(false));
    let inc_index = {
        let mut cache = SUBTITLE_TRANSLATE_CACHE.lock().unwrap();
        if let Some(sig) = cache.stop_sig.take() {
            sig.store(true, Ordering::Relaxed);
        }
        cache.inc_index += 1;
        cache.stop_sig = Some(stop_sig.clone());
        cache.inc_index
    };

    let ui_weak = ui.as_weak();
    let prompt_str = prompt;

    tokio::spawn(async move {
        let total_count = subtitle_entries.len();

        // Build input entries
        let input_entries: Vec<TranslateInput> = subtitle_entries
            .iter()
            .enumerate()
            .map(|(index, entry)| TranslateInput {
                index,
                text: entry.text.clone(),
            })
            .collect();

        // Track which indices have been successfully translated
        let mut translations: HashMap<usize, String> = HashMap::new();
        let mut failed_indices: Vec<usize> = (0..input_entries.len()).collect();

        for retry in 0..=max_retries {
            if failed_indices.is_empty() {
                break;
            }

            if stop_sig.load(Ordering::Relaxed) {
                break;
            }

            let entries_to_translate: Vec<TranslateInput> = failed_indices
                .iter()
                .filter_map(|&idx| input_entries.get(idx).cloned())
                .collect();

            let mut new_failures = Vec::new();
            let finished_count =
                Arc::new(AtomicU32::new((total_count - failed_indices.len()) as u32));

            for (chunk_idx, chunk) in entries_to_translate.chunks(batch_size).enumerate() {
                if stop_sig.load(Ordering::Relaxed) {
                    break;
                }

                let chunk_vec = chunk.to_vec();
                let chunk_indices: Vec<usize> = chunk_vec.iter().map(|e| e.index).collect();

                match ai_translate_subtitles(
                    ui_weak.clone(),
                    chunk_vec,
                    &prompt_str,
                    stop_sig.clone(),
                )
                .await
                {
                    Ok(corrections) => {
                        for (idx, text) in &corrections {
                            translations.insert(*idx, text.clone());
                        }

                        // Track which chunk entries were not in the response
                        for index in &chunk_indices {
                            if !corrections.contains_key(index) {
                                new_failures.push(*index);
                            }
                        }

                        let count =
                            finished_count.fetch_add(corrections.len() as u32, Ordering::Relaxed);
                        let progress =
                            (count + corrections.len() as u32) as f32 / total_count as f32;

                        _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            global_store!(ui)
                                .set_video_editor_subtitle_translate_progress(progress);
                        });
                    }
                    Err(e) => {
                        log::warn!(
                            "Subtitle translate chunk {} retry {} failed: {e}",
                            chunk_idx,
                            retry
                        );
                        // All entries in this chunk failed
                        for index in &chunk_indices {
                            new_failures.push(*index);
                        }
                    }
                }
            }

            failed_indices = new_failures;

            if !failed_indices.is_empty() && retry < max_retries {
                log::info!(
                    "Retrying {} failed subtitle entries (attempt {}/{})",
                    failed_indices.len(),
                    retry + 1,
                    max_retries
                );
            }
        }

        if stop_sig.load(Ordering::Relaxed) {
            return;
        }

        if inc_index != SUBTITLE_TRANSLATE_CACHE.lock().unwrap().inc_index {
            return;
        }

        // Report failures
        if !failed_indices.is_empty() {
            let msg = format!(
                "{}: {}/{}",
                tr("Failed to translate some subtitles"),
                failed_indices.len(),
                total_count
            );
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                crate::toast_warn!(ui, msg);
            });
        }

        // Build translated subtitle entries
        let translated_entries: Vec<SubtitleEntry> = subtitle_entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                translations.get(&index).map(|text| SubtitleEntry {
                    start: entry.start,
                    end: entry.end,
                    text: text.clone(),
                })
            })
            .collect();

        if translated_entries.is_empty() {
            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                global_store!(ui).set_video_editor_subtitle_translate_progress_type(
                    UISubtitleTranslateProgressType::Failed,
                );
                crate::toast_warn!(ui, tr("No subtitles were translated"));
            });
            return;
        }

        // Add new subtitle track with translations
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let subtitle_style: SubtitleStyleConfig =
                global_ve_filter!(ui).get_subtitle_style().into();

            let result = with_history_manager(|state| {
                state
                    .history_manager
                    .begin_batch("Add translated subtitle track".to_string());

                // Create new subtitle track
                let metadata = Arc::new(Metadata::default());
                let inner_track = InnerTrack::new(metadata, Duration::ZERO, vec![]);
                let track_name = format!("{source_track_name} [tr]");
                let subtitle_track = SubtitleTrack::new(inner_track).with_name(track_name.clone());
                let add_track_cmd = AddTrackCommand::new(Track::Subtitle(Arc::new(subtitle_track)));
                state
                    .history_manager
                    .execute(&mut state.tracks_manager, Box::new(add_track_cmd))?;

                // Find the new track index by name
                let new_track_index = state
                    .tracks_manager
                    .tracks
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, t)| matches!(t, Track::Subtitle(st) if st.name == track_name))
                    .map(|(i, _)| i)
                    .ok_or_else(|| {
                        video_editor::Error::InvalidConfig(
                            "Failed to find added subtitle track".into(),
                        )
                    })?;

                // Add each translated subtitle with style filters
                let style_filters = create_subtitle_style_filters_from_config(&subtitle_style);
                for entry in &translated_entries {
                    let add_cmd = AddSubtitleCommand::new(new_track_index, entry.clone());
                    state
                        .history_manager
                        .execute(&mut state.tracks_manager, Box::new(add_cmd))?;

                    // Find the inserted segment index
                    let segment_index = state
                        .tracks_manager
                        .get(new_track_index)
                        .and_then(|t| {
                            if let Track::Subtitle(inner) = t {
                                inner
                                    .track
                                    .segments
                                    .iter()
                                    .position(|seg| seg.timeline_offset == entry.start)
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| {
                            video_editor::Error::InvalidConfig(
                                "Failed to find inserted subtitle index".into(),
                            )
                        })?;

                    // Add subtitle style filters
                    for filter in &style_filters {
                        let filter_cmd = AddFilterCommand::new_subtitle(
                            new_track_index,
                            segment_index,
                            filter.clone_box(),
                        );
                        state
                            .history_manager
                            .execute(&mut state.tracks_manager, Box::new(filter_cmd))?;
                    }
                }

                state.history_manager.end_batch()?;

                Ok::<(), video_editor::Error>(())
            });

            match result {
                Ok(_) => {
                    sync_manager_to_ui(&ui);
                    global_store!(ui).set_video_editor_subtitle_translate_progress(1.0);
                    global_store!(ui).set_video_editor_subtitle_translate_progress_type(
                        UISubtitleTranslateProgressType::Finished,
                    );
                    crate::toast_success!(ui, tr("Subtitle translation completed"));
                }
                Err(e) => {
                    crate::toast_warn!(ui, e.to_string());
                    global_store!(ui).set_video_editor_subtitle_translate_progress_type(
                        UISubtitleTranslateProgressType::Failed,
                    );
                }
            }
        });
    });
}

fn video_editor_subtitle_translate_cancel(ui: &AppWindow) {
    {
        let mut cache = SUBTITLE_TRANSLATE_CACHE.lock().unwrap();
        if let Some(sig) = cache.stop_sig.take() {
            sig.store(true, Ordering::Relaxed);
        }
        cache.inc_index += 1;
    }

    global_store!(ui).set_video_editor_subtitle_translate_progress_type(
        UISubtitleTranslateProgressType::Cancelled,
    );
}

async fn ai_translate_subtitles(
    ui_weak: Weak<AppWindow>,
    subtitles: Vec<TranslateInput>,
    custom_prompt: &str,
    stop_sig: Arc<AtomicBool>,
) -> Result<HashMap<usize, String>> {
    let prompt = format!(
        r#"You are a subtitle translation assistant. {custom_prompt}

<Input format>
[{{"index": 0, "text": "original text"}}, ...]
</Input format>

<Output format>
[{{"index": 0, "translation": "translated text"}}, ...]
</Output format>

Rules:
- Translate each subtitle text as instructed
- Keep the translation natural and concise
- Maintain the same index for each entry
- Only output the JSON array, no additional text
"#
    );

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamTextItem>(100);
    let question = serde_json::to_string(&subtitles)?;
    let model_config = config::all().ai_model.clone();
    let request_config = APIConfig {
        api_base_url: model_config.api_base_url,
        api_model: model_config.model_name,
        api_key: model_config.api_key,
        temperature: Some(0.3),
    };

    let ui_weak_for_chat = ui_weak.clone();
    tokio::spawn(async move {
        let chat_config = ChatConfig { tx };
        let chat = Chat::new(&prompt, question, chat_config, request_config, vec![]);
        if let Err(e) = chat.start().await {
            toast::async_toast_warn(
                ui_weak_for_chat,
                format!("{}: {e}", tr("Start AI translation failed")),
            );
        }
    });

    let mut resp = String::new();
    while let Some(item) = rx.recv().await {
        if stop_sig.load(Ordering::Relaxed) {
            return Ok(HashMap::new());
        }

        if let Some(ref text) = item.text {
            resp.push_str(text);
        }
    }

    if stop_sig.load(Ordering::Relaxed) {
        return Ok(HashMap::new());
    }

    let resp = resp
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    log::debug!("Subtitle translate response: {resp}");

    let output: Vec<TranslateOutput> = serde_json::from_str(&resp)
        .map_err(|e| anyhow!("Failed to parse AI response as JSON: {e}. Response: {resp}"))?;

    let translations = output
        .into_iter()
        .map(|item| (item.index, item.translation))
        .collect();

    Ok(translations)
}
