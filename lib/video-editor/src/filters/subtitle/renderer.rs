use crate::{
    Error, Result,
    filters::{
        subtitle::style::{SubtitleStyle, TextAlignment},
        video::border::{draw_rounded_border_aa, draw_rounded_rect_fill_aa},
    },
    font::parse_font_style,
};
use cosmic_text::fontdb::{Style as FontStyle, Weight as FontWeight};
use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent, SwashImage,
};
use draw_utils::{composite_image, dilate_mask, fill_from_mask};
use image::{Rgba, RgbaImage};
use std::path::Path;

// Calculate x and y position based on alignment and text dimensions.
// ASS alignment values (numpad style):
// 7-8-9: top-left, top-center, top-right
// 4-5-6: middle-left, middle-center, middle-right
// 1-2-3: bottom-left, bottom-center, bottom-right
fn calculate_position(
    alignment: Option<u32>,
    margin_v: Option<u32>,
    margin_h: Option<u32>,
    text_width: u32,
    text_height: u32,
    canvas_width: u32,
    canvas_height: u32,
) -> (u32, u32) {
    let align = alignment.unwrap_or(2); // bottom-center
    let margin_v = margin_v.unwrap_or(30);
    let margin_h = margin_h.unwrap_or(0);

    match align {
        1 => (
            margin_h,
            canvas_height
                .saturating_sub(margin_v)
                .saturating_sub(text_height),
        ), // bottom-left
        2 => (
            canvas_width.saturating_sub(text_width) / 2,
            canvas_height
                .saturating_sub(margin_v)
                .saturating_sub(text_height),
        ), // bottom-center
        3 => (
            canvas_width
                .saturating_sub(margin_h)
                .saturating_sub(text_width),
            canvas_height
                .saturating_sub(margin_v)
                .saturating_sub(text_height),
        ), // bottom-right
        4 => (margin_h, canvas_height.saturating_sub(text_height) / 2), // middle-left
        5 => (
            canvas_width.saturating_sub(text_width) / 2,
            canvas_height.saturating_sub(text_height) / 2,
        ), // middle-center
        6 => (
            canvas_width
                .saturating_sub(margin_h)
                .saturating_sub(text_width),
            canvas_height.saturating_sub(text_height) / 2,
        ), // middle-right
        7 => (margin_h, margin_v),                                      // top-left
        8 => (canvas_width.saturating_sub(text_width) / 2, margin_v),   // top-center
        9 => (
            canvas_width
                .saturating_sub(margin_h)
                .saturating_sub(text_width),
            margin_v,
        ), // top-right
        _ => (
            canvas_width.saturating_sub(text_width) / 2,
            canvas_height
                .saturating_sub(margin_v)
                .saturating_sub(text_height),
        ), // Default to bottom-center
    }
}

fn calculate_line_x_offset(text_alignment: TextAlignment, max_width: f32, line_width: f32) -> i32 {
    match text_alignment {
        TextAlignment::Left => 0,
        TextAlignment::Center => ((max_width - line_width) / 2.0).round() as i32,
        TextAlignment::Right => (max_width - line_width).round() as i32,
    }
}

