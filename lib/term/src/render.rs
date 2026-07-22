// Span building module
//
// This module handles:
// - Building TermSpan structs from terminal content
// - CJK/emoji detection for font fallback
// - ANSI color conversion to Slint colors

use crate::{
    input::SelectionState,
    terminal::{TerminalCell, TerminalContent},
};

/// Span builder that converts terminal content into TermSpan structs
/// for Slint's native Text element rendering.
#[derive(derivative::Derivative)]
#[derivative(Default)]
pub struct SpanBuilder {
    #[derivative(Default(value = "[30, 30, 30]"))]
    default_bg: [u8; 3],
    #[derivative(Default(value = "[230, 230, 230]"))]
    default_fg: [u8; 3],
    #[derivative(Default(value = "[255, 255, 255]"))]
    cursor_color: [u8; 3],
    #[derivative(Default(value = "[255, 255, 255]"))]
    selection_bg: [u8; 3],

    selection: Option<SelectionState>,
}

impl SpanBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build TermSpan structs from terminal content
    pub fn build_spans(&self, content: &TerminalContent) -> Vec<TermSpanData> {
        let mut spans = Vec::new();

        if content.cells.is_empty() {
            return spans;
        }

        // Debug: log total cell count
        log::trace!(
            "build_spans: {} cells, rows={}, cols={}, display_offset={}",
            content.cells.len(),
            content.rows,
            content.cols,
            content.display_offset
        );

        // Group cells by row using a more efficient approach:
        // iterate cells in order and detect row boundaries
        let mut row_start = 0;
        let mut current_row = content.cells[0].row;

        for (i, cell) in content.cells.iter().enumerate() {
            if cell.row != current_row {
                // Process the previous row
                self.build_row_spans(&mut spans, &content.cells[row_start..i], current_row as i32);
                row_start = i;
                current_row = cell.row;
            }
        }

        // Process the last row
        if row_start < content.cells.len() {
            let row = content.cells[row_start].row;
            self.build_row_spans(&mut spans, &content.cells[row_start..], row as i32);
        }

        // Debug: log span count and first few spans
        if !spans.is_empty() {
            log::trace!(
                "Built {} spans, first: row={} col={} text='{}' fg={:?} cells={} cjk={}",
                spans.len(),
                spans[0].row,
                spans[0].col,
                &spans[0].text,
                spans[0].fg,
                spans[0].cells,
                spans[0].cjk
            );
            if spans.len() > 1 {
                log::trace!(
                    "  second: row={} col={} text='{}' cells={} cjk={}",
                    spans[1].row,
                    spans[1].col,
                    &spans[1].text,
                    spans[1].cells,
                    spans[1].cjk
                );
            }
        }

        spans
    }

    /// Build spans for a single row of cells
    fn build_row_spans(&self, spans: &mut Vec<TermSpanData>, cells: &[TerminalCell], row: i32) {
        if cells.is_empty() {
            return;
        }

        let mut idx = 0;
        while idx < cells.len() {
            let cell = &cells[idx];

            // Skip wide char spacers — the first cell (wide=true) already
            // handles both cells
            if cell.wide_spacer {
                idx += 1;
                continue;
            }

            // Wide (CJK/emoji) characters get their own span occupying
            // exactly 2 grid cells, so the UI can box + centre + clip
            // the glyph on the monospace grid.
            if cell.wide {
                let fg = self.resolve_fg(cell);
                let bg = self.resolve_bg(cell, cell.col, row as usize);

                if cell.c != ' ' || !bg_equals_default(&bg, &self.default_bg) {
                    spans.push(TermSpanData {
                        text: cell.c.to_string(),
                        fg,
                        bg,
                        bold: cell.bold,
                        row,
                        col: cell.col as i32,
                        cells: 2,
                        cjk: contains_cjk_or_emoji(cell.c),
                        underline: cell.underline,
                        italic: cell.italic,
                    });
                }

                idx += 1;

                // Skip the wide spacer if present
                if idx < cells.len() && cells[idx].wide_spacer {
                    idx += 1;
                }
                continue;
            }

            // Group consecutive narrow cells that share fg + bg + bold +
            // italic + underline into one run.  We keep blank cells inside
            // a run (so a coloured bar of spaces still gets a background
            // fill) and break on attribute change, a wide cell, or a CJK
            // character (CJK chars need their own span so they use the UI
            // font, while ASCII stays on the monospace terminal font).
            let start_col = cell.col;
            let mut text = cell.c.to_string();
            let fg = self.resolve_fg(cell);
            let bg = self.resolve_bg(cell, cell.col, row as usize);
            let bold = cell.bold;
            let italic = cell.italic;
            let underline = cell.underline;
            let is_cjk = contains_cjk_or_emoji(cell.c);

            idx += 1;
            while idx < cells.len() {
                let next = &cells[idx];

                // Wide spacers and wide chars break the run
                if next.wide_spacer || next.wide {
                    break;
                }

                let next_fg = self.resolve_fg(next);
                let next_bg = self.resolve_bg(next, next.col, row as usize);
                let next_is_cjk = contains_cjk_or_emoji(next.c);

                // Break on attribute change
                if next_fg != fg
                    || next_bg != bg
                    || next.bold != bold
                    || next.italic != italic
                    || next.underline != underline
                {
                    break;
                }

                // Break on CJK boundary: CJK and non-CJK chars need
                // different fonts, so they must be in separate spans.
                if next_is_cjk != is_cjk {
                    break;
                }

                text.push(next.c);
                idx += 1;
            }

            let cell_count = if idx < cells.len() {
                // The next cell that broke the run starts at this column.
                // Our run covers [start_col, next_col).
                cells[idx].col - start_col
            } else if idx > 0 {
                // Last run in the row — covers [start_col, last_cell_col]
                cells[idx - 1].col - start_col + 1
            } else {
                1
            };

            // Skip runs that contribute nothing visible: blank text and default bg
            let is_blank = text.chars().all(|ch| ch == ' ');
            if is_blank && bg_equals_default(&bg, &self.default_bg) {
                continue;
            }

            spans.push(TermSpanData {
                text,
                fg,
                bg,
                bold,
                row,
                col: start_col as i32,
                cells: cell_count as i32,
                cjk: is_cjk,
                underline,
                italic,
            });
        }
    }

    /// Resolve foreground color for a cell
    fn resolve_fg(&self, cell: &TerminalCell) -> Color {
        let rgb = cell.fg.unwrap_or(self.default_fg);

        // Apply bold brightening
        let final_rgb = if cell.bold {
            [
                ((rgb[0] as u32 * 140 / 100).min(255)) as u8,
                ((rgb[1] as u32 * 140 / 100).min(255)) as u8,
                ((rgb[2] as u32 * 140 / 100).min(255)) as u8,
            ]
        } else {
            rgb
        };

        Color::from_rgb(final_rgb)
    }

    /// Resolve background color for a cell, accounting for selection
    fn resolve_bg(&self, cell: &TerminalCell, col: usize, row: usize) -> Color {
        // Check if this cell is selected
        if let Some(ref sel) = self.selection {
            let (start_row, start_col, end_row, end_col) = sel.normalized();

            let in_selection = if start_row == end_row {
                row == start_row && col >= start_col && col <= end_col
            } else {
                if row == start_row && col >= start_col {
                    true
                } else if row == end_row && col <= end_col {
                    true
                } else if row > start_row && row < end_row {
                    true
                } else {
                    false
                }
            };

            if in_selection {
                return Color::from_rgb(self.selection_bg);
            }
        }

        match cell.bg {
            Some(rgb) => Color::from_rgb(rgb),
            None => Color::from_rgb(self.default_bg),
        }
    }

    /// Set default background color
    pub fn set_default_bg(&mut self, color: [u8; 3]) {
        self.default_bg = color;
    }

    /// Set default foreground color
    pub fn set_default_fg(&mut self, color: [u8; 3]) {
        self.default_fg = color;
    }

    /// Set cursor color
    pub fn set_cursor_color(&mut self, color: [u8; 3]) {
        self.cursor_color = color;
    }

    /// Set selection background color
    pub fn set_selection_bg(&mut self, color: [u8; 3]) {
        self.selection_bg = color;
    }

    /// Set selection state
    pub fn set_selection(&mut self, selection: Option<SelectionState>) {
        self.selection = selection;
    }

    /// Get cursor color
    pub fn cursor_color(&self) -> [u8; 3] {
        self.cursor_color
    }

    /// Get default background color
    pub fn default_bg(&self) -> [u8; 3] {
        self.default_bg
    }

    /// Get default foreground color
    pub fn default_fg(&self) -> [u8; 3] {
        self.default_fg
    }
}

