use super::{
    item_preview::show_preview_item,
    project::{LIBRARY_ID, PROJECT_STATE, ProjectState},
};
use crate::{
    db::VIDEO_EDITOR_TABLE,
    global_logic, global_store,
    logic::tr::tr,
    slint_generatedAppWindow::{
        AppWindow, ConfirmDialogSetting as UIConfirmDialogSetting, MediaType as UIMediaType,
        VideoEditorPlaylistItem as UIVideoEditorPlaylistItem,
    },
};
use slint::{ComponentHandle, Image, Model, ModelRc, SharedString, VecModel, Weak};
use std::path::Path;
use video_editor::media::{MediaList, library::LibraryNode, media_type::MediaType};

#[derive(Clone, Copy)]
pub enum MediaListContext {
    Library,
    Playlist,
}

/// Macro to dispatch store access by context. Binds the ModelRc to a local variable
/// before downcasting to avoid lifetime issues with temporary values.
///
/// Usage: `with_media_list_store!(ctx, ui, store, { body })`
/// where `store` is bound to `&VecModel<UIVideoEditorPlaylistItem>`
#[macro_export]
macro_rules! with_media_list_store {
    ($ctx:expr, $ui:expr, $store:ident, $body:expr) => {
        match $ctx {
            $crate::logic::video_editor::media_list_common::MediaListContext::Library => {
                let _model_rc = $crate::global_store!($ui).get_video_editor_media_library();
                let $store = _model_rc.as_any().downcast_ref::<slint::VecModel<$crate::slint_generatedAppWindow::VideoEditorPlaylistItem>>().expect("We know we set a VecModel earlier");
                $body
            }
            $crate::logic::video_editor::media_list_common::MediaListContext::Playlist => {
                let _model_rc = $crate::global_store!($ui).get_video_editor_playlist();
                let $store = _model_rc.as_any().downcast_ref::<slint::VecModel<$crate::slint_generatedAppWindow::VideoEditorPlaylistItem>>().expect("We know we set a VecModel earlier");
                $body
            }
        }
    };
}

impl MediaListContext {
    pub fn get_media_list<'a>(&self, state: &'a ProjectState) -> &'a MediaList {
        match self {
            MediaListContext::Library => &state.library,
            MediaListContext::Playlist => &state.playlist,
        }
    }

    pub fn get_media_list_mut<'a>(&self, state: &'a mut ProjectState) -> &'a mut MediaList {
        match self {
            MediaListContext::Library => &mut state.library,
            MediaListContext::Playlist => &mut state.playlist,
        }
    }

    pub fn get_current_folder_id(&self, ui: &AppWindow) -> Option<String> {
        let id_str = match self {
            MediaListContext::Library => {
                global_store!(ui).get_video_editor_media_library_current_folder_id()
            }
            MediaListContext::Playlist => {
                global_store!(ui).get_video_editor_playlist_current_folder_id()
            }
        };
        if id_str.is_empty() {
            None
        } else {
            Some(id_str.to_string())
        }
    }

    pub fn set_current_folder_id(&self, ui: &AppWindow, id: SharedString) {
        match self {
            MediaListContext::Library => {
                global_store!(ui).set_video_editor_media_library_current_folder_id(id)
            }
            MediaListContext::Playlist => {
                global_store!(ui).set_video_editor_playlist_current_folder_id(id)
            }
        }
    }

    pub fn set_breadcrumb(&self, ui: &AppWindow, items: Vec<UIVideoEditorPlaylistItem>) {
        let model = ModelRc::new(VecModel::from(items));
        match self {
            MediaListContext::Library => {
                global_store!(ui).set_video_editor_media_library_breadcrumb(model)
            }
            MediaListContext::Playlist => {
                global_store!(ui).set_video_editor_playlist_breadcrumb(model)
            }
        }
    }

    pub fn get_in_edit_mode(&self, ui: &AppWindow) -> bool {
        match self {
            MediaListContext::Library => {
                global_store!(ui).get_video_editor_media_library_in_edit_mode()
            }
            MediaListContext::Playlist => {
                global_store!(ui).get_video_editor_playlist_in_edit_mode()
            }
        }
    }

    pub fn set_in_edit_mode(&self, ui: &AppWindow, val: bool) {
        match self {
            MediaListContext::Library => {
                global_store!(ui).set_video_editor_media_library_in_edit_mode(val)
            }
            MediaListContext::Playlist => {
                global_store!(ui).set_video_editor_playlist_in_edit_mode(val)
            }
        }
    }

    pub fn get_selection_anchor(&self, ui: &AppWindow) -> i32 {
        match self {
            MediaListContext::Library => {
                global_store!(ui).get_video_editor_media_library_selection_anchor()
            }
            MediaListContext::Playlist => {
                global_store!(ui).get_video_editor_playlist_selection_anchor()
            }
        }
    }

    pub fn set_selection_anchor(&self, ui: &AppWindow, val: i32) {
        match self {
            MediaListContext::Library => {
                global_store!(ui).set_video_editor_media_library_selection_anchor(val)
            }
            MediaListContext::Playlist => {
                global_store!(ui).set_video_editor_playlist_selection_anchor(val)
            }
        }
    }

    pub fn set_selected_count(&self, ui: &AppWindow, val: i32) {
        match self {
            MediaListContext::Library => {
                global_store!(ui).set_video_editor_media_library_selected_count(val)
            }
            MediaListContext::Playlist => {
                global_store!(ui).set_video_editor_playlist_selected_count(val)
            }
        }
    }

    pub fn get_is_searching(&self, ui: &AppWindow) -> bool {
        match self {
            MediaListContext::Library => {
                global_store!(ui).get_video_editor_media_library_is_searching()
            }
            MediaListContext::Playlist => {
                global_store!(ui).get_video_editor_playlist_is_searching()
            }
        }
    }

    pub fn get_search_keyword(&self, ui: &AppWindow) -> SharedString {
        match self {
            MediaListContext::Library => {
                global_store!(ui).get_video_editor_media_library_search_keyword()
            }
            MediaListContext::Playlist => {
                global_store!(ui).get_video_editor_playlist_search_keyword()
            }
        }
    }

    pub fn get_checked_index(&self, ui: &AppWindow) -> i32 {
        match self {
            MediaListContext::Library => {
                global_store!(ui).get_video_editor_media_library_checked_index()
            }
            MediaListContext::Playlist => {
                global_store!(ui).get_video_editor_playlist_checked_index()
            }
        }
    }

    /// Save the media list. Library saves to DB; Playlist is saved with the project file.
    pub fn save(&self, ui_weak: Weak<AppWindow>) {
        match self {
            MediaListContext::Library => save_library_to_db(ui_weak),
            MediaListContext::Playlist => {
                // Playlist is saved as part of the project file, no separate save needed
            }
        }
    }

    /// Label used in toast messages (e.g. "from library" / "from playlist")
    fn label(&self) -> &'static str {
        match self {
            MediaListContext::Library => "library",
            MediaListContext::Playlist => "playlist",
        }
    }
}

