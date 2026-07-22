use crate::{
    config,
    db::{TTSConfigData, VIDEO_EDITOR_TABLE},
    global_logic, global_store,
    logic::video_editor::project::{TTS_CONFIG_ID, TTS_IMPORTED_VOICE_ID, TTS_MARKED_VOICE_ID},
    logic::{
        recorder::picker_directory, share_screen::picker_file, toast, tr::tr,
        video_editor::playlist::import_file_to_playlist,
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, Downloader as UIDownloader, DownloaderState as UIDownloaderState,
        TTSConfig as UITTSConfig, TTSEntry as UITTSEntry, TTSVoice as UITTSVoice,
    },
};
use anyhow::Result;
use bot::{APIConfig, Chat, ChatConfig, StreamTextItem};
use hf_hub::{HFClient, split_id};
use hound::{WavSpec, WavWriter};
use rodio::{ChannelCount, MixerDeviceSink, Player, SampleRate, buffer::SamplesBuffer};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};
use voxcpm_rs::{
    CancelToken, GenerateOptions, Prompt, PromptAudio, VoxCPM,
    burn::{
        backend::{NdArray, Wgpu, ndarray::NdArrayDevice, wgpu::WgpuDevice},
        tensor::backend::Backend,
    },
};

// Female voices
static FEMALE_LEISURE_SENIOR_SISTER_1: &[u8] =
    include_bytes!("../../../ui/audio/female_leisure_senior_sister_1_24k_mono.wav");
static FEMALE_ENERGETIC_SISTER_2: &[u8] =
    include_bytes!("../../../ui/audio/female_energetic_sister_2_24k_mono.wav");
static FEMALE_WORKPLACE_ELITE_3: &[u8] =
    include_bytes!("../../../ui/audio/female_workplace_elite_3_24k_mono.wav");
static FEMALE_PURE_SWEET_GIRL_4: &[u8] =
    include_bytes!("../../../ui/audio/female_pure_sweet_girl_4_24k_mono.wav");
static FEMALE_QUICK_TALKATIVE_ONEE_SAN_5: &[u8] =
    include_bytes!("../../../ui/audio/female_quick_talkative_onee-san_5_24k_mono.wav");
static FEMALE_RADIO_ANCHOR_6: &[u8] =
    include_bytes!("../../../ui/audio/female_radio_anchor_6_24k_mono.wav");
static FEMALE_KNOWLEDGE_EXPLANATION_7: &[u8] =
    include_bytes!("../../../ui/audio/female_knowledge_explanation_7_24k_mono.wav");
static FEMALE_INTELLECTUAL_SISTER_8: &[u8] =
    include_bytes!("../../../ui/audio/female_intellectual_sister_8_24k_mono.wav");
static FEMALE_FRIENDLY_ANNOUNCEMENT_9: &[u8] =
    include_bytes!("../../../ui/audio/female_friendly_announcement_9_24k_mono.wav");
static FEMALE_CALM_NARRATION_10: &[u8] =
    include_bytes!("../../../ui/audio/female_calm_narration_10_24k_mono.wav");
static FEMALE_ATMOSPHERIC_NARRATION_11: &[u8] =
    include_bytes!("../../../ui/audio/female_atmospheric_narration_11_24k_mono.wav");
static FEMALE_NEWS_ANCHOR_12: &[u8] =
    include_bytes!("../../../ui/audio/female_news_anchor_12_24k_mono.wav");

// Male voices
static MALE_VIVID_EXPLANATION_1: &[u8] =
    include_bytes!("../../../ui/audio/male_vivid_explanation_1_24k_mono.wav");
static MALE_CLEAR_BRIGHT_2: &[u8] =
    include_bytes!("../../../ui/audio/male_clear_bright_2_24k_mono.wav");
static MALE_MAGNETIC_3: &[u8] = include_bytes!("../../../ui/audio/male_magnetic_3_24k_mono.wav");
static MALE_CLEAR_YOUNG_MAN_4: &[u8] =
    include_bytes!("../../../ui/audio/male_clear_oung_man_4_24k_mono.wav");
static MALE_BROADCASTING_HOSTING_5: &[u8] =
    include_bytes!("../../../ui/audio/male_broadcasting_hosting_5_24k_mono.wav");
static MALE_INTELLECTUAL_GUY_6: &[u8] =
    include_bytes!("../../../ui/audio/male_intellectual_guy_6_24k_mono.wav");
static MALE_SCIENCE_MEN_VOICE_7: &[u8] =
    include_bytes!("../../../ui/audio/male_science_men_voice_7_24k_mono.wav");
static MALE_CIVILIZED_MALE_VOICE_8: &[u8] =
    include_bytes!("../../../ui/audio/male_civilized_male_voice_8_24k_mono.wav");
static MALE_STEADY_MATURE_MAN_9: &[u8] =
    include_bytes!("../../../ui/audio/male_steady_mature_man_9_24k_mono.wav");
static MALE_DEEP_LOVE_MALE_LEAD_10: &[u8] =
    include_bytes!("../../../ui/audio/male_deep_love_male_lead_10_24k_mono.wav");
static MALE_AFFABLE_MALE_VOICE_11: &[u8] =
    include_bytes!("../../../ui/audio/male_affable_male_voice_11_24k_mono.wav");
static MALE_FINANCIAL_ANCHOR_12: &[u8] =
    include_bytes!("../../../ui/audio/male_financial_anchor_12_24k_mono.wav");
static VOXCPM_MODEL: OnceLock<Arc<Mutex<Option<VoxCpmModel>>>> = OnceLock::new();
static AUDIO_DEVICE: OnceLock<Arc<MixerDeviceSink>> = OnceLock::new();
static CURRENT_PLAYER: OnceLock<Mutex<Option<Arc<Player>>>> = OnceLock::new();
static GENERATE_ALL_CANCEL: Mutex<Option<CancelToken>> = Mutex::new(None);
static PROGRESS_TASK_CANCEL: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
static PLAYER_INC_INDEX: AtomicU32 = AtomicU32::new(0);
static MARKED_VOICE_NAMES: Mutex<Option<HashSet<String>>> = Mutex::new(None);

const VOXCPM_CHANNEL: u16 = 1;
const VOXCPM_SAMPLE_RATE: u32 = 48000;
const BUILT_IN_VOICES: &[(&str, &[u8])] = &[
    // Male voices
    ("Vivid Explanation", MALE_VIVID_EXPLANATION_1),
    ("Clear Bright", MALE_CLEAR_BRIGHT_2),
    ("Magnetic", MALE_MAGNETIC_3),
    ("Clear Young Man", MALE_CLEAR_YOUNG_MAN_4),
    ("Broadcasting Hosting", MALE_BROADCASTING_HOSTING_5),
    ("Intellectual Guy", MALE_INTELLECTUAL_GUY_6),
    ("Science Men Voice", MALE_SCIENCE_MEN_VOICE_7),
    ("Civilized Male Voice", MALE_CIVILIZED_MALE_VOICE_8),
    ("Steady Mature Man", MALE_STEADY_MATURE_MAN_9),
    ("Deep Love Male Lead", MALE_DEEP_LOVE_MALE_LEAD_10),
    ("Affable Male Voice", MALE_AFFABLE_MALE_VOICE_11),
    ("Financial Anchor", MALE_FINANCIAL_ANCHOR_12),
    // Female voices
    ("Leisure Senior Sister", FEMALE_LEISURE_SENIOR_SISTER_1),
    ("Energetic Sister", FEMALE_ENERGETIC_SISTER_2),
    ("Workplace Elite", FEMALE_WORKPLACE_ELITE_3),
    ("Pure Sweet Girl", FEMALE_PURE_SWEET_GIRL_4),
    (
        "Quick Talkative Onee-san",
        FEMALE_QUICK_TALKATIVE_ONEE_SAN_5,
    ),
    ("Radio Anchor", FEMALE_RADIO_ANCHOR_6),
    ("Knowledge Explanation", FEMALE_KNOWLEDGE_EXPLANATION_7),
    ("Intellectual Sister", FEMALE_INTELLECTUAL_SISTER_8),
    ("Friendly Announcement", FEMALE_FRIENDLY_ANNOUNCEMENT_9),
    ("Calm Narration", FEMALE_CALM_NARRATION_10),
    ("Atmospheric Narration", FEMALE_ATMOSPHERIC_NARRATION_11),
    ("News Anchor", FEMALE_NEWS_ANCHOR_12),
];

enum VoxCpmModel {
    Wgpu {
        model: VoxCPM<Wgpu<f32, i32>>,
        device: WgpuDevice,
    },
    NdArray(VoxCPM<NdArray<f32>>),
}