// TermSpanData — Rust-side span representation (converted to Slint TermSpan
// in the example's slint_bridge module)
/// A coloured run of text on the terminal grid (Rust-side representation)
#[derive(Debug, Clone)]
pub struct TermSpanData {
    pub text: String,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub row: i32,
    pub col: i32,
    pub cells: i32,
    pub cjk: bool,
    pub underline: bool,
    pub italic: bool,
}

/// Simplified color representation (avoids depending on slint crate in render.rs)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8, // 0 = fully transparent
}

impl Color {
    pub fn from_rgb(rgb: [u8; 3]) -> Self {
        Self {
            r: rgb[0],
            g: rgb[1],
            b: rgb[2],
            a: 255,
        }
    }

    pub fn transparent() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }
    }
}

/// Check if a background color matches the given default background
fn bg_equals_default(bg: &Color, default_bg: &[u8; 3]) -> bool {
    bg.r == default_bg[0] && bg.g == default_bg[1] && bg.b == default_bg[2] && bg.a == 255
}

/// True if a character is CJK or emoji — needs font fallback to a
/// CJK-capable or emoji-capable font.  The monospace terminal font
/// typically lacks CJK glyphs and Slint's per-script fallback may not
/// render isolated CJK punctuation correctly, so these spans are drawn
/// with the CJK-capable UI font instead.  Emoji codepoints also need
/// fallback to a color emoji font, which Slint handles automatically
/// when the span is rendered as a Text element.
///
/// NOTE: We only include ranges that the terminal font is unlikely to
/// have.  Dingbats (0x2700-0x27BF), Miscellaneous Symbols (0x2600-0x26FF),
/// and arrow-like symbols are commonly included in Nerd Font patches, so
/// they are NOT flagged as CJK here — the terminal font handles them.
fn contains_cjk_or_emoji(c: char) -> bool {
    // Fast path: ASCII characters are always in the monospace font
    if c.is_ascii() {
        return false;
    }

    let cp = c as u32;

    matches!(cp,
        // CJK radicals
        0x2E80..=0x2EFF
        // CJK symbols & punctuation (、。「」…)
        | 0x3000..=0x303F
        // hiragana + katakana
        | 0x3040..=0x30FF
        // bopomofo
        | 0x3100..=0x312F
        // CJK ext A
        | 0x3400..=0x4DBF
        // CJK unified ideographs
        | 0x4E00..=0x9FFF
        // CJK compatibility ideographs
        | 0xF900..=0xFAFF
        // fullwidth / halfwidth forms (，！？：；)
        | 0xFF00..=0xFFEF
        // CJK ext B–F + compat supplement
        | 0x20000..=0x2FA1F
        // Hangul syllables
        | 0xAC00..=0xD7AF
        // Hangul jamo
        | 0x1100..=0x11FF
        // --- Emoji ranges (modern color emoji) ---
        // Emoticons 😀😁😂
        | 0x1F600..=0x1F64F
        // Misc Symbols and Pictographs 🌍🔥
        | 0x1F300..=0x1F5FF
        // Transport and Map Symbols 🚗🚀
        | 0x1F680..=0x1F6FF
        // Supplemental Symbols and Pictographs 🤖🥳
        | 0x1F900..=0x1F9FF
        // Chess Symbols ♟
        | 0x1FA00..=0x1FA6F
        // Symbols and Pictographs Extended-A 🪗🪄
        | 0x1FA70..=0x1FAFF
        // Stars (only ⭐, not the full misc symbols range)
        | 0x2B50..=0x2B55
        // Regional Indicator Symbols (flag emoji 🇺🇸)
        | 0x1F1E6..=0x1F1FF
        // Enclosed Alphanumeric Supplement
        | 0x1F100..=0x1F1FF
    )
}

/// True if a string contains any CJK or emoji character
pub fn contains_cjk_str(s: &str) -> bool {
    s.chars().any(contains_cjk_or_emoji)
}
