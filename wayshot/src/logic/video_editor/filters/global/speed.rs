use crate::{
    db::GlobalSpeedFilterConfigData,
    global_store,
    logic::video_editor::{
        command::sync_and_refresh_simple, filters::global::save_global_filter_config,
        project::PROJECT_STATE,
    },
    logic_cb,
    slint_generatedAppWindow::AppWindow,
};
use slint::ComponentHandle;
use std::sync::Arc;
use video_editor::{
    commands::segment::SetGlobalSpeedCommand,
    filters::{global::GlobalSpeedFilter, traits::GlobalFilterWrapper},
};

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_global_filter_speed_update, ui);
    logic_cb!(video_editor_global_filter_speed_toggle_enable, ui);
}

pub fn init_from_project(ui: &AppWindow) {
    let mut state = PROJECT_STATE.lock().unwrap();
    if let Some(ref mut s) = *state {
        let global_speed = s.tracks_manager.get_global_speed();

        for track in s.tracks_manager.tracks.iter_mut() {
            track.set_global_speed(global_speed);
        }
        s.tracks_manager.update_duration();

        let speed_filter = s
            .tracks_manager
            .get_global_filters()
            .iter()
            .find(|f| f.inner.name() == GlobalSpeedFilter::NAME);

        if let Some(filter) = speed_filter
            && let Some(speed) = filter.inner.as_any().downcast_ref::<GlobalSpeedFilter>()
        {
            let speed_config_data: GlobalSpeedFilterConfigData = speed.clone().into();

            let mut config = global_store!(ui).get_video_editor_global_filter_config();
            config.global_speed = speed_config_data.into();
            config.global_speed.enabled = filter.enabled();

            global_store!(ui).set_video_editor_global_filter_config(config);
        }
    }
}

pub fn sync_speed_to_project_state(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_global_filter_config();
    let speed_config = &config.global_speed;
    let enabled = speed_config.enabled;
    let new_speed = if enabled { speed_config.speed } else { 1.0 };

    let mut state = PROJECT_STATE.lock().unwrap();
    if let Some(ref mut s) = *state {
        let old_speed = s.tracks_manager.get_global_speed();
        if old_speed != new_speed {
            s.history_manager
                .execute(
                    &mut s.tracks_manager,
                    Box::new(SetGlobalSpeedCommand::new(old_speed, new_speed)),
                )
                .ok();
        }

        let filter_idx = s
            .tracks_manager
            .get_global_filters()
            .iter()
            .position(|f| f.inner.name() == GlobalSpeedFilter::NAME);

        let new_filter: GlobalSpeedFilter =
            GlobalSpeedFilterConfigData::from(speed_config.clone()).into();

        let new_wrapper = GlobalFilterWrapper::new(enabled, Box::new(new_filter));
        if let Some(idx) = filter_idx {
            s.tracks_manager.global_filters[idx] = Arc::new(new_wrapper);
        } else {
            s.tracks_manager.add_global_filter(Arc::new(new_wrapper));
        }
    }

    drop(state);
    save_global_filter_config(ui);
    sync_and_refresh_simple(ui);
}

fn video_editor_global_filter_speed_update(ui: &AppWindow) {
    sync_speed_to_project_state(ui);
}

fn video_editor_global_filter_speed_toggle_enable(ui: &AppWindow) {
    sync_speed_to_project_state(ui);
}

pub fn clear_ui_state(ui: &AppWindow) {
    let mut config = global_store!(ui).get_video_editor_global_filter_config();
    config.global_speed.enabled = false;
    config.global_speed.speed = 1.0;
    global_store!(ui).set_video_editor_global_filter_config(config);
}