#[macro_export]
macro_rules! store_video_editor_tts_entries {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_tts_entries()
            .as_any()
            .downcast_ref::<VecModel<UITTSEntry>>()
            .expect("We know we set a VecModel<UITTSEntry> earlier")
    };
}

#[macro_export]
macro_rules! store_video_editor_tts_voices {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_tts_voices()
            .as_any()
            .downcast_ref::<VecModel<UITTSVoice>>()
            .expect("We know we set a VecModel<UITTSVoice> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_tts_generate_speech, ui, index);
    logic_cb!(video_editor_tts_play_speech, ui, index);
    logic_cb!(video_editor_tts_play_all_speech, ui, start_ms, end_ms);
    logic_cb!(video_editor_tts_stop_playing_all_speech, ui);
    logic_cb!(video_editor_tts_export_to_playlist, ui);
    logic_cb_pure!(video_editor_tts_has_speech, ui);
    logic_cb_pure!(video_editor_tts_has_empty_speech, ui);
    logic_cb_pure!(video_editor_tts_get_total_duration, ui, entry, flag);
    logic_cb!(video_editor_tts_generate_all_speech, ui);
    logic_cb!(video_editor_tts_cancel_generating_all, ui);
    logic_cb!(video_editor_tts_remove_all_speech, ui);
    logic_cb!(video_editor_tts_remove_all_entries, ui);
    logic_cb!(video_editor_tts_update_config, ui, config);
    logic_cb!(video_editor_tts_add_entry, ui, text);
    logic_cb!(video_editor_tts_auto_segment, ui, text);
    logic_cb!(video_editor_tts_ai_segment, ui, text);
    logic_cb!(video_editor_tts_insert_entry, ui, index);
    logic_cb!(video_editor_tts_remove_entry, ui, index);
    logic_cb!(video_editor_tts_clear_entry_audio, ui, index);
    logic_cb!(video_editor_tts_update_entry, ui, index, text);
    logic_cb!(video_editor_tts_audio_player_sound_changed, ui, sound);
    logic_cb!(video_editor_tts_audio_player_sound_released, ui, sound);
    logic_cb!(video_editor_tts_audio_player_progress_changed, ui, value);
    logic_cb!(video_editor_tts_audio_player_progress_released, ui, value);
    logic_cb!(video_editor_tts_audio_player_progress_pressed, ui, value);

    logic_cb!(video_editor_tts_open_choose_save_dir, ui);
    logic_cb!(video_editor_tts_choose_model_dir, ui);
    logic_cb!(video_editor_tts_model_start_download, ui, url);
    logic_cb!(video_editor_tts_setting_is_valid, ui);
    logic_cb!(video_editor_tts_choose_reference_audio, ui);
    logic_cb!(video_editor_tts_play_reference_audio, ui);

    logic_cb!(video_editor_tts_import_voice, ui);
    logic_cb!(video_editor_tts_toggle_voice_marked, ui, index);
    logic_cb!(video_editor_tts_play_voice, ui, path);
    logic_cb!(video_editor_tts_remove_voice, ui, index);
    logic_cb!(video_editor_tts_release_resource, ui);
    logic_cb!(video_editor_tts_add_preamble_entry, ui);
}

fn inner_init(ui: &AppWindow) {
    store_video_editor_tts_entries!(ui).set_vec(vec![]);
    let built_in_voices: Vec<UITTSVoice> = BUILT_IN_VOICES
        .iter()
        .enumerate()
        .map(|(idx, (name, _))| UITTSVoice {
            name: (*name).into(),
            path: format!("built_in://{}", idx).into(),
            marked: false,
            built_in: true,
        })
        .collect();
    store_video_editor_tts_voices!(ui).set_vec(built_in_voices);

    let device =
        rodio::DeviceSinkBuilder::open_default_sink().expect("Failed to open default audio device");
    AUDIO_DEVICE
        .set(Arc::new(device))
        .ok()
        .expect("AUDIO_DEVICE already initialized");

    CURRENT_PLAYER
        .set(Mutex::new(None))
        .ok()
        .expect("CURRENT_PLAYER already initialized");

    VOXCPM_MODEL
        .set(Arc::new(Mutex::new(None)))
        .ok()
        .expect("VOXCPM_MODEL already initialized");

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        load_imported_voices(ui_weak.clone()).await;

        let config = load_tts_config().await.unwrap_or_else(|| TTSConfigData {
            id: TTS_CONFIG_ID.to_string(),
            ..Default::default()
        });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_tts_config(config.into());
        });
    });

    let downloader = UIDownloader {
        url: "https://huggingface.co/openbmb/VoxCPM2".to_string().into(),
        filename: "VoxCPM2".to_string().into(),
        state: UIDownloaderState::UnStart,
        progress: 0.0,
    };
    global_store!(ui)
        .set_video_editor_tts_models_dowloader(ModelRc::new(VecModel::from_slice(&[downloader])));
}

fn video_editor_tts_generate_speech(ui: &AppWindow, index: i32) {
    let model_dir = match get_and_check_model_setting(ui) {
        Ok(item) => item,
        Err(e) => {
            global_store!(ui).set_video_editor_is_show_tts_dialog(true);
            crate::toast_warn!(ui, format!("{e}"));
            return;
        }
    };

    let Some(entry) = store_video_editor_tts_entries!(ui).row_data(index as usize) else {
        crate::toast_warn!(ui, format!("{} {}", tr("No entry at index"), index));
        return;
    };

    let text = entry.text.to_string();
    if text.is_empty() {
        crate::toast_warn!(ui, tr("Text is empty"));
        return;
    }

    let config = global_store!(ui).get_video_editor_tts_config();
    let max_char_count = config.max_char_count;
    let char_count = text.chars().count() as i32;
    if char_count > max_char_count {
        crate::toast_warn!(
            ui,
            format!(
                "{}: [{}] {} / {}",
                tr("Entry exceeds the character limit"),
                index + 1,
                char_count,
                max_char_count
            )
        );
        return;
    }

    // 获取上一段已生成语音的上下文（用于续写，保持音色一致性）
    let prev_context = if index > 0 {
        store_video_editor_tts_entries!(ui)
            .row_data((index - 1) as usize)
            .and_then(|prev_entry| {
                let prev_samples: Vec<f32> = prev_entry.samples.iter().collect();
                if prev_samples.is_empty() || prev_entry.text.is_empty() {
                    None
                } else {
                    Some((prev_samples, prev_entry.text.to_string()))
                }
            })
    } else {
        None
    };

    let config = global_store!(ui).get_video_editor_tts_config();

    update_entry_generating(ui, index, true);
    let use_gpu = config.use_gpu;
    let reference_audio_path = config.reference_audio_path.to_string();
    let cfg_value = config.cfg_value;

    if !reference_audio_path.is_empty()
        && !reference_audio_path.starts_with("built_in://")
        && !Path::new(&reference_audio_path).exists()
    {
        crate::toast_warn!(ui, tr("Reference audio file not found"));
        return;
    }

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let ref_audio_opt = if reference_audio_path.is_empty() {
            None
        } else {
            Some(reference_audio_path.as_str())
        };

        let model_guard = match get_or_load_model(&model_dir, use_gpu).await {
            Ok(g) => g,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {e}", tr("Failed to load TTS model")),
                );
                async_update_entry_generating(ui_weak, index, false);
                return;
            }
        };

        let (prev_samples, prev_text) = match prev_context {
            Some((samples, text)) => (Some(samples), Some(text)),
            None => (None, None),
        };

        // 首段无续写上下文但有参考音频时，先用 Reference 模式生成短引导音频，
        // 再用 Combined 模式生成实际文本，避免注意力稀释导致音色丢失
        let (prev_samples, prev_text) = if prev_samples.is_none() && ref_audio_opt.is_some() {
            match generate_primer_audio(&model_guard, ref_audio_opt, cfg_value) {
                Ok(primer_samples) => (Some(primer_samples), Some(String::new())),
                Err(e) => {
                    log::warn!(
                        "Failed to generate primer audio: {e}, falling back to Reference mode"
                    );
                    (None, None)
                }
            }
        } else {
            (prev_samples, prev_text)
        };

        let opts = build_generate_options_with_context(
            ref_audio_opt,
            prev_samples,
            prev_text,
            None,
            &text,
            cfg_value,
            config.max_token_count,
        );

        let samples_result: voxcpm_rs::Result<Vec<f32>> = {
            let guard = model_guard.lock().unwrap();
            match guard.as_ref() {
                Some(VoxCpmModel::Wgpu { model, .. }) => model.generate(&text, opts),
                Some(VoxCpmModel::NdArray(m)) => m.generate(&text, opts),
                None => Err(voxcpm_rs::Error::Other("Model not loaded".into())),
            }
        };

        let samples = match samples_result {
            Ok(s) => s,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {e}", tr("Speech synthesis failed")),
                );
                async_update_entry_generating(ui_weak, index, false);
                return;
            }
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if let Some(mut e) = store_video_editor_tts_entries!(ui).row_data(index as usize) {
                e.samples = ModelRc::new(VecModel::from_slice(&samples));
                e.duration = (samples.len() / VOXCPM_SAMPLE_RATE as usize) as i32;
                e.is_generating = false;
                store_video_editor_tts_entries!(ui).set_row_data(index as usize, e);
            }

            global_store!(ui).set_video_editor_tts_update_total_time_flag(
                !global_store!(ui).get_video_editor_tts_update_total_time_flag(),
            );
            crate::toast_success!(ui, format!("{} {}", tr("TTS generated for entry"), index));
        });
    });
}