pub fn update_breadcrumb(ctx: MediaListContext, ui: &AppWindow) {
    let current_folder_id = ctx.get_current_folder_id(ui);

    let state = PROJECT_STATE.lock().unwrap();
    let breadcrumb: Vec<UIVideoEditorPlaylistItem> = if let Some(ref fid) = current_folder_id {
        if let Some(ref s) = *state {
            ctx.get_media_list(s)
                .folder_path(fid)
                .into_iter()
                .map(|f| f.clone().into())
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };
    drop(state);

    ctx.set_breadcrumb(ui, breadcrumb);
}

pub fn navigate_to_parent(ctx: MediaListContext, ui: &AppWindow) {
    let current_folder_id = ctx.get_current_folder_id(ui);

    let parent_id = {
        let state = PROJECT_STATE.lock().unwrap();
        if let Some(ref s) = *state {
            if let Some(ref fid) = current_folder_id {
                ctx.get_media_list(s)
                    .get_node(fid)
                    .and_then(|n| n.as_folder())
                    .and_then(|f| f.parent_id.clone())
            } else {
                None
            }
        } else {
            None
        }
    };

    let new_id_str = parent_id.map(|id| id.into()).unwrap_or_default();
    ctx.set_current_folder_id(ui, new_id_str);
    update_breadcrumb(ctx, ui);
    refresh_ui_filtered(ctx, ui);
    clear_selection(ctx, ui);
}

pub fn enter_edit_mode(ctx: MediaListContext, ui: &AppWindow) {
    ctx.set_in_edit_mode(ui, true);
}

pub fn exit_edit_mode(ctx: MediaListContext, ui: &AppWindow) {
    ctx.set_in_edit_mode(ui, false);
    ctx.set_selection_anchor(ui, -1);

    with_media_list_store!(ctx, ui, store, {
        let count = store.row_count();
        for i in 0..count {
            if let Some(mut item) = store.row_data(i)
                && item.is_selected
            {
                item.is_selected = false;
                store.set_row_data(i, item);
            }
        }
    });
}

pub fn toggle_select_all(ctx: MediaListContext, ui: &AppWindow, selected: bool) {
    with_media_list_store!(ctx, ui, store, {
        let count = store.row_count();
        for i in 0..count {
            if let Some(mut item) = store.row_data(i) {
                if item.item_id == ".." {
                    continue;
                }
                if selected && !item.is_selected {
                    item.is_selected = true;
                } else if !selected && item.is_selected {
                    item.is_selected = false;
                } else {
                    continue;
                }
                store.set_row_data(i, item);
            }
        }
    });
}

pub fn remove_all(ctx: MediaListContext, ui: &AppWindow) {
    {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            ctx.get_media_list_mut(s).clear();
        }
    }

    with_media_list_store!(ctx, ui, store, {
        store.set_vec(vec![]);
    });
    clear_selection(ctx, ui);
    ctx.set_in_edit_mode(ui, false);
    ctx.save(ui.as_weak());
    crate::toast_success!(
        ui,
        if matches!(ctx, MediaListContext::Library) {
            tr("Library cleared")
        } else {
            tr("Playlist cleared")
        }
    );
}

