use crate::{
    config, global_logic, global_store,
    logic::{toast, tr::tr, video_editor::project::PROJECT_STATE},
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, ChapterSummaryEntry as UIChapterSummaryEntry,
        ChapterSummaryProgressType as UIChapterSummaryProgressType,
    },
    toast_info, toast_warn,
};
use bot::{APIConfig, Chat, ChatConfig, StreamTextItem};
use cutil::time::seconds_to_media_timestamp;
use once_cell::sync::Lazy;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use video_editor::{filters::traits::SubtitleEntry, project::ChapterSummaryData, tracks::Track};

fn sync_entries_to_project_state(ui: &AppWindow) {
    let entries = global_store!(ui).get_video_editor_chapter_summary_entries();
    let data: Vec<ChapterSummaryData> = entries
        .iter()
        .map(|e| ChapterSummaryData {
            start_ms: e.start_ms as u64,
            end_ms: e.end_ms as u64,
            title: e.title.to_string(),
        })
        .collect();
    let mut state = PROJECT_STATE.lock().unwrap();
    if let Some(ref mut s) = *state {
        s.chapter_summary = data;
    }
}

#[derive(serde::Serialize)]
struct ChapterSummaryInput {
    subtitles: Vec<InputSubtitle>,
    total_duration_ms: u64,
}

#[derive(serde::Serialize)]
struct InputSubtitle {
    index: usize,
    start_ms: u64,
    end_ms: u64,
    text: String,
}

#[derive(serde::Deserialize)]
struct ChapterSummaryOutput {
    chapters: Vec<ChapterOutput>,
}

#[derive(serde::Deserialize)]
struct ChapterOutput {
    start_ms: u64,
    end_ms: u64,
    title: String,
}

#[derive(Default)]
struct ChapterSummaryCache {
    stop_sig: Option<Arc<AtomicBool>>,
    inc_index: u64,
}

static CHAPTER_SUMMARY_CACHE: Lazy<Mutex<ChapterSummaryCache>> =
    Lazy::new(|| Mutex::new(ChapterSummaryCache::default()));

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_chapter_summary_start, ui);
    logic_cb!(video_editor_chapter_summary_cancel, ui);
    logic_cb!(video_editor_chapter_summary_copy, ui);
    logic_cb!(video_editor_chapter_summary_remove_all, ui);
    logic_cb!(
        video_editor_chapter_summary_update_chapter,
        ui,
        index,
        title
    );
}

