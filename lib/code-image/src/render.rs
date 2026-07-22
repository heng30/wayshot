use crate::{
    TerminalStyle,
    config::CodeHighlightConfig,
    error::{CodeImageError, Result},
    theme::{get_bg_color, get_line_num_color},
};
use image::RgbaImage;
use resvg::{render, tiny_skia::Pixmap};
use std::path::Path;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};
use ttf_parser::Face;
use usvg::{Options, Tree as SvgTree, fontdb};

const TITLE_FONT_SIZE_RATIO: f64 = 0.8; // title ≈ 80% of code font_size
const BORDER_RADIUS: f64 = 10.0;
const COLOR_BLACK: &str = "#000000";
const COLOR_DARK_BG: &str = "#1e1e1e";
const COLOR_RED_BTN: &str = "#ff5f57";
const COLOR_YELLOW_BTN: &str = "#febc2e";
const COLOR_GREEN_BTN: &str = "#28c840";
const COLOR_WHITE: &str = "#ffffff";
const COLOR_GRAY: &str = "#888888";
const COLOR_DARKER_BG: &str = "#0d0d0d";
const COLOR_DARKER_TITLE: &str = "#1a1a1a";
const COLOR_WIN_BG: &str = "#0c0c0c";
const COLOR_WIN_TITLE: &str = "#1f1f1f";
const COLOR_WIN_TEXT: &str = "#cccccc";
const COLOR_WIN_LIGHT_BG: &str = "#ffffff";
const COLOR_WIN_LIGHT_TITLE: &str = "#f0f0f0";
const COLOR_WIN_LIGHT_TEXT: &str = "#333333";
const COLOR_GNOME_BG: &str = "#2d2d2d";
const COLOR_GNOME_TITLE: &str = "#3d3d3d";
const COLOR_GNOME_RED: &str = "#ff7b72";
const COLOR_GNOME_YELLOW: &str = "#ffa657";
const COLOR_GNOME_GREEN: &str = "#7ee787";
const COLOR_ITERM_RED: &str = "#ff5f56";
const COLOR_ITERM_RED_INNER: &str = "#e0443e";
const COLOR_ITERM_YELLOW: &str = "#ffbd2e";
const COLOR_ITERM_YELLOW_INNER: &str = "#dea123";
const COLOR_ITERM_GREEN: &str = "#27c93f";
const COLOR_ITERM_GREEN_INNER: &str = "#1aab29";
const COLOR_MACOS_BG: &str = "#ffffff";
const COLOR_MACOS_TITLE: &str = "#e8e8e8";
const COLOR_MACOS_TEXT: &str = "#333333";

// Escape special characters for XML/SVG.
pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .replace(' ', "&#160;") // Preserve spaces (using non-breaking space)
}

// Split text by ASCII/non-ASCII segments, returning (is_ascii, text_slice) list.
pub fn split_by_ascii(text: &str) -> Vec<(bool, &str)> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut current_is_ascii = text.chars().next().map(|c| c.is_ascii()).unwrap_or(true);

    for (i, c) in text.char_indices() {
        let is_ascii = c.is_ascii();
        if is_ascii != current_is_ascii {
            result.push((current_is_ascii, &text[start..i]));
            start = i;
            current_is_ascii = is_ascii;
        }
    }
    if start < text.len() {
        result.push((current_is_ascii, &text[start..]));
    }
    result
}

pub fn get_char_width(face: &Face, c: char) -> f64 {
    face.glyph_index(c)
        .and_then(|gid| face.glyph_hor_advance(gid))
        .map(|adv| adv as f64)
        .unwrap_or_else(|| {
            // fallback: ASCII uses 600 units, Chinese uses 1000 units
            if c.is_ascii() { 600.0 } else { 1000.0 }
        })
}

pub fn font_units_to_px(units: f64, units_per_em: u16, font_size: f64) -> f64 {
    units * font_size / units_per_em as f64
}