pub fn remove_selections(ctx: MediaListContext, ui: &AppWindow) {
    let (item_ids, folder_ids) = with_media_list_store!(ctx, ui, store, {
        let count = store.row_count();
        let selected: Vec<UIVideoEditorPlaylistItem> = (0..count)
            .filter_map(|i| {
                store
                    .row_data(i)
                    .and_then(|item| if item.is_selected { Some(item) } else { None })
            })
            .collect();

        let item_ids: Vec<String> = selected
            .iter()
            .filter(|item| !item.is_folder)
            .filter_map(|item| {
                let id = item.item_id.to_string();
                if id.is_empty() { None } else { Some(id) }
            })
            .collect();

        let folder_ids: Vec<String> = selected
            .iter()
            .filter(|item| item.is_folder)
            .filter_map(|item| {
                let id = item.item_id.to_string();
                if id.is_empty() { None } else { Some(id) }
            })
            .collect();

        (item_ids, folder_ids)
    });

    {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            let list = ctx.get_media_list_mut(s);
            for id in &folder_ids {
                _ = list.remove_node(id);
            }
            for id in &item_ids {
                _ = list.remove_node(id);
            }
        }
    }

    refresh_ui_filtered(ctx, ui);
    clear_selection(ctx, ui);
    ctx.set_in_edit_mode(ui, false);
    ctx.save(ui.as_weak());

    let removed_count = item_ids.len() + folder_ids.len();
    if removed_count > 0 {
        crate::toast_success!(
            ui,
            format!(
                "{} {} {}",
                tr("Removed"),
                removed_count,
                if matches!(ctx, MediaListContext::Library) {
                    tr("item(s) from library")
                } else {
                    tr("item(s) from playlist")
                }
            )
        );
    }
}

pub fn toggle_selection(ctx: MediaListContext, ui: &AppWindow, index: i32) {
    let idx = index as usize;
    let modifiers = global_store!(ui).get_video_editor_keyboard_modifiers();
    let current_anchor = ctx.get_selection_anchor(ui);
    let in_edit_mode = ctx.get_in_edit_mode(ui);

    with_media_list_store!(ctx, ui, store, {
        let count = store.row_count();

        // Prevent selecting the ".." parent navigation entry
        if idx < count
            && store
                .row_data(idx)
                .map(|item| item.item_id == "..")
                .unwrap_or(false)
        {
            return;
        }

        if modifiers.shift && current_anchor >= 0 && current_anchor < count as i32 {
            let anchor_idx = current_anchor as usize;
            let start = std::cmp::min(anchor_idx, idx);
            let end = std::cmp::max(anchor_idx, idx);

            for i in start..=end {
                if let Some(mut item) = store.row_data(i) {
                    if item.item_id == ".." {
                        continue;
                    }
                    item.is_selected = true;
                    store.set_row_data(i, item);
                }
            }
        } else if modifiers.control || in_edit_mode {
            if idx < count
                && let Some(mut item) = store.row_data(idx)
            {
                item.is_selected = !item.is_selected;
                let is_selected = item.is_selected;
                store.set_row_data(idx, item);

                if is_selected {
                    ctx.set_selection_anchor(ui, index);
                }
            }
        } else {
            let is_currently_selected = idx < count
                && store
                    .row_data(idx)
                    .map(|item| item.is_selected)
                    .unwrap_or(false);

            if is_currently_selected {
                if let Some(mut item) = store.row_data(idx) {
                    item.is_selected = false;
                    store.set_row_data(idx, item);
                }
                ctx.set_selection_anchor(ui, -1);
            } else {
                for i in 0..count {
                    if i != idx
                        && let Some(mut item) = store.row_data(i)
                        && item.is_selected
                    {
                        item.is_selected = false;
                        store.set_row_data(i, item);
                    }
                }

                if idx < count
                    && let Some(mut item) = store.row_data(idx)
                {
                    item.is_selected = true;
                    store.set_row_data(idx, item);
                    ctx.set_selection_anchor(ui, index);
                }
            }
        }
    });

    update_selected_count(ctx, ui);
}

pub fn search(ctx: MediaListContext, ui: &AppWindow, index: i32, keyword: SharedString) {
    let keyword = keyword.to_string();

    if keyword.is_empty() {
        let media_type = match index {
            1 => Some(MediaType::Video),
            2 => Some(MediaType::Audio),
            3 => Some(MediaType::Image),
            4 if matches!(ctx, MediaListContext::Playlist) => Some(MediaType::Subtitle),
            _ => None,
        };
        filter_by_type(ctx, ui, media_type);
        return;
    }

    // With keyword: global search across all folders, files only, ignore type filter
    let state = PROJECT_STATE.lock().unwrap();
    if let Some(ref s) = *state {
        let list = ctx.get_media_list(s);

        let mut file_items: Vec<UIVideoEditorPlaylistItem> = list
            .search(&keyword)
            .into_iter()
            .map(|item| item.clone().into())
            .collect();

        drop(state);

        sort_ui_items(&mut file_items);

        with_media_list_store!(ctx, ui, store, {
            store.set_vec(file_items);
        });
    }

    // Clear selection because selected items may not be in the search results,
    // which would cause drag-to-add to fail.
    clear_selection(ctx, ui);
}

