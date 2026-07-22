use crate::{
    GlobalFilterType,
    db::{GlobalFilterConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::GLOBAL_FILTER_CONFIG_ID,
    logic_cb,
    slint_generatedAppWindow::AppWindow,
};
use slint::ComponentHandle;

mod danmaku;
mod progress_bar;
mod rotation;
mod speed;
mod timer;

pub fn init(ui: &AppWindow) {
    inner_init(ui);
    progress_bar::init(ui);
    timer::init(ui);
    speed::init(ui);
    rotation::init(ui);
    danmaku::init(ui);

    logic_cb!(
        video_editor_global_filter_progress_update_config,
        ui,
        filter_type
    );
}

pub fn inner_init(ui: &AppWindow) {
    load_global_filter_config(ui);
}

fn load_global_filter_config(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, GLOBAL_FILTER_CONFIG_ID).await {
            Ok(entry) => {
                serde_json::from_str::<GlobalFilterConfigData>(&entry.data).unwrap_or_default()
            }
            Err(_) => GlobalFilterConfigData::default(),
        };
        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_global_filter_config(config.into());
        });
    });
}

pub fn init_from_project(ui: &AppWindow) {
    progress_bar::init_from_project(ui);
    timer::init_from_project(ui);
    speed::init_from_project(ui);
    rotation::init_from_project(ui);
    danmaku::init_from_project(ui);
}

pub fn video_editor_global_filter_progress_update_config(
    ui: &AppWindow,
    filter_type: GlobalFilterType,
) {
    match filter_type {
        GlobalFilterType::ProgressBar => progress_bar::sync_segments_to_project_state(ui),
        GlobalFilterType::Timer => timer::sync_timer_to_project_state(ui),
        GlobalFilterType::GlobalSpeed => speed::sync_speed_to_project_state(ui),
        GlobalFilterType::Rotation => rotation::sync_rotation_to_project_state(ui),
        GlobalFilterType::Danmaku => danmaku::sync_danmaku_to_project_state(ui),
    }
}

pub fn save_global_filter_config(ui: &AppWindow) {
    let mut config: GlobalFilterConfigData = global_store!(ui)
        .get_video_editor_global_filter_config()
        .into();

    // 这是项目相关的配置。不保存列表内容，只保留配置。为的是可以复用配置。
    config.progress_bar.enabled = false;
    config.progress_bar.items.clear();
    config.timer.enabled = false;
    config.timer.items.clear();
    config.global_speed.enabled = false;
    config.global_speed.speed = 1.0;
    config.rotation.enabled = false;
    config.rotation.rotation = 0.0;
    config.danmaku.enabled = false;
    config.danmaku.items.clear();

    tokio::spawn(async move {
        let json = serde_json::to_string(&config).expect("serialize global filter config failed");
        if let Err(e) =
            sqldb::entry::upsert(VIDEO_EDITOR_TABLE, GLOBAL_FILTER_CONFIG_ID, &json).await
        {
            log::warn!("Failed to save global filter config: {:?}", e);
        }
    });
}

pub fn clear_ui_state(ui: &AppWindow) {
    progress_bar::clear_ui_state(ui);
    timer::clear_ui_state(ui);
    speed::clear_ui_state(ui);
    rotation::clear_ui_state(ui);
    danmaku::clear_ui_state(ui);
}
