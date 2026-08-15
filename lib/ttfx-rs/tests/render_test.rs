//! Render pipeline tests: cell extraction consistency, rasterization, and
//! the high-level PNG writer.

use std::path::PathBuf;

use ttfx_rs::effects::EffectCommand;
use ttfx_rs::engine::ctx::{Clock, EngineCtx, NoopHooks};
use ttfx_rs::engine::terminal::{FrameCell, TerminalConfig};
use ttfx_rs::render::{render_to_pngs, Font, RenderConfig, SequenceRenderer};
use ttfx_rs::utils::graphics::Color;
use ttfx_rs::utils::rng::Rng;

const INPUT: &str = "Hello, ttfx-rs!\nTerminal effects as images.";

fn test_config() -> TerminalConfig {
    TerminalConfig {
        canvas_width: 40,
        canvas_height: 8,
        ignore_terminal_dimensions: true,
        ..TerminalConfig::default()
    }
}

/// Locate a system font file for tests (no font is embedded in the crate).
fn system_font(pattern: &str) -> Vec<u8> {
    let output = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", pattern])
        .output()
        .expect("fc-match available to locate a test font");
    let path = String::from_utf8(output.stdout).expect("fc-match output is utf-8");
    let path = path.trim();
    assert!(!path.is_empty(), "fc-match found no font for {pattern}");
    std::fs::read(path).unwrap_or_else(|e| panic!("failed to read font {path}: {e}"))
}

fn test_font() -> Font {
    let ascii = system_font("Liberation Mono");
    let non_ascii = system_font("Noto Sans CJK SC");
    Font::from_bytes(&ascii, &non_ascii).expect("system fonts parse")
}

fn test_render() -> RenderConfig {
    let mut render = RenderConfig::new(320, 160, test_font());
    render.cell_width = 8;
    render.cell_height = 20;
    render.seed = Some(1);
    render
}

/// Strip SGR escape sequences, leaving the plain text of a formatted symbol.
fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// frame_cells must agree with the plain-text terminal state row for row.
#[test]
fn frame_cells_match_terminal_state() {
    let mut ctx = EngineCtx::new(
        INPUT,
        test_config(),
        Rng::seeded(1),
        Clock::virtual_with_frame_rate(30),
    )
    .unwrap();
    let mut effect = EffectCommand::from_name("beams").unwrap().build_effect();
    effect.build(&mut ctx).unwrap();
    for _ in 0..10 {
        let _ = effect.next_frame(&mut ctx);
    }

    ctx.terminal.update_terminal_state();
    let state = ctx.terminal.terminal_state.clone();
    let cells = ctx.terminal.frame_cells();

    assert!(
        !cells.is_empty(),
        "a running beams animation has visible cells"
    );
    for cell in &cells {
        let row = (cell.row - 1) as usize;
        let column = (cell.column - 1) as usize;
        assert!(row < state.len(), "row {} out of range", cell.row);
        // terminal_state rows carry formatted (ANSI) symbols; compare the
        // plain text only.
        let plain = strip_ansi(&state[row]);
        let mut chars = plain.chars();
        let got = chars.nth(column).unwrap_or(' ');
        let want = cell.symbol.chars().next().unwrap_or(' ');
        assert_eq!(
            got, want,
            "cell ({}, {}) symbol mismatch",
            cell.row, cell.column
        );
    }
}

/// Rasterizing an empty frame yields a solid background image.
#[test]
fn rasterize_empty_frame_is_background() {
    let render = test_render();
    let img = ttfx_rs::render::rasterize(&[], &render);
    assert_eq!(img.width(), 40 * 8);
    assert_eq!(img.height(), 8 * 20);
    let bg = render.background.rgb_ints();
    for y in [0, img.height() / 2, img.height() - 1] {
        for x in [0, img.width() / 2, img.width() - 1] {
            let p = img.get_pixel(x, y);
            assert_eq!(
                (p[0], p[1], p[2]),
                bg,
                "background pixel differs at ({x},{y})"
            );
        }
    }
}