fn video_editor_tts_play_speech(ui: &AppWindow, index: i32) {
    if let Some(entry) = store_video_editor_tts_entries!(ui).row_data(index as usize) {
        let samples = entry.samples.iter().collect::<Vec<f32>>();
        if samples.is_empty() {
            crate::toast_warn!(ui, tr("No found audio samples, please generating speech"));
            return;
        }

        let sound = global_store!(ui).get_video_editor_tts_audio_sound();
        play_audio_samples_without_progress(samples, sound);
    }
}

fn video_editor_tts_export_to_playlist(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_tts_config();
    let wav_path = PathBuf::from(config.save_dir.to_string());

    if !wav_path.exists() {
        crate::toast_warn!(
            ui,
            format!("{}: {}", tr("No found save directory"), wav_path.display())
        );
        return;
    }

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let mode_suffix = if !config.reference_audio_path.is_empty() {
        "cloned"
    } else {
        "default"
    };
    let filename = format!("voxcpm_{}_{}.wav", mode_suffix, timestamp);
    let wav_path = wav_path.join(filename);

    let samples = store_video_editor_tts_entries!(ui)
        .iter()
        .map(|item| item.samples.iter().collect::<Vec<f32>>())
        .collect::<Vec<Vec<f32>>>();

    let samples = samples
        .into_iter()
        .flat_map(|item| item.into_iter())
        .collect::<Vec<f32>>();

    if let Err(e) = write_wav(&wav_path, &samples) {
        crate::toast_warn!(
            ui,
            format!(
                "{} `{}`. {e}",
                tr("Save directory failed"),
                wav_path.display()
            )
        );
        return;
    }

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        import_file_to_playlist(ui_weak, wav_path, None).await;
    });
}

fn video_editor_tts_update_config(ui: &AppWindow, config: UITTSConfig) {
    global_store!(ui).set_video_editor_tts_config(config.clone());
    save_tts_config(config.into());
}

fn video_editor_tts_add_entry(ui: &AppWindow, text: SharedString) {
    for line in text.lines() {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let entry = UITTSEntry {
            text: line.clone().into(),
            samples: ModelRc::default(),
            is_generating: false,
            duration: 0,
        };
        store_video_editor_tts_entries!(ui).push(entry);
    }
}

fn video_editor_tts_auto_segment(ui: &AppWindow, text: SharedString) {
    let text = text.to_string();
    if text.trim().is_empty() {
        crate::toast_warn!(ui, tr("Text is empty"));
        return;
    }

    let is_english = text.chars().all(|c| c.is_ascii());
    let mut segments: Vec<String> = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut start = 0;

    for (i, &ch) in chars.iter().enumerate() {
        if is_english {
            // English sentence-end punctuation: . ? ; ! — only split when followed by whitespace or end
            if ch == '.' || ch == '?' || ch == ';' || ch == '!' {
                let next_char = chars.get(i + 1);
                if next_char.is_none() || next_char.map_or(false, |c| c.is_whitespace()) {
                    let seg: String = chars[start..=i].iter().collect();
                    let seg = seg.trim().to_string();
                    if !seg.is_empty() {
                        segments.push(seg);
                    }
                    start = i + 1;
                }
            }
        } else {
            // Chinese sentence-end punctuation: 。？；！?;!
            if ch == '。'
                || ch == '？'
                || ch == '；'
                || ch == '！'
                || ch == '?'
                || ch == ';'
                || ch == '!'
            {
                let seg: String = chars[start..=i].iter().collect();
                let seg = seg.trim().to_string();
                if !seg.is_empty() {
                    segments.push(seg);
                }
                start = i + 1;
            }
        }
    }

    // Handle remaining text that doesn't end with a separator
    if start < chars.len() {
        let remaining: String = chars[start..].iter().collect();
        let remaining = remaining.trim().to_string();
        if !remaining.is_empty() {
            segments.push(remaining);
        }
    }

    let result = segments.join("\n");
    global_store!(ui).set_video_editor_tts_input_text(result.into());
}

fn video_editor_tts_ai_segment(ui: &AppWindow, text: SharedString) {
    let setting = config::all().ai_model.clone();
    if setting.api_base_url.is_empty()
        || setting.model_name.is_empty()
        || setting.api_key.is_empty()
    {
        crate::toast_info!(ui, tr("Please setup AI model and try again."));
        return;
    }

    let text = text.to_string();
    if text.trim().is_empty() {
        crate::toast_warn!(ui, tr("Text is empty"));
        return;
    }

    crate::toast_info!(
        ui,
        tr("Please wait patiently for AI processing to complete")
    );

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        match ai_segment_text(ui_weak.clone(), text).await {
            Ok(segments) => {
                if segments.is_empty() {
                    toast::async_toast_warn(ui_weak, tr("AI segment returned empty result"));
                    return;
                }

                let result = segments.join("\n");
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    global_store!(ui).set_video_editor_tts_input_text(result.into());
                    crate::toast_success!(ui, tr("AI segment completed"));
                });
            }
            Err(e) => {
                toast::async_toast_warn(ui_weak, format!("{}: {e}", tr("AI segment failed")));
            }
        }
    });
}

async fn ai_segment_text(ui_weak: Weak<AppWindow>, text: String) -> Result<Vec<String>> {
    let prompt = r#"You are a text segmentation assistant. Your task is to split the given text into individual sentences, one per line. Follow these rules:

1. Each line should be a complete sentence.
2. For Chinese text, split at 。？；！ punctuation marks.
3. For English text, split at . ? ; ! followed by a space or end of text.
4. Remove any leading/trailing whitespace from each line.
5. Only output the segmented text, one sentence per line. No additional text, no numbering, no explanation."#;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamTextItem>(100);
    let model_config = config::all().ai_model.clone();
    let request_config = APIConfig {
        api_base_url: model_config.api_base_url,
        api_model: model_config.model_name,
        api_key: model_config.api_key,
        temperature: None,
    };

    tokio::spawn(async move {
        let chat_config = ChatConfig { tx };
        let chat = Chat::new(prompt, text, chat_config, request_config, vec![]);
        if let Err(e) = chat.start().await {
            toast::async_toast_warn(ui_weak, format!("{}: {e}", tr("Start AI segment failed")));
        }
    });

    let mut resp = String::new();
    while let Some(item) = rx.recv().await {
        if let Some(ref text) = item.text {
            resp.push_str(text);
        }
    }

    let segments: Vec<String> = resp
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    Ok(segments)
}

fn video_editor_tts_insert_entry(ui: &AppWindow, index: i32) {
    let entry = UITTSEntry {
        text: "".into(),
        samples: ModelRc::default(),
        is_generating: false,
        duration: 0,
    };
    store_video_editor_tts_entries!(ui).insert(index as usize + 1, entry);
}

fn video_editor_tts_add_preamble_entry(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_tts_config();
    let preamble = config.preamble.to_string();
    if preamble.is_empty() {
        crate::toast_warn!(ui, tr("preamble text is empty"));
        return;
    }

    let entry = UITTSEntry {
        text: preamble.into(),
        samples: ModelRc::default(),
        is_generating: false,
        duration: 0,
    };
    store_video_editor_tts_entries!(ui).insert(0, entry);
}

fn video_editor_tts_remove_entry(ui: &AppWindow, index: i32) {
    store_video_editor_tts_entries!(ui).remove(index as usize);
}

