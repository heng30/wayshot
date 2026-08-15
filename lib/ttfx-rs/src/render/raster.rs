//! Rasterization: turn a canvas of cells into an RGBA bitmap.
//!
//! Each character cell becomes a `cell_width x cell_height` pixel block:
//! background color fills the block, then the whole line of text is laid out
//! with cosmic-text in one pass (like the subtitle renderer) so CJK/full-width
//! glyphs keep their natural size instead of being squeezed into an ASCII
//! cell. Terminal-style visual attributes (reverse video, dim, underline,
//! strikethrough, hidden) are honored.

use image::{Rgba, RgbaImage};

use crate::engine::terminal::FrameCell;
use crate::utils::graphics::Color;

use super::font::Font;
use super::RenderConfig;

const DEFAULT_FOREGROUND_RGBA: Rgba<u8> = Rgba([255, 255, 255, 255]);

fn rgba(color: &Color) -> Rgba<u8> {
    let (r, g, b) = color.rgb_ints();
    Rgba([r, g, b, 255])
}

fn dim(color: Rgba<u8>, factor: f32) -> Rgba<u8> {
    Rgba([
        (color[0] as f32 * factor) as u8,
        (color[1] as f32 * factor) as u8,
        (color[2] as f32 * factor) as u8,
        255,
    ])
}

/// Fill the rect [x0, x0+w) x [y0, y0+h), clipped to the image.
fn fill_rect(img: &mut RgbaImage, x0: i64, y0: i64, w: u32, h: u32, color: Rgba<u8>) {
    let (img_w, img_h) = (img.width() as i64, img.height() as i64);
    let x1 = (x0 + w as i64).min(img_w);
    let y1 = (y0 + h as i64).min(img_h);
    let x0 = x0.max(0);
    let y0 = y0.max(0);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    for y in y0..y1 {
        for x in x0..x1 {
            img.get_pixel_mut(x as u32, y as u32).0 = color.0;
        }
    }
}

/// Source-over blend of a coverage-weighted foreground pixel.
fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, fg: Rgba<u8>, coverage: f32) {
    if coverage <= 0.0 {
        return;
    }
    let px = img.get_pixel_mut(x, y);
    let a = coverage;
    for i in 0..3 {
        px.0[i] = (fg.0[i] as f32 * a + px.0[i] as f32 * (1.0 - a)) as u8;
    }
    px.0[3] = 255;
}

/// Draw a horizontal line across a cell at relative height `fraction`
/// (0 = top of the cell, 1 = bottom), thickness in pixels.
#[allow(clippy::too_many_arguments)]
fn draw_hline(
    img: &mut RgbaImage,
    x0: i64,
    y0: i64,
    w: u32,
    h: u32,
    fraction: f32,
    thickness: u32,
    color: Rgba<u8>,
) {
    let y = y0 + (h as f32 * fraction) as i64 - thickness as i64 / 2;
    fill_rect(img, x0, y, w, thickness, color);
}

