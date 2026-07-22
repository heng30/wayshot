use crate::{
    db::{CodeImageConfigData, VIDEO_EDITOR_TABLE},
    logic::{
        toast, tr::tr, video_editor::playlist::import_file_to_playlist,
        video_editor::project::CODE_IMAGE_CONFIG_ID,
    },
    logic_cb,
    slint_generatedAppWindow::{AppWindow, CodeImageConfig as UICodeImageConfig},
};
use code_image::{CodeHighlightConfig, Language, TerminalStyle, highlight_code_to_image};
use slint::{ComponentHandle, Image, Weak};
use std::path::{Path, PathBuf};

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_generate_code_image, ui, config);
    logic_cb!(video_editor_add_code_image_to_playlist, ui, image);
    logic_cb!(video_editor_update_code_image_config, ui, config);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let config = load_code_image_config()
            .await
            .unwrap_or_else(|| CodeImageConfigData::default());

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let ui_config = UICodeImageConfig::from(config.clone());
            crate::global_store!(ui).set_video_editor_code_image_config(ui_config);
        });
    });
}

fn video_editor_generate_code_image(ui: &AppWindow, config: UICodeImageConfig) -> slint::Image {
    let lang = Language::from_str(&config.language).unwrap_or_default();
    let terminal_style = if config.enable_terminal {
        TerminalStyle::from_str(&config.terminal_style)
    } else {
        None
    };

    let ascii_font_path = PathBuf::from(&config.ascii_font_path);
    if !Path::new(&config.ascii_font_path).exists() {
        crate::toast_warn!(
            ui,
            format!("{}: {}", tr("No found ascii font path"), config.ascii_font_path)
        );
        return Default::default();
    }

    let non_ascii_font_path = PathBuf::from(&config.non_ascii_font_path);
    if !Path::new(&config.non_ascii_font_path).exists() {
        crate::toast_warn!(
            ui,
            format!(
                "{}: {}",
                tr("No found non-ascii font path"),
                config.non_ascii_font_path
            )
        );
        return Default::default();
    };

    let terminal_title = if config.terminal_title.is_empty() {
        None
    } else {
        Some(config.terminal_title.to_string())
    };

    let bg_color = if config.bg_color.is_empty() {
        None
    } else {
        Some(config.bg_color.to_string())
    };

    let highlight_config = CodeHighlightConfig::new(ascii_font_path, non_ascii_font_path)
        .with_ascii_font(config.ascii_font_family.to_string())
        .with_non_ascii_font(config.non_ascii_font_family.to_string())
        .with_line_numbers(config.line_numbers)
        .with_theme(config.syntax_theme.to_string())
        .with_font_size(config.font_size as f64)
        .with_line_height_ratio(config.line_height_ratio as f64)
        .with_padding(config.padding as f64)
        .with_scale(config.scale as f64)
        .with_language(lang)
        .with_terminal(terminal_style)
        .with_terminal_title(terminal_title)
        .with_bg_color(bg_color);

    match highlight_code_to_image(&config.code, &highlight_config) {
        Ok(image) => {
            let width = image.width();
            let height = image.height();
            let pixels = image.into_raw();

            slint::Image::from_rgba8(
                slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &pixels,
                    width as u32,
                    height as u32,
                ),
            )
        }
        Err(e) => {
            log::warn!("Failed to generate code image: {e}");
            slint::Image::default()
        }
    }
}

fn video_editor_add_code_image_to_playlist(ui: &AppWindow, image: Image) {
    let ui_weak = ui.as_weak();

    let pixel_buffer = match image.to_rgba8() {
        Some(buffer) => buffer,
        None => {
            crate::toast_warn!(ui, tr("Failed to get image pixel buffer"));
            return;
        }
    };

    let width = pixel_buffer.width() as u32;
    let height = pixel_buffer.height() as u32;
    let pixels: Vec<u8> = pixel_buffer
        .as_slice()
        .iter()
        .flat_map(|p| [p.r, p.g, p.b, p.a])
        .collect();

    tokio::spawn(async move {
        save_image_and_add_to_playlist(ui_weak, width, height, pixels).await;
    });
}

async fn save_image_and_add_to_playlist(
    ui_weak: Weak<AppWindow>,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
) {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("code_image_{}.png", timestamp);
    let cache_dir = crate::config::all().cache_dir.join("code_images");

    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        toast::async_toast_warn(
            ui_weak.clone(),
            format!("{}: {}", tr("Failed to create cache directory"), e),
        );
        return;
    }

    let file_path = cache_dir.join(&filename);

    let rgba_image =
        image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_vec(width, height, pixels)
            .expect("Failed to create image buffer from pixels");

    if let Err(e) = rgba_image.save(&file_path) {
        toast::async_toast_warn(ui_weak.clone(), format!("{}: {}", tr("Failed to save image"), e));
        return;
    }

    import_file_to_playlist(ui_weak.clone(), file_path, None).await;
}

fn video_editor_update_code_image_config(ui: &AppWindow, config: UICodeImageConfig) {
    crate::global_store!(ui).set_video_editor_code_image_config(config.clone());
    let data = CodeImageConfigData::from(config);
    tokio::spawn(async move {
        save_code_image_config(data).await;
    });
}

async fn save_code_image_config(config: CodeImageConfigData) {
    let data = serde_json::to_string(&config).expect("serialize code image config failed");
    if sqldb::entry::insert(VIDEO_EDITOR_TABLE, CODE_IMAGE_CONFIG_ID, &data)
        .await
        .is_err()
    {
        if let Err(e) = sqldb::entry::update(VIDEO_EDITOR_TABLE, CODE_IMAGE_CONFIG_ID, &data).await
        {
            log::warn!("Failed to save code image config: {:?}", e);
        }
    }
}

async fn load_code_image_config() -> Option<CodeImageConfigData> {
    match sqldb::entry::select(VIDEO_EDITOR_TABLE, CODE_IMAGE_CONFIG_ID).await {
        Ok(entry) => serde_json::from_str(&entry.data).ok(),
        Err(_) => Some(CodeImageConfigData::default()),
    }
}