pub fn filter_by_type(ctx: MediaListContext, ui: &AppWindow, media_type: Option<MediaType>) {
    let current_folder_id = ctx.get_current_folder_id(ui);

    let state = PROJECT_STATE.lock().unwrap();
    if let Some(ref s) = *state {
        let list = ctx.get_media_list(s);

        let mut all_items: Vec<UIVideoEditorPlaylistItem> = list
            .folder_children(current_folder_id.as_deref())
            .into_iter()
            .map(|f| f.clone().into())
            .collect();

        let file_items: Vec<UIVideoEditorPlaylistItem> = if let Some(mt) = media_type {
            list.items_by_type_in_folder(mt, current_folder_id.as_deref())
                .into_iter()
                .map(|item| item.clone().into())
                .collect()
        } else {
            list.file_children(current_folder_id.as_deref())
                .into_iter()
                .map(|item| item.clone().into())
                .collect()
        };

        all_items.extend(file_items);
        drop(state);

        sort_ui_items(&mut all_items);
        let ui_items = with_parent_entry(current_folder_id.as_deref(), all_items);

        with_media_list_store!(ctx, ui, store, {
            store.set_vec(ui_items);
        });
    }

    // Clear selection because selected items may not be in the filtered results,
    // which would cause drag-to-add to fail.
    clear_selection(ctx, ui);
}

pub fn refresh_ui(ctx: MediaListContext, ui: &AppWindow) {
    let current_folder_id = ctx.get_current_folder_id(ui);

    let state = PROJECT_STATE.lock().unwrap();
    if let Some(ref s) = *state {
        let list = ctx.get_media_list(s);

        let mut all_items: Vec<UIVideoEditorPlaylistItem> = list
            .folder_children(current_folder_id.as_deref())
            .into_iter()
            .map(|f| f.clone().into())
            .collect();

        let file_items: Vec<UIVideoEditorPlaylistItem> = list
            .file_children(current_folder_id.as_deref())
            .into_iter()
            .map(|item| item.clone().into())
            .collect();

        all_items.extend(file_items);
        drop(state);

        sort_ui_items(&mut all_items);
        let ui_items = with_parent_entry(current_folder_id.as_deref(), all_items);

        with_media_list_store!(ctx, ui, store, {
            store.set_vec(ui_items);
        });
    }
}

pub fn refresh_ui_filtered(ctx: MediaListContext, ui: &AppWindow) {
    let is_searching = ctx.get_is_searching(ui);
    if is_searching {
        let keyword = ctx.get_search_keyword(ui).to_string();
        let checked_index = ctx.get_checked_index(ui);

        // No keyword: restore current folder view with type filter
        if keyword.is_empty() {
            let media_type = match checked_index {
                1 => Some(MediaType::Video),
                2 => Some(MediaType::Audio),
                3 => Some(MediaType::Image),
                4 if matches!(ctx, MediaListContext::Playlist) => Some(MediaType::Subtitle),
                _ => None,
            };
            filter_by_type(ctx, ui, media_type);
            return;
        }

        // Global search: search all files across all folders, skip folders
        let state = PROJECT_STATE.lock().unwrap();
        if let Some(ref s) = *state {
            let list = ctx.get_media_list(s);

            let mut file_items: Vec<UIVideoEditorPlaylistItem> = list
                .search(&keyword)
                .into_iter()
                .map(|item| item.clone().into())
                .collect();

            drop(state);

            sort_ui_items(&mut file_items);

            with_media_list_store!(ctx, ui, store, {
                store.set_vec(file_items);
            });
        }
    } else {
        let checked_index = ctx.get_checked_index(ui);
        let media_type = match checked_index {
            1 => Some(MediaType::Video),
            2 => Some(MediaType::Audio),
            3 => Some(MediaType::Image),
            4 if matches!(ctx, MediaListContext::Playlist) => Some(MediaType::Subtitle),
            _ => None,
        };
        filter_by_type(ctx, ui, media_type);
    }
}

pub fn item_copy_path(ctx: MediaListContext, ui: &AppWindow, index: i32) {
    with_media_list_store!(ctx, ui, store, {
        let Some(item) = store.row_data(index as usize) else {
            log::warn!("invalid {} index: {index}", ctx.label());
            return;
        };

        global_logic!(ui).invoke_copy_to_clipboard(item.file_path.clone());
    });
}