fn video_editor_chapter_summary_start(ui: &AppWindow) {
    let setting = config::all().ai_model;
    if setting.api_base_url.is_empty()
        || setting.model_name.is_empty()
        || setting.api_key.is_empty()
    {
        toast_info!(ui, tr("Please setup AI model and try again"));
        return;
    }

    let (subtitle_entries, total_duration_ms) = {
        let state = PROJECT_STATE.lock().unwrap();
        let Some(ref s) = *state else {
            toast_warn!(ui, tr("No project opened"));
            return;
        };

        let mut found_entries: Vec<SubtitleEntry> = vec![];
        for track in &s.tracks_manager.tracks {
            if let Track::Subtitle(st) = track {
                found_entries = st.get_subtitle_entries();
                break;
            }
        }

        let duration_ms = s.tracks_manager.duration.as_millis() as u64;
        (found_entries, duration_ms)
    };

    if subtitle_entries.is_empty() {
        toast_warn!(ui, tr("No subtitle track found or subtitle track is empty"));
        return;
    }

    global_store!(ui)
        .set_video_editor_chapter_summary_progress_type(UIChapterSummaryProgressType::Generating);
    global_store!(ui).set_video_editor_chapter_summary_progress(0.0);
    global_store!(ui).set_video_editor_chapter_summary_entries(ModelRc::new(VecModel::default()));

    let stop_sig = Arc::new(AtomicBool::new(false));
    let inc_index = {
        let mut cache = CHAPTER_SUMMARY_CACHE.lock().unwrap();
        if let Some(sig) = cache.stop_sig.take() {
            sig.store(true, Ordering::Relaxed);
        }
        cache.inc_index += 1;
        cache.stop_sig = Some(stop_sig.clone());
        cache.inc_index
    };

    let ui_weak = ui.as_weak();
    let model_config = setting;
    tokio::spawn(async move {
        let input_subtitles: Vec<InputSubtitle> = subtitle_entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| InputSubtitle {
                index,
                start_ms: entry.start.as_millis() as u64,
                end_ms: entry.end.as_millis() as u64,
                text: entry.text,
            })
            .collect();

        let input = ChapterSummaryInput {
            subtitles: input_subtitles,
            total_duration_ms,
        };

        let prompt = r#"You are a video chapter summary assistant. Given subtitle entries with timestamps, group them into meaningful chapters. Each chapter should cover a coherent topic or section of the video.

<Input format>
{ "total_duration_ms": 300000, "subtitles": [{"index":0,"start_ms":0,"end_ms":3000,"text":"..."}] }
</Input format>

<Output format>
{ "chapters": [{"start_ms":0,"end_ms":30000,"title":"Introduction"},...] }
</Output format>

Rules:
- Chapters should cover the entire video duration without gaps
- Each chapter title should be concise (less than 100 words)
- Group subtitles that belong to the same topic into one chapter
- The first chapter's start_ms should be 0
- The last chapter's end_ms should match total_duration_ms
- Only output the JSON, no additional text
"#;

        let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamTextItem>(100);
        let question = match serde_json::to_string(&input) {
            Ok(q) => q,
            Err(e) => {
                toast::async_toast_warn(
                    ui_weak,
                    format!("{}: {e}", tr("Failed to serialize subtitle data")),
                );
                return;
            }
        };

        let request_config = APIConfig {
            api_base_url: model_config.api_base_url,
            api_model: model_config.model_name,
            api_key: model_config.api_key,
            temperature: Some(0.3),
        };

        let ui_weak_for_chat = ui_weak.clone();
        tokio::spawn(async move {
            let chat_config = ChatConfig { tx };
            let chat = Chat::new(prompt, question, chat_config, request_config, vec![]);
            if let Err(e) = chat.start().await {
                toast::async_toast_warn(
                    ui_weak_for_chat,
                    format!("Start AI chapter summary failed: {e}"),
                );
            }
        });

        let mut resp = String::new();
        while let Some(item) = rx.recv().await {
            if stop_sig.load(Ordering::Relaxed) {
                return;
            }

            if let Some(ref text) = item.text {
                resp.push_str(text);
            }
        }

        if stop_sig.load(Ordering::Relaxed) {
            return;
        }

        if inc_index != CHAPTER_SUMMARY_CACHE.lock().unwrap().inc_index {
            return;
        }

        let resp = resp
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string();

        log::debug!("Chapter summary response: {resp}");

        let output: ChapterSummaryOutput = match serde_json::from_str(&resp) {
            Ok(o) => o,
            Err(e) => {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    crate::toast_warn!(
                        ui,
                        format!("{}: {e}", tr("Failed to parse chapter summary response"))
                    );

                    global_store!(ui).set_video_editor_chapter_summary_progress_type(
                        UIChapterSummaryProgressType::Failed,
                    );
                });
                return;
            }
        };

        let entries: Vec<UIChapterSummaryEntry> = output
            .chapters
            .into_iter()
            .map(|ch| UIChapterSummaryEntry {
                start_ms: ch.start_ms as i32,
                end_ms: ch.end_ms as i32,
                title: ch.title.into(),
            })
            .collect();

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_chapter_summary_progress(1.0);
            global_store!(ui).set_video_editor_chapter_summary_progress_type(
                UIChapterSummaryProgressType::Finished,
            );
            global_store!(ui).set_video_editor_chapter_summary_entries(ModelRc::new(
                VecModel::from_slice(&entries),
            ));
            sync_entries_to_project_state(&ui);
        });
    });
}

fn video_editor_chapter_summary_cancel(ui: &AppWindow) {
    {
        let mut cache = CHAPTER_SUMMARY_CACHE.lock().unwrap();
        if let Some(sig) = cache.stop_sig.take() {
            sig.store(true, Ordering::Relaxed);
        }
        cache.inc_index += 1;
    }

    global_store!(ui)
        .set_video_editor_chapter_summary_progress_type(UIChapterSummaryProgressType::Cancelled);
}

fn video_editor_chapter_summary_copy(ui: &AppWindow) {
    let entries = global_store!(ui).get_video_editor_chapter_summary_entries();
    if entries.row_count() == 0 {
        return;
    }

    let lines: Vec<String> = entries
        .iter()
        .map(|e| {
            let start = seconds_to_media_timestamp(e.start_ms as f64 / 1000.0);
            let end = seconds_to_media_timestamp(e.end_ms as f64 / 1000.0);
            format!("{} - {}: {}", start, end, e.title)
        })
        .collect();

    let text = lines.join("\n");
    global_logic!(ui).invoke_copy_to_clipboard(text.into());
    crate::toast_success!(ui, tr("Text copied"));
}

fn video_editor_chapter_summary_remove_all(ui: &AppWindow) {
    global_store!(ui).set_video_editor_chapter_summary_entries(ModelRc::new(VecModel::default()));
    global_store!(ui)
        .set_video_editor_chapter_summary_progress_type(UIChapterSummaryProgressType::None);
    sync_entries_to_project_state(ui);
}

fn video_editor_chapter_summary_update_chapter(ui: &AppWindow, index: i32, title: SharedString) {
    let entries = global_store!(ui).get_video_editor_chapter_summary_entries();
    let vec_model = entries
        .as_any()
        .downcast_ref::<VecModel<UIChapterSummaryEntry>>()
        .expect("chapter summary entries model");

    if let Some(mut entry) = vec_model.row_data(index as usize) {
        entry.title = title;
        vec_model.set_row_data(index as usize, entry);
    }
    sync_entries_to_project_state(ui);
}