/// Draw one full line of text into the bitmap at pixel row `y0`, horizontally
/// centered on the image.
///
/// The whole line is shaped in a single cosmic-text buffer (fallback fonts
/// such as CJK are picked up automatically) and every glyph is drawn at its
/// real position, so full-width glyphs keep their natural size instead of
/// being squeezed into an ASCII cell. The glyph color is taken from the
/// character cell the glyph's cluster belongs to (`fg_by_char` is indexed by
/// character position); `default_fg` covers spacing gaps.
#[allow(clippy::too_many_arguments)]
fn draw_text_line(
    img: &mut RgbaImage,
    font: &Font,
    text: &str,
    y0: i64,
    cell_w: u32,
    cell_h: u32,
    size: f32,
    fg_by_char: &[Rgba<u8>],
    columns: &[i64],
    default_fg: Rgba<u8>,
) {
    use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, SwashCache};

    if text.is_empty() || fg_by_char.is_empty() {
        return;
    }
    let mut system = font.font_system.lock().expect("font system poisoned");
    let attrs = Attrs::new().family(Family::Name(&font.primary_family));

    // Layout the whole line once. Width budget is generous so nothing wraps;
    // the real extents are measured below for vertical centering.
    let mut buffer = Buffer::new(&mut system, Metrics::new(size, size));
    buffer.set_size(
        Some(text.chars().count() as f32 * cell_w as f32 * 2.0 + 1.0),
        Some(size * 1.2),
    );
    buffer.set_text(text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut system, false);

    let mut cache = SwashCache::new();

    // 字形在行内相对 cosmic-text 布局的动画偏移：字符当前列与静态列（text
    // 索引）之差乘 cell_w。静止时偏移为 0，字形正好落在 cosmic-text 布局位置；
    // 动画中字符移动（飞入/散开）时字形跟随 cell 位置。
    let col_offset = |char_idx: usize| -> f32 {
        columns
            .get(char_idx)
            .map(|&c| (c - char_idx as i64 - 1) as f32 * cell_w as f32)
            .unwrap_or(0.0)
    };

    // Measure the glyph bounding box for vertical centering inside the row.
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    for line in buffer.layout_runs() {
        for glyph in line.glyphs {
            let char_idx = text[..glyph.start].chars().count();
            let physical = glyph.physical((0.0, line.line_y), 1.0);
            if let Some(image) = cache.get_image(&mut system, physical.cache_key) {
                let gx = physical.x as f32 + image.placement.left as f32 + col_offset(char_idx);
                let gy = physical.y as f32 - image.placement.top as f32;
                min_x = min_x.min(gx);
                max_x = max_x.max(gx + image.placement.width as f32);
                min_y = min_y.min(gy);
                max_y = max_y.max(gy + image.placement.height as f32);
            }
        }
    }
    if min_y > max_y || min_x > max_x {
        return;
    }
    // 水平居中到整个画布，垂直居中到行高。
    let dx = (img.width() as f32 - (max_x - min_x)) / 2.0 - min_x;
    let dy = y0 as f32 + (cell_h as f32 - (max_y - min_y)) / 2.0 - min_y;

    let img_w = img.width() as i64;
    let img_h = img.height() as i64;
    for line in buffer.layout_runs() {
        for glyph in line.glyphs {
            // Map the cluster's byte offset back to a character index.
            let char_idx = text[..glyph.start].chars().count();
            let color = fg_by_char.get(char_idx).copied().unwrap_or(default_fg);

            let physical = glyph.physical((0.0, line.line_y), 1.0);
            let Some(image) = cache.get_image(&mut system, physical.cache_key) else {
                continue;
            };
            let gx = (physical.x as f32 + image.placement.left as f32 + dx + col_offset(char_idx))
                as i64;
            let gy = (physical.y as f32 - image.placement.top as f32 + dy) as i64;
            let (bw, bh) = (image.placement.width as i64, image.placement.height as i64);
            let stride = match image.content {
                cosmic_text::SwashContent::Mask => 1,
                cosmic_text::SwashContent::SubpixelMask => 3,
                cosmic_text::SwashContent::Color => 4,
            };
            for py in 0..bh {
                for px in 0..bw {
                    let i = ((py * bw + px) * stride) as usize;
                    let alpha = match image.content {
                        cosmic_text::SwashContent::Mask => image.data[i] as f32 / 255.0,
                        cosmic_text::SwashContent::SubpixelMask => {
                            (image.data[i] + image.data[i + 1] + image.data[i + 2]) as f32
                                / 3.0
                                / 255.0
                        }
                        cosmic_text::SwashContent::Color => image.data[i + 3] as f32 / 255.0,
                    };
                    if alpha <= 0.0 {
                        continue;
                    }
                    let x = gx + px;
                    let y = gy + py;
                    if x >= 0 && y >= 0 && x < img_w && y < img_h {
                        blend_pixel(img, x as u32, y as u32, color, alpha);
                    }
                }
            }
        }
    }
}

/// Resolve the foreground color of a cell, applying reverse/dim the same way
/// the background pass does.
fn cell_fg(cell: &FrameCell<'_>, background: Rgba<u8>) -> Rgba<u8> {
    let mut fg = match cell.fg {
        Some(f) => rgba(f),
        None => DEFAULT_FOREGROUND_RGBA,
    };
    if cell.reverse {
        fg = match cell.bg {
            Some(b) => rgba(b),
            None => background,
        };
    }
    if cell.dim {
        fg = dim(fg, 0.5);
    }
    fg
}

