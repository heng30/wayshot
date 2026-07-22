use crate::{
    db::{PureColorImageConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        recorder::picker_directory, toast, tr::tr, video_editor::playlist::import_file_to_playlist,
        video_editor::project::PURE_COLOR_IMAGE_CONFIG_ID,
    },
    logic_cb,
    slint_generatedAppWindow::{AppWindow, PureColorImageConfig as UIPureColorImageConfig},
};
use slint::{Color, ComponentHandle, Weak};
use std::path::PathBuf;

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_pure_color_image_select_dir, ui);
    logic_cb!(video_editor_update_pure_color_image_config, ui, config);
    logic_cb!(video_editor_generate_pure_color_image, ui, c, config);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_pure_color_image_config()
            .await
            .unwrap_or_else(|| PureColorImageConfigData::default());

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_pure_color_image_config(config.into());
        });
    });
}

fn video_editor_generate_pure_color_image(
    ui: &AppWindow,
    c: Color,
    config: UIPureColorImageConfig,
) {
    let r = c.red();
    let g = c.green();
    let b = c.blue();
    let a = c.alpha();

    let ui_weak = ui.as_weak();
    let width = config.width.max(1) as u32;
    let height = config.height.max(1) as u32;
    let save_dir = config.save_dir.to_string();

    tokio::spawn(async move {
        save_pure_color_image_and_add_to_playlist(ui_weak, r, g, b, a, width, height, &save_dir)
            .await;
    });
}

async fn save_pure_color_image_and_add_to_playlist(
    ui_weak: Weak<AppWindow>,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    width: u32,
    height: u32,
    save_dir: &str,
) {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("pure_color_{}_{}x{}.png", timestamp, width, height);

    let dir = if save_dir.is_empty() {
        crate::config::all().cache_dir.join("pure_color_images")
    } else {
        PathBuf::from(save_dir)
    };

    if let Err(e) = std::fs::create_dir_all(&dir) {
        toast::async_toast_warn(
            ui_weak.clone(),
            format!("{}: {}", tr("Failed to create directory"), e),
        );
        return;
    }

    let file_path = dir.join(&filename);

    let mut rgba_image = image::RgbaImage::new(width, height);
    for pixel in rgba_image.pixels_mut() {
        *pixel = image::Rgba([r, g, b, a]);
    }

    if let Err(e) = rgba_image.save(&file_path) {
        toast::async_toast_warn(
            ui_weak.clone(),
            format!("{}: {}", tr("Failed to save image"), e),
        );
        return;
    }

    import_file_to_playlist(ui_weak.clone(), file_path, None).await;
}

fn video_editor_update_pure_color_image_config(ui: &AppWindow, config: UIPureColorImageConfig) {
    global_store!(ui).set_video_editor_pure_color_image_config(config.clone());
    save_pure_color_image_config(config.into());
}

fn video_editor_pure_color_image_select_dir(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    let mut config = global_store!(ui).get_video_editor_pure_color_image_config();

    tokio::spawn(async move {
        let Some(dir) = picker_directory(ui_weak.clone(), &tr("Choose save directory")) else {
            return;
        };

        config.save_dir = dir.to_string_lossy().to_string().into();
        save_pure_color_image_config(config.clone().into());

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            global_store!(ui).set_video_editor_pure_color_image_config(config);
        });
    });
}

fn save_pure_color_image_config(config: PureColorImageConfigData) {
    tokio::spawn(async move {
        let data =
            serde_json::to_string(&config).expect("serialize pure color image config failed");
        if sqldb::entry::insert(VIDEO_EDITOR_TABLE, PURE_COLOR_IMAGE_CONFIG_ID, &data)
            .await
            .is_err()
        {
            if let Err(e) =
                sqldb::entry::update(VIDEO_EDITOR_TABLE, PURE_COLOR_IMAGE_CONFIG_ID, &data).await
            {
                log::warn!("Failed to save pure color image config: {:?}", e);
            }
        }
    });
}

async fn load_pure_color_image_config() -> Option<PureColorImageConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, PURE_COLOR_IMAGE_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => Some(PureColorImageConfigData::default()),
    }
}