pub fn item_open_directory(ctx: MediaListContext, ui: &AppWindow, index: i32) {
    let file_path_str = with_media_list_store!(ctx, ui, store, {
        let Some(item) = store.row_data(index as usize) else {
            log::warn!("invalid {} index: {index}", ctx.label());
            return;
        };
        item.file_path.clone()
    });

    let file_path = Path::new(file_path_str.as_str());
    log::debug!("open {}", file_path.display());

    let Some(parent) = file_path.parent() else {
        crate::toast_warn!(
            ui,
            format!(
                "{}: {}",
                tr("Cannot find parent directory"),
                file_path.display()
            )
        );
        return;
    };

    if let Err(e) = open::that_detached(&parent) {
        crate::toast_warn!(
            ui,
            format!("{}: {e}. {}", tr("Open directory failed"), parent.display())
        );
    }
}

pub fn clear_selection(ctx: MediaListContext, ui: &AppWindow) {
    with_media_list_store!(ctx, ui, store, {
        let count = store.row_count();
        for i in 0..count {
            if let Some(mut item) = store.row_data(i)
                && item.is_selected
            {
                item.is_selected = false;
                store.set_row_data(i, item);
            }
        }
    });
    ctx.set_selection_anchor(ui, -1);
    ctx.set_selected_count(ui, 0);
}

pub fn update_selected_count(ctx: MediaListContext, ui: &AppWindow) {
    let selected_count = with_media_list_store!(ctx, ui, store, {
        let count = store.row_count();
        (0..count)
            .filter(|&i| {
                store
                    .row_data(i)
                    .map(|item| item.is_selected)
                    .unwrap_or(false)
            })
            .count() as i32
    });
    ctx.set_selected_count(ui, selected_count);
}

pub fn create_folder(ctx: MediaListContext, ui: &AppWindow) {
    let action = match ctx {
        MediaListContext::Library => "video-editor-library-create-folder",
        MediaListContext::Playlist => "video-editor-playlist-create-folder",
    };
    let context_str = match ctx {
        MediaListContext::Library => "library",
        MediaListContext::Playlist => "playlist",
    };
    let current_folder_id = ctx.get_current_folder_id(ui);
    global_store!(ui).set_video_editor_rename_dialog_title(tr("New Folder").into());
    global_store!(ui).set_video_editor_rename_dialog_text(SharedString::default());
    global_store!(ui).set_video_editor_rename_dialog_action(action.into());
    global_store!(ui)
        .set_video_editor_rename_dialog_id(current_folder_id.unwrap_or_default().into());
    global_store!(ui).set_video_editor_rename_dialog_context(context_str.into());
    global_store!(ui).set_video_editor_is_show_rename_dialog(true);
}

pub fn rename_folder(ctx: MediaListContext, ui: &AppWindow, id: SharedString) {
    let folder_id = id.to_string();
    let action = match ctx {
        MediaListContext::Library => "video-editor-library-rename-folder",
        MediaListContext::Playlist => "video-editor-playlist-rename-folder",
    };
    let context_str = match ctx {
        MediaListContext::Library => "library",
        MediaListContext::Playlist => "playlist",
    };

    // Get current folder name for pre-filling
    let current_name = {
        let state = PROJECT_STATE.lock().unwrap();
        if let Some(ref s) = *state {
            ctx.get_media_list(s)
                .get_node(&folder_id)
                .and_then(|n| n.as_folder())
                .map(|f| f.name.clone())
        } else {
            None
        }
    };

    global_store!(ui).set_video_editor_rename_dialog_title(tr("Rename").into());
    global_store!(ui).set_video_editor_rename_dialog_text(current_name.unwrap_or_default().into());
    global_store!(ui).set_video_editor_rename_dialog_action(action.into());
    global_store!(ui).set_video_editor_rename_dialog_id(folder_id.into());
    global_store!(ui).set_video_editor_rename_dialog_context(context_str.into());
    global_store!(ui).set_video_editor_is_show_rename_dialog(true);
}

