use crate::{
    db::{FONT_TABLE as DB_TABLE, FontEntry, FontSource},
    db_select_all,
    logic::{toast, tr::tr, video_editor::playlist::picker_files},
    logic_cb,
    slint_generatedAppWindow::{AppWindow, FontEntry as UIFontEntry, FontSource as UIFontSource},
};
use image::Rgba;
use once_cell::sync::Lazy;
use slint::{ComponentHandle, Model, SharedString, VecModel, Weak};
use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::RwLock,
};
use video_editor::{
    filters::subtitle::{renderer::create_text_image, style::SubtitleStyle},
    font::{get_font_family_from_file, get_fonts_info},
};

const CN_TEXT: &str = "欢迎使用！";
const EN_TEXT: &str = "Welcome to Wayshot!";
const FONT_DISPLAY_SIZE: u32 = 32;

static FONT_CACHE: Lazy<RwLock<Vec<FontEntry>>> = Lazy::new(|| RwLock::new(Vec::new()));

#[macro_export]
macro_rules! store_font_entries {
    ($ui:expr) => {
        $ui.global::<crate::slint_generatedAppWindow::Store>()
            .get_font_entries()
            .as_any()
            .downcast_ref::<VecModel<UIFontEntry>>()
            .expect("We know we set a VecModel<UIFontEntry> earlier")
    };
}

pub fn init(ui: &AppWindow) {
    inner_int(ui);

    logic_cb!(init_font_dialog, ui);
    logic_cb!(refresh_font_entries, ui);
    logic_cb!(import_font_files, ui);
    logic_cb!(search_font_entries, ui, text);
    logic_cb!(toggle_font_marked, ui, index);
}

pub fn inner_int(ui: &AppWindow) {
    store_font_entries!(ui).set_vec(vec![]);
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        load_fonts(ui_weak).await;
    });
}

fn init_font_dialog(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let cached_fonts = {
            let cache = FONT_CACHE.read().unwrap();
            if cache.is_empty() {
                None
            } else {
                Some(cache.clone())
            }
        };

        if let Some(fonts) = cached_fonts {
            sync_fonts_to_ui(ui_weak, fonts).await;
        } else {
            load_fonts(ui_weak).await;
        }
    });
}

fn refresh_font_entries(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        load_fonts(ui_weak).await;
    });

    crate::toast_success!(ui, tr("Refresh..."));
}

async fn load_fonts(ui_weak: Weak<AppWindow>) {
    let fonts = load_fonts_from_db().await;
    if !fonts.is_empty() {
        {
            let mut cache = FONT_CACHE.write().unwrap();
            *cache = fonts.clone();
        }
        sync_fonts_to_ui(ui_weak.clone(), fonts.clone()).await;
    }

    load_and_merge_system_fonts(ui_weak, fonts).await;
}

async fn load_and_merge_system_fonts(ui_weak: Weak<AppWindow>, db_fonts: Vec<FontEntry>) {
    let marked_paths: HashSet<String> = db_fonts
        .iter()
        .filter(|f| f.marked)
        .map(|f| f.path.clone())
        .collect();

    let imported_paths: HashSet<String> = db_fonts
        .iter()
        .filter(|f| f.source == FontSource::Imported)
        .map(|f| f.path.clone())
        .collect();

    let system_fonts = match get_fonts_info() {
        Ok(fonts) => fonts,
        Err(e) => {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!("{}: {}", tr("Failed to get fonts"), e),
            );
            return;
        }
    };

    let mut font_entries: Vec<FontEntry> = db_fonts;
    for (family, path, style) in system_fonts {
        let path_str = path.to_string_lossy().to_string();

        // Skip if already in list (imported or marked system font)
        if imported_paths.contains(&path_str) || marked_paths.contains(&path_str) {
            continue;
        }

        let entry = FontEntry {
            id: format!("{}::{}::{}", path_str, family, style),
            family,
            path: path_str,
            style,
            marked: false,
            source: FontSource::System,
        };
        font_entries.push(entry);
    }

    {
        let mut cache = FONT_CACHE.write().unwrap();
        *cache = font_entries.clone();
    }

    sync_fonts_to_ui(ui_weak, font_entries).await;
}

async fn sync_fonts_to_ui(ui_weak: Weak<AppWindow>, mut fonts: Vec<FontEntry>) {
    // Sort: marked (desc) > imported before system > family name (asc)
    fonts.sort_by(|a, b| match b.marked.cmp(&a.marked) {
        std::cmp::Ordering::Equal => match (&a.source, &b.source) {
            (FontSource::Imported, FontSource::System) => std::cmp::Ordering::Less,
            (FontSource::System, FontSource::Imported) => std::cmp::Ordering::Greater,
            _ => a.family.cmp(&b.family),
        },
        other => other,
    });

    // Render display images for each font (may take time, done on tokio thread)
    // Store raw pixel data (which is Send-safe) instead of slint::Image (which is not Send)
    let ui_font_data: Vec<(FontEntry, Option<(u32, u32, Vec<u8>)>)> = fonts
        .into_iter()
        .map(|f| {
            let display_data = render_font_display_image(&f);
            (f, display_data)
        })
        .collect();

    _ = ui_weak.upgrade_in_event_loop(move |ui| {
        let ui_fonts: Vec<UIFontEntry> = ui_font_data
            .into_iter()
            .map(|(f, display_data)| {
                let mut entry: UIFontEntry = f.into();
                if let Some((w, h, pixels)) = display_data {
                    entry.display_image = rgba_to_slint_image(w, h, pixels);
                }
                entry
            })
            .collect();
        store_font_entries!(ui).set_vec(ui_fonts);
    });
}

