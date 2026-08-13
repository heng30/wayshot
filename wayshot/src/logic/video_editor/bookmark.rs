use crate::{
    global_store,
    logic::{tr::tr, video_editor::project::PROJECT_STATE},
    logic_cb,
    slint_generatedAppWindow::{AppWindow, VideoEditorBookmarkEntry as UIVideoEditorBookmarkEntry},
};
use slint::{ComponentHandle, ModelRc, VecModel};
use video_editor::project::BookmarkData;

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_bookmark_add, ui);
    logic_cb!(video_editor_bookmark_remove, ui, time_ms);
    logic_cb!(video_editor_bookmark_remove_all, ui);
}

pub fn sync_bookmarks_to_ui(ui: &AppWindow) {
    let bookmarks = {
        let state = PROJECT_STATE.lock().unwrap();
        state
            .as_ref()
            .map(|s| s.bookmarks.clone())
            .unwrap_or_default()
    };

    let entries: Vec<UIVideoEditorBookmarkEntry> = bookmarks
        .iter()
        .map(|b| UIVideoEditorBookmarkEntry {
            time_ms: b.time_ms as i32,
        })
        .collect();

    global_store!(ui).set_video_editor_bookmarks(ModelRc::new(VecModel::from_slice(&entries)));
}

fn video_editor_bookmark_add(ui: &AppWindow) {
    let time_ms = global_store!(ui).get_video_editor_timeline_offset().max(0) as u64;

    let added = {
        let mut state = PROJECT_STATE.lock().unwrap();
        match *state {
            Some(ref mut s) => {
                if s.bookmarks.iter().any(|b| b.time_ms == time_ms) {
                    false
                } else {
                    s.bookmarks.push(BookmarkData { time_ms });
                    s.bookmarks.sort_by_key(|b| b.time_ms);
                    true
                }
            }
            None => false,
        }
    };

    if added {
        global_store!(ui).set_video_editor_is_unsaved(true);
        sync_bookmarks_to_ui(ui);
        crate::toast_success!(ui, tr("Bookmark added"));
    } else {
        crate::toast_warn!(ui, tr("Bookmark already exists at the current position"));
    }
}

fn video_editor_bookmark_remove(ui: &AppWindow, time_ms: i32) {
    let removed = {
        let mut state = PROJECT_STATE.lock().unwrap();
        match *state {
            Some(ref mut s) => {
                let before = s.bookmarks.len();
                s.bookmarks.retain(|b| b.time_ms != time_ms as u64);
                before != s.bookmarks.len()
            }
            None => false,
        }
    };

    if removed {
        global_store!(ui).set_video_editor_is_unsaved(true);
        sync_bookmarks_to_ui(ui);
    }
}

fn video_editor_bookmark_remove_all(ui: &AppWindow) {
    {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            s.bookmarks.clear();
        }
    }

    global_store!(ui).set_video_editor_is_unsaved(true);
    sync_bookmarks_to_ui(ui);
}