pub fn rename_dialog_confirmed(ui: &AppWindow, text: SharedString) {
    let context_str = global_store!(ui)
        .get_video_editor_rename_dialog_context()
        .to_string();
    let action = global_store!(ui)
        .get_video_editor_rename_dialog_action()
        .to_string();
    let id = global_store!(ui)
        .get_video_editor_rename_dialog_id()
        .to_string();
    let name = text.to_string();

    let ctx = match context_str.as_str() {
        "library" => MediaListContext::Library,
        "playlist" => MediaListContext::Playlist,
        _ => {
            log::warn!("Unknown rename dialog context: {}", context_str);
            return;
        }
    };

    if name.is_empty() {
        return;
    }

    match action.as_str() {
        a if a.ends_with("create-folder") => {
            let parent_id = if id.is_empty() {
                None
            } else {
                Some(id.clone())
            };

            // Check for duplicate folder name
            let has_duplicate = {
                let state = PROJECT_STATE.lock().unwrap();
                if let Some(ref s) = *state {
                    ctx.get_media_list(s)
                        .folder_children(parent_id.as_deref())
                        .iter()
                        .any(|f| f.name == name)
                } else {
                    false
                }
            };

            if has_duplicate {
                crate::toast_warn!(ui, format!("{}: {}", tr("Folder already exists"), name));
                return;
            }

            let result = {
                let mut state = PROJECT_STATE.lock().unwrap();
                if let Some(ref mut s) = *state {
                    ctx.get_media_list_mut(s)
                        .create_folder(name.clone(), parent_id)
                } else {
                    Err(video_editor::Error::InvalidConfig(
                        "Project state not initialized".to_string(),
                    ))
                }
            };

            match result {
                Ok(_) => {
                    refresh_ui_filtered(ctx, ui);
                    ctx.save(ui.as_weak());
                    crate::toast_success!(ui, format!("{}: {}", tr("Created folder"), name));
                }
                Err(e) => {
                    crate::toast_warn!(ui, format!("{}: {}", tr("Failed to create folder"), e));
                }
            }
        }
        a if a.ends_with("rename-folder") => {
            let result = {
                let mut state = PROJECT_STATE.lock().unwrap();
                if let Some(ref mut s) = *state {
                    ctx.get_media_list_mut(s).rename_folder(&id, name.clone())
                } else {
                    Err(video_editor::Error::InvalidConfig(
                        "Project state not initialized".to_string(),
                    ))
                }
            };

            match result {
                Ok(_) => {
                    refresh_ui_filtered(ctx, ui);
                    update_breadcrumb(ctx, ui);
                    ctx.save(ui.as_weak());
                    crate::toast_success!(ui, format!("{}: {}", tr("Renamed folder"), name));
                }
                Err(e) => {
                    crate::toast_warn!(ui, format!("{}: {}", tr("Failed to rename folder"), e));
                }
            }
        }
        _ => {
            log::warn!("Unknown rename dialog action: {}", action);
        }
    }
}

pub fn remove_folder(ctx: MediaListContext, ui: &AppWindow, id: SharedString) {
    let folder_id = id.to_string();
    let handle_type = match ctx {
        MediaListContext::Library => "video-editor-library-remove-folder",
        MediaListContext::Playlist => "video-editor-playlist-remove-folder",
    };
    ui.global::<UIConfirmDialogSetting>().invoke_set(
        true,
        tr("Warning").into(),
        tr("Remove folder and all its contents?").into(),
        handle_type.into(),
        folder_id.into(),
    );
}

/// Called when the user confirms folder removal via ConfirmDialog.
pub fn remove_folder_confirmed(ctx: MediaListContext, ui: &AppWindow, folder_id: &str) {
    let current_folder_id = ctx.get_current_folder_id(ui);

    let result = {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            ctx.get_media_list_mut(s).remove_node(folder_id).map(|_| ())
        } else {
            Err(video_editor::Error::InvalidConfig(
                "Project state not initialized".to_string(),
            ))
        }
    };

    match result {
        Ok(_) => {
            refresh_ui_filtered(ctx, ui);
            clear_selection(ctx, ui);
            if current_folder_id.as_deref() == Some(folder_id) {
                navigate_to_parent(ctx, ui);
            }
            ctx.save(ui.as_weak());
            crate::toast_success!(ui, tr("Folder removed"));
        }
        Err(e) => {
            crate::toast_warn!(ui, format!("{}: {}", tr("Failed to remove folder"), e));
        }
    }
}

pub fn navigate_to_folder(ctx: MediaListContext, ui: &AppWindow, id: SharedString) {
    ctx.set_current_folder_id(ui, id);
    update_breadcrumb(ctx, ui);
    refresh_ui_filtered(ctx, ui);
    clear_selection(ctx, ui);
}

pub fn navigate_up(ctx: MediaListContext, ui: &AppWindow) {
    navigate_to_parent(ctx, ui);
}

/// Resolve a folder_id from the UI into the actual target folder ID.
/// Handles: "" → None (root), ".." → parent of current folder, otherwise → Some(id).
fn resolve_folder_target(
    ctx: MediaListContext,
    ui: &AppWindow,
    folder_id: &SharedString,
) -> Option<String> {
    if folder_id.is_empty() {
        None
    } else if folder_id == ".." {
        let current_folder_id = ctx.get_current_folder_id(ui);
        let state = PROJECT_STATE.lock().unwrap();
        if let Some(ref s) = *state {
            if let Some(ref cid) = current_folder_id {
                ctx.get_media_list(s)
                    .get_node(cid)
                    .and_then(|n| n.as_folder())
                    .and_then(|f| f.parent_id.clone())
            } else {
                None // Already at root
            }
        } else {
            None
        }
    } else {
        Some(folder_id.to_string())
    }
}