pub fn load_font(
    font_path: impl AsRef<Path>,
    font_system: &mut FontSystem,
    preferred_family: Option<&str>,
    font_style: Option<&str>,
) -> Result<(String, FontWeight, FontStyle)> {
    let font_path = font_path.as_ref();
    let canonical_path = font_path.canonicalize().map_err(|_| {
        Error::InvalidConfig(format!("Font file not found: {}", font_path.display()))
    })?;

    font_system
        .db_mut()
        .load_font_file(font_path)
        .map_err(|_| {
            Error::InvalidConfig(format!("Failed to load font file: {}", font_path.display()))
        })?;

    let loaded_faces: Vec<&cosmic_text::fontdb::FaceInfo> = font_system
        .db()
        .faces()
        .filter(|face| match &face.source {
            cosmic_text::fontdb::Source::File(path) => path
                .canonicalize()
                .map(|p| p == canonical_path)
                .unwrap_or(false),
            cosmic_text::fontdb::Source::SharedFile(path, _) => path
                .canonicalize()
                .map(|p| p == canonical_path)
                .unwrap_or(false),
            _ => false,
        })
        .collect();

    // Parse the desired weight and style from the font_style string
    let (desired_weight, desired_style) = font_style
        .map(|s| parse_font_style(s))
        .unwrap_or((FontWeight::NORMAL, FontStyle::Normal));

    // If preferred_family provided, search for matching face (important for TTC fonts)
    if let Some(preferred) = preferred_family {
        // First try: match family + weight + style exactly
        for face_info in loaded_faces.iter() {
            if face_info.families.iter().any(|(f, _)| f == preferred)
                && face_info.weight == desired_weight
                && face_info.style == desired_style
            {
                log::trace!(
                    "Loaded font: {} (matched preferred family: {}, weight: {:?}, style: {:?})",
                    face_info.post_script_name,
                    preferred,
                    face_info.weight,
                    face_info.style,
                );
                return Ok((preferred.to_string(), face_info.weight, face_info.style));
            }
        }

        // Second try: match family + style (weight may differ slightly)
        for face_info in loaded_faces.iter() {
            if face_info.families.iter().any(|(f, _)| f == preferred)
                && face_info.style == desired_style
            {
                log::trace!(
                    "Loaded font: {} (matched preferred family: {}, style: {:?}, weight: {:?})",
                    face_info.post_script_name,
                    preferred,
                    face_info.style,
                    face_info.weight,
                );
                return Ok((preferred.to_string(), face_info.weight, face_info.style));
            }
        }

        // Third try: match family only (fallback)
        for face_info in loaded_faces.iter() {
            if face_info.families.iter().any(|(f, _)| f == preferred) {
                log::trace!(
                    "Loaded font: {} (matched preferred family: {}, weight: {:?}, style: {:?})",
                    face_info.post_script_name,
                    preferred,
                    face_info.weight,
                    face_info.style,
                );
                return Ok((preferred.to_string(), face_info.weight, face_info.style));
            }
        }
        log::warn!(
            "Preferred family '{}' not found in font file, falling back to default face",
            preferred
        );
    }

    if let Some(face_info) = loaded_faces.first()
        && let Some((family, _lang)) = face_info.families.first()
    {
        log::trace!(
            "Loaded font: {} (family: {})",
            face_info.post_script_name,
            family
        );
        return Ok((family.clone(), face_info.weight, face_info.style));
    }

    // Fallback: try to use the post_script_name as family name
    if let Some(face_info) = loaded_faces.first() {
        log::trace!("Loaded font (fallback): {}", face_info.post_script_name);
        return Ok((
            face_info.post_script_name.clone(),
            face_info.weight,
            face_info.style,
        ));
    }

    Err(Error::InvalidConfig(format!(
        "Could not determine font family from: {}",
        font_path.display()
    )))
}

