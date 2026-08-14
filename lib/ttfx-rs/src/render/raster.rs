//! Rasterization: turn a canvas of cells into an RGBA bitmap.
//!
//! Each character cell becomes a `cell_width x cell_height` pixel block:
//! background color fills the block, the glyph is drawn centered with
//! antialiasing, and terminal-style visual attributes (reverse video, dim,
//! underline, strikethrough, hidden) are honored.

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

/// Draw `text` into the cell starting at pixel (x0, y0), horizontally
/// centered, using `size`-point glyphs. Glyphs missing from every loaded
/// font are skipped silently. Layout and rasterization go through
/// cosmic-text, so fallback fonts (e.g. CJK) are picked up automatically.
#[allow(clippy::too_many_arguments)]
fn draw_text(
    img: &mut RgbaImage,
    font: &Font,
    text: &str,
    x0: i64,
    y0: i64,
    cell_w: u32,
    cell_h: u32,
    size: f32,
    color: Rgba<u8>,
) {
    use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, SwashCache};

    if text.is_empty() {
        return;
    }
    let mut system = font.font_system.lock().expect("font system poisoned");
    let attrs = Attrs::new().family(Family::Name(&font.primary_family));

    // Layout and measure; shrink the font size when the run is wider than
    // the cell (full-width glyphs such as CJK are wider than the monospace
    // advance) so adjacent cells never overlap.
    let (buffer, dx, dy) = {
        let mut size = size;
        let (mut buffer, mut min_x, mut min_y, mut max_x, mut max_y);
        loop {
            buffer = Buffer::new(&mut system, Metrics::new(size, size));
            buffer.set_size(Some(cell_w as f32), Some(cell_h as f32));
            buffer.set_text(text, &attrs, Shaping::Advanced, None);
            buffer.shape_until_scroll(&mut system, false);
            let mut cache = SwashCache::new();
            min_x = f32::MAX;
            min_y = f32::MAX;
            max_x = f32::MIN;
            max_y = f32::MIN;
            for line in buffer.layout_runs() {
                for glyph in line.glyphs {
                    let physical = glyph.physical((0.0, line.line_y), 1.0);
                    if let Some(image) = cache.get_image(&mut system, physical.cache_key) {
                        let gx = physical.x as f32 + image.placement.left as f32;
                        let gy = physical.y as f32 - image.placement.top as f32;
                        min_x = min_x.min(gx);
                        min_y = min_y.min(gy);
                        max_x = max_x.max(gx + image.placement.width as f32);
                        max_y = max_y.max(gy + image.placement.height as f32);
                    }
                }
            }
            let width = max_x - min_x;
            if width <= cell_w as f32 || size <= 2.0 {
                break;
            }
            size = (size * cell_w as f32 / width * 0.95).max(2.0);
        }
        if min_x > max_x || min_y > max_y {
            return;
        }
        let dx = x0 as f32 + (cell_w as f32 - (max_x - min_x)) / 2.0 - min_x;
        let dy = y0 as f32 + (cell_h as f32 - (max_y - min_y)) / 2.0 - min_y;
        (buffer, dx, dy)
    };

    let mut cache = SwashCache::new();
    let img_w = img.width() as i64;
    let img_h = img.height() as i64;
    for line in buffer.layout_runs() {
        for glyph in line.glyphs {
            let physical = glyph.physical((0.0, line.line_y), 1.0);
            let Some(image) = cache.get_image(&mut system, physical.cache_key) else {
                continue;
            };
            let gx = (physical.x as f32 + image.placement.left as f32 + dx) as i64;
            let gy = (physical.y as f32 - image.placement.top as f32 + dy) as i64;
            let (bw, bh) = (image.placement.width as i64, image.placement.height as i64);
            // SwashCache renders with Format::Alpha (1 channel) unless a color
            // bitmap font wins, in which case the image is RGBA.
            let stride = match image.content {
                cosmic_text::SwashContent::Mask => 1,
                cosmic_text::SwashContent::SubpixelMask => 3,
                cosmic_text::SwashContent::Color => 4,
            };
            for py in 0..bh {
                for px in 0..bw {
                    let i = ((py * bw + px) * stride) as usize;
                    let alpha = image.data[i] as f32 / 255.0;
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

/// Rasterize a frame's cells into a fresh RGBA bitmap of the configured size.
pub fn rasterize(cells: &[FrameCell<'_>], render: &RenderConfig) -> RgbaImage {
    let (cell_w, cell_h) = render.resolved_cell_size();
    let (img_w, img_h) = (render.width, render.height);
    let background = rgba(&render.background);

    let mut img = RgbaImage::from_pixel(img_w, img_h, background);

    // Font size: cell height usually; shrink when the font is wider than the
    // cell so glyphs never overflow their block.
    let font = &render.font;
    let size = {
        let preferred = cell_h as f32;
        let advance = font.advance(preferred);
        if advance > cell_w as f32 {
            (preferred * cell_w as f32 / advance).max(2.0)
        } else {
            preferred
        }
    };

    for cell in cells {
        let x0 = (cell.column - 1) * cell_w as i64;
        // Engine rows are 1-based from the bottom; pixel y grows downward.
        let y0 = img_h as i64 - cell.row * cell_h as i64;

        let (mut fg, mut bg) = match (cell.fg, cell.bg) {
            (Some(f), Some(b)) => (rgba(f), rgba(b)),
            (Some(f), None) => (rgba(f), background),
            (None, Some(b)) => (DEFAULT_FOREGROUND_RGBA, rgba(b)),
            (None, None) => (DEFAULT_FOREGROUND_RGBA, background),
        };
        if cell.reverse {
            std::mem::swap(&mut fg, &mut bg);
        }
        if cell.dim {
            fg = dim(fg, 0.5);
        }

        fill_rect(&mut img, x0, y0, cell_w, cell_h, bg);
        if cell.hidden || cell.symbol == " " {
            continue;
        }

        draw_text(
            &mut img,
            font,
            cell.symbol,
            x0,
            y0,
            cell_w,
            cell_h,
            size,
            fg,
        );

        if cell.underline {
            draw_hline(
                &mut img,
                x0,
                y0,
                cell_w,
                cell_h,
                0.85,
                1.max(cell_h / 16),
                fg,
            );
        }
        if cell.strike {
            draw_hline(
                &mut img,
                x0,
                y0,
                cell_w,
                cell_h,
                0.5,
                1.max(cell_h / 16),
                fg,
            );
        }
    }

    img
}