/// Move all selected items (both files and folders) into the target folder
pub fn move_to_folder(
    ctx: MediaListContext,
    ui: &AppWindow,
    dragged_item_id: SharedString,
    target_folder_id: SharedString,
) {
    let target = resolve_folder_target(ctx, ui, &target_folder_id);

    // Collect all selected item IDs (both files and folders), excluding ".." and the target folder itself
    let selected_ids: Vec<String> = with_media_list_store!(ctx, ui, store, {
        let count = store.row_count();
        let mut ids: Vec<String> = Vec::new();
        for i in 0..count {
            if let Some(item) = store.row_data(i)
                && item.is_selected
                && item.item_id != ".."
                && item.item_id != target_folder_id
            {
                ids.push(item.item_id.to_string());
            }
        }
        ids
    });

    // If no items are selected (or the dragged item isn't in the selection), include the dragged item
    let ids_to_move: Vec<String> = if selected_ids.is_empty() {
        vec![dragged_item_id.to_string()]
    } else {
        selected_ids
    };

    log::debug!("move_to_folder: ids={:?}, target={:?}", ids_to_move, target);

    let mut any_moved = false;
    let mut any_folder_moved = false;
    for id in &ids_to_move {
        let result = {
            let mut state = PROJECT_STATE.lock().unwrap();
            if let Some(ref mut s) = *state {
                let is_folder = ctx
                    .get_media_list(s)
                    .get_node(id)
                    .map(|n| n.is_folder())
                    .unwrap_or(false);
                let move_result = ctx.get_media_list_mut(s).move_node(id, target.clone());
                if move_result.is_ok() && is_folder {
                    any_folder_moved = true;
                }
                move_result
            } else {
                Err(video_editor::Error::InvalidConfig(
                    "Project state not initialized".to_string(),
                ))
            }
        };
        match result {
            Ok(_) => any_moved = true,
            Err(e) => log::warn!("Failed to move item {}: {}", id, e),
        }
    }

    if any_moved {
        refresh_ui_filtered(ctx, ui);
        if any_folder_moved {
            update_breadcrumb(ctx, ui);
        }
        ctx.save(ui.as_weak());
    }
}

pub fn move_item_to_folder(
    ctx: MediaListContext,
    ui: &AppWindow,
    item_id: SharedString,
    folder_id: SharedString,
) {
    move_to_folder(ctx, ui, item_id, folder_id);
}

pub fn move_folder(
    ctx: MediaListContext,
    ui: &AppWindow,
    folder_id: SharedString,
    new_parent_id: SharedString,
) {
    move_to_folder(ctx, ui, folder_id, new_parent_id);
}

pub fn item_preview(ctx: MediaListContext, ui: &AppWindow, index: i32) {
    let item = with_media_list_store!(ctx, ui, store, store.row_data(index as usize));

    let Some(item) = item else {
        log::warn!("invalid {} index: {index}", ctx.label());
        return;
    };

    global_store!(ui).set_video_editor_preview_item(item);
    global_store!(ui).set_video_editor_is_show_item_preview_dialog(true);
    show_preview_item(ui);
}

pub fn item_remove(ctx: MediaListContext, ui: &AppWindow, index: i32) {
    // If there are selected items, remove all selected items instead of just the one at index
    let selected_count = with_media_list_store!(ctx, ui, store, {
        let count = store.row_count();
        (0..count)
            .filter(|&i| {
                store
                    .row_data(i)
                    .map(|item| item.is_selected)
                    .unwrap_or(false)
            })
            .count()
    });

    if selected_count > 0 {
        remove_selections(ctx, ui);
        return;
    }

    // 没有选中文件，移除当前文件
    let ui_item = with_media_list_store!(ctx, ui, store, store.row_data(index as usize));
    let Some(ui_item) = ui_item else {
        crate::toast_warn!(ui, tr("Invalid item index"));
        return;
    };

    // Handle folder removal
    if ui_item.is_folder {
        remove_folder(ctx, ui, ui_item.item_id);
        return;
    }

    let item_id = ui_item.item_id.to_string();

    let mut state = PROJECT_STATE.lock().unwrap();
    if let Some(ref mut s) = *state {
        if let Ok(Some(removed_node)) = ctx.get_media_list_mut(s).remove_node(&item_id) {
            let name = match &removed_node {
                LibraryNode::File(item) => item.name.clone(),
                LibraryNode::Folder(folder) => folder.name.clone(),
            };
            drop(state);

            refresh_ui_filtered(ctx, ui);
            clear_selection(ctx, ui);
            ctx.save(ui.as_weak());
            crate::toast_success!(
                ui,
                format!(
                    "{} {} {}",
                    tr("Removed"),
                    name,
                    if matches!(ctx, MediaListContext::Library) {
                        tr("from library")
                    } else {
                        tr("from playlist")
                    }
                )
            );
            return;
        }
    }

    crate::toast_warn!(ui, tr("Failed to remove item"));
}