// Calculate text dimensions for the given text and font settings
// Returns (width, height, baseline_offset, line_widths)
//
// The baseline_offset is calculated based on actual glyph extents to ensure
// proper vertical positioning for fonts where punctuation glyphs may have
// unusual design (e.g., SourceHanSansCN.otf has punctuation centered vertically).
pub fn calculate_text_size(
    font_system: &mut FontSystem,
    text: &str,
    font_size: f32,
    font_family: &str,
    font_weight: FontWeight,
    font_style: FontStyle,
    max_text_width: Option<f32>,
) -> Result<(u32, u32, i32, Vec<f32>)> {
    let metrics = Metrics {
        font_size,
        line_height: font_size * 1.2,
    };

    let attrs = Attrs::new()
        .family(Family::Name(font_family))
        .weight(font_weight)
        .style(font_style);

    // Get font metrics for fallback calculation
    let mut font_ascent: f32 = 0.0;
    let mut font_descent: f32 = 0.0;

    // Track actual glyph extents for more accurate baseline calculation
    let mut max_glyph_top: f32 = 0.0;
    let mut min_glyph_bottom: f32 = 0.0;

    let mut max_width: f32 = 0.0;
    let mut total_height: f32 = 0.0;
    let mut line_widths: Vec<f32> = Vec::new();

    // Split by \\N first, then process each segment with word-wrap independently.
    // This ensures line widths are calculated per-segment and not merged across explicit breaks.
    let lines: Vec<&str> = text.split("\\N").collect();

    for line in lines {
        let mut buffer = Buffer::new(font_system, metrics);
        buffer.set_text(line, &attrs, Shaping::Basic, None);
        if let Some(text_max_w) = max_text_width {
            buffer.set_size(Some(text_max_w), None);
        }
        buffer.shape_until_scroll(font_system, false);

        let mut runs: Vec<f32> = Vec::new();
        for run in buffer.layout_runs() {
            runs.push(run.line_w);
            max_width = max_width.max(run.line_w);

            // Get font metrics and actual glyph extents
            for glyph in run.glyphs {
                use cosmic_text::fontdb::Weight;
                if let Some(font) = font_system.get_font(glyph.font_id, Weight(400)) {
                    let font_metrics = font.metrics();
                    let scale = font_size / font_metrics.units_per_em as f32;
                    font_ascent = font_metrics.ascent * scale;
                    font_descent = font_metrics.descent * scale;
                    break;
                }
            }

            // Get actual glyph placement info using swash cache
            let mut swash_cache = SwashCache::new();
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                if let Some(glyph_img) = swash_cache.get_image(font_system, physical.cache_key) {
                    let placement = &glyph_img.placement;
                    let glyph_top = placement.top as f32;
                    let glyph_bottom = (placement.top as f32) - (placement.height as f32);
                    max_glyph_top = max_glyph_top.max(glyph_top);
                    min_glyph_bottom = min_glyph_bottom.min(glyph_bottom);
                }
            }

            total_height += metrics.line_height;
        }

        // Empty line or no runs produced
        if runs.is_empty() {
            runs.push(0.0);
            total_height += metrics.line_height;
        }

        line_widths.extend(runs);
    }

    // Calculate baseline offset to center text vertically within line_height
    // Use actual glyph extents when available for fonts with unusual punctuation design
    let baseline_offset: i32;

    if max_glyph_top > 0.0 {
        // Use actual glyph extents for more accurate positioning
        // glyph_height = max_glyph_top - min_glyph_bottom
        // extra_space = line_height - glyph_height
        // baseline_offset = (extra_space / 2) + max_glyph_top
        let glyph_height = max_glyph_top - min_glyph_bottom;
        let extra_space = metrics.line_height - glyph_height;
        baseline_offset = (extra_space / 2.0 + max_glyph_top).ceil() as i32;
    } else {
        // Fallback to font metrics-based calculation
        let font_height = font_ascent - font_descent; // descent is negative
        let extra_space = metrics.line_height - font_height;
        baseline_offset = (extra_space / 2.0 + font_ascent).ceil() as i32;
    }

    Ok((
        max_width.ceil() as u32,
        total_height.ceil() as u32,
        baseline_offset,
        line_widths,
    ))
}

