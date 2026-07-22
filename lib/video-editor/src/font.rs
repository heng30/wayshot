use crate::Result;
use cosmic_text::fontdb;
use std::path::{Path, PathBuf};

pub fn get_fonts_info() -> Result<Vec<(String, PathBuf, String)>> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let mut fonts = Vec::new();

    for face in db.faces() {
        if let Some((source, _index)) = db.face_source(face.id)
            && let fontdb::Source::File(path) = &source
            && let Some(family) = face.families.first()
        {
            let style = format_font_style(face.weight, face.style);
            fonts.push((family.0.clone(), path.clone(), style));
        }
    }

    // Sort by path, family, style - ensures TTC fonts with multiple faces stay grouped
    fonts.sort_by(|a, b| (&a.1, &a.0, &a.2).cmp(&(&b.1, &b.0, &b.2)));
    // Dedup by path+family+style - keeps all regional variants (SC, TC, K, HC) for TTC files
    fonts.dedup_by(|a, b| a.1 == b.1 && a.0 == b.0 && a.2 == b.2);

    // Filter out fonts whose files no longer exist on disk
    fonts.retain(|(_, path, _)| path.exists() && Path::is_file(path));

    Ok(fonts)
}

/// Parse a font style string (as produced by `format_font_style`) back into
/// `fontdb::Weight` and `fontdb::Style`.
///
/// Recognised tokens (case-insensitive, space-separated):
///   Weight: Thin, ExtraLight, Light, Medium, SemiBold, Bold, ExtraBold, Black
///   Style: Italic, Oblique
///   "Normal" → weight NORMAL + style Normal
pub fn parse_font_style(style: &str) -> (fontdb::Weight, fontdb::Style) {
    let lower = style.to_lowercase();
    if lower == "normal" || lower.is_empty() {
        return (fontdb::Weight::NORMAL, fontdb::Style::Normal);
    }

    let mut weight = fontdb::Weight::NORMAL;
    let mut fs_style = fontdb::Style::Normal;

    for token in lower.split_whitespace() {
        match token {
            "thin" => weight = fontdb::Weight::THIN,
            "extralight" => weight = fontdb::Weight::EXTRA_LIGHT,
            "light" => weight = fontdb::Weight::LIGHT,
            "medium" => weight = fontdb::Weight::MEDIUM,
            "semibold" => weight = fontdb::Weight::SEMIBOLD,
            "bold" => weight = fontdb::Weight::BOLD,
            "extrabold" => weight = fontdb::Weight::EXTRA_BOLD,
            "black" => weight = fontdb::Weight::BLACK,
            "italic" => fs_style = fontdb::Style::Italic,
            "oblique" => fs_style = fontdb::Style::Oblique,
            _ => {}
        }
    }

    (weight, fs_style)
}

fn format_font_style(weight: fontdb::Weight, style: fontdb::Style) -> String {
    let weight_str = match weight {
        fontdb::Weight::THIN => "Thin",
        fontdb::Weight::EXTRA_LIGHT => "ExtraLight",
        fontdb::Weight::LIGHT => "Light",
        fontdb::Weight::NORMAL => "",
        fontdb::Weight::MEDIUM => "Medium",
        fontdb::Weight::SEMIBOLD => "SemiBold",
        fontdb::Weight::BOLD => "Bold",
        fontdb::Weight::EXTRA_BOLD => "ExtraBold",
        fontdb::Weight::BLACK => "Black",
        _ => "",
    };

    let style_str = match style {
        fontdb::Style::Normal => "",
        fontdb::Style::Italic => "Italic",
        fontdb::Style::Oblique => "Oblique",
    };

    if weight_str.is_empty() && style_str.is_empty() {
        "Normal".to_string()
    } else if weight_str.is_empty() {
        style_str.to_string()
    } else if style_str.is_empty() {
        weight_str.to_string()
    } else {
        format!("{} {}", weight_str, style_str)
    }
}

pub fn get_font_family_from_file(path: &PathBuf) -> Result<(String, String)> {
    let mut db = fontdb::Database::new();
    db.load_font_file(path.clone())?;

    for face in db.faces() {
        if let Some(family) = face.families.first() {
            let style = format_font_style(face.weight, face.style);
            return Ok((family.0.clone(), style));
        }
    }

    Ok((
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string(),
        "Normal".to_string(),
    ))
}