fn import_font_files(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        let file_paths = match picker_files(
            ui_weak.clone(),
            &tr("Select font files"),
            &tr("Font Files"),
            &["ttf", "otf", "woff", "woff2"],
        ) {
            Some(paths) => paths,
            None => return,
        };

        let mut imported_fonts: Vec<(FontEntry, Option<(u32, u32, Vec<u8>)>)> = Vec::new();
        for file_path in file_paths {
            if let Some(font_entry) = import_font_to_db(ui_weak.clone(), file_path).await {
                let display_data = render_font_display_image(&font_entry);
                imported_fonts.push((font_entry, display_data));
            }
        }

        if !imported_fonts.is_empty() {
            {
                let mut cache = FONT_CACHE.write().unwrap();
                for (font_entry, _) in &imported_fonts {
                    cache.push(font_entry.clone());
                }
            }

            _ = ui_weak.upgrade_in_event_loop(move |ui| {
                let new_entries: Vec<UIFontEntry> = imported_fonts
                    .into_iter()
                    .map(|(f, display_data)| {
                        let mut entry: UIFontEntry = f.into();
                        if let Some((w, h, pixels)) = display_data {
                            entry.display_image = rgba_to_slint_image(w, h, pixels);
                        }
                        entry
                    })
                    .collect();
                let mut fonts: Vec<UIFontEntry> = store_font_entries!(ui).iter().collect();
                fonts.splice(0..0, new_entries);
                store_font_entries!(ui).set_vec(fonts);
            });
        }
    });
}

async fn import_font_to_db(ui_weak: Weak<AppWindow>, file_path: PathBuf) -> Option<FontEntry> {
    let (family, style) = match get_font_family_from_file(&file_path) {
        Ok(f) => f,
        Err(e) => {
            toast::async_toast_warn(
                ui_weak.clone(),
                format!(
                    "{} {}: {}",
                    tr("Failed to read font file"),
                    file_path.display(),
                    e
                ),
            );
            return None;
        }
    };

    let path_str = file_path.to_string_lossy().to_string();

    let entry = FontEntry {
        id: format!("{}::{}::{}", path_str, family, style),
        family,
        path: path_str,
        style,
        marked: true,
        source: FontSource::Imported,
    };

    let data = serde_json::to_string(&entry).expect("Serialize font entry");
    if let Err(e) = sqldb::entry::insert(DB_TABLE, entry.id.as_str(), &data).await {
        toast::async_toast_warn(
            ui_weak.clone(),
            format!("{} {}: {}", tr("Failed to import font"), entry.family, e),
        );
        return None;
    }

    toast::async_toast_success(
        ui_weak.clone(),
        format!("{} {}", tr("Imported font"), entry.family),
    );
    Some(entry)
}

fn toggle_font_marked(ui: &AppWindow, index: i32) {
    let idx = index as usize;

    if idx < store_font_entries!(ui).row_count()
        && let Some(mut font_info) = store_font_entries!(ui).row_data(idx)
    {
        font_info.marked = !font_info.marked;
        let id = font_info.id.to_string();
        let is_imported = font_info.source == UIFontSource::Imported;
        let is_marked = font_info.marked;
        let font_entry: FontEntry = font_info.clone().into();

        let ui_weak = ui.as_weak();
        let mut fonts: Vec<FontEntry> = store_font_entries!(ui).iter().map(|f| f.into()).collect();
        fonts[idx] = font_info.clone().into();
        tokio::spawn(async move {
            sync_fonts_to_ui(ui_weak, fonts).await;
        });

        {
            let mut cache = FONT_CACHE.write().unwrap();
            if let Some(entry) = cache.iter_mut().find(|e| e.id == font_info.id.as_str()) {
                entry.marked = font_info.marked;
            }
        }

        tokio::spawn(async move {
            if is_imported {
                if is_marked {
                    let data = serde_json::to_string(&font_entry).expect("Serialize font entry");
                    if let Err(e) =
                        sqldb::entry::update(DB_TABLE, font_entry.id.as_str(), &data).await
                    {
                        log::warn!("Failed to update font marked status: {}", e);
                    }
                }
            } else if !is_imported {
                if is_marked {
                    let data = serde_json::to_string(&font_entry).expect("Serialize font entry");
                    if let Err(e) =
                        sqldb::entry::upsert(DB_TABLE, font_entry.id.as_str(), &data).await
                    {
                        log::warn!("Failed to upsert marked system font to DB: {}", e);
                    }
                } else {
                    if let Err(e) = sqldb::entry::delete(DB_TABLE, &id).await {
                        log::warn!("Failed to delete unmarked system font from DB: {}", e);
                    }
                }
            }
        });
    }
}

