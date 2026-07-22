use super::{
    media_list_common::{self as common, MediaListContext},
    playlist::{
        async_add_item_to_track, async_add_item_to_track_end, collect_media_dir_entries,
        picker_files, picker_folder,
    },
    project::{LIBRARY_ID, PROJECT_STATE},
};
use crate::{
    db::VIDEO_EDITOR_TABLE,
    global_store,
    logic::{toast, tr::tr},
    logic_cb,
    slint_generatedAppWindow::{
        AppWindow, ConfirmDialogSetting as UIConfirmDialogSetting,
        VideoEditorPlaylistItem as UIVideoEditorPlaylistItem,
    },
};
use slint::{ComponentHandle, Model, SharedString, VecModel, Weak};
use std::{collections::HashMap, path::PathBuf};
use video_editor::media::{MediaList, SUPPORT_EXT, media_type::MediaType};

#[macro_export]
macro_rules! store_video_editor_media_library {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_media_library()
            .as_any()
            .downcast_ref::<VecModel<UIVideoEditorPlaylistItem>>()
            .expect("We know we set a VecModel<UIVideoEditorPlaylistItem> earlier")
    };
}

const CTX: MediaListContext = MediaListContext::Library;

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_library_item_import, ui);
    logic_cb!(video_editor_library_folder_import, ui);
    logic_cb!(video_editor_library_item_remove, ui, index);
    logic_cb!(video_editor_library_item_add_to_track, ui, index);
    logic_cb!(video_editor_library_item_add_to_track_end, ui, index);
    logic_cb!(video_editor_library_item_preview, ui, index);
    logic_cb!(video_editor_library_all_files, ui);
    logic_cb!(video_editor_library_all_videos, ui);
    logic_cb!(video_editor_library_all_audios, ui);
    logic_cb!(video_editor_library_all_images, ui);
    logic_cb!(video_editor_library_enter_edit_mode, ui);
    logic_cb!(video_editor_library_exit_edit_mode, ui);
    logic_cb!(video_editor_library_toggle_select_all, ui, selected);
    logic_cb!(video_editor_library_remove_all, ui);
    logic_cb!(video_editor_library_show_remove_all_confirm_dialog, ui);
    logic_cb!(
        video_editor_library_show_remove_selections_confirm_dialog,
        ui
    );
    logic_cb!(video_editor_library_remove_selections, ui);
    logic_cb!(video_editor_library_toggle_selection, ui, index);
    logic_cb!(video_editor_library_toggle_mark, ui, index);
    logic_cb!(video_editor_library_search, ui, index, keyword);
    logic_cb!(video_editor_library_item_copy_path, ui, index);
    logic_cb!(video_editor_library_item_open_directory, ui, index);
    logic_cb!(video_editor_library_create_folder, ui);
    logic_cb!(video_editor_library_rename_folder, ui, id);
    logic_cb!(video_editor_library_remove_folder, ui, id);
    logic_cb!(video_editor_library_navigate_to_folder, ui, id);
    logic_cb!(video_editor_library_navigate_up, ui);
    logic_cb!(
        video_editor_library_move_item_to_folder,
        ui,
        item_id,
        folder_id
    );
    logic_cb!(
        video_editor_library_move_folder,
        ui,
        folder_id,
        new_parent_id
    );
    logic_cb!(video_editor_library_sync_folder, ui, id);
    logic_cb!(video_editor_library_refresh, ui);
    logic_cb!(video_editor_rename_dialog_confirmed, ui, text);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let library = match sqldb::entry::select(VIDEO_EDITOR_TABLE, LIBRARY_ID).await {
            Ok(setting) => match MediaList::from_json(&setting.data) {
                Ok(lib) => Some(lib),
                Err(e) => {
                    log::warn!("Failed to parse library from JSON: {}", e);
                    None
                }
            },
            Err(_) => {
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, LIBRARY_ID, "{}").await;
                None
            }
        };

        if let Some(lib) = library {
            let mut state = PROJECT_STATE.lock().unwrap();
            if let Some(ref mut s) = *state {
                s.library = lib;
            }
        }

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            refresh_library_ui(&ui);
        });
    });
}

fn video_editor_library_item_import(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let current_folder_id = CTX.get_current_folder_id(ui);
    log::debug!("Importing to folder: {:?}", current_folder_id);

    tokio::spawn(async move {
        let Some(file_paths) = picker_files(
            ui_weak.clone(),
            &tr("Select media files"),
            &tr("Media Files"),
            SUPPORT_EXT,
        ) else {
            return;
        };

        toast::async_toast_info(
            ui_weak.clone(),
            tr("Importing files, this may take a while..."),
        );

        for file_path in file_paths {
            tokio::spawn(import_file_to_library(
                ui_weak.clone(),
                file_path,
                current_folder_id.clone(),
            ));
        }
    });
}

