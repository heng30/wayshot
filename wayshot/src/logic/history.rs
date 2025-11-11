use crate::{
    config,
    db::{HISTORY_TABLE as DB_TABLE, HistoryEntry},
    db_select_all,
    logic::tr::tr,
    logic_cb,
    slint_generatedAppWindow::{AppWindow, HistoryEntry as UIHistoryEntry},
    toast_success,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{fs, path::PathBuf};
use uuid::Uuid;

#[macro_export]
macro_rules! store_history_entries {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_history_entries()
            .as_any()
            .downcast_ref::<VecModel<UIHistoryEntry>>()
            .expect("We know we set a VecModel<UIHistoryEntry> earlier")
    };
}

crate::db_add!(DB_TABLE, HistoryEntry);
crate::db_remove!(DB_TABLE);
crate::db_remove_all!(DB_TABLE);

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(history_statistics, ui, etries, _flag);
    logic_cb!(toggle_sort_history, ui);
    logic_cb!(refresh_histories, ui);
    logic_cb!(add_history, ui, file);
    logic_cb!(remove_history, ui, index);
    logic_cb!(remove_no_found_histories, ui);
    logic_cb!(remove_all_histories, ui);
}

fn inner_init(ui: &AppWindow) {
    store_history_entries!(ui).set_vec(vec![]);

    let ui = ui.as_weak();
    tokio::spawn(async move {
        let save_dir = PathBuf::from(&config::all().recorder.save_dir);
        let entries = db_select_all!(DB_TABLE, HistoryEntry);

        _ = ui.upgrade_in_event_loop(move |ui| {
            let entries = entries
                .into_iter()
                .map(|entry| {
                    let mut entry: UIHistoryEntry = entry.into();
                    if save_dir.join(&entry.file).exists() {
                        entry.status = SharedString::default();
                    } else {
                        entry.status = tr("No Found").into();
                    }
                    entry
                })
                .rev()
                .collect::<Vec<UIHistoryEntry>>();

            store_history_entries!(ui).set_vec(entries);
        });
    });
}

fn history_statistics(
    _ui: &AppWindow,
    entries: ModelRc<UIHistoryEntry>,
    _flag: i32,
) -> ModelRc<i32> {
    let mut statistics = [0; 3];

    for entry in entries.iter() {
        if entry.status.is_empty() {
            statistics[1] += 1;
        } else {
            statistics[2] += 1;
        }

        statistics[0] += 1;
    }

    ModelRc::new(VecModel::from_slice(&statistics))
}

fn toggle_sort_history(ui: &AppWindow) {
    let items = store_history_entries!(ui)
        .iter()
        .collect::<Vec<UIHistoryEntry>>()
        .into_iter()
        .rev()
        .collect::<Vec<UIHistoryEntry>>();

    store_history_entries!(ui).set_vec(items);
}

fn refresh_histories(ui: &AppWindow) {
    inner_init(ui);
    toast_success!(ui, tr("refresh histories successfully"));
}

fn add_history(ui: &AppWindow, file_path: SharedString) {
    let file_size = cutil::fs::file_size(&file_path);
    let file_size = cutil::fs::pretty_bytes_size(file_size);

    let file_duration = match mp4_player::metadata::parse(&file_path) {
        Ok(metadata) => cutil::time::seconds_to_media_timestamp(metadata.duration.as_secs_f64()),
        Err(e) => {
            log::warn!("{e}");
            "00:00".to_string()
        }
    };

    let entry = HistoryEntry {
        id: Uuid::new_v4().to_string(),
        file: cutil::fs::file_name(&file_path),
        size: file_size,
        duration: file_duration,
        status: String::default(),
    };

    store_history_entries!(ui).insert(0, entry.clone().into());
    db_add(ui.as_weak(), entry);
}

fn remove_history(ui: &AppWindow, index: i32) {
    let rows = store_history_entries!(ui).row_count();
    if index < 0 || index as usize >= rows {
        return;
    }

    let index = index as usize;
    let entry = store_history_entries!(ui).row_data(index).unwrap();
    store_history_entries!(ui).remove(index);
    db_remove(ui.as_weak(), &entry.id);

    let file = PathBuf::from(&config::all().recorder.save_dir).join(&entry.file);

    if file.exists() {
        _ = fs::remove_file(file);
    }

    toast_success!(ui, tr("remove history successfully"));
}

fn remove_no_found_histories(ui: &AppWindow) {
    let found_items = store_history_entries!(ui)
        .iter()
        .filter(|item| item.status.is_empty())
        .collect::<Vec<UIHistoryEntry>>();

    let no_found_items = store_history_entries!(ui)
        .iter()
        .filter(|item| !item.status.is_empty())
        .collect::<Vec<UIHistoryEntry>>();

    store_history_entries!(ui).set_vec(found_items);

    no_found_items.into_iter().for_each(|item| {
        db_remove(ui.as_weak(), &item.id);
    });
}

fn remove_all_histories(ui: &AppWindow) {
    let save_dir = PathBuf::from(&config::all().recorder.save_dir);
    if save_dir.exists() {
        store_history_entries!(ui).iter().for_each(|item| {
            let file = save_dir.join(&item.file);
            if file.exists() {
                _ = fs::remove_file(file);
            }
        });
    }

    store_history_entries!(ui).set_vec(vec![]);
    db_remove_all(ui.as_weak());
}