fn video_editor_tts_clear_entry_audio(ui: &AppWindow, index: i32) {
    let index = index as usize;
    if let Some(mut entry) = store_video_editor_tts_entries!(ui).row_data(index) {
        entry.samples = ModelRc::default();
        entry.is_generating = false;
        entry.duration = 0;
        store_video_editor_tts_entries!(ui).set_row_data(index, entry);
    }

    global_store!(ui).set_video_editor_tts_update_total_time_flag(
        !global_store!(ui).get_video_editor_tts_update_total_time_flag(),
    );
}

fn update_entry_generating(ui: &AppWindow, index: i32, generating: bool) {
    let index = index as usize;
    if let Some(mut e) = store_video_editor_tts_entries!(ui).row_data(index) {
        e.is_generating = generating;
        store_video_editor_tts_entries!(ui).set_row_data(index, e);
    }
}

fn async_update_entry_generating(ui_weak: Weak<AppWindow>, index: i32, generating: bool) {
    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        update_entry_generating(&ui, index, generating);
    });
}

fn video_editor_tts_generate_all_speech(ui: &AppWindow) {
    let model_dir = match get_and_check_model_setting(ui) {
        Ok(item) => item,
        Err(e) => {
            global_store!(ui).set_video_editor_is_show_tts_dialog(true);
            crate::toast_warn!(ui, format!("{e}"));
            return;
        }
    };

    let mut items = vec![];
    for (index, entry) in store_video_editor_tts_entries!(ui).iter().enumerate() {
        if entry.samples.row_count() != 0 {
            continue;
        }

        if entry.text.trim().is_empty() {
            crate::toast_warn!(ui, format!("{}[{}]", tr("Entry has no text"), index));
            return;
        }

        items.push((index, entry.text.to_string()));
    }

    if items.is_empty() {
        crate::toast_warn!(ui, tr("All entries have been generated"));
        return;
    }

    let config = global_store!(ui).get_video_editor_tts_config();
    let max_char_count = config.max_char_count;

    // 检查是否有条目超过字数限制
    let over_limit_entries: Vec<(usize, i32)> = items
        .iter()
        .map(|&(index, ref text)| (index, text.chars().count() as i32))
        .filter(|&(_, char_count)| char_count > max_char_count)
        .collect();

    if !over_limit_entries.is_empty() {
        let details: Vec<String> = over_limit_entries
            .iter()
            .map(|&(index, char_count)| {
                format!("[{}] {} / {}", index + 1, char_count, max_char_count)
            })
            .collect();
        crate::toast_warn!(
            ui,
            format!(
                "{}: {}",
                tr("Some entries exceed the character limit"),
                details.join(", ")
            )
        );
        return;
    }

    let use_gpu = config.use_gpu;
    let reference_audio_path = config.reference_audio_path.to_string();
    let cfg_value = config.cfg_value;
    let max_token_count = config.max_token_count;
    let context_reset_interval = config.context_reset_interval.max(1) as usize;

    if !reference_audio_path.is_empty()
        && !reference_audio_path.starts_with("built_in://")
        && !Path::new(&reference_audio_path).exists()
    {
        crate::toast_warn!(ui, tr("Reference audio file not found"));
        return;
    }

    // 查找上一段已生成语音的上下文（用于续写，保持音色一致性）
    let first_index = items.first().map(|(i, _)| *i).unwrap_or(0);
    let prev_context_from_ui = if first_index > 0 {
        store_video_editor_tts_entries!(ui)
            .row_data(first_index - 1)
            .and_then(|prev_entry| {
                let prev_samples: Vec<f32> = prev_entry.samples.iter().collect();
                if prev_samples.is_empty() || prev_entry.text.is_empty() {
                    None
                } else {
                    Some((prev_samples, prev_entry.text.to_string()))
                }
            })
    } else {
        None
    };

    global_store!(ui).set_video_editor_tts_generating_all_progress(0.0);

    let cancel = CancelToken::new();
    *GENERATE_ALL_CANCEL.lock().unwrap() = Some(cancel.clone());
    global_store!(ui).set_video_editor_tts_is_generating_all(true);

    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let ref_audio_opt = if reference_audio_path.is_empty() {
            None
        } else {
            Some(reference_audio_path.as_str())
        };

        let model_guard = match get_or_load_model(&model_dir, use_gpu).await {
            Ok(g) => g,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {e}", tr("Failed to load TTS model")),
                );
                async_clear_generate_all_state(ui_weak);
                return;
            }
        };

        let total_counts = items.len();
        let mut current_count = 0;

        // 逐句链式续写 + 周期性重置：
        // - Combined 模式中 reference_audio 锚定音色，减少漂移
        // - 每生成一句作为下一句的续写上下文，保持自然衔接
        // - 每隔 context_reset_interval 句重置到首句锚点，防止错误累积

        let (mut prev_samples, mut prev_text) = match prev_context_from_ui {
            Some((s, t)) => (Some(s), Some(t)),
            None => (None, None),
        };

        // 首段无续写上下文但有参考音频时，先生成短引导音频
        if prev_samples.is_none() && ref_audio_opt.is_some() {
            match generate_primer_audio(&model_guard, ref_audio_opt, cfg_value) {
                Ok(primer_samples) => {
                    prev_samples = Some(primer_samples);
                    prev_text = Some(String::new());
                }
                Err(e) => {
                    log::warn!(
                        "Failed to generate primer audio: {e}, falling back to Reference mode"
                    );
                }
            }
        }

        let mut anchor_samples: Option<Vec<f32>> = None;
        let mut anchor_text: Option<String> = None;
        let mut chain_position: usize = 0;

        for item in items.into_iter() {
            if cancel.is_cancelled() {
                break;
            }

            let (index, text) = item;

            // 周期性重置：回到首句锚点，防止错误累积
            if chain_position >= context_reset_interval {
                if let (Some(a_s), Some(a_t)) = (&anchor_samples, &anchor_text) {
                    prev_samples = Some(a_s.clone());
                    prev_text = Some(a_t.clone());
                }
                chain_position = 0;
                log::debug!("TTS context reset at entry {} to prevent drift", index);
            }

            let (ctx_samples, ctx_text) = (prev_samples.clone(), prev_text.clone());

            let opts = build_generate_options_with_context(
                ref_audio_opt,
                ctx_samples,
                ctx_text,
                Some(cancel.clone()),
                &text,
                cfg_value,
                max_token_count,
            );

            let samples_result: voxcpm_rs::Result<Vec<f32>> = {
                let guard = model_guard.lock().unwrap();
                match guard.as_ref() {
                    Some(VoxCpmModel::Wgpu { model, .. }) => model.generate(&text, opts),
                    Some(VoxCpmModel::NdArray(m)) => m.generate(&text, opts),
                    None => Err(voxcpm_rs::Error::Other("Model not loaded".into())),
                }
            };

            match samples_result {
                Ok(samples) => {
                    // 首句作为锚点保存
                    if anchor_samples.is_none() {
                        anchor_samples = Some(samples.clone());
                        anchor_text = Some(text.clone());
                    }

                    // 链式续写：将当前生成的语音作为下一句的上下文
                    prev_samples = Some(samples.clone());
                    prev_text = Some(text.clone());
                    chain_position += 1;

                    current_count += 1;

                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if let Some(mut entry) = store_video_editor_tts_entries!(ui).row_data(index)
                        {
                            entry.is_generating = false;
                            entry.duration = (samples.len() / VOXCPM_SAMPLE_RATE as usize) as i32;
                            entry.samples = ModelRc::new(VecModel::from_slice(&samples));
                            store_video_editor_tts_entries!(ui).set_row_data(index, entry);
                            global_store!(ui).set_video_editor_tts_generating_all_progress(
                                current_count as f32 / total_counts as f32,
                            );
                        }
                    });
                }
                Err(e) => {
                    // 生成失败时清除上下文并重置链位置，避免用错误数据续写
                    prev_samples = anchor_samples.clone();
                    prev_text = anchor_text.clone();
                    chain_position = 0;
                    log::warn!("Speech synthesis failed for entry {}: {e}", index);
                }
            }
        }

        async_clear_generate_all_state(ui_weak);
    });
}

fn async_clear_generate_all_state(ui_weak: Weak<AppWindow>) {
    GENERATE_ALL_CANCEL.lock().unwrap().take();
    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        global_store!(ui).set_video_editor_tts_is_generating_all(false);
        global_store!(ui).set_video_editor_tts_update_total_time_flag(
            !global_store!(ui).get_video_editor_tts_update_total_time_flag(),
        );
    });
}