fn video_editor_library_folder_import(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let current_folder_id = CTX.get_current_folder_id(ui);
    tokio::spawn(async move {
        let Some(dir_path) = picker_folder(ui_weak.clone(), &tr("Select directory to import"))
        else {
            return;
        };

        toast::async_toast_info(
            ui_weak.clone(),
            tr("Importing folder, this may take a while..."),
        );

        let dir_path_clone = dir_path.clone();
        let dir_entries =
            tokio::task::spawn_blocking(move || collect_media_dir_entries(&dir_path_clone))
                .await
                .unwrap_or_default();

        if dir_entries.is_empty() {
            toast::async_toast_warn(ui_weak.clone(), tr("No media files found in directory"));
            return;
        }

        // Create folder hierarchy and import files into their respective folders
        let file_imports = {
            let mut state = PROJECT_STATE.lock().unwrap();
            if let Some(ref mut s) = *state {
                // Create root folder from source
                let root_folder_id = match s
                    .library
                    .create_folder_from_source(dir_path.clone(), current_folder_id.clone())
                {
                    Ok(id) => id,
                    Err(e) => {
                        log::warn!("Failed to create folder from source: {}", e);
                        // Fallback: import to current folder
                        current_folder_id.unwrap_or_default()
                    }
                };

                let mut dir_to_folder_id = HashMap::new();
                dir_to_folder_id.insert(dir_path.clone(), root_folder_id.clone());

                let mut file_imports: Vec<(PathBuf, String)> = Vec::new();

                for (file_path, relative_dir) in dir_entries {
                    let folder_id = if relative_dir == PathBuf::new() {
                        root_folder_id.clone()
                    } else {
                        // Ensure all parent directories exist in the folder tree
                        let mut current_path = dir_path.clone();
                        let mut parent_folder_id = root_folder_id.clone();

                        for component in relative_dir.components() {
                            current_path = current_path.join(component);
                            if let Some(fid) = dir_to_folder_id.get(&current_path) {
                                parent_folder_id = fid.clone();
                            } else {
                                match s.library.create_folder_from_source(
                                    current_path.clone(),
                                    Some(parent_folder_id.clone()),
                                ) {
                                    Ok(new_id) => {
                                        dir_to_folder_id
                                            .insert(current_path.clone(), new_id.clone());
                                        parent_folder_id = new_id;
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to create subfolder: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                        parent_folder_id
                    };

                    file_imports.push((file_path, folder_id));
                }

                file_imports
            } else {
                Vec::new()
            }
        };

        for (file_path, folder_id) in file_imports {
            tokio::spawn(import_file_to_library(
                ui_weak.clone(),
                file_path,
                Some(folder_id.into()),
            ));
        }
    });
}

async fn import_file_to_library(
    ui_weak: Weak<AppWindow>,
    file_path: PathBuf,
    folder_id: Option<String>,
) {
    log::debug!(
        "import_file_to_library: folder_id={:?}, file={}",
        folder_id,
        file_path.display()
    );

    let result = {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            s.library.add_file(file_path.clone(), folder_id)
        } else {
            Err(video_editor::Error::InvalidConfig(
                "Project state not initialized".to_string(),
            ))
        }
    };

    match result {
        Ok(item_id) => {
            let library_item = {
                let state = PROJECT_STATE.lock().unwrap();
                if let Some(ref s) = *state {
                    s.library
                        .get_node(&item_id)
                        .and_then(|n| n.as_file())
                        .cloned()
                } else {
                    None
                }
            };

            if let Some(item) = library_item {
                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let ui_item: UIVideoEditorPlaylistItem = item.into();
                    crate::toast_success!(ui, format!("{}: {}", tr("Imported"), ui_item.name));

                    common::refresh_ui_filtered(CTX, &ui);
                });
            }

            save_library_to_db(ui_weak);
        }
        Err(video_editor::Error::DuplicateEntry(msg)) => {
            if let Some(name) = file_path.file_name().and_then(|s| s.to_str()) {
                toast::async_toast_warn(
                    ui_weak.clone(),
                    format!("{}: {} {}", name, tr("already exists in"), msg),
                );
            }
        }
        Err(e) => {
            if let Some(name) = file_path.file_name().and_then(|s| s.to_str()) {
                toast::async_toast_warn(ui_weak.clone(), format!("{}: {}", name, e));
            }
        }
    }
}

fn video_editor_library_item_remove(ui: &AppWindow, index: i32) {
    common::item_remove(CTX, ui, index);
}

fn video_editor_library_item_add_to_track(ui: &AppWindow, index: i32) {
    video_editor_library_clear_selection(ui);

    let Some(ui_item) = store_video_editor_media_library!(ui).row_data(index as usize) else {
        crate::toast_warn!(ui, tr("Invalid library item index"));
        return;
    };

    let item_id = ui_item.item_id.to_string();

    let state = PROJECT_STATE.lock().unwrap();
    if let Some(ref s) = *state {
        if let Some(item) = s.library.get_node(&item_id).and_then(|n| n.as_file()) {
            let name = item.name.clone();
            let file_path = item.file_path.clone();
            drop(state);
            async_add_item_to_track(ui.as_weak(), name, file_path);
        } else {
            crate::toast_warn!(ui, tr("Item not found in library"));
        }
    } else {
        crate::toast_warn!(ui, tr("Library not initialized"));
    }
}

fn video_editor_library_item_add_to_track_end(ui: &AppWindow, index: i32) {
    video_editor_library_clear_selection(ui);

    let Some(ui_item) = store_video_editor_media_library!(ui).row_data(index as usize) else {
        crate::toast_warn!(ui, tr("Invalid library item index"));
        return;
    };

    let item_id = ui_item.item_id.to_string();

    let state = PROJECT_STATE.lock().unwrap();
    if let Some(ref s) = *state {
        if let Some(item) = s.library.get_node(&item_id).and_then(|n| n.as_file()) {
            let name = item.name.clone();
            let file_path = item.file_path.clone();
            drop(state);
            async_add_item_to_track_end(ui.as_weak(), name, file_path);
        } else {
            crate::toast_warn!(ui, tr("Item not found in library"));
        }
    } else {
        crate::toast_warn!(ui, tr("Library not initialized"));
    }
}

fn video_editor_library_item_preview(ui: &AppWindow, index: i32) {
    common::item_preview(CTX, ui, index);
}

fn video_editor_library_all_files(ui: &AppWindow) {
    common::filter_by_type(CTX, ui, None);
}

fn video_editor_library_all_videos(ui: &AppWindow) {
    common::filter_by_type(CTX, ui, Some(MediaType::Video));
}

fn video_editor_library_all_audios(ui: &AppWindow) {
    common::filter_by_type(CTX, ui, Some(MediaType::Audio));
}

fn video_editor_library_all_images(ui: &AppWindow) {
    common::filter_by_type(CTX, ui, Some(MediaType::Image));
}

fn video_editor_library_enter_edit_mode(ui: &AppWindow) {
    common::enter_edit_mode(CTX, ui);
}

fn video_editor_library_exit_edit_mode(ui: &AppWindow) {
    common::exit_edit_mode(CTX, ui);
}

fn video_editor_library_toggle_select_all(ui: &AppWindow, selected: bool) {
    common::toggle_select_all(CTX, ui, selected);
    if selected {
        let count = global_store!(ui)
            .get_video_editor_media_library()
            .iter()
            .filter(|item| item.item_id != "..")
            .count() as i32;
        global_store!(ui).set_video_editor_media_library_selected_count(count);
    } else {
        global_store!(ui).set_video_editor_media_library_selected_count(0);
    }
    global_store!(ui).set_video_editor_playlist_selected_count(0);
}

fn video_editor_library_remove_all(ui: &AppWindow) {
    common::remove_all(CTX, ui);
}

fn video_editor_library_show_remove_all_confirm_dialog(ui: &AppWindow) {
    ui.global::<UIConfirmDialogSetting>().invoke_set(
        true,
        tr("Warning").into(),
        tr("Remove all items from library or not?").into(),
        "video-editor-library-remove-all".into(),
        "".into(),
    );
}

fn video_editor_library_show_remove_selections_confirm_dialog(ui: &AppWindow) {
    ui.global::<UIConfirmDialogSetting>().invoke_set(
        true,
        tr("Warning").into(),
        tr("Remove selected items from library or not?").into(),
        "video-editor-library-remove-selections".into(),
        "".into(),
    );
}

fn video_editor_library_remove_selections(ui: &AppWindow) {
    common::remove_selections(CTX, ui);
    global_store!(ui).set_video_editor_media_library_selected_count(0);
    global_store!(ui).set_video_editor_playlist_selected_count(0);
}

fn video_editor_library_toggle_selection(ui: &AppWindow, index: i32) {
    common::toggle_selection(CTX, ui, index);
}

fn video_editor_library_toggle_mark(ui: &AppWindow, index: i32) {
    common::toggle_mark(CTX, ui, index);
}

fn video_editor_library_search(ui: &AppWindow, index: i32, keyword: SharedString) {
    common::search(CTX, ui, index, keyword);
}

fn video_editor_library_item_copy_path(ui: &AppWindow, index: i32) {
    common::item_copy_path(CTX, ui, index);
}

fn video_editor_library_item_open_directory(ui: &AppWindow, index: i32) {
    common::item_open_directory(CTX, ui, index);
}

fn video_editor_library_create_folder(ui: &AppWindow) {
    common::create_folder(CTX, ui);
}

fn video_editor_library_rename_folder(ui: &AppWindow, id: SharedString) {
    common::rename_folder(CTX, ui, id);
}

fn video_editor_library_remove_folder(ui: &AppWindow, id: SharedString) {
    common::remove_folder(CTX, ui, id);
}

fn video_editor_library_sync_folder(ui: &AppWindow, id: SharedString) {
    let ui_weak = ui.as_weak();
    let folder_id = id.to_string();

    tokio::spawn(async move {
        toast::async_toast_info(
            ui_weak.clone(),
            tr("Syncing folder, this may take a while..."),
        );

        let result = tokio::task::spawn_blocking(move || {
            let mut state = PROJECT_STATE.lock().unwrap();
            if let Some(ref mut s) = *state {
                s.library.sync_folder(&folder_id)
            } else {
                Err(video_editor::Error::InvalidConfig(
                    "Project state not initialized".to_string(),
                ))
            }
        })
        .await
        .unwrap_or_else(|e| Err(video_editor::Error::InvalidConfig(e.to_string())));

        _ = ui_weak.upgrade_in_event_loop(move |ui| match result {
            Ok(sync_result) => {
                if !sync_result.removed.is_empty() || !sync_result.added.is_empty() {
                    let msg = if sync_result.added.is_empty() {
                        format!(
                            "{}: {} {}",
                            tr("Synced"),
                            sync_result.removed.len(),
                            tr("removed")
                        )
                    } else if sync_result.removed.is_empty() {
                        format!(
                            "{}: {} {}",
                            tr("Synced"),
                            sync_result.added.len(),
                            tr("added")
                        )
                    } else {
                        format!(
                            "{}: {} {}, {} {}",
                            tr("Synced"),
                            sync_result.removed.len(),
                            tr("removed"),
                            sync_result.added.len(),
                            tr("added")
                        )
                    };
                    crate::toast_success!(ui, msg);
                } else {
                    crate::toast_success!(ui, tr("Already up to date"));
                }
                common::refresh_ui_filtered(CTX, &ui);
                CTX.save(ui.as_weak());
            }
            Err(e) => {
                crate::toast_warn!(ui, format!("{}: {}", tr("Sync failed"), e));
            }
        });
    });
}

fn video_editor_library_refresh(ui: &AppWindow) {
    common::refresh_media_list(CTX, ui);
}

fn video_editor_library_navigate_to_folder(ui: &AppWindow, id: SharedString) {
    common::navigate_to_folder(CTX, ui, id);
}

fn video_editor_library_navigate_up(ui: &AppWindow) {
    common::navigate_up(CTX, ui);
}

fn video_editor_library_move_item_to_folder(
    ui: &AppWindow,
    item_id: SharedString,
    folder_id: SharedString,
) {
    common::move_item_to_folder(CTX, ui, item_id, folder_id);
}

fn video_editor_library_move_folder(
    ui: &AppWindow,
    folder_id: SharedString,
    new_parent_id: SharedString,
) {
    common::move_folder(CTX, ui, folder_id, new_parent_id);
}

fn save_library_to_db(ui_weak: Weak<AppWindow>) {
    tokio::spawn(async move {
        let library_json = {
            let state = PROJECT_STATE.lock().unwrap();
            if let Some(ref s) = *state {
                s.library.to_json(true).ok()
            } else {
                None
            }
        };

        if let Some(json_str) = library_json
            && let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, LIBRARY_ID, &json_str).await
        {
            log::warn!("Failed to save library to database: {}", e);
            toast::async_toast_warn(ui_weak, format!("{}: {}", tr("Failed to save library"), e));
        }
    });
}

pub fn refresh_library_ui(ui: &AppWindow) {
    common::refresh_ui(CTX, ui);
}

pub fn video_editor_library_clear_selection(ui: &AppWindow) {
    common::clear_selection(CTX, ui);
}

fn video_editor_rename_dialog_confirmed(ui: &AppWindow, text: SharedString) {
    common::rename_dialog_confirmed(ui, text);
}
