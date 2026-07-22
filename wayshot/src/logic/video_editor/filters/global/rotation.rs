use crate::{
    db::RotationGlobalFilterConfigData,
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
use video_editor::filters::{global::RotationGlobalFilter, traits::GlobalFilterWrapper};

pub fn init(ui: &AppWindow) {
    logic_cb!(video_editor_global_filter_rotation_update, ui);
    logic_cb!(video_editor_global_filter_rotation_toggle_enable, ui);
}

pub fn init_from_project(ui: &AppWindow) {
    let mut state = PROJECT_STATE.lock().unwrap();
    if let Some(ref mut s) = *state {
        let filter_idx = s
            .tracks_manager
            .get_global_filters()
            .iter()
            .position(|f| f.inner.name() == RotationGlobalFilter::NAME);

        if let Some(idx) = filter_idx {
            let filter = &s.tracks_manager.get_global_filters()[idx];
            if let Some(rotation) = filter.inner.as_any().downcast_ref::<RotationGlobalFilter>() {
                let config_data: RotationGlobalFilterConfigData = rotation.clone().into();

                let mut config = global_store!(ui).get_video_editor_global_filter_config();
                config.rotation = config_data.into();
                config.rotation.enabled = filter.enabled();
                global_store!(ui).set_video_editor_global_filter_config(config);
            }
        }
    }
}

pub fn sync_rotation_to_project_state(ui: &AppWindow) {
    let config = global_store!(ui).get_video_editor_global_filter_config();
    let rotation_config = &config.rotation;
    let enabled = rotation_config.enabled;

    {
        let mut state = PROJECT_STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            let filter_idx = s
                .tracks_manager
                .get_global_filters()
                .iter()
                .position(|f| f.inner.name() == RotationGlobalFilter::NAME);

            let new_filter: RotationGlobalFilter =
                RotationGlobalFilterConfigData::from(rotation_config.clone()).into();
            let new_wrapper = GlobalFilterWrapper::new(enabled, Box::new(new_filter));

            if let Some(idx) = filter_idx {
                s.tracks_manager.global_filters[idx] = Arc::new(new_wrapper);
            } else {
                s.tracks_manager.add_global_filter(Arc::new(new_wrapper));
            }
        }
    }

    save_global_filter_config(ui);
    sync_and_refresh_simple(ui);
}

fn video_editor_global_filter_rotation_update(ui: &AppWindow) {
    sync_rotation_to_project_state(ui);
}

fn video_editor_global_filter_rotation_toggle_enable(ui: &AppWindow) {
    sync_rotation_to_project_state(ui);
}

pub fn clear_ui_state(ui: &AppWindow) {
    let mut config = global_store!(ui).get_video_editor_global_filter_config();
    config.rotation.enabled = false;
    config.rotation.rotation = 0.0;
    global_store!(ui).set_video_editor_global_filter_config(config);
}