pub fn render_to_svg(code: &str, config: &CodeHighlightConfig) -> Result<String> {
    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = ps
        .find_syntax_by_extension(config.language.extension())
        .ok_or_else(|| {
            CodeImageError::SyntaxNotFoundError(config.language.extension().to_string())
        })?;

    let theme = ts
        .themes
        .get(&config.theme)
        .unwrap_or_else(|| &ts.themes["Solarized (dark)"]);

    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);
    let ranges: Vec<_> = syntect::util::LinesWithEndings::from(code)
        .map(|line| highlighter.highlight_line(line, &ps).unwrap())
        .collect();

    // Load fonts for precise width calculation
    let ascii_data =
        std::fs::read(&config.ascii_font_path).map_err(|e| CodeImageError::FontLoadError {
            path: config.ascii_font_path.to_string_lossy().to_string(),
            source: e,
        })?;
    let non_ascii_data =
        std::fs::read(&config.non_ascii_font_path).map_err(|e| CodeImageError::FontLoadError {
            path: config.non_ascii_font_path.to_string_lossy().to_string(),
            source: e,
        })?;

    let ascii_face = Face::parse(&ascii_data, 0)
        .map_err(|e| CodeImageError::FontParseError(format!("ASCII font: {}", e)))?;
    let non_ascii_face = Face::parse(&non_ascii_data, 0)
        .map_err(|e| CodeImageError::FontParseError(format!("Non-ASCII font: {}", e)))?;

    let line_height = config.line_height();
    let bg_color = config
        .bg_color
        .as_deref()
        .unwrap_or_else(|| get_bg_color(&config.theme));
    let line_num_color = get_line_num_color(&config.theme);

    // Calculate line number column width
    let line_count = ranges.len();
    let line_num_width = line_count.to_string().len();
    let line_num_padding = if config.line_numbers {
        (line_num_width as f64 + 1.0) * config.font_size * 0.6
    } else {
        0.0
    };

    let mut svg_lines = Vec::new();
    let mut y = config.padding + config.font_size; // Baseline y coordinate
    let mut line_num = 1;
    let mut max_x = config.padding + line_num_padding; // Track max x for adaptive width

    for line_ranges in &ranges {
        let line_text: String = line_ranges.iter().map(|(_, t)| *t).collect();

        if config.line_numbers {
            svg_lines.push(format!(
                r#"<text x="{}" y="{}" fill="{}" font-family="{}" font-size="{}">{}</text>"#,
                config.padding, y, line_num_color, config.ascii_font, config.font_size, line_num
            ));
        }

        if line_text.trim().is_empty() {
            y += line_height;
            line_num += 1;
            continue;
        }

        let code_x = config.padding + line_num_padding;
        let mut line_svg = String::new();
        let mut current_x = code_x; // Maintain cumulative x coordinate

        for (style, text) in line_ranges {
            let text = text.trim_end_matches('\n');
            if text.is_empty() {
                continue;
            }
            let color = style.foreground;
            let hex = format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b);

            // Split by ASCII/non-ASCII for font-specific rendering
            for (is_ascii, segment) in split_by_ascii(text) {
                let escaped_seg = escape_xml(segment);
                let font = if is_ascii {
                    &config.ascii_font
                } else {
                    &config.non_ascii_font
                };

                // Use font metrics for precise width calculation
                let (face, units_per_em) = if is_ascii {
                    (&ascii_face, ascii_face.units_per_em())
                } else {
                    (&non_ascii_face, non_ascii_face.units_per_em())
                };
                let segment_width: f64 = segment
                    .chars()
                    .map(|c| {
                        let units = get_char_width(face, c);
                        font_units_to_px(units, units_per_em, config.font_size)
                    })
                    .sum();

                // Use explicit x positioning
                line_svg.push_str(&format!(
                    r#"<tspan x="{}" fill="{}" font-family="{}">{}</tspan>"#,
                    current_x, hex, font, escaped_seg
                ));

                current_x += segment_width; // Accumulate x coordinate
            }
        }
        if !line_svg.is_empty() {
            svg_lines.push(format!(
                r#"<text y="{}" font-size="{}">{}</text>"#,
                y, config.font_size, line_svg
            ));
        }
        max_x = max_x.max(current_x); // Track max x
        y += line_height;
        line_num += 1;
    }

    // Adaptive width: max x coordinate + right padding
    let total_width = max_x + config.padding;
    let total_height = y + config.padding;

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{total_width}" height="{total_height}" viewBox="0 0 {total_width} {total_height}">
  <rect width="100%" height="100%" fill="{bg_color}"/>
  {lines}
</svg>"#,
        total_width = total_width,
        total_height = total_height,
        bg_color = bg_color,
        lines = svg_lines.join("\n")
    );

    // Wrap in terminal decoration if requested
    if let Some(terminal_style) = config.terminal {
        return wrap_in_terminal(
            svg,
            total_width,
            total_height,
            terminal_style,
            config,
            &bg_color,
        );
    }

    Ok(svg)
}