fn video_editor_tts_cancel_generating_all(ui: &AppWindow) {
    if let Some(ref cancel) = *GENERATE_ALL_CANCEL.lock().unwrap() {
        cancel.cancel();
    }

    global_store!(ui).set_video_editor_tts_is_generating_all(false);
    global_store!(ui).set_video_editor_tts_update_total_time_flag(
        !global_store!(ui).get_video_editor_tts_update_total_time_flag(),
    );
}

fn video_editor_tts_remove_all_speech(ui: &AppWindow) {
    for (index, mut entry) in store_video_editor_tts_entries!(ui).iter().enumerate() {
        entry.samples = ModelRc::default();
        entry.is_generating = false;
        entry.duration = 0;
        store_video_editor_tts_entries!(ui).set_row_data(index, entry);
    }

    global_store!(ui).set_video_editor_tts_update_total_time_flag(
        !global_store!(ui).get_video_editor_tts_update_total_time_flag(),
    );
}

fn video_editor_tts_remove_all_entries(ui: &AppWindow) {
    store_video_editor_tts_entries!(ui).set_vec(vec![]);
}

fn video_editor_tts_has_speech(ui: &AppWindow) -> bool {
    store_video_editor_tts_entries!(ui)
        .iter()
        .any(|entry| entry.samples.row_count() > 0)
}

fn video_editor_tts_has_empty_speech(ui: &AppWindow) -> bool {
    if store_video_editor_tts_entries!(ui).row_count() == 0 {
        return true;
    }

    store_video_editor_tts_entries!(ui)
        .iter()
        .any(|entry| entry.samples.row_count() == 0)
}

fn video_editor_tts_update_entry(ui: &AppWindow, index: i32, text: SharedString) {
    let mut index = index as usize;
    let mut is_first = true;

    for line in text.lines() {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }

        if is_first {
            if let Some(mut entry) = store_video_editor_tts_entries!(ui).row_data(index) {
                entry.text = text.into();
                store_video_editor_tts_entries!(ui).set_row_data(index, entry);
            }
            is_first = false;
        } else {
            let entry = UITTSEntry {
                text: text.into(),
                samples: ModelRc::default(),
                is_generating: false,
                duration: 0,
            };
            store_video_editor_tts_entries!(ui).insert(index, entry);
        }

        index += 1;
    }
}

fn video_editor_tts_get_total_duration(
    _ui: &AppWindow,
    entry: ModelRc<UITTSEntry>,
    _flag: bool,
) -> i32 {
    let total_samples: usize = entry.iter().map(|e| e.samples.row_count()).sum();
    (total_samples * 1000 / VOXCPM_SAMPLE_RATE as usize) as i32
}

fn video_editor_tts_stop_playing_all_speech(ui: &AppWindow) {
    global_store!(ui).set_video_editor_tts_audio_player_is_playing(false);
    if let Some(ref cancel) = *PROGRESS_TASK_CANCEL.lock().unwrap() {
        cancel.store(true, Ordering::Relaxed);
    }
    if let Some(ref current) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
        current.stop();
        current.clear();
    }
}

fn video_editor_tts_play_all_speech(ui: &AppWindow, mut start_ms: i32, end_ms: i32) {
    let samples: Vec<f32> = store_video_editor_tts_entries!(ui)
        .iter()
        .flat_map(|entry| entry.samples.iter().collect::<Vec<f32>>())
        .collect();

    if samples.is_empty() {
        return;
    }

    let start_timestamps: Vec<u128> = store_video_editor_tts_entries!(ui)
        .iter()
        .map(|entry| entry.samples.row_count() as u128 * 1000 / VOXCPM_SAMPLE_RATE as u128)
        .collect::<Vec<u128>>()
        .into_iter()
        .scan(0, |acc, x| {
            *acc += x;
            Some(*acc)
        })
        .collect();

    if start_ms == end_ms {
        start_ms = 0;
        global_store!(ui).set_video_editor_tts_audio_player_progress(0.0);
    }

    let start_sample = (start_ms as usize * VOXCPM_SAMPLE_RATE as usize / 1000).min(samples.len());
    let end_sample = (end_ms as usize * VOXCPM_SAMPLE_RATE as usize / 1000).min(samples.len());
    let samples = samples[start_sample..end_sample].to_vec();

    if samples.is_empty() {
        return;
    }

    global_store!(ui).set_video_editor_tts_audio_player_is_playing(true);
    play_audio_samples(ui, samples, start_timestamps);
}

fn video_editor_tts_audio_player_sound_changed(ui: &AppWindow, sound: f32) {
    global_store!(ui).set_video_editor_tts_audio_sound(sound);
    if let Some(ref player) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
        player.set_volume(sound);
    }
}

fn video_editor_tts_audio_player_sound_released(ui: &AppWindow, sound: f32) {
    global_store!(ui).set_video_editor_tts_audio_sound(sound);
    if let Some(ref player) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
        player.set_volume(sound);
    }
}

fn video_editor_tts_audio_player_progress_changed(ui: &AppWindow, value: f32) {
    global_store!(ui).set_video_editor_tts_audio_player_progress(value);
}

fn video_editor_tts_audio_player_progress_released(ui: &AppWindow, value: f32) {
    global_store!(ui).set_video_editor_tts_audio_player_progress(value);
    if value >= 1.0 {
        return;
    }

    let total_samples: usize = store_video_editor_tts_entries!(ui)
        .iter()
        .map(|e| e.samples.row_count())
        .sum();
    let total_duration_ms = (total_samples * 1000 / VOXCPM_SAMPLE_RATE as usize) as i32;
    if total_duration_ms > 0 {
        let start_ms = (value * total_duration_ms as f32) as i32;
        video_editor_tts_play_all_speech(ui, start_ms, total_duration_ms);
    }
}

fn video_editor_tts_audio_player_progress_pressed(ui: &AppWindow, value: f32) {
    PLAYER_INC_INDEX.fetch_add(1, Ordering::Relaxed);
    global_store!(ui).set_video_editor_tts_audio_player_progress(value);
    video_editor_tts_stop_playing_all_speech(ui);
}

fn play_audio_samples_without_progress(samples: Vec<f32>, sound: f32) {
    tokio::task::spawn_blocking(move || {
        if let Some(device) = AUDIO_DEVICE.get() {
            if let Some(ref current) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
                current.stop();
                current.clear();
            }

            let source = SamplesBuffer::new(
                ChannelCount::new(VOXCPM_CHANNEL as u16).unwrap(),
                SampleRate::new(VOXCPM_SAMPLE_RATE as u32).unwrap(),
                samples,
            );

            let mixer = device.mixer();
            let player = Arc::new(Player::connect_new(&mixer));
            player.set_volume(sound.clamp(0.0, 1.0));
            player.append(source);

            *CURRENT_PLAYER.get().unwrap().lock().unwrap() = Some(Arc::clone(&player));
            player.sleep_until_end();
            CURRENT_PLAYER.get().unwrap().lock().unwrap().take();
        }
    });
}

fn play_embedded_audio(audio_bytes: &[u8], sound: f32) {
    let audio_bytes = audio_bytes.to_vec();
    tokio::task::spawn_blocking(move || {
        let cursor = std::io::Cursor::new(audio_bytes);
        let reader = match hound::WavReader::new(cursor) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("Failed to parse embedded WAV: {e}");
                return;
            }
        };
        let spec = reader.spec();

        let samples: Vec<f32> = reader
            .into_samples()
            .filter_map(|s: hound::Result<i16>| s.ok())
            .map(|s| s as f32 / 32768.0)
            .collect();

        let samples = if spec.sample_rate != VOXCPM_SAMPLE_RATE {
            match voxcpm_rs::audio::resample(&samples, spec.sample_rate, VOXCPM_SAMPLE_RATE) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("Failed to resample embedded audio: {e}");
                    samples
                }
            }
        } else {
            samples
        };

        play_audio_samples_without_progress(samples, sound);
    });
}

