use crate::{
    global_store,
    logic::video_editor::project::PROJECT_STATE,
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, VideoEditorSubtitleSearchEntry as UISubtitleSearchEntry,
    },
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::time::Duration;
use video_editor::{filters::traits::SubtitleEntry, tracks::Track};
use video_utils::subtitle::ms_to_srt_timestamp;

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_subtitle_search_load, ui);
    logic_cb!(video_editor_subtitle_search_filter, ui, text);
    logic_cb!(video_editor_subtitle_search_jump, ui, index);
}

fn collect_subtitle_entries() -> Vec<SubtitleEntry> {
    let state = PROJECT_STATE.lock().unwrap();
    let Some(ref s) = *state else {
        return vec![];
    };

    let mut entries: Vec<SubtitleEntry> = vec![];
    for track in &s.tracks_manager.tracks {
        if let Track::Subtitle(st) = track {
            entries.extend(st.get_subtitle_entries());
        }
    }
    entries.sort_by_key(|e| e.start);
    entries
}

fn to_ui_entries(entries: &[SubtitleEntry]) -> Vec<UISubtitleSearchEntry> {
    entries
        .iter()
        .map(|e| UISubtitleSearchEntry {
            start_time: ms_to_srt_timestamp(e.start.as_millis() as u64).into(),
            end_time: ms_to_srt_timestamp(e.end.as_millis() as u64).into(),
            text: e.text.clone().into(),
            start_ms: e.start.as_millis() as i32,
        })
        .collect()
}

fn video_editor_subtitle_search_load(ui: &AppWindow) {
    let ui_entries = to_ui_entries(&collect_subtitle_entries());
    global_store!(ui)
        .set_video_editor_subtitle_search_entries(ModelRc::new(VecModel::from(ui_entries.clone())));
    global_store!(ui).set_video_editor_subtitle_search_filtered_entries(ModelRc::new(
        VecModel::from(ui_entries),
    ));
}

fn video_editor_subtitle_search_filter(ui: &AppWindow, text: SharedString) {
    let keyword = text.to_lowercase();
    let entries = global_store!(ui).get_video_editor_subtitle_search_entries();
    let filtered: Vec<UISubtitleSearchEntry> = entries
        .iter()
        .filter(|e| keyword.is_empty() || e.text.to_lowercase().contains(&keyword))
        .collect();
    global_store!(ui)
        .set_video_editor_subtitle_search_filtered_entries(ModelRc::new(VecModel::from(filtered)));
}

fn video_editor_subtitle_search_jump(ui: &AppWindow, index: i32) {
    let entries = global_store!(ui).get_video_editor_subtitle_search_filtered_entries();
    let Some(entry) = entries.iter().nth(index as usize) else {
        return;
    };

    super::preview::seek_to_position(ui, Duration::from_millis(entry.start_ms as u64), true);
    global_store!(ui).set_video_editor_is_show_subtitle_search_dialog(false);
}