// Draw a glyph from SwashImage to the image buffer
pub fn draw_glyph(
    img: &mut RgbaImage,
    glyph_img: &SwashImage,
    offset_x: i32,
    offset_y: i32,
    color: Rgba<u8>,
) {
    let placement = &glyph_img.placement;
    let data = &glyph_img.data;

    match &glyph_img.content {
        SwashContent::Mask => {
            // Grayscale mask - use color with alpha from mask
            let mut idx = 0;
            for dy in 0..placement.height as i32 {
                for dx in 0..placement.width as i32 {
                    let alpha = data[idx] as f32 / 255.0;
                    idx += 1;

                    if alpha > 0.0 {
                        let img_x = (offset_x + placement.left + dx) as u32;
                        let img_y = (offset_y - placement.top + dy) as u32;

                        if img_x < img.width() && img_y < img.height() {
                            let pixel = img.get_pixel_mut(img_x, img_y);

                            let src_alpha = alpha * (color[3] as f32 / 255.0);
                            let dst_alpha = pixel[3] as f32 / 255.0;
                            let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

                            if out_alpha > 0.0 {
                                pixel[0] = ((color[0] as f32 * src_alpha
                                    + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[1] = ((color[1] as f32 * src_alpha
                                    + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[2] = ((color[2] as f32 * src_alpha
                                    + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[3] = (out_alpha * 255.0) as u8;
                            }
                        }
                    }
                }
            }
        }
        SwashContent::Color => {
            // Full color image
            let mut idx = 0;
            for dy in 0..placement.height as i32 {
                for dx in 0..placement.width as i32 {
                    let r = data[idx];
                    let g = data[idx + 1];
                    let b = data[idx + 2];
                    let a = data[idx + 3];
                    idx += 4;

                    if a > 0 {
                        let img_x = (offset_x + placement.left + dx) as u32;
                        let img_y = (offset_y - placement.top + dy) as u32;

                        if img_x < img.width() && img_y < img.height() {
                            let pixel = img.get_pixel_mut(img_x, img_y);

                            let src_alpha = a as f32 / 255.0;
                            let dst_alpha = pixel[3] as f32 / 255.0;
                            let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

                            if out_alpha > 0.0 {
                                pixel[0] = ((r as f32 * src_alpha
                                    + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[1] = ((g as f32 * src_alpha
                                    + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[2] = ((b as f32 * src_alpha
                                    + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[3] = (out_alpha * 255.0) as u8;
                            }
                        }
                    }
                }
            }
        }
        SwashContent::SubpixelMask => {
            // Subpixel mask - not commonly used for subtitles
            // Fall back to treating as mask
            let mut idx = 0;
            for dy in 0..placement.height as i32 {
                for dx in 0..placement.width as i32 {
                    // SubpixelMask has 3 values per pixel (RGB)
                    let r = data[idx] as f32 / 255.0;
                    let g = data[idx + 1] as f32 / 255.0;
                    let b = data[idx + 2] as f32 / 255.0;
                    let alpha = (r + g + b) / 3.0;
                    idx += 3;

                    if alpha > 0.0 {
                        let img_x = (offset_x + placement.left + dx) as u32;
                        let img_y = (offset_y - placement.top + dy) as u32;

                        if img_x < img.width() && img_y < img.height() {
                            let pixel = img.get_pixel_mut(img_x, img_y);

                            let src_alpha = alpha * (color[3] as f32 / 255.0);
                            let dst_alpha = pixel[3] as f32 / 255.0;
                            let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);

                            if out_alpha > 0.0 {
                                pixel[0] = ((color[0] as f32 * src_alpha
                                    + pixel[0] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[1] = ((color[1] as f32 * src_alpha
                                    + pixel[1] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[2] = ((color[2] as f32 * src_alpha
                                    + pixel[2] as f32 * dst_alpha * (1.0 - src_alpha))
                                    / out_alpha) as u8;
                                pixel[3] = (out_alpha * 255.0) as u8;
                            }
                        }
                    }
                }
            }
        }
    }
}

// Draw text with the given color and offset (for outline rendering)
pub fn draw_text_line(
    img: &mut RgbaImage,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    line: &str,
    x: i32,
    y: i32,
    font_size: f32,
    line_height: f32,
    color: Rgba<u8>,
    offset_x: i32,
    offset_y: i32,
    font_family: &str,
    font_weight: FontWeight,
    font_style: FontStyle,
) {
    let metrics = Metrics {
        font_size,
        line_height,
    };
    let attrs = Attrs::new()
        .family(Family::Name(font_family))
        .weight(font_weight)
        .style(font_style);

    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_text(line, &attrs, Shaping::Basic, None);
    buffer.shape_until_scroll(font_system, false);

    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let physical = glyph.physical((0.0, 0.0), 1.0);

            if let Some(glyph_img) = swash_cache.get_image(font_system, physical.cache_key) {
                let glyph_x = x + physical.x + offset_x;
                let glyph_y = y + physical.y + offset_y;
                draw_glyph(img, glyph_img, glyph_x, glyph_y, color);
            }
        }
    }
}

/// Write a single glyph's alpha channel into the mask buffer using max compositing.
///
/// This avoids the blur that would result from normal alpha blending when
/// multiple overlapping glyphs are rendered to the same mask.
pub fn write_glyph_alpha(
    mask: &mut [u8],
    mask_width: u32,
    mask_height: u32,
    glyph_img: &SwashImage,
    offset_x: i32,
    offset_y: i32,
) {
    let placement = &glyph_img.placement;
    let data = &glyph_img.data;
    let w = mask_width as i32;
    let h = mask_height as i32;

    match &glyph_img.content {
        SwashContent::Mask => {
            let mut idx = 0;
            for dy in 0..placement.height as i32 {
                for dx in 0..placement.width as i32 {
                    let a = data[idx];
                    idx += 1;
                    if a > 0 {
                        let px = offset_x + placement.left + dx;
                        let py = offset_y - placement.top + dy;
                        if px >= 0 && px < w && py >= 0 && py < h {
                            let off = (py as u32 * mask_width + px as u32) as usize;
                            mask[off] = mask[off].max(a);
                        }
                    }
                }
            }
        }
        SwashContent::Color => {
            let mut idx = 0;
            for dy in 0..placement.height as i32 {
                for dx in 0..placement.width as i32 {
                    let a = data[idx + 3];
                    idx += 4;
                    if a > 0 {
                        let px = offset_x + placement.left + dx;
                        let py = offset_y - placement.top + dy;
                        if px >= 0 && px < w && py >= 0 && py < h {
                            let off = (py as u32 * mask_width + px as u32) as usize;
                            mask[off] = mask[off].max(a);
                        }
                    }
                }
            }
        }
        SwashContent::SubpixelMask => {
            let mut idx = 0;
            for dy in 0..placement.height as i32 {
                for dx in 0..placement.width as i32 {
                    let r = data[idx] as u32;
                    let g = data[idx + 1] as u32;
                    let b = data[idx + 2] as u32;
                    let a = ((r + g + b) / 3) as u8;
                    idx += 3;
                    if a > 0 {
                        let px = offset_x + placement.left + dx;
                        let py = offset_y - placement.top + dy;
                        if px >= 0 && px < w && py >= 0 && py < h {
                            let off = (py as u32 * mask_width + px as u32) as usize;
                            mask[off] = mask[off].max(a);
                        }
                    }
                }
            }
        }
    }
}

/// Render text lines to an alpha mask buffer.
///
/// Uses max-alpha compositing (not blending) to avoid blur from overlapping glyphs.
/// Each logical line (split by `\\N`) is shaped and laid out independently.
///
/// - `line_x_offsets`: Additional per-line x offset (for text alignment).
///   If empty or shorter than `lines`, missing entries default to 0.
pub fn render_text_alpha_mask(
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    lines: &[&str],
    base_x: i32,
    baseline_y: i32,
    line_x_offsets: &[i32],
    font_size: f32,
    line_height: f32,
    font_family: &str,
    font_weight: FontWeight,
    font_style: FontStyle,
    mask_width: u32,
    mask_height: u32,
) -> Vec<u8> {
    let mut mask = vec![0u8; (mask_width * mask_height) as usize];
    let metrics = Metrics {
        font_size,
        line_height,
    };
    let attrs = Attrs::new()
        .family(Family::Name(font_family))
        .weight(font_weight)
        .style(font_style);

    let mut current_y = baseline_y;
    for (i, line) in lines.iter().enumerate() {
        let x_offset = line_x_offsets.get(i).copied().unwrap_or(0);
        let x = base_x + x_offset;

        let mut buffer = Buffer::new(font_system, metrics);
        buffer.set_text(line, &attrs, Shaping::Basic, None);
        buffer.shape_until_scroll(font_system, false);

        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                if let Some(glyph_img) = swash_cache.get_image(font_system, physical.cache_key) {
                    let glyph_x = x + physical.x;
                    let glyph_y = current_y + physical.y;
                    write_glyph_alpha(
                        &mut mask,
                        mask_width,
                        mask_height,
                        &glyph_img,
                        glyph_x,
                        glyph_y,
                    );
                }
            }
        }

        current_y += line_height as i32;
    }

    mask
}

/// Create a text image with externally provided font system and cache.
///
/// This is the batch-friendly variant of [`create_text_image`] — callers can
/// reuse the same `FontSystem` and `SwashCache` across multiple calls, which
/// is essential for high-throughput scenarios like danmaku rendering where
/// hundreds of text items share the same font.
pub fn create_text_image_with_cache(
    text: &str,
    style: &SubtitleStyle,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
) -> Result<RgbaImage> {
    if !style.font_path.exists() {
        return Err(Error::InvalidFile(format!(
            "No found font: `{}`",
            style.font_path.display()
        )));
    }

    let preferred_family = if style.font_family.is_empty() {
        None
    } else {
        Some(style.font_family.as_str())
    };
    let preferred_style = if style.font_style.is_empty() {
        None
    } else {
        Some(style.font_style.as_str())
    };
    let (font_family, font_weight, font_fs_style) = load_font(
        &style.font_path,
        font_system,
        preferred_family,
        preferred_style,
    )?;
    let font_size = style.font_size.max(1) as f32;

    let (text_width, text_height, baseline_offset, line_widths) = calculate_text_size(
        font_system,
        text,
        font_size,
        &font_family,
        font_weight,
        font_fs_style,
        None,
    )?;

    let padding = style.padding.unwrap_or(0);
    let outline_width = style.outline_width.unwrap_or(0);
    let border_width = style.border_width.unwrap_or(0);
    let total_padding = padding + outline_width + border_width;

    let img_width = text_width + 2 * total_padding;
    let img_height = text_height + 2 * total_padding;

    let mut img = RgbaImage::new(img_width.max(1), img_height.max(1));

    if text_width == 0 || text_height == 0 || text.is_empty() {
        return Ok(img);
    }

    let primary_color = style.primary_color.unwrap_or(Rgba([255, 255, 255, 255]));
    let outline_color = style.outline_color;
    let background_color = style.background_color.as_ref().map(|c| *c);
    let border_radius = style.border_radius.unwrap_or(0);
    let border_color = style.border_color.as_ref().map(|c| *c);

    let lines: Vec<&str> = text.split("\\N").collect();
    let line_height = font_size * 1.2;
    let text_alignment = style.text_alignment;

    // Background and border area dimensions
    let bg_width = text_width + 2 * (padding + outline_width);
    let bg_height = text_height + 2 * (padding + outline_width);
    let bg_x = border_width;
    let bg_y = border_width;

    // Draw background first (if specified)
    if let Some(bg_color) = background_color {
        draw_rounded_rect_fill_aa(
            &mut img,
            bg_x,
            bg_y,
            bg_width,
            bg_height,
            border_radius,
            bg_color,
        );
    }

    // Draw border with anti-aliasing (stroke only, around background)
    if let Some(b_color) = border_color {
        if border_width > 0 {
            draw_rounded_border_aa(
                &mut img,
                bg_x,
                bg_y,
                bg_width,
                bg_height,
                border_width,
                border_radius,
                b_color,
            );
        }
    }

    let base_x = total_padding as i32;
    let baseline_y = (total_padding as i32) + baseline_offset;
    let max_width = text_width as f32;

    // Render text to alpha mask (single pass, max-alpha compositing)
    let line_x_offsets: Vec<i32> = line_widths
        .iter()
        .map(|&w| calculate_line_x_offset(text_alignment, max_width, w))
        .collect();
    let text_mask = render_text_alpha_mask(
        font_system,
        swash_cache,
        &lines,
        base_x,
        baseline_y,
        &line_x_offsets,
        font_size,
        line_height,
        &font_family,
        font_weight,
        font_fs_style,
        img_width.max(1),
        img_height.max(1),
    );

    // Draw outline: dilate the text mask, then fill with outline color
    // Skip if outline color is not set (None) or transparent (alpha == 0)
    if let Some(oc) = outline_color
        && outline_width > 0
        && oc[3] > 0
    {
        let outline_mask = dilate_mask(
            &text_mask,
            img_width.max(1),
            img_height.max(1),
            outline_width,
        );
        fill_from_mask(&mut img, &outline_mask, oc);
    }

    // Draw main text on top of outline
    fill_from_mask(&mut img, &text_mask, primary_color);

    Ok(img)
}

pub fn create_text_image(text: &str, style: &SubtitleStyle) -> Result<RgbaImage> {
    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();
    create_text_image_with_cache(text, style, &mut font_system, &mut swash_cache)
}

pub fn render_text_to_image(img: &mut RgbaImage, text: &str, style: &SubtitleStyle) -> Result<()> {
    let scaled_style = style.scaled_for_resolution(img.width(), img.height());

    if !scaled_style.font_path.exists() {
        return Err(Error::InvalidFile(format!(
            "No found fond: `{}`",
            scaled_style.font_path.display()
        )));
    };

    let mut font_system = FontSystem::new();
    let preferred_family = if scaled_style.font_family.is_empty() {
        None
    } else {
        Some(scaled_style.font_family.as_str())
    };
    let preferred_style = if scaled_style.font_style.is_empty() {
        None
    } else {
        Some(scaled_style.font_style.as_str())
    };
    let (font_family, font_weight, font_fs_style) = load_font(
        &scaled_style.font_path,
        &mut font_system,
        preferred_family,
        preferred_style,
    )?;
    let font_size = scaled_style.font_size.max(1) as f32;

    // Calculate available width for word-wrap based on horizontal margin.
    // Only enable word-wrap when margin_horizontal is explicitly set (not None).
    let max_text_width = scaled_style.margin_horizontal.and_then(|margin_h| {
        let available_width = img.width().saturating_sub(2 * margin_h);
        if available_width > 0 {
            Some(available_width as f32)
        } else {
            None
        }
    });

    let (text_width, text_height, baseline_offset, line_widths) = calculate_text_size(
        &mut font_system,
        text,
        font_size,
        &font_family,
        font_weight,
        font_fs_style,
        max_text_width,
    )?;

    let (x, y) = calculate_position(
        scaled_style.alignment,
        scaled_style.margin_vertical,
        scaled_style.margin_horizontal,
        text_width,
        text_height,
        img.width(),
        img.height(),
    );

    let primary_color = scaled_style
        .primary_color
        .unwrap_or(Rgba([255, 255, 255, 255]));

    let outline_color = scaled_style.outline_color;

    let background_color = scaled_style.background_color.as_ref().map(|c| *c);
    let border_color = scaled_style.border_color.as_ref().map(|c| *c);
    let outline_width = scaled_style.outline_width.unwrap_or(0);
    let padding = scaled_style.padding.unwrap_or(0);
    let border_radius = scaled_style.border_radius.unwrap_or(0);
    let border_width = scaled_style.border_width.unwrap_or(0);

    let line_height = font_size * 1.2;
    let text_alignment = scaled_style.text_alignment;
    let max_width = text_width as f32;

    // Background area dimensions (text + padding)
    let bg_x = x.saturating_sub(padding);
    let bg_y = y.saturating_sub(padding);
    let bg_width = text_width + 2 * padding;
    let bg_height = text_height + 2 * padding;

    // Draw background first (if specified)
    if let Some(bg_color) = background_color {
        draw_rounded_rect_fill_aa(
            img,
            bg_x,
            bg_y,
            bg_width,
            bg_height,
            border_radius,
            bg_color,
        );
    }

    // Draw border with anti-aliasing (stroke only, around background)
    if let Some(b_color) = border_color {
        if border_width > 0 {
            draw_rounded_border_aa(
                img,
                bg_x,
                bg_y,
                bg_width,
                bg_height,
                border_width,
                border_radius,
                b_color,
            );
        }
    }

    let mut swash_cache = SwashCache::new();

    // Create temporary image for text + outline area \\N segment with word-wrap, matching calculate_text_size logic
    let lines: Vec<&str> = text.split("\\N").collect();
    let render_metrics = Metrics {
        font_size,
        line_height,
    };
    let render_attrs = Attrs::new()
        .family(Family::Name(&font_family))
        .weight(font_weight)
        .style(font_fs_style);

    // Collect all layout runs from per-segment buffers
    // Each segment produces one or more visual lines (via word-wrap)
    struct VisualLine {
        glyphs: Vec<cosmic_text::LayoutGlyph>,
    }
    let mut visual_lines: Vec<VisualLine> = Vec::new();

    for line in &lines {
        let mut buffer = Buffer::new(&mut font_system, render_metrics);
        buffer.set_text(line, &render_attrs, Shaping::Basic, None);
        if max_text_width.is_some() {
            buffer.set_size(max_text_width, None);
        }
        buffer.shape_until_scroll(&mut font_system, false);

        let mut has_run = false;
        for run in buffer.layout_runs() {
            has_run = true;
            visual_lines.push(VisualLine {
                glyphs: run.glyphs.to_vec(),
            });
        }
        if !has_run {
            visual_lines.push(VisualLine { glyphs: Vec::new() });
        }
    }

    // Create temporary image for text + outline area and render via alpha mask
    // This avoids blur from overlapping anti-aliased glyph renders
    let temp_w = text_width + 2 * outline_width;
    let temp_h = text_height + 2 * outline_width;
    if temp_w == 0 || temp_h == 0 {
        return Ok(());
    }

    let mut temp_img = RgbaImage::new(temp_w, temp_h);
    let temp_base_x = outline_width as i32;
    let temp_baseline_y = outline_width as i32 + baseline_offset;

    // Render visual lines to alpha mask
    let mut text_mask = vec![0u8; (temp_w * temp_h) as usize];
    let mut current_y = temp_baseline_y;
    for (i, visual_line) in visual_lines.iter().enumerate() {
        let line_x_offset = calculate_line_x_offset(text_alignment, max_width, line_widths[i]);
        let line_x = temp_base_x + line_x_offset;
        for glyph in &visual_line.glyphs {
            let physical = glyph.physical((0.0, 0.0), 1.0);
            if let Some(glyph_img) = swash_cache.get_image(&mut font_system, physical.cache_key) {
                let glyph_x = line_x + physical.x;
                let glyph_y = current_y + physical.y;
                write_glyph_alpha(&mut text_mask, temp_w, temp_h, &glyph_img, glyph_x, glyph_y);
            }
        }
        current_y += line_height as i32;
    }

    // Draw outline: dilate the text mask, then fill with outline color
    // Skip if outline color is not set (None) or transparent (alpha == 0)
    if let Some(oc) = outline_color {
        if outline_width > 0 && oc[3] > 0 {
            let outline_mask = dilate_mask(&text_mask, temp_w, temp_h, outline_width);
            fill_from_mask(&mut temp_img, &outline_mask, oc);
        }
    }

    // Draw main text on top of outline
    fill_from_mask(&mut temp_img, &text_mask, primary_color);

    // Composite temporary image onto target
    let composite_x = x as i32 - outline_width as i32;
    let composite_y = y as i32 - outline_width as i32;
    composite_image(img, &temp_img, composite_x, composite_y);

    Ok(())
}