/// Rasterize a frame's cells into a fresh RGBA bitmap of the configured size.
pub fn rasterize(cells: &[FrameCell<'_>], render: &RenderConfig) -> RgbaImage {
    let (cell_w, cell_h) = render.resolved_cell_size();
    let (img_w, img_h) = (render.width, render.height);
    let (br, bg, bb) = render.background.rgb_ints();
    let background = Rgba([br, bg, bb, render.background_alpha]);

    let mut img = RgbaImage::from_pixel(img_w, img_h, background);

    // Font size: cell height. Full-width glyphs may spill past the cell
    // width; the canvas was sized with wcwidth so nothing gets clipped.
    let font = &render.font;
    let size = (cell_h as f32).max(2.0);

    // Group cells by engine row (1-based from the bottom; pixel y grows down).
    let mut rows: Vec<Vec<&FrameCell<'_>>> = Vec::new();
    for cell in cells {
        let idx = (cell.row - 1) as usize;
        if idx >= rows.len() {
            rows.resize(idx + 1, Vec::new());
        }
        rows[idx].push(cell);
    }

    for row_cells in &rows {
        if row_cells.is_empty() {
            continue;
        }
        let mut row_cells = row_cells.clone();
        row_cells.sort_by_key(|c| c.column);

        // Fill backgrounds cell by cell (keeps per-cell bg colors exact).
        for cell in &row_cells {
            let x0 = (cell.column - 1) * cell_w as i64;
            let y0 = img_h as i64 - cell.row * cell_h as i64;
            let bg = match cell.bg {
                Some(b) => rgba(b),
                None => background,
            };
            let mut bg = if cell.reverse {
                match cell.fg {
                    Some(f) => rgba(f),
                    None => DEFAULT_FOREGROUND_RGBA,
                }
            } else {
                bg
            };
            if cell.dim && cell.reverse {
                bg = dim(bg, 0.5);
            }
            fill_rect(&mut img, x0, y0, cell_w, cell_h, bg);
        }

        let first = row_cells[0];
        let y0 = img_h as i64 - first.row * cell_h as i64;

        // Rebuild the line text, padding gaps the engine dropped (default
        // background cells) with spaces so glyph positions stay aligned.
        let mut text = String::new();
        let mut fg_by_char: Vec<Rgba<u8>> = Vec::new();
        let mut columns: Vec<i64> = Vec::new();
        let mut cell_idx = 0usize;
        for col in first.column..=row_cells[row_cells.len() - 1].column {
            if cell_idx < row_cells.len() && row_cells[cell_idx].column == col {
                let cell = row_cells[cell_idx];
                let sym = cell.symbol.chars().next().unwrap_or(' ');
                // Hidden cells render as spacing-only.
                text.push(if cell.hidden { ' ' } else { sym });
                fg_by_char.push(cell_fg(cell, background));
                columns.push(cell.column);
                cell_idx += 1;
            } else {
                text.push(' ');
                fg_by_char.push(DEFAULT_FOREGROUND_RGBA);
                columns.push(col);
            }
        }

        // 去掉行首/行尾的空格 cell（非内容，参与字形测量会把整行挤偏）。
        let chars: Vec<char> = text.chars().collect();
        let mut start = 0;
        let mut end = chars.len();
        while start < end && chars[start] == ' ' {
            start += 1;
        }
        while end > start && chars[end - 1] == ' ' {
            end -= 1;
        }
        if start < end {
            let text: String = chars[start..end].iter().collect();
            let fg = &fg_by_char[start..end];
            let cols = &columns[start..end];
            draw_text_line(
                &mut img,
                font,
                &text,
                y0,
                cell_w,
                cell_h,
                size,
                fg,
                cols,
                DEFAULT_FOREGROUND_RGBA,
            );
        }

        // Decoration lines stay per cell.
        for cell in &row_cells {
            let x0 = (cell.column - 1) * cell_w as i64;
            let y0 = img_h as i64 - cell.row * cell_h as i64;
            let fg = cell_fg(cell, background);
            if cell.underline {
                draw_hline(&mut img, x0, y0, cell_w, cell_h, 0.85, 1.max(cell_h / 16), fg);
            }
            if cell.strike {
                draw_hline(&mut img, x0, y0, cell_w, cell_h, 0.5, 1.max(cell_h / 16), fg);
            }
        }
    }

    img
}
