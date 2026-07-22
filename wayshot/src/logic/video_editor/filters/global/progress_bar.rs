use crate::{
    db::ProgressBarGlobalFilterConfigData,
    global_store,
    logic::video_editor::{
        command::refresh_preview, filters::global::save_global_filter_config,
        project::PROJECT_STATE,
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{AppWindow, ProgressBarItem as UIProgressBarItem},
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::sync::Arc;
use video_editor::filters::{global::ProgressBarFilter, traits::GlobalFilterWrapper};

#[macro_export]
macro_rules! store_video_editor_global_filter_progress_bar_items {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_global_filter_config()
            .progress_bar
            .items
            .as_any()
            .downcast_ref::<VecModel<UIProgressBarItem>>()
            .expect("We know we set a VecModel<UIProgressBarItem> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_global_filter_progress_bar_add_item, ui);
    logic_cb!(
        video_editor_global_filter_progress_bar_remove_item,
        ui,
        index
    );
    logic_cb!(
        video_editor_global_filter_progress_bar_insert_item,
        ui,
        index
    );
    logic_cb!(
        video_editor_global_filter_progress_bar_move_item,
        ui,
        from_index,
        to_index
    );
    logic_cb!(video_editor_global_filter_progress_bar_remove_all_items, ui);
    logic_cb!(
        video_editor_global_filter_progress_bar_update_text,
        ui,
        index,
        text
    );
    logic_cb!(
        video_editor_global_filter_progress_bar_update_timeline_offset,
        ui,
        index,
        timeline_offset
    );
    logic_cb!(video_editor_global_filter_progress_bar_toggle_enable, ui);
    logic_cb_pure!(
        video_editor_global_filter_progress_bar_is_valid,
        ui,
        index,
        flag
    );
}

pub fn init_from_project(ui: &AppWindow) {
    clear_ui_state(ui);

    let state = PROJECT_STATE.lock().unwrap();
    if let Some(ref s) = *state {
        let progress_bar_filter = s
            .tracks_manager
            .get_global_filters()
            .iter()
            .find(|f| f.inner.name() == ProgressBarFilter::NAME);

        if let Some(filter) = progress_bar_filter
            && let Some(pb) = filter.inner.as_any().downcast_ref::<ProgressBarFilter>()
        {
            let pb_config_data: ProgressBarGlobalFilterConfigData = pb.clone().into();

            let mut config = global_store!(ui).get_video_editor_global_filter_config();
            config.progress_bar = pb_config_data.into();
            config.progress_bar.enabled = filter.enabled();

            global_store!(ui).set_video_editor_global_filter_config(config);
        }
    }
}

pub fn sync_segments_to_project_state(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_global_filter_config();
    let pb_config = &config.progress_bar;
    let enabled = pb_config.enabled;

    {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            let filter_idx = s
                .tracks_manager
                .get_global_filters()
                .iter()
                .position(|f| f.inner.name() == ProgressBarFilter::NAME);

            let new_filter: ProgressBarFilter =
                ProgressBarGlobalFilterConfigData::from(pb_config.clone()).into();

            let new_wrapper = GlobalFilterWrapper::new(enabled, Box::new(new_filter));
            if let Some(idx) = filter_idx {
                s.tracks_manager.global_filters[idx] = Arc::new(new_wrapper);
            } else {
                s.tracks_manager.add_global_filter(Arc::new(new_wrapper));
            }
        }
    }

    save_global_filter_config(&ui);
    refresh_preview(ui);
}

fn video_editor_global_filter_progress_bar_add_item(ui: &AppWindow) {
    store_video_editor_global_filter_progress_bar_items!(ui).push(UIProgressBarItem::default());
    sync_segments_to_project_state(ui);
}

fn video_editor_global_filter_progress_bar_remove_item(ui: &AppWindow, index: i32) {
    if index >= 0
        && index < store_video_editor_global_filter_progress_bar_items!(ui).row_count() as i32
    {
        store_video_editor_global_filter_progress_bar_items!(ui).remove(index as usize);
        sync_segments_to_project_state(ui);
    }
}

fn video_editor_global_filter_progress_bar_insert_item(ui: &AppWindow, index: i32) {
    let insert_index = (index as usize + 1)
        .min(store_video_editor_global_filter_progress_bar_items!(ui).row_count());

    store_video_editor_global_filter_progress_bar_items!(ui)
        .insert(insert_index, UIProgressBarItem::default());
    sync_segments_to_project_state(ui);
}

fn video_editor_global_filter_progress_bar_move_item(
    ui: &AppWindow,
    from_index: i32,
    to_index: i32,
) {
    let len = store_video_editor_global_filter_progress_bar_items!(ui).row_count() as i32;

    if from_index >= 0 && from_index < len && to_index >= 0 && to_index < len {
        let item =
            store_video_editor_global_filter_progress_bar_items!(ui).row_data(from_index as usize);
        if let Some(item) = item {
            store_video_editor_global_filter_progress_bar_items!(ui).remove(from_index as usize);
            store_video_editor_global_filter_progress_bar_items!(ui)
                .insert(to_index as usize, item);
            sync_segments_to_project_state(ui);
        }
    }
}

fn video_editor_global_filter_progress_bar_remove_all_items(ui: &AppWindow) {
    store_video_editor_global_filter_progress_bar_items!(ui).set_vec(vec![]);
    sync_segments_to_project_state(ui);
}

fn video_editor_global_filter_progress_bar_update_text(
    ui: &AppWindow,
    index: i32,
    text: SharedString,
) {
    let count = store_video_editor_global_filter_progress_bar_items!(ui).row_count() as i32;

    if index >= 0 && index < count {
        if let Some(item) =
            store_video_editor_global_filter_progress_bar_items!(ui).row_data(index as usize)
        {
            let updated = UIProgressBarItem {
                text,
                timeline_offset: item.timeline_offset,
            };
            store_video_editor_global_filter_progress_bar_items!(ui)
                .set_row_data(index as usize, updated);
            sync_segments_to_project_state(ui);
        }
    }
}

fn video_editor_global_filter_progress_bar_update_timeline_offset(
    ui: &AppWindow,
    index: i32,
    timeline_offset: i32,
) {
    let count = store_video_editor_global_filter_progress_bar_items!(ui).row_count() as i32;

    if index >= 0 && index < count {
        if let Some(item) =
            store_video_editor_global_filter_progress_bar_items!(ui).row_data(index as usize)
        {
            let updated = UIProgressBarItem {
                text: item.text,
                timeline_offset,
            };
            store_video_editor_global_filter_progress_bar_items!(ui)
                .set_row_data(index as usize, updated);
            sync_segments_to_project_state(ui);
        }
    }
}

fn video_editor_global_filter_progress_bar_is_valid(
    ui: &AppWindow,
    index: i32,
    _flag: bool,
) -> bool {
    let len = store_video_editor_global_filter_progress_bar_items!(ui).row_count() as i32;

    if index < 0 || index >= len {
        return false;
    }

    if let Some(current_item) =
        store_video_editor_global_filter_progress_bar_items!(ui).row_data(index as usize)
    {
        if current_item.timeline_offset < 0 {
            return false;
        }

        if index > 0 {
            if let Some(prev_item) = store_video_editor_global_filter_progress_bar_items!(ui)
                .row_data(index as usize - 1)
                && current_item.timeline_offset <= prev_item.timeline_offset
            {
                return false;
            }
        }

        if index < len - 1 {
            if let Some(next_item) = store_video_editor_global_filter_progress_bar_items!(ui)
                .row_data(index as usize + 1)
                && current_item.timeline_offset >= next_item.timeline_offset
            {
                return false;
            }
        }

        return true;
    }

    false
}

fn video_editor_global_filter_progress_bar_toggle_enable(ui: &AppWindow) {
    sync_segments_to_project_state(ui);
}

pub fn clear_ui_state(ui: &AppWindow) {
    let mut config = global_store!(ui).get_video_editor_global_filter_config();
    config.progress_bar.items = ModelRc::new(VecModel::default());
    config.progress_bar.enabled = false;
    global_store!(ui).set_video_editor_global_filter_config(config);
}