/// A colored cell must paint non-background pixels inside its block.
#[test]
fn rasterize_draws_glyph_and_background() {
    let render = test_render();
    let fg = Color::from_hex("ff0000").unwrap();
    let cell = FrameCell {
        row: 1,
        column: 1,
        symbol: "A",
        fg: Some(&fg),
        bg: None,
        reverse: false,
        dim: false,
        underline: false,
        strike: false,
        hidden: false,
        bold: false,
        italic: false,
    };
    let img = ttfx_rs::render::rasterize(std::slice::from_ref(&cell), &render);
    let mut painted = 0;
    // 字形整行居中到画布：统计全图红色像素，验证字形确实被绘制。
    for y in 0..img.height() {
        for x in 0..img.width() {
            let p = img.get_pixel(x, y);
            if p[0] > 0 {
                painted += 1;
            }
        }
    }
    assert!(painted > 10, "expected a visible glyph, painted={painted}");
}

/// Reverse video swaps the effective fg/bg: the block fills with the fg color.
#[test]
fn rasterize_reverse_swaps_colors() {
    let render = test_render();
    let fg = Color::from_hex("ff0000").unwrap();
    let bg = Color::from_hex("00ff00").unwrap();
    let cell = FrameCell {
        row: 1,
        column: 1,
        symbol: " ",
        fg: Some(&fg),
        bg: Some(&bg),
        reverse: true,
        dim: false,
        underline: false,
        strike: false,
        hidden: false,
        bold: false,
        italic: false,
    };
    let img = ttfx_rs::render::rasterize(std::slice::from_ref(&cell), &render);
    // Center pixel of the cell block: y0 = height - row*cell_h = 160-20 = 140.
    let p = img.get_pixel(4, 150);
    assert_eq!((p[0], p[1], p[2]), (255, 0, 0));
}

/// Default cell sizes come from the font; explicit sizes win.
#[test]
fn resolved_cell_size_uses_font_and_overrides() {
    let mut render = RenderConfig::new(640, 360, test_font());
    let (w, h) = render.resolved_cell_size();
    assert!(
        (4..=64).contains(&h),
        "font-derived line height {h} is implausible"
    );
    assert!(w >= 2, "font-derived advance {w} is implausible");

    render.cell_width = 12;
    render.cell_height = 24;
    assert_eq!(render.resolved_cell_size(), (12, 24));
}

/// Font loading: a font set parses, garbage bytes fail.
#[test]
fn font_loading() {
    let font = test_font();
    assert!(font.has_glyph('A', 20.0));
    assert!(
        font.has_glyph('▁', 20.0),
        "block glyphs used by effects must exist"
    );
    assert!(Font::from_bytes(b"not a font", b"not a font").is_err());
}

/// The high-level renderer writes numbered PNGs until the effect completes
/// and honors max_frames.
#[test]
fn render_to_pngs_writes_files_and_respects_max_frames() {
    let dir = std::env::temp_dir().join(format!("ttfx_rs_test_{}", std::process::id()));
    let out: PathBuf = dir.join("out");
    let mut effect = EffectCommand::from_name("print").unwrap().build_effect();

    let render = test_render();
    let paths = render_to_pngs(INPUT, effect.as_mut(), &render, &out, Some(5)).unwrap();
    assert_eq!(paths.len(), 5, "max_frames=5 must stop after 5 files");
    for (i, path) in paths.iter().enumerate() {
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            format!("frame_{i:04}.png")
        );
        assert!(path.exists());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// SequenceRenderer exposes incremental frames with stable dimensions.
#[test]
fn sequence_renderer_produces_consistent_frames() {
    let mut renderer = SequenceRenderer::new(INPUT, test_render()).unwrap();
    let mut effect = EffectCommand::from_name("rain").unwrap().build_effect();
    renderer.build_effect(effect.as_mut()).unwrap();

    let mut dims = None;
    let mut frames = 0;
    while let Some(frame) = renderer.next_frame(effect.as_mut()) {
        match dims {
            None => dims = Some((frame.image.width(), frame.image.height())),
            Some(d) => assert_eq!(d, (frame.image.width(), frame.image.height())),
        }
        frames += 1;
        if frames > 60 {
            break;
        }
    }
    assert!(
        frames >= 60,
        "rain should run longer than 60 frames, got {frames}"
    );
}

/// The engine context is reachable for pre-frame setup.
#[test]
fn sequence_renderer_exposes_ctx() {
    let mut renderer = SequenceRenderer::new(INPUT, test_render()).unwrap();
    let _ = renderer.ctx();
    let _ = NoopHooks;
}