fn play_audio_samples(ui: &AppWindow, samples: Vec<f32>, start_timestamps: Vec<u128>) {
    let total_duration = video_editor_tts_get_total_duration(
        &ui,
        global_store!(ui).get_video_editor_tts_entries(),
        true,
    ) as f32;

    let current_duration =
        global_store!(ui).get_video_editor_tts_audio_player_progress() * total_duration;

    let cancel = Arc::new(AtomicBool::new(false));
    *PROGRESS_TASK_CANCEL.lock().unwrap() = Some(Arc::clone(&cancel));

    let ui_weak = ui.as_weak();
    let cancel_for_progress = Arc::clone(&cancel);
    let player_index = PLAYER_INC_INDEX.fetch_add(1, Ordering::Relaxed) + 1;

    tokio::spawn(async move {
        let start = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let current_timstamp = current_duration as u128 + start.elapsed().as_millis();
            let progress = (current_timstamp as f32 / total_duration).min(1.0);
            let audio_player_index =
                start_timestamps.partition_point(|&s| s < current_timstamp) as i32;

            if cancel_for_progress.load(Ordering::Relaxed) {
                if progress >= 1.0 {
                    _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        if PLAYER_INC_INDEX.load(Ordering::Relaxed) == player_index {
                            global_store!(ui).set_video_editor_tts_audio_player_progress(1.0);
                            global_store!(ui).set_video_editor_tts_audio_playing_index(
                                audio_player_index.max(0),
                            );
                        }
                    });
                }
                break;
            }

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                if PLAYER_INC_INDEX.load(Ordering::Relaxed) == player_index {
                    if progress >= 1.0 {
                        global_store!(ui).set_video_editor_tts_audio_player_progress(1.0);
                    } else {
                        global_store!(ui).set_video_editor_tts_audio_player_progress(progress);
                    }
                    global_store!(ui)
                        .set_video_editor_tts_audio_playing_index(audio_player_index.max(0));
                }
            });

            if progress >= 1.0 {
                break;
            }
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            if PLAYER_INC_INDEX.load(Ordering::Relaxed) == player_index {
                global_store!(ui).set_video_editor_tts_audio_playing_index(-1);
            }
        });
    });

    let sound = global_store!(ui).get_video_editor_tts_audio_sound();
    let ui_weak = ui.as_weak();
    tokio::task::spawn_blocking(move || {
        if let Some(device) = AUDIO_DEVICE.get() {
            if let Some(ref current) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
                current.stop();
                current.clear();
            }

            let source = SamplesBuffer::new(
                ChannelCount::new(VOXCPM_CHANNEL as u16).unwrap(),
                SampleRate::new(VOXCPM_SAMPLE_RATE as u32).unwrap(),
                samples,
            );

            let mixer = device.mixer();
            let player = Arc::new(Player::connect_new(&mixer));
            player.set_volume(sound.clamp(0.0, 1.0));
            player.append(source);

            *CURRENT_PLAYER.get().unwrap().lock().unwrap() = Some(Arc::clone(&player));
            player.sleep_until_end();
            CURRENT_PLAYER.get().unwrap().lock().unwrap().take();

            cancel.store(true, Ordering::Relaxed);
            *PROGRESS_TASK_CANCEL.lock().unwrap() = None;

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                if PLAYER_INC_INDEX.load(Ordering::Relaxed) == player_index {
                    global_store!(ui).set_video_editor_tts_audio_player_is_playing(false);
                }
            });
        }
    });
}

/// 用 Reference 模式生成一段极短的引导音频，作为后续 Combined 模式的 prompt_audio。
/// 这样参考音频在 KV cache 中有更强的锚定效果，避免长文本时注意力稀释导致音色丢失。
fn generate_primer_audio(
    model_guard: &Arc<Mutex<Option<VoxCpmModel>>>,
    ref_audio_opt: Option<&str>,
    cfg_value: f32,
) -> voxcpm_rs::Result<Vec<f32>> {
    let ref_path = ref_audio_opt.ok_or(voxcpm_rs::Error::Other("No reference audio".into()))?;
    let ref_audio = resolve_reference_audio(ref_path).ok_or(voxcpm_rs::Error::Other(
        "Failed to resolve reference audio".into(),
    ))?;

    let opts = GenerateOptions::builder()
        .timesteps(10)
        .cfg(cfg_value)
        .max_len(100)
        .prompt(Prompt::Reference { audio: ref_audio })
        .build();

    let primer_text = "";
    let guard = model_guard.lock().unwrap();
    match guard.as_ref() {
        Some(VoxCpmModel::Wgpu { model, .. }) => model.generate(primer_text, opts),
        Some(VoxCpmModel::NdArray(m)) => m.generate(primer_text, opts),
        None => Err(voxcpm_rs::Error::Other("Model not loaded".into())),
    }
}

fn get_built_in_voice_cache_path(idx: usize) -> Option<PathBuf> {
    if idx >= BUILT_IN_VOICES.len() {
        return None;
    }
    let cache_dir = crate::config::all().cache_dir.join("tts_builtin_voices");

    if !cache_dir.exists() && std::fs::create_dir_all(&cache_dir).is_err() {
        return None;
    }

    let (_, audio_bytes) = BUILT_IN_VOICES[idx];
    let cache_path = cache_dir.join(format!("voice_{}.wav", idx));

    if !cache_path.exists() {
        if std::fs::write(&cache_path, audio_bytes).is_err() {
            return None;
        }
    } else {
        let metadata = cache_path.metadata();
        if metadata.map(|m| m.len() == 0).unwrap_or(false)
            && std::fs::write(&cache_path, audio_bytes).is_err()
        {
            return None;
        }
    }

    Some(cache_path)
}

fn build_generate_options_with_context(
    reference_audio_path: Option<&str>,
    prev_samples: Option<Vec<f32>>,
    prev_text: Option<String>,
    cancel: Option<CancelToken>,
    text: &str,
    cfg_value: f32,
    max_token_count: i32,
) -> GenerateOptions {
    let prompt = match (reference_audio_path, prev_samples, prev_text) {
        // 有参考音色 + 有上一段 → Combined 模式（保持音色一致性和上下文衔接）
        (Some(ref_path), Some(samples), Some(text)) => match resolve_reference_audio(ref_path) {
            Some(ref_audio) => Prompt::Combined {
                reference_audio: ref_audio,
                prompt_audio: PromptAudio::Pcm {
                    samples,
                    sample_rate: VOXCPM_SAMPLE_RATE,
                },
                prompt_text: text,
            },
            None => Prompt::Continuation {
                audio: PromptAudio::Pcm {
                    samples,
                    sample_rate: VOXCPM_SAMPLE_RATE,
                },
                text,
            },
        },
        // 有参考音色 + 无上一段 → Reference 模式（首段）
        (Some(ref_path), None, None) => match resolve_reference_audio(ref_path) {
            Some(audio) => Prompt::Reference { audio },
            None => Prompt::None,
        },
        // 无参考音色 + 有上一段 → Continuation 模式
        (None, Some(samples), Some(text)) => Prompt::Continuation {
            audio: PromptAudio::Pcm {
                samples,
                sample_rate: VOXCPM_SAMPLE_RATE,
            },
            text,
        },
        // 无参考音色 + 无上一段 → None 模式（首段）
        (None, None, None) => Prompt::None,
        // 不完整组合：退化为 Reference 或 None
        (Some(ref_path), _, _) => match resolve_reference_audio(ref_path) {
            Some(audio) => Prompt::Reference { audio },
            None => Prompt::None,
        },
        (None, _, _) => Prompt::None,
    };

    // max_token_count 控制生成语音的最大 token 数（latent patch 数量）
    // 每个字符约需 0.3 秒语音，每个 latent patch 约 80ms
    // estimated = 字符数 × 0.3s / 0.08s ≈ 字符数 × 4，再加 50% 余量
    // 上限 max_token_count，下限 50
    let char_count = text.chars().count().max(1);
    let estimated_max_len =
        ((char_count as f32 * 4.0 * 1.5) as usize).clamp(50, max_token_count as usize);

    let mut builder = GenerateOptions::builder()
        .timesteps(10)
        .cfg(cfg_value)
        .max_len(estimated_max_len)
        .prompt(prompt);

    if let Some(c) = cancel {
        builder = builder.cancel(c);
    }

    builder.build()
}

fn resolve_reference_audio(ref_path: &str) -> Option<PromptAudio> {
    match ref_path.strip_prefix("built_in://") {
        Some(idx_str) => {
            let idx: usize = idx_str.parse().unwrap_or(0);
            if idx < BUILT_IN_VOICES.len() {
                get_built_in_voice_cache_path(idx).map(|cache_path| cache_path.into())
            } else {
                None
            }
        }
        None => {
            let path = PathBuf::from(ref_path);
            if path.exists() {
                Some(path.into())
            } else {
                log::warn!("Reference audio file not found: {}", ref_path);
                None
            }
        }
    }
}

