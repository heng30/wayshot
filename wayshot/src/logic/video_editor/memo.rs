use crate::{
    db::VIDEO_EDITOR_TABLE,
    global_store,
    logic::{
        toast,
        tr::tr,
        video_editor::project::{GLOBAL_MEMO_ID, PROJECT_STATE},
    },
    logic_cb,
    slint_generatedAppWindow::AppWindow,
};
use serde::{Deserialize, Serialize};
use slint::ComponentHandle;

#[derive(Serialize, Deserialize)]
struct GlobalMemoData {
    text: String,
}

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_memo_save, ui, tab_index);
    logic_cb!(video_editor_memo_close, ui);
    logic_cb!(video_editor_load_project_memo, ui);

    load_global_memo(ui);
}

fn load_global_memo(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let text = match sqldb::entry::select(VIDEO_EDITOR_TABLE, GLOBAL_MEMO_ID).await {
            Ok(entry) => serde_json::from_str::<GlobalMemoData>(&entry.data)
                .map(|d| d.text)
                .unwrap_or_default(),
            Err(_) => {
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, GLOBAL_MEMO_ID, "{}").await;
                String::new()
            }
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_global_memo_text(text.into());
        });
    });
}

fn video_editor_memo_save(ui: &AppWindow, tab_index: i32) {
    if tab_index == 0 {
        let text = global_store!(ui).get_video_editor_global_memo_text();
        save_global_memo_to_db(ui.as_weak(), text.to_string());
    } else {
        let text = global_store!(ui).get_video_editor_project_memo_text();
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            s.memo = text.to_string();
        }
    }
}

fn video_editor_memo_close(ui: &AppWindow) {
    video_editor_memo_save(ui, 0);
    video_editor_memo_save(ui, 1);
}

fn video_editor_load_project_memo(ui: &AppWindow) {
    let state = PROJECT_STATE.lock().unwrap();
    if let Some(ref s) = *state {
        global_store!(ui).set_video_editor_project_memo_text(s.memo.clone().into());
    }
}

fn save_global_memo_to_db(ui: slint::Weak<AppWindow>, text: String) {
    tokio::spawn(async move {
        let data = GlobalMemoData { text };
        let json = serde_json::to_string(&data).unwrap_or_default();
        if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, GLOBAL_MEMO_ID, &json).await {
            toast::async_toast_warn(ui, format!("{}. {e}", tr("update entry failed")));
        }
    });
}