fn wrap_in_terminal(
    content_svg: String,
    content_width: f64,
    content_height: f64,
    style: TerminalStyle,
    config: &CodeHighlightConfig,
    bg_color: &str,
) -> Result<String> {
    // Content already includes its own padding, don't add extra
    let terminal_width = content_width;
    let terminal_height = content_height + title_bar_height(config.font_size);

    let decoration = match style {
        TerminalStyle::MacOS => {
            render_macos_decoration(terminal_width, terminal_height, bg_color, config)
        }
        TerminalStyle::MacOSDark => {
            render_macos_dark_decoration(terminal_width, terminal_height, bg_color, config)
        }
        TerminalStyle::Windows => {
            render_windows_decoration(terminal_width, terminal_height, bg_color, config)
        }
        TerminalStyle::WindowsDark => {
            render_windows_dark_decoration(terminal_width, terminal_height, bg_color, config)
        }
        TerminalStyle::Gnome => {
            render_gnome_decoration(terminal_width, terminal_height, bg_color, config)
        }
        TerminalStyle::ITerm => {
            render_iterm_decoration(terminal_width, terminal_height, bg_color, config)
        }
    };

    // Extract inner content from the original SVG (everything inside the svg tag)
    let inner_content = extract_inner_content(&content_svg);

    // Create the wrapped SVG
    let wrapped = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">
{}
  <g transform="translate(0, {})">
    {}
  </g>
</svg>"#,
        terminal_width,
        terminal_height,
        terminal_width,
        terminal_height,
        decoration,
        title_bar_height(config.font_size),
        inner_content
    );

    Ok(wrapped)
}