async fn get_or_load_model(
    model_dir: &Path,
    use_gpu: bool,
) -> Result<Arc<Mutex<Option<VoxCpmModel>>>> {
    let model_store = VOXCPM_MODEL.get().unwrap();

    {
        let guard = model_store.lock().unwrap();
        if let Some(model) = guard.as_ref() {
            match model {
                VoxCpmModel::Wgpu { .. } if use_gpu => return Ok(Arc::clone(model_store)),
                VoxCpmModel::NdArray(_) if !use_gpu => return Ok(Arc::clone(model_store)),
                _ => {} // Backend mismatch, need to reload
            }
        }
    }

    let model = if use_gpu {
        let device = WgpuDevice::default();
        match VoxCPM::<Wgpu<f32, i32>>::from_local(model_dir, &device) {
            Ok(m) => VoxCpmModel::Wgpu { model: m, device },
            Err(e) => {
                log::warn!("GPU backend failed, falling back to CPU: {e}");
                VoxCpmModel::NdArray(VoxCPM::<NdArray<f32>>::from_local(
                    model_dir,
                    &NdArrayDevice::default(),
                )?)
            }
        }
    } else {
        VoxCpmModel::NdArray(VoxCPM::<NdArray<f32>>::from_local(
            model_dir,
            &NdArrayDevice::default(),
        )?)
    };

    {
        let mut guard = model_store.lock().unwrap();
        *guard = Some(model);
    }

    Ok(Arc::clone(model_store))
}

fn write_wav(path: &Path, samples: &[f32]) -> Result<()> {
    let spec = WavSpec {
        channels: VOXCPM_CHANNEL,
        sample_rate: VOXCPM_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = WavWriter::create(path, spec)?;
    for sample in samples {
        writer.write_sample(*sample)?;
    }

    writer.finalize()?;
    Ok(())
}

fn save_tts_config(config: TTSConfigData) {
    tokio::spawn(async move {
        let data = serde_json::to_string(&config).expect("serialize tts config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, TTS_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, TTS_CONFIG_ID, &data).await {
                log::warn!("Failed to save TTS config: {:?}", e);
            }
        }
    });
}

async fn load_tts_config() -> Option<TTSConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, TTS_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => None,
    }
}

fn get_and_check_model_setting(ui: &AppWindow) -> Result<PathBuf> {
    let config = global_store!(ui).get_video_editor_tts_config();
    let save_dir = PathBuf::from(&config.save_dir);
    let model_dir = PathBuf::from(&config.model_dir);

    if !save_dir.exists() {
        anyhow::bail!("No found save directory {}", save_dir.display());
    }

    if !model_dir.exists() {
        anyhow::bail!("No found model directory {}", model_dir.display());
    }

    let config_json = model_dir.join("config.json");
    let tokenizer_json = model_dir.join("tokenizer.json");
    let model_safetensors = model_dir.join("model.safetensors");
    let audiovae_safetensors = model_dir.join("audiovae.pth");

    if !config_json.exists() {
        anyhow::bail!("Missing config.json in model directory");
    }
    if !tokenizer_json.exists() {
        anyhow::bail!("Missing tokenizer.json in model directory");
    }
    if !model_safetensors.exists() {
        anyhow::bail!("Missing model.safetensors in model directory");
    }
    if !audiovae_safetensors.exists() {
        anyhow::bail!("Missing audiovae.pth in model directory");
    }

    Ok(model_dir)
}

fn video_editor_tts_open_choose_save_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(dir) = picker_directory(ui_weak.clone(), &tr("Choose save directory")) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut setting = global_store!(ui).get_video_editor_tts_config();
            setting.save_dir = dir.to_string_lossy().to_string().into();
            global_logic!(ui).invoke_video_editor_tts_update_config(setting);
        });
    });
}

fn video_editor_tts_choose_model_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(dir) = picker_directory(ui_weak.clone(), &tr("Choose model directory")) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut setting = global_store!(ui).get_video_editor_tts_config();
            setting.model_dir = dir.to_string_lossy().to_string().into();
            global_logic!(ui).invoke_video_editor_tts_update_config(setting);
        });
    });
}

fn video_editor_tts_model_start_download(ui: &AppWindow, url: SharedString) {
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

        let target_dir = base_dir.join("VoxCPM2");
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
            let downloaders = global_store!(ui).get_video_editor_tts_models_dowloader();
            if let Some(mut item) = downloaders.row_data(0) {
                item.state = UIDownloaderState::Downloading;
                downloaders.set_row_data(0, item);
            }
        });

        tokio::task::spawn_blocking(move || {
            let api = HFClient::builder()
                .build_sync()
                .expect("Failed to build hf_hub API");

            let (owner, name) = split_id(&model_id_clone);
            let repo = api.model(owner, name);

            let files_to_download = [
                "config.json",
                "tokenizer.json",
                "tokenizer_config.json",
                "special_tokens_map.json",
                "model.safetensors",
                "audiovae.pth",
            ];

            let mut downloaded_count = 0;

            for filename in files_to_download {
                match repo.download_file().filename(filename).send() {
                    Ok(cache_path) => {
                        let target_path = target_dir_clone.join(filename);
                        if let Err(e) = std::fs::copy(&cache_path, &target_path) {
                            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                                let downloaders =
                                    global_store!(ui).get_video_editor_tts_models_dowloader();
                                if let Some(mut item) = downloaders.row_data(0) {
                                    item.state = UIDownloaderState::Failed;
                                    downloaders.set_row_data(0, item);
                                }

                                crate::toast_warn!(
                                    ui,
                                    format!("{}: {}", tr("Failed to copy file to target"), e)
                                );
                            });
                            return;
                        }

                        downloaded_count += 1;
                        let progress = downloaded_count as f32 / files_to_download.len() as f32;

                        _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            let downloaders =
                                global_store!(ui).get_video_editor_tts_models_dowloader();
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
                                global_store!(ui).get_video_editor_tts_models_dowloader();
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
                let downloaders = global_store!(ui).get_video_editor_tts_models_dowloader();
                if let Some(mut item) = downloaders.row_data(0) {
                    item.state = UIDownloaderState::Finished;
                    item.progress = 1.0;
                    downloaders.set_row_data(0, item);
                }

                let mut setting = global_store!(ui).get_video_editor_tts_config();
                setting.model_dir = target_dir_clone.to_string_lossy().to_string().into();
                global_logic!(ui).invoke_video_editor_tts_update_config(setting);
                crate::toast_success!(ui, tr("VoxCPM model downloaded successfully"));
            });
        });
    });
}

fn video_editor_tts_setting_is_valid(ui: &AppWindow) -> bool {
    get_and_check_model_setting(ui).is_ok()
}

fn video_editor_tts_choose_reference_audio(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Choose reference audio"),
            "reference.wav",
            &["wav", "mp3", "flac"],
        ) else {
            return;
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut setting = global_store!(ui).get_video_editor_tts_config();
            setting.reference_audio_path = filepath.to_string_lossy().to_string().into();
            global_logic!(ui).invoke_video_editor_tts_update_config(setting);
        });
    });
}

fn video_editor_tts_play_reference_audio(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_tts_config();
    let reference_path = config.reference_audio_path.to_string();

    if reference_path.is_empty() {
        crate::toast_warn!(ui, tr("No reference audio selected"));
        return;
    }

    let sound = global_store!(ui).get_video_editor_tts_audio_sound();

    if let Some(idx_str) = reference_path.strip_prefix("built_in://") {
        let idx: usize = idx_str.parse().unwrap_or(0);
        if idx < BUILT_IN_VOICES.len() {
            let (_, audio_bytes) = BUILT_IN_VOICES[idx];
            play_embedded_audio(audio_bytes, sound);
            return;
        }
    }

    if !Path::new(&reference_path).exists() {
        crate::toast_warn!(ui, tr("Reference audio file not found"));
        return;
    }

    tokio::task::spawn_blocking(move || {
        let (samples, sr) = match voxcpm_rs::audio::load_audio(&reference_path) {
            Ok(result) => result,
            Err(e) => {
                log::warn!("Failed to load reference audio: {e}");
                return;
            }
        };

        let samples = match voxcpm_rs::audio::resample(&samples, sr, VOXCPM_SAMPLE_RATE) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to resample reference audio: {e}");
                return;
            }
        };

        play_audio_samples_without_progress(samples, sound);
    });
}