fn search_font_entries(ui: &AppWindow, text: SharedString) {
    let ui_weak = ui.as_weak();
    let text = text.to_string();

    tokio::spawn(async move {
        let cache = FONT_CACHE.read().unwrap().clone();

        let filtered = if text.is_empty() {
            cache
        } else {
            let keyword = text.to_lowercase();
            cache
                .into_iter()
                .filter(|f| f.family.to_lowercase().contains(&keyword))
                .collect()
        };

        sync_fonts_to_ui(ui_weak, filtered).await;
    });
}

async fn load_fonts_from_db() -> Vec<FontEntry> {
    let fonts: Vec<FontEntry> = db_select_all!(DB_TABLE, FontEntry);
    // Filter out fonts whose files no longer exist on disk
    fonts
        .into_iter()
        .filter(|f| std::path::Path::new(&f.path).exists())
        .collect()
}

/// Check if a font file supports CJK characters by attempting to parse it with ttf-parser.
/// Returns true if the font contains glyphs for common CJK Unified Ideographs.
fn font_supports_cjk(font_path: &str) -> bool {
    let path = Path::new(font_path);
    if !path.exists() {
        return false;
    }
    match std::fs::read(path) {
        Ok(data) => {
            // Check a few representative CJK characters
            let test_chars = ['欢', '迎', '使', '用'];
            for font_index in 0..ttf_parser::fonts_in_collection(&data).unwrap_or(1) {
                if let Ok(face) = ttf_parser::Face::parse(&data, font_index) {
                    let all_supported = test_chars.iter().all(|&ch| face.glyph_index(ch).is_some());
                    if all_supported {
                        return true;
                    }
                }
            }
            false
        }
        Err(_) => false,
    }
}

/// Render display text image for a font entry, using cache if available.
/// Black text on white background (no outline).
/// If the font doesn't support Chinese, only render the English text.
/// Returns (width, height, pixels) as raw RGBA bytes, which are Send-safe.
fn render_font_display_image(font_entry: &FontEntry) -> Option<(u32, u32, Vec<u8>)> {
    let cache_dir = crate::config::all().cache_dir.join("font_display_images");
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        log::warn!("Failed to create font display cache dir: {}", e);
        return None;
    }

    let mut hasher = DefaultHasher::new();
    font_entry.id.hash(&mut hasher);
    let cache_key = format!("{:016x}", hasher.finish());
    let cache_path = cache_dir.join(format!("{}.png", cache_key));

    // Check cache first
    if cache_path.exists()
        && let Ok(img) = image::open(&cache_path)
    {
        let rgba_img = img.to_rgba8();
        let width = rgba_img.width();
        let height = rgba_img.height();
        return Some((width, height, rgba_img.into_raw()));
    }

    // Determine display text based on CJK support
    let text = if font_supports_cjk(&font_entry.path) {
        format!("{}{}", CN_TEXT, EN_TEXT)
    } else {
        EN_TEXT.to_string()
    };

    let font_path = PathBuf::from(&font_entry.path);
    if !font_path.exists() {
        return None;
    }

    let preferred_family = if font_entry.family.is_empty() {
        None
    } else {
        Some(font_entry.family.as_str())
    };
    let preferred_style = if font_entry.style.is_empty() {
        None
    } else {
        Some(font_entry.style.as_str())
    };

    let style = SubtitleStyle::new()
        .with_font_size(FONT_DISPLAY_SIZE)
        .with_font_path(font_path)
        .with_font_family(preferred_family.unwrap_or_default().to_string())
        .with_font_style(preferred_style.unwrap_or_default().to_string())
        .with_primary_color(Some(Rgba([0, 0, 0, 255]))) // black text
        .with_background_color(Some(Rgba([0, 0, 0, 0]))) // transparent background
        .with_outline_color(None) // no outline
        .with_outline_width(Some(0))
        .with_padding(Some(8));

    match create_text_image(&text, &style) {
        Ok(rgba_img) => {
            let width = rgba_img.width();
            let height = rgba_img.height();

            // Save to cache
            if let Err(e) = rgba_img.save(&cache_path) {
                log::warn!("Failed to cache font display image: {}", e);
            }

            Some((width, height, rgba_img.into_raw()))
        }
        Err(e) => {
            log::debug!(
                "Failed to render font display image for '{}': {}",
                font_entry.family,
                e
            );
            None
        }
    }
}

fn rgba_to_slint_image(width: u32, height: u32, pixels: Vec<u8>) -> slint::Image {
    slint::Image::from_rgba8(
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&pixels, width, height),
    )
}