fn extract_inner_content(svg: &str) -> String {
    // Extract content between the svg opening tag and closing tag
    // Find the first > after <svg and the last </svg>
    if let Some(start) = svg.find('>') {
        if let Some(end) = svg.rfind("</svg>") {
            let inner = &svg[start + 1..end];
            // Remove the background rect and get just the code lines
            let lines: Vec<&str> = inner
                .lines()
                .filter(|line| !line.contains("rect width=\"100%\" height=\"100%\""))
                .collect();
            lines.join("\n").trim().to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

fn render_macos_decoration(
    width: f64,
    height: f64,
    _bg_color: &str,
    config: &CodeHighlightConfig,
) -> String {
    let title = config.terminal_title.as_deref().unwrap_or("Terminal");
    let title_font_size = config.font_size * TITLE_FONT_SIZE_RATIO;
    let title_y = title_bar_height(config.font_size) * 0.65;
    let title_text = render_styled_title(
        title,
        title_font_size,
        title_y,
        COLOR_MACOS_TEXT,
        &config.ascii_font,
        &config.non_ascii_font,
        &config.ascii_font_path,
        &config.non_ascii_font_path,
        TitleAnchor::Center(width / 2.0),
    );

    format!(
        r#"  <rect x="0" y="0" width="{width}" height="{height}" rx="{radius}" fill="{macos_bg}"/>
  <!-- Title bar background -->
  <rect x="0" y="0" width="{width}" height="{tbh}" rx="{radius}" fill="{macos_title}"/>
  <!-- Fix top corners for title bar -->
  <rect x="0" y="{fix_y}" width="{width}" height="10" fill="{macos_title}"/>
  <!-- Window buttons (red, yellow, green) -->
  <circle cx="20" cy="{btn_cy}" r="{btn_r}" fill="{red_btn}"/>
  <circle cx="{btn2_x}" cy="{btn_cy}" r="{btn_r}" fill="{yellow_btn}"/>
  <circle cx="{btn3_x}" cy="{btn_cy}" r="{btn_r}" fill="{green_btn}"/>
  <!-- Title text -->
  {title_text}"#,
        width = width,
        height = height,
        radius = BORDER_RADIUS,
        macos_bg = COLOR_MACOS_BG,
        macos_title = COLOR_MACOS_TITLE,
        tbh = title_bar_height(config.font_size),
        fix_y = title_bar_height(config.font_size) - BORDER_RADIUS,
        btn_cy = title_bar_height(config.font_size) / 2.0,
        btn_r = button_radius(config.font_size),
        red_btn = COLOR_RED_BTN,
        btn2_x = 20.0 + button_spacing(config.font_size),
        yellow_btn = COLOR_YELLOW_BTN,
        btn3_x = 20.0 + button_spacing(config.font_size) * 2.0,
        green_btn = COLOR_GREEN_BTN,
        title_text = title_text
    )
}

fn render_macos_dark_decoration(
    width: f64,
    height: f64,
    _bg_color: &str,
    config: &CodeHighlightConfig,
) -> String {
    let title = config.terminal_title.as_deref().unwrap_or("Terminal");
    let title_font_size = config.font_size * TITLE_FONT_SIZE_RATIO;
    let title_y = title_bar_height(config.font_size) * 0.65;
    let title_text = render_styled_title(
        title,
        title_font_size,
        title_y,
        COLOR_GRAY,
        &config.ascii_font,
        &config.non_ascii_font,
        &config.ascii_font_path,
        &config.non_ascii_font_path,
        TitleAnchor::Center(width / 2.0),
    );

    format!(
        r#"  <rect x="0" y="0" width="{width}" height="{height}" rx="{radius}" fill="{darker_bg}"/>
  <!-- Title bar background (darker) -->
  <rect x="0" y="0" width="{width}" height="{tbh}" rx="{radius}" fill="{darker_title}"/>
  <rect x="0" y="{fix_y}" width="{width}" height="10" fill="{darker_title}"/>
  <!-- Window buttons (dimmer for dark mode) -->
  <circle cx="20" cy="{btn_cy}" r="{btn_r}" fill="{red_btn}" opacity="0.8"/>
  <circle cx="{btn2_x}" cy="{btn_cy}" r="{btn_r}" fill="{yellow_btn}" opacity="0.8"/>
  <circle cx="{btn3_x}" cy="{btn_cy}" r="{btn_r}" fill="{green_btn}" opacity="0.8"/>
  <!-- Title text -->
  {title_text}"#,
        width = width,
        height = height,
        radius = BORDER_RADIUS,
        darker_bg = COLOR_DARKER_BG,
        darker_title = COLOR_DARKER_TITLE,
        tbh = title_bar_height(config.font_size),
        fix_y = title_bar_height(config.font_size) - BORDER_RADIUS,
        btn_cy = title_bar_height(config.font_size) / 2.0,
        btn_r = button_radius(config.font_size),
        red_btn = COLOR_RED_BTN,
        btn2_x = 20.0 + button_spacing(config.font_size),
        yellow_btn = COLOR_YELLOW_BTN,
        btn3_x = 20.0 + button_spacing(config.font_size) * 2.0,
        green_btn = COLOR_GREEN_BTN,
        title_text = title_text
    )
}

fn render_windows_decoration(
    width: f64,
    height: f64,
    _bg_color: &str,
    config: &CodeHighlightConfig,
) -> String {
    let title = config
        .terminal_title
        .as_deref()
        .unwrap_or("Windows Terminal");
    let win_border_radius = title_bar_border_radius(config.font_size);
    let win_title_height = title_bar_height(config.font_size);
    let wbw = win_button_width(config.font_size);
    let title_font_size = config.font_size * TITLE_FONT_SIZE_RATIO;
    let title_y = win_title_height * 0.64;
    let title_text = render_styled_title(
        title,
        title_font_size,
        title_y,
        COLOR_WIN_LIGHT_TEXT,
        &config.ascii_font,
        &config.non_ascii_font,
        &config.ascii_font_path,
        &config.non_ascii_font_path,
        TitleAnchor::Left(16.0),
    );

    // Button positions
    let close_x = width - wbw;
    let max_x = width - wbw * 2.0;
    let min_x = width - wbw * 3.0;

    // Minimize button line coordinates (horizontal line in center)
    let min_lx1 = min_x + wbw * 15.0 / 46.0;
    let min_lx2 = min_x + wbw * 31.0 / 46.0;
    let min_ly = win_title_height * 0.5;

    // Maximize button rect coordinates (square outline)
    let max_rx = max_x + wbw * 17.0 / 46.0;
    let max_ry = win_title_height * 0.33;

    // Close button × coordinates (2/3 size, centered in button area)
    let close_x1 = close_x + wbw * 17.0 / 46.0;
    let close_x2 = close_x + wbw * 29.0 / 46.0;
    let close_y1 = win_title_height * 0.36;
    let close_y2 = win_title_height * 0.69;

    format!(
        r#"  <rect x="0" y="0" width="{width}" height="{height}" rx="{radius}" fill="{win_bg}"/>
  <!-- Title bar -->
  <rect x="0" y="0" width="{width}" height="{tbh}" rx="{radius}" fill="{win_title}"/>
  <rect x="0" y="{fix_y}" width="{width}" height="{radius}" fill="{win_title}"/>
  <!-- Title text -->
  {title_text}
  <!-- Minimize button: horizontal line -->
  <rect x="{min_x}" y="0" width="{wbw}" height="{tbh}" fill="transparent"/>
  <line x1="{min_lx1}" y1="{min_ly}" x2="{min_lx2}" y2="{min_ly}" stroke="{win_text}" stroke-width="2" stroke-linecap="round"/>
  <!-- Maximize button: square outline -->
  <rect x="{max_x}" y="0" width="{wbw}" height="{tbh}" fill="transparent"/>
  <rect x="{max_rx}" y="{max_ry}" width="{max_size}" height="{max_size}" stroke="{win_text}" fill="transparent" stroke-width="1.5" rx="1"/>
  <!-- Close button: × shape (no background) -->
  <line x1="{close_x1}" y1="{close_y1}" x2="{close_x2}" y2="{close_y2}" stroke="{win_text}" stroke-width="2" stroke-linecap="round"/>
  <line x1="{close_x2}" y1="{close_y1}" x2="{close_x1}" y2="{close_y2}" stroke="{win_text}" stroke-width="2" stroke-linecap="round"/>"#,
        width = width,
        height = height,
        radius = win_border_radius,
        win_bg = COLOR_WIN_LIGHT_BG,
        win_title = COLOR_WIN_LIGHT_TITLE,
        tbh = win_title_height,
        fix_y = win_title_height - win_border_radius,
        win_text = COLOR_WIN_LIGHT_TEXT,
        title_text = title_text,
        min_x = min_x,
        min_lx1 = min_lx1,
        min_lx2 = min_lx2,
        min_ly = min_ly,
        max_x = max_x,
        max_rx = max_rx,
        max_ry = max_ry,
        close_x1 = close_x1,
        close_x2 = close_x2,
        close_y1 = close_y1,
        close_y2 = close_y2,
        wbw = wbw,
        max_size = wbw * 12.0 / 46.0
    )
}

fn render_windows_dark_decoration(
    width: f64,
    height: f64,
    _bg_color: &str,
    config: &CodeHighlightConfig,
) -> String {
    let title = config
        .terminal_title
        .as_deref()
        .unwrap_or("Windows Terminal");
    let win_border_radius = title_bar_border_radius(config.font_size);
    let win_title_height = title_bar_height(config.font_size);
    let wbw = win_button_width(config.font_size);
    let title_font_size = config.font_size * TITLE_FONT_SIZE_RATIO;
    let title_y = win_title_height * 0.64;
    let title_text = render_styled_title(
        title,
        title_font_size,
        title_y,
        COLOR_WIN_TEXT,
        &config.ascii_font,
        &config.non_ascii_font,
        &config.ascii_font_path,
        &config.non_ascii_font_path,
        TitleAnchor::Left(16.0),
    );

    // Button positions
    let close_x = width - wbw;
    let max_x = width - wbw * 2.0;
    let min_x = width - wbw * 3.0;

    // Minimize button line coordinates (horizontal line in center)
    let min_lx1 = min_x + wbw * 15.0 / 46.0;
    let min_lx2 = min_x + wbw * 31.0 / 46.0;
    let min_ly = win_title_height * 0.5;

    // Maximize button rect coordinates (square outline)
    let max_rx = max_x + wbw * 17.0 / 46.0;
    let max_ry = win_title_height * 0.33;

    // Close button × coordinates (2/3 size, centered in button area)
    let close_x1 = close_x + wbw * 17.0 / 46.0;
    let close_x2 = close_x + wbw * 29.0 / 46.0;
    let close_y1 = win_title_height * 0.36;
    let close_y2 = win_title_height * 0.69;

    format!(
        r#"  <rect x="0" y="0" width="{width}" height="{height}" rx="{radius}" fill="{win_bg}"/>
  <!-- Title bar -->
  <rect x="0" y="0" width="{width}" height="{tbh}" rx="{radius}" fill="{win_title}"/>
  <rect x="0" y="{fix_y}" width="{width}" height="{radius}" fill="{win_title}"/>
  <!-- Title text -->
  {title_text}
  <!-- Minimize button: horizontal line -->
  <rect x="{min_x}" y="0" width="{wbw}" height="{tbh}" fill="transparent"/>
  <line x1="{min_lx1}" y1="{min_ly}" x2="{min_lx2}" y2="{min_ly}" stroke="{win_text}" stroke-width="2" stroke-linecap="round"/>
  <!-- Maximize button: square outline -->
  <rect x="{max_x}" y="0" width="{wbw}" height="{tbh}" fill="transparent"/>
  <rect x="{max_rx}" y="{max_ry}" width="{max_size}" height="{max_size}" stroke="{win_text}" fill="transparent" stroke-width="1.5" rx="1"/>
  <!-- Close button: × shape (no background) -->
  <line x1="{close_x1}" y1="{close_y1}" x2="{close_x2}" y2="{close_y2}" stroke="{win_text}" stroke-width="2" stroke-linecap="round"/>
  <line x1="{close_x2}" y1="{close_y1}" x2="{close_x1}" y2="{close_y2}" stroke="{win_text}" stroke-width="2" stroke-linecap="round"/>"#,
        width = width,
        height = height,
        radius = win_border_radius,
        win_bg = COLOR_WIN_BG,
        win_title = COLOR_WIN_TITLE,
        tbh = win_title_height,
        fix_y = win_title_height - win_border_radius,
        win_text = COLOR_WIN_TEXT,
        title_text = title_text,
        min_x = min_x,
        min_lx1 = min_lx1,
        min_lx2 = min_lx2,
        min_ly = min_ly,
        max_x = max_x,
        max_rx = max_rx,
        max_ry = max_ry,
        close_x1 = close_x1,
        close_x2 = close_x2,
        close_y1 = close_y1,
        close_y2 = close_y2,
        wbw = wbw,
        max_size = wbw * 12.0 / 46.0
    )
}

fn render_gnome_decoration(
    width: f64,
    height: f64,
    _bg_color: &str,
    config: &CodeHighlightConfig,
) -> String {
    let title = config.terminal_title.as_deref().unwrap_or("Terminal");
    let gnome_border_radius = 12.0;
    let title_font_size = config.font_size * TITLE_FONT_SIZE_RATIO;
    let title_y = title_bar_height(config.font_size) * 0.65;
    let title_text = render_styled_title(
        title,
        title_font_size,
        title_y,
        COLOR_WHITE,
        &config.ascii_font,
        &config.non_ascii_font,
        &config.ascii_font_path,
        &config.non_ascii_font_path,
        TitleAnchor::Center(width / 2.0),
    );

    format!(
        r#"  <rect x="0" y="0" width="{width}" height="{height}" rx="{radius}" fill="{gnome_bg}"/>
  <!-- Title bar (minimal GNOME style) -->
  <rect x="0" y="0" width="{width}" height="{tbh}" rx="{radius}" fill="{gnome_title}"/>
  <rect x="0" y="{fix_y}" width="{width}" height="12" fill="{gnome_title}"/>
  <!-- Window buttons (close, minimize, maximize) - GNOME order -->
  <circle cx="{btn1_x}" cy="{btn_cy}" r="{btn_r}" fill="{gnome_red}"/>
  <circle cx="{btn2_x}" cy="{btn_cy}" r="{btn_r}" fill="{gnome_yellow}"/>
  <circle cx="{btn3_x}" cy="{btn_cy}" r="{btn_r}" fill="{gnome_green}"/>
  <!-- Title text (centered) -->
  {title_text}"#,
        width = width,
        height = height,
        radius = gnome_border_radius,
        gnome_bg = COLOR_GNOME_BG,
        gnome_title = COLOR_GNOME_TITLE,
        tbh = title_bar_height(config.font_size),
        fix_y = title_bar_height(config.font_size) - gnome_border_radius,
        btn_cy = title_bar_height(config.font_size) / 2.0,
        btn_r = button_radius(config.font_size),
        btn1_x = width - button_spacing(config.font_size) * 3.0,
        btn2_x = width - button_spacing(config.font_size) * 2.0,
        btn3_x = width - button_spacing(config.font_size),
        gnome_red = COLOR_GNOME_RED,
        gnome_yellow = COLOR_GNOME_YELLOW,
        gnome_green = COLOR_GNOME_GREEN,
        title_text = title_text
    )
}

fn render_iterm_decoration(
    width: f64,
    height: f64,
    _bg_color: &str,
    config: &CodeHighlightConfig,
) -> String {
    let title = config.terminal_title.as_deref().unwrap_or("iTerm2");
    let iterm_border_radius = 10.0;
    let title_font_size = config.font_size * TITLE_FONT_SIZE_RATIO;
    let title_y = title_bar_height(config.font_size) * 0.65;
    let title_text = render_styled_title(
        title,
        title_font_size,
        title_y,
        COLOR_WHITE,
        &config.ascii_font,
        &config.non_ascii_font,
        &config.ascii_font_path,
        &config.non_ascii_font_path,
        TitleAnchor::Center(width / 2.0),
    );

    format!(
        r#"  <rect x="0" y="0" width="{width}" height="{height}" rx="{radius}" fill="{black}"/>
  <!-- iTerm2 title bar -->
  <rect x="0" y="0" width="{width}" height="{tbh}" rx="{radius}" fill="{dark_bg}"/>
  <rect x="0" y="{fix_y}" width="{width}" height="10" fill="{dark_bg}"/>
  <!-- iTerm2 buttons with subtle gradient effect -->
  <circle cx="20" cy="{btn_cy}" r="{btn_r}" fill="{iterm_red}"/>
  <circle cx="20" cy="{btn_cy}" r="{inner_r}" fill="{iterm_red_inner}"/>
  <circle cx="{btn2_x}" cy="{btn_cy}" r="{btn_r}" fill="{iterm_yellow}"/>
  <circle cx="{btn2_x}" cy="{btn_cy}" r="{inner_r}" fill="{iterm_yellow_inner}"/>
  <circle cx="{btn3_x}" cy="{btn_cy}" r="{btn_r}" fill="{iterm_green}"/>
  <circle cx="{btn3_x}" cy="{btn_cy}" r="{inner_r}" fill="{iterm_green_inner}"/>
  <!-- Title text -->
  {title_text}"#,
        width = width,
        height = height,
        radius = iterm_border_radius,
        black = COLOR_BLACK,
        dark_bg = COLOR_DARK_BG,
        tbh = title_bar_height(config.font_size),
        fix_y = title_bar_height(config.font_size) - iterm_border_radius,
        btn_cy = title_bar_height(config.font_size) / 2.0,
        btn_r = button_radius(config.font_size),
        btn2_x = 20.0 + button_spacing(config.font_size),
        btn3_x = 20.0 + button_spacing(config.font_size) * 2.0,
        inner_r = button_radius(config.font_size) * 0.67,
        iterm_red = COLOR_ITERM_RED,
        iterm_red_inner = COLOR_ITERM_RED_INNER,
        iterm_yellow = COLOR_ITERM_YELLOW,
        iterm_yellow_inner = COLOR_ITERM_YELLOW_INNER,
        iterm_green = COLOR_ITERM_GREEN,
        iterm_green_inner = COLOR_ITERM_GREEN_INNER,
        title_text = title_text
    )
}

fn render_svg_to_png(svg: &str, config: &CodeHighlightConfig) -> Result<Vec<u8>> {
    let pixmap = render_svg_to_pixmap(svg, config)?;
    let png_bytes = pixmap
        .encode_png()
        .map_err(|e| CodeImageError::RenderError(e.to_string()))?;
    Ok(png_bytes)
}

pub fn render_to_png(code: &str, config: &CodeHighlightConfig) -> Result<Vec<u8>> {
    let svg = render_to_svg(code, config)?;
    render_svg_to_png(&svg, config)
}

fn render_svg_to_pixmap(svg: &str, config: &CodeHighlightConfig) -> Result<Pixmap> {
    let mut db = fontdb::Database::new();
    db.load_font_file(&config.ascii_font_path)
        .map_err(|e| CodeImageError::FontLoadError {
            path: config.ascii_font_path.to_string_lossy().to_string(),
            source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
        })?;
    db.load_font_file(&config.non_ascii_font_path)
        .map_err(|e| CodeImageError::FontLoadError {
            path: config.non_ascii_font_path.to_string_lossy().to_string(),
            source: std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
        })?;

    let opts = Options {
        fontdb: db.into(),
        ..Default::default()
    };
    let tree =
        SvgTree::from_str(svg, &opts).map_err(|e| CodeImageError::SvgParseError(e.to_string()))?;

    let pixmap_size = tree.size().to_int_size();
    let scaled_width = (pixmap_size.width() as f64 * config.scale) as u32;
    let scaled_height = (pixmap_size.height() as f64 * config.scale) as u32;
    let mut pixmap = Pixmap::new(scaled_width, scaled_height)
        .ok_or_else(|| CodeImageError::RenderError("Failed to create pixmap".to_string()))?;

    render(
        &tree,
        tiny_skia::Transform::from_scale(config.scale as f32, config.scale as f32),
        &mut pixmap.as_mut(),
    );

    Ok(pixmap)
}

fn pixmap_to_rgba_image(pixmap: &Pixmap) -> RgbaImage {
    let width = pixmap.width();
    let height = pixmap.height();
    let data = pixmap.data();

    image::RgbaImage::from_raw(width, height, data.to_vec())
        .expect("Failed to create RgbaImage from pixmap data")
}

pub fn render_to_image(code: &str, config: &CodeHighlightConfig) -> Result<RgbaImage> {
    let svg = render_to_svg(code, config)?;
    let pixmap = render_svg_to_pixmap(&svg, config)?;
    Ok(pixmap_to_rgba_image(&pixmap))
}

fn title_bar_border_radius(font_size: f64) -> f64 {
    (font_size * 0.5).max(8.0)
}

fn title_bar_height(font_size: f64) -> f64 {
    (font_size * 2.0).max(32.0)
}

fn button_radius(font_size: f64) -> f64 {
    (font_size * 0.375).max(5.0)
}

fn button_spacing(font_size: f64) -> f64 {
    (font_size * 1.25).max(16.0)
}

fn win_button_width(font_size: f64) -> f64 {
    (title_bar_height(font_size) * 1.4375).max(46.0)
}

enum TitleAnchor {
    Center(f64),
    Left(f64),
}

fn render_styled_title(
    title: &str,
    font_size: f64,
    y: f64,
    color: &str,
    ascii_font: &str,
    non_ascii_font: &str,
    ascii_font_path: &Path,
    non_ascii_font_path: &Path,
    anchor: TitleAnchor,
) -> String {
    // Load fonts for width calculation
    let ascii_data = std::fs::read(ascii_font_path).unwrap();
    let non_ascii_data = std::fs::read(non_ascii_font_path).unwrap();
    let ascii_face = Face::parse(&ascii_data, 0).unwrap();
    let non_ascii_face = Face::parse(&non_ascii_data, 0).unwrap();

    let segments = split_by_ascii(title);
    if segments.is_empty() {
        return String::new();
    }

    // Calculate total width
    let total_width: f64 = segments
        .iter()
        .map(|(is_ascii, segment)| {
            let (face, units_per_em) = if *is_ascii {
                (&ascii_face, ascii_face.units_per_em())
            } else {
                (&non_ascii_face, non_ascii_face.units_per_em())
            };
            segment
                .chars()
                .map(|c| {
                    let units = get_char_width(face, c);
                    font_units_to_px(units, units_per_em, font_size)
                })
                .sum::<f64>()
        })
        .sum();

    // Calculate starting x position
    let start_x = match anchor {
        TitleAnchor::Center(center_x) => center_x - total_width / 2.0,
        TitleAnchor::Left(x) => x,
    };

    let mut current_x = start_x;
    let mut tspans = String::new();
    for (is_ascii, segment) in segments {
        let escaped = escape_xml(segment);
        let font = if is_ascii { ascii_font } else { non_ascii_font };
        let (face, units_per_em) = if is_ascii {
            (&ascii_face, ascii_face.units_per_em())
        } else {
            (&non_ascii_face, non_ascii_face.units_per_em())
        };
        let segment_width: f64 = segment
            .chars()
            .map(|c| {
                let units = get_char_width(face, c);
                font_units_to_px(units, units_per_em, font_size)
            })
            .sum();

        tspans.push_str(&format!(
            r#"<tspan x="{}" fill="{}" font-family="{}">{}</tspan>"#,
            current_x, color, font, escaped
        ));

        current_x += segment_width;
    }

    format!(
        r#"<text y="{}" font-size="{}">{}</text>"#,
        y, font_size, tspans
    )
}