fn video_editor_tts_import_voice(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let Some(filepath) = picker_file(
            ui_weak.clone(),
            &tr("Import voice audio"),
            "voice.wav",
            &["wav", "mp3", "flac"],
        ) else {
            return;
        };

        let name = filepath
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown Voice")
            .to_string();

        let path = filepath.to_string_lossy().to_string();
        let name_for_ui = name.clone();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let already_exists = store_video_editor_tts_voices!(&ui)
                .iter()
                .any(|v| v.path == path);

            if already_exists {
                crate::toast_warn!(ui, tr("Voice already imported"));
                return;
            }

            let marked = MARKED_VOICE_NAMES
                .lock()
                .unwrap()
                .as_ref()
                .map(|set| set.contains(&path))
                .unwrap_or(false);

            let voice = UITTSVoice {
                name: name_for_ui.into(),
                path: path.into(),
                marked,
                built_in: false,
            };

            store_video_editor_tts_voices!(&ui).push(voice);

            save_imported_voices(&ui);
            crate::toast_success!(ui, format!("{} '{}' {}", tr("Voice"), name, tr("imported")));
        });
    });
}

fn resort_imported_voices(ui: &AppWindow) {
    let voices: Vec<UITTSVoice> = store_video_editor_tts_voices!(ui).iter().collect();
    let built_in: Vec<UITTSVoice> = voices.iter().filter(|v| v.built_in).cloned().collect();
    let mut imported: Vec<UITTSVoice> = voices.iter().filter(|v| !v.built_in).cloned().collect();

    imported.sort_by(|a, b| match (a.marked, b.marked) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    store_video_editor_tts_voices!(ui)
        .set_vec(built_in.into_iter().chain(imported).collect::<Vec<_>>());
}

fn video_editor_tts_toggle_voice_marked(ui: &AppWindow, index: i32) {
    let index = index as usize;

    if let Some(mut voice) = store_video_editor_tts_voices!(ui).row_data(index) {
        if voice.built_in {
            return;
        }

        voice.marked = !voice.marked;
        {
            let mut marked_set = MARKED_VOICE_NAMES.lock().unwrap();
            if let Some(set) = marked_set.as_mut() {
                if voice.marked {
                    set.insert(voice.path.to_string());
                } else {
                    set.remove(&voice.path.to_string());
                }
            }
        }

        store_video_editor_tts_voices!(ui).set_row_data(index, voice);
        save_marked_voice_names();
        resort_imported_voices(ui);
    }
}

fn video_editor_tts_play_voice(ui: &AppWindow, path: SharedString) {
    let sound = global_store!(ui).get_video_editor_tts_audio_sound();
    let path_str = path.to_string();

    if let Some(idx_str) = path_str.strip_prefix("built_in://") {
        let idx: usize = idx_str.parse().unwrap_or(0);
        if idx < BUILT_IN_VOICES.len() {
            let (_, audio_bytes) = BUILT_IN_VOICES[idx];
            play_embedded_audio(audio_bytes, sound);
            return;
        }
    }

    if !Path::new(&path_str).exists() {
        crate::toast_warn!(ui, tr("Voice file not found"));
        return;
    }

    tokio::task::spawn_blocking(move || {
        let (samples, sr) = match voxcpm_rs::audio::load_audio(&path_str) {
            Ok(result) => result,
            Err(e) => {
                log::warn!("Failed to load voice audio: {e}");
                return;
            }
        };

        let samples = match voxcpm_rs::audio::resample(&samples, sr, VOXCPM_SAMPLE_RATE) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to resample voice audio: {e}");
                return;
            }
        };

        play_audio_samples_without_progress(samples, sound);
    });
}

fn video_editor_tts_remove_voice(ui: &AppWindow, index: i32) {
    let index = index as usize;

    if let Some(voice) = store_video_editor_tts_voices!(ui).row_data(index) {
        if voice.built_in {
            return;
        }

        if voice.marked {
            let mut marked_set = MARKED_VOICE_NAMES.lock().unwrap();
            if let Some(set) = marked_set.as_mut() {
                set.remove(&voice.path.to_string());
            }

            drop(marked_set);
            save_marked_voice_names();
        }

        store_video_editor_tts_voices!(ui).remove(index);
        save_imported_voices(ui);

        let mut config = global_store!(ui).get_video_editor_tts_config();
        if config.reference_audio_path == voice.path {
            config.reference_audio_path = "".into();
            global_logic!(ui).invoke_video_editor_tts_update_config(config);
        }
    }
}

fn save_marked_voice_names() {
    let marked_set = MARKED_VOICE_NAMES.lock().unwrap().clone();
    if let Some(set) = marked_set {
        tokio::spawn(async move {
            let data = serde_json::to_string(&set).expect("serialize marked voices failed");
            if sqldb::entry::insert(VIDEO_EDITOR_TABLE, TTS_MARKED_VOICE_ID, &data)
                .await
                .is_err()
            {
                if let Err(e) =
                    sqldb::entry::update(VIDEO_EDITOR_TABLE, TTS_MARKED_VOICE_ID, &data).await
                {
                    log::warn!("Failed to save marked voices: {:?}", e);
                }
            }
        });
    }
}

fn save_imported_voices(ui: &AppWindow) {
    let voices: Vec<UITTSVoice> = store_video_editor_tts_voices!(ui)
        .iter()
        .filter(|v| !v.built_in)
        .collect();

    tokio::spawn(async move {
        let voices_data: Vec<(String, String)> = voices
            .iter()
            .map(|v| (v.name.to_string(), v.path.to_string()))
            .collect();

        let data = serde_json::to_string(&voices_data).expect("serialize voices failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, TTS_IMPORTED_VOICE_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, TTS_IMPORTED_VOICE_ID, &data).await
            {
                log::warn!("Failed to save imported voices: {:?}", e);
            }
        }
    });
}

async fn load_imported_voices(ui_weak: Weak<AppWindow>) {
    let marked_names = match sqldb::entry::select(VIDEO_EDITOR_TABLE, TTS_MARKED_VOICE_ID).await {
        Ok(entry) => serde_json::from_str::<HashSet<String>>(&entry.data).unwrap_or_default(),
        Err(_) => HashSet::new(),
    };
    *MARKED_VOICE_NAMES.lock().unwrap() = Some(marked_names);

    if let Ok(entry) = sqldb::entry::select(VIDEO_EDITOR_TABLE, TTS_IMPORTED_VOICE_ID).await
        && let Ok(voices_data) = serde_json::from_str::<Vec<(String, String)>>(&entry.data)
    {
        let marked_set = MARKED_VOICE_NAMES.lock().unwrap().clone();
        let mut imported_voices: Vec<UITTSVoice> = voices_data
            .into_iter()
            .map(|(name, path)| {
                let marked = marked_set
                    .as_ref()
                    .map(|set| set.contains(&path))
                    .unwrap_or(false);

                UITTSVoice {
                    name: name.into(),
                    path: path.into(),
                    marked,
                    built_in: false,
                }
            })
            .collect();

        // Sort: marked first, then by name alphabetically
        imported_voices.sort_by(|a, b| match (a.marked, b.marked) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            for voice in imported_voices {
                store_video_editor_tts_voices!(&ui).push(voice);
            }
        });
    }
}

fn video_editor_tts_release_resource(_ui: &AppWindow) {
    tokio::task::spawn_blocking(move || {
        if let Some(ref cancel) = *GENERATE_ALL_CANCEL.lock().unwrap() {
            cancel.cancel();
        }
        *GENERATE_ALL_CANCEL.lock().unwrap() = None;

        if let Some(ref current) = *CURRENT_PLAYER.get().unwrap().lock().unwrap() {
            current.stop();
            current.clear();
        }
        *CURRENT_PLAYER.get().unwrap().lock().unwrap() = None;

        let cleanup_device = if let Some(model_store) = VOXCPM_MODEL.get() {
            let mut guard = model_store.lock().unwrap();
            let device = match guard.as_ref() {
                Some(VoxCpmModel::Wgpu { device, .. }) => Some(device.clone()),
                Some(VoxCpmModel::NdArray(_)) => None,
                None => None,
            };
            *guard = None;
            device
        } else {
            None
        };

        if let Some(device) = cleanup_device {
            // 1. 第一次 sync - 确保之前所有 GPU 操作完成
            if let Err(e) = <Wgpu<f32, i32> as Backend>::sync(&device) {
                log::warn!("TTS GPU pre-cleanup sync failed: {:?}", e);
            }

            // 2. memory_cleanup - 发送 cleanup 命令到队列
            <Wgpu<f32, i32> as Backend>::memory_cleanup(&device);

            // 3. 第二次 sync - 等待 cleanup 执行、dealloc 发生、GPU buffer drop 完成
            if let Err(e) = <Wgpu<f32, i32> as Backend>::sync(&device) {
                log::warn!("TTS GPU post-cleanup sync failed: {:?}", e);
            }

            log::info!("TTS GPU memory cleanup completed");
        }
    });

    log::info!("TTS resources released");
}
