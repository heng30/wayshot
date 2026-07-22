use crate::{
    db::{TimerGlobalFilterConfigData, TimerStyleData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::video_editor::{
        command::refresh_preview,
        filters::global::save_global_filter_config,
        project::{PROJECT_STATE, TIMER_STYLE_DEFAULT_ID},
    },
    logic_cb, logic_cb_pure,
    slint_generatedAppWindow::{
        AppWindow, TimerItem as UITimerItem, TimerMode as UITimerMode, TimerStyle as UITimerStyle,
    },
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::sync::Arc;
use video_editor::filters::{global::TimerFilter, traits::GlobalFilterWrapper};

#[macro_export]
macro_rules! store_video_editor_global_filter_timer_items {
    ($ui:expr) => {
        crate::global_store!($ui)
            .get_video_editor_global_filter_config()
            .timer
            .items
            .as_any()
            .downcast_ref::<VecModel<UITimerItem>>()
            .expect("We know we set a VecModel<UITimerItem> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_global_filter_timer_add_item, ui);
    logic_cb!(video_editor_global_filter_timer_remove_item, ui, index);
    logic_cb!(video_editor_global_filter_timer_insert_item, ui, index);
    logic_cb!(
        video_editor_global_filter_timer_move_item,
        ui,
        from_index,
        to_index
    );
    logic_cb!(video_editor_global_filter_timer_remove_all_items, ui);
    logic_cb!(
        video_editor_global_filter_timer_update_offsets,
        ui,
        index,
        start_offset,
        end_offset
    );
    logic_cb!(
        video_editor_global_filter_timer_update_mode,
        ui,
        index,
        mode
    );
    logic_cb!(
        video_editor_global_filter_timer_update_style,
        ui,
        index,
        style
    );
    logic_cb!(video_editor_global_filter_timer_toggle_enable, ui);
    logic_cb_pure!(video_editor_global_filter_timer_is_valid, ui, index, flag);
}

fn inner_init(ui: &AppWindow) {
    load_timer_style_from_db(ui);
}

pub fn init_from_project(ui: &AppWindow) {
    clear_ui_state(ui);

    let state = PROJECT_STATE.lock().unwrap();
    if let Some(ref s) = *state {
        let timer_filter = s
            .tracks_manager
            .get_global_filters()
            .iter()
            .find(|f| f.inner.name() == TimerFilter::NAME);

        if let Some(filter) = timer_filter
            && let Some(timer) = filter.inner.as_any().downcast_ref::<TimerFilter>()
        {
            let timer_config_data: TimerGlobalFilterConfigData = timer.clone().into();

            let mut config = global_store!(ui).get_video_editor_global_filter_config();
            config.timer = timer_config_data.into();
            config.timer.enabled = filter.enabled();

            global_store!(ui).set_video_editor_global_filter_config(config);
        }
    }
}

pub fn sync_timer_to_project_state(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_global_filter_config();
    let timer_config = &config.timer;
    let enabled = timer_config.enabled;

    {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            let filter_idx = s
                .tracks_manager
                .get_global_filters()
                .iter()
                .position(|f| f.inner.name() == TimerFilter::NAME);

            let new_filter: TimerFilter =
                TimerGlobalFilterConfigData::from(timer_config.clone()).into();

            let new_wrapper = GlobalFilterWrapper::new(enabled, Box::new(new_filter));
            if let Some(idx) = filter_idx {
                s.tracks_manager.global_filters[idx] = Arc::new(new_wrapper);
            } else {
                s.tracks_manager.add_global_filter(Arc::new(new_wrapper));
            }
        }
    }

    save_global_filter_config(ui);
    refresh_preview(ui);
}

fn video_editor_global_filter_timer_add_item(ui: &AppWindow) {
    let default_style = global_store!(ui)
        .get_video_editor_global_filter_config()
        .timer
        .default_style;

    let new_item = UITimerItem {
        start_offset: 0,
        end_offset: 0,
        mode: UITimerMode::CountUp,
        style: default_style,
    };

    store_video_editor_global_filter_timer_items!(ui).push(new_item);
    sync_timer_to_project_state(ui);
}

fn video_editor_global_filter_timer_remove_item(ui: &AppWindow, index: i32) {
    if index >= 0 && index < store_video_editor_global_filter_timer_items!(ui).row_count() as i32 {
        store_video_editor_global_filter_timer_items!(ui).remove(index as usize);
        sync_timer_to_project_state(ui);
    }
}

fn video_editor_global_filter_timer_insert_item(ui: &AppWindow, index: i32) {
    let insert_index =
        (index as usize + 1).min(store_video_editor_global_filter_timer_items!(ui).row_count());

    let default_style = global_store!(ui)
        .get_video_editor_global_filter_config()
        .timer
        .default_style;

    let new_item = UITimerItem {
        start_offset: 0,
        end_offset: 0,
        mode: UITimerMode::CountUp,
        style: default_style,
    };

    store_video_editor_global_filter_timer_items!(ui).insert(insert_index, new_item);
    sync_timer_to_project_state(ui);
}

fn video_editor_global_filter_timer_move_item(ui: &AppWindow, from_index: i32, to_index: i32) {
    let len = store_video_editor_global_filter_timer_items!(ui).row_count() as i32;

    if from_index >= 0 && from_index < len && to_index >= 0 && to_index < len {
        let item = store_video_editor_global_filter_timer_items!(ui).row_data(from_index as usize);
        if let Some(item) = item {
            store_video_editor_global_filter_timer_items!(ui).remove(from_index as usize);
            store_video_editor_global_filter_timer_items!(ui).insert(to_index as usize, item);
            sync_timer_to_project_state(ui);
        }
    }
}

fn video_editor_global_filter_timer_remove_all_items(ui: &AppWindow) {
    store_video_editor_global_filter_timer_items!(ui).set_vec(vec![]);
    sync_timer_to_project_state(ui);
}

fn video_editor_global_filter_timer_update_offsets(
    ui: &AppWindow,
    index: i32,
    start_offset: i32,
    end_offset: i32,
) {
    let count = store_video_editor_global_filter_timer_items!(ui).row_count() as i32;

    if index >= 0 && index < count {
        if let Some(item) =
            store_video_editor_global_filter_timer_items!(ui).row_data(index as usize)
        {
            let updated = UITimerItem {
                start_offset,
                end_offset,
                mode: item.mode,
                style: item.style,
            };
            store_video_editor_global_filter_timer_items!(ui).set_row_data(index as usize, updated);
            sync_timer_to_project_state(ui);
        }
    }
}

fn video_editor_global_filter_timer_update_mode(ui: &AppWindow, index: i32, mode: i32) {
    let count = store_video_editor_global_filter_timer_items!(ui).row_count() as i32;

    if index >= 0 && index < count {
        if let Some(item) =
            store_video_editor_global_filter_timer_items!(ui).row_data(index as usize)
        {
            let timer_mode = match mode {
                0 => UITimerMode::CountUp,
                1 => UITimerMode::CountDown,
                _ => UITimerMode::CountUp,
            };
            let updated = UITimerItem {
                start_offset: item.start_offset,
                end_offset: item.end_offset,
                mode: timer_mode,
                style: item.style,
            };
            store_video_editor_global_filter_timer_items!(ui).set_row_data(index as usize, updated);
            sync_timer_to_project_state(ui);
        }
    }
}

fn video_editor_global_filter_timer_update_style(ui: &AppWindow, index: i32, style: UITimerStyle) {
    let count = store_video_editor_global_filter_timer_items!(ui).row_count() as i32;

    if index >= 0 && index < count {
        if let Some(item) =
            store_video_editor_global_filter_timer_items!(ui).row_data(index as usize)
        {
            let updated = UITimerItem {
                start_offset: item.start_offset,
                end_offset: item.end_offset,
                mode: item.mode,
                style: style.clone(),
            };
            store_video_editor_global_filter_timer_items!(ui).set_row_data(index as usize, updated);

            save_timer_style_to_db(ui, style);
            sync_timer_to_project_state(ui);
        }
    }
}

fn video_editor_global_filter_timer_is_valid(ui: &AppWindow, index: i32, _flag: bool) -> bool {
    let len = store_video_editor_global_filter_timer_items!(ui).row_count() as i32;

    if index < 0 || index >= len {
        return false;
    }

    if let Some(current_item) =
        store_video_editor_global_filter_timer_items!(ui).row_data(index as usize)
    {
        if current_item.start_offset < 0 {
            return false;
        }

        if current_item.end_offset <= current_item.start_offset {
            return false;
        }

        return true;
    }

    false
}

fn video_editor_global_filter_timer_toggle_enable(ui: &AppWindow) {
    sync_timer_to_project_state(ui);
}

pub fn clear_ui_state(ui: &AppWindow) {
    let mut config = global_store!(ui).get_video_editor_global_filter_config();
    config.timer.items = ModelRc::new(VecModel::default());
    config.timer.enabled = false;
    global_store!(ui).set_video_editor_global_filter_config(config);
}

fn save_timer_style_to_db(ui: &AppWindow, style: UITimerStyle) {
    let mut config = global_store!(ui).get_video_editor_global_filter_config();
    config.timer.default_style = style.clone();
    global_store!(ui).set_video_editor_global_filter_config(config);

    let style_data: TimerStyleData = style.into();
    tokio::spawn(async move {
        let data = serde_json::to_string(&style_data).unwrap_or_default();
        if let Err(e) =
            sqldb::entry::update(VIDEO_EDITOR_TABLE, TIMER_STYLE_DEFAULT_ID, &data).await
        {
            log::warn!("Failed to save timer style to database: {}", e);
        }
    });
}

pub fn load_timer_style_from_db(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let style_data = match sqldb::entry::select(VIDEO_EDITOR_TABLE, TIMER_STYLE_DEFAULT_ID)
            .await
        {
            Ok(item) => serde_json::from_str::<TimerStyleData>(&item.data).unwrap_or_default(),
            Err(_) => {
                _ = sqldb::entry::insert(VIDEO_EDITOR_TABLE, TIMER_STYLE_DEFAULT_ID, "{}").await;
                TimerStyleData::default()
            }
        };

        let ui_style: UITimerStyle = style_data.into();
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let mut config = global_store!(ui).get_video_editor_global_filter_config();
            config.timer.default_style = ui_style;
            global_store!(ui).set_video_editor_global_filter_config(config);
        });
    });
}