pub fn toggle_mark(ctx: MediaListContext, ui: &AppWindow, index: i32) {
    let idx = index as usize;

    with_media_list_store!(ctx, ui, store, {
        let count = store.row_count();

        if idx < count
            && let Some(mut item) = store.row_data(idx)
        {
            let item_id = item.item_id.to_string();
            item.is_marked = !item.is_marked;
            let is_marked = item.is_marked;
            store.set_row_data(idx, item);

            // Need to reorder
            let mut items: Vec<UIVideoEditorPlaylistItem> = store.iter().collect();
            // Separate ".." entry if present
            let has_parent = !items.is_empty() && items[0].item_id == "..";
            let parent_entry = if has_parent {
                Some(items.remove(0))
            } else {
                None
            };
            sort_ui_items(&mut items);
            if let Some(pe) = parent_entry {
                items.insert(0, pe);
            }
            store.set_vec(items);

            if let Some(ref mut s) = PROJECT_STATE.lock().unwrap().as_mut() {
                let list = ctx.get_media_list_mut(s);
                if let Some(backend_item) =
                    list.get_node_mut(&item_id).and_then(|n| n.as_file_mut())
                {
                    backend_item.is_marked = is_marked;
                    ctx.save(ui.as_weak());
                } else if let Some(backend_folder) =
                    list.get_node_mut(&item_id).and_then(|n| n.as_folder_mut())
                {
                    backend_folder.is_marked = is_marked;
                    ctx.save(ui.as_weak());
                }
            }
        }
    });
}

pub fn refresh_media_list(ctx: MediaListContext, ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    crate::toast_info!(ui, tr("Refreshing files, this may take a while..."));

    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            let mut state = PROJECT_STATE.lock().unwrap();
            if let Some(ref mut s) = *state {
                let list = ctx.get_media_list_mut(s);

                // 1. Remove file items whose file_path no longer exists
                let file_ids_to_remove: Vec<String> = list
                    .items()
                    .iter()
                    .filter(|item| !item.file_path.exists())
                    .map(|item| item.id.clone())
                    .collect();

                let mut removed_count = 0;
                for id in &file_ids_to_remove {
                    if list.remove_node(id).is_ok() {
                        removed_count += 1;
                    }
                }

                // 2. Remove folder items whose source_path no longer exists (only folders with source_path)
                let folder_ids_to_remove: Vec<String> = list
                    .all_folders()
                    .iter()
                    .filter(|f| f.source_path.as_ref().is_some_and(|sp| !sp.exists()))
                    .map(|f| f.id.clone())
                    .collect();

                let mut folder_removed_count = 0;
                for id in &folder_ids_to_remove {
                    if list.remove_node(id).is_ok() {
                        folder_removed_count += 1;
                    }
                }

                // 3. Refresh thumbnails for all remaining file items
                list.refresh_thumbnails();

                // 4. Update status for all remaining items
                list.check_all_status();

                (removed_count, folder_removed_count)
            } else {
                (0, 0)
            }
        })
        .await
        .unwrap_or((0, 0));

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let (removed_count, folder_removed_count) = result;
            let total = removed_count + folder_removed_count;

            refresh_ui_filtered(ctx, &ui);
            ctx.save(ui.as_weak());

            if total > 0 {
                crate::toast_success!(
                    ui,
                    format!(
                        "{}: {} {}",
                        tr("Refreshed"),
                        total,
                        tr("invalid item(s) removed")
                    )
                );
            } else {
                crate::toast_success!(ui, tr("Already up to date"));
            }
        });
    });
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
            crate::logic::toast::async_toast_warn(
                ui_weak,
                format!("{}: {}", tr("Failed to save library"), e),
            );
        }
    });
}

/// Create the ".." pseudo-entry for navigating to parent folder.
fn make_parent_entry() -> UIVideoEditorPlaylistItem {
    UIVideoEditorPlaylistItem {
        file_path: SharedString::default(),
        name: SharedString::from(".."),
        media_type: UIMediaType::Video, // placeholder, not meaningful
        duration: SharedString::default(),
        file_size: SharedString::default(),
        thumbnail: Image::default(),
        is_selected: false,
        is_marked: false,
        is_folder: true,
        folder_id: SharedString::default(),
        item_id: SharedString::from(".."),
        folder_source_path: SharedString::default(),
    }
}

/// Sort key for media type: Subtitle(0) > Video(1) > Image(2) > Audio(3).
fn media_type_sort_key(mt: &UIMediaType) -> u8 {
    match mt {
        UIMediaType::Subtitle => 0,
        UIMediaType::Video => 1,
        UIMediaType::Image => 2,
        UIMediaType::Audio => 3,
        UIMediaType::Text => 4,
    }
}

/// Sort items: marked > unmarked, folder > file, by media type priority, then by name.
/// The ".." entry is NOT affected by sorting — it must be placed at the front separately.
fn sort_ui_items(items: &mut Vec<UIVideoEditorPlaylistItem>) {
    items.sort_by(|a, b| {
        b.is_marked
            .cmp(&a.is_marked)
            .then_with(|| b.is_folder.cmp(&a.is_folder))
            .then_with(|| {
                if !a.is_folder && !b.is_folder {
                    media_type_sort_key(&a.media_type).cmp(&media_type_sort_key(&b.media_type))
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// Prepend `..` entry if inside a folder. Returns the final list.
fn with_parent_entry(
    current_folder_id: Option<&str>,
    mut items: Vec<UIVideoEditorPlaylistItem>,
) -> Vec<UIVideoEditorPlaylistItem> {
    if current_folder_id.is_some() {
        let mut result = vec![make_parent_entry()];
        result.append(&mut items);
        result
    } else {
        items
    }
}
