//! Image rendering: drive an effect to completion and rasterize every frame.
//!
//! Two entry points:
//!
//! - [`render_to_pngs`] — one call renders an effect into a directory of
//!   numbered PNGs.
//! - [`SequenceRenderer`] — incremental control: pull frames one at a time as
//!   [`Frame`]s (raw pixel buffers) and do what you want with them (write
//!   PNGs, encode a GIF, composite, ...).
//!
//! The engine is deterministic when a seed is provided: the same input,
//! effect config, seed and [`RenderConfig`] reproduce the same frame stream.

pub mod font;
pub mod raster;

use std::path::{Path, PathBuf};

use image::RgbaImage;

use crate::engine::ctx::{Clock, EngineCtx};
use crate::engine::effect::Effect;
use crate::engine::error::EngineError;
use crate::engine::terminal::TerminalConfig;
use crate::utils::graphics::Color;
use crate::utils::rng::Rng;

pub use font::Font;
pub use raster::rasterize;

/// Rendering options for the image output.
///
/// `width`/`height` are the output image size in **pixels**. The engine
/// canvas (a grid of character cells) is derived from them: the number of
/// columns is `width / cell_width`, the number of rows `height / cell_height`,
/// and the produced image is exactly `cols * cell_width` by `rows *
/// cell_height` pixels (a fractional remainder is dropped).
///
/// When `auto_size` is true the canvas is instead derived from the input
/// text itself: the canvas is the text bounding box (lines × widest line)
/// plus `padding_x`/`padding_y` character cells, so the produced image wraps
/// the animation tightly (see [`RenderConfig::auto`]).
///
/// `fps` drives the virtual clock: time-gated effects (matrix, thunderstorm)
/// advance `1/fps` seconds per frame, so the animation lasts
/// `frame_count / fps` seconds of simulated time.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Output image width in pixels. Ignored when `auto_size` is true.
    pub width: u32,
    /// Output image height in pixels. Ignored when `auto_size` is true.
    pub height: u32,
    /// When true the canvas size is derived from the input text (bounding
    /// box + padding) instead of `width`/`height`.
    pub auto_size: bool,
    /// Font size in points; drives the character-cell dimensions.
    pub font_size: f32,
    /// Horizontal padding in character cells, only used with `auto_size`.
    pub padding_x: u32,
    /// Vertical padding in character cells, only used with `auto_size`.
    pub padding_y: u32,
    /// Pixel width of one character cell. 0 derives it from the font's
    /// advance at the font size.
    pub cell_width: u32,
    /// Pixel height of one character cell. 0 derives the line height from the
    /// font at the configured font size.
    pub cell_height: u32,
    /// Frames per second of the virtual clock.
    pub fps: u32,
    /// Background color of the image (cells without an explicit background).
    pub background: Color,
    /// Font used to draw the glyphs.
    pub font: Font,
    /// Optional seed for reproducible renders.
    pub seed: Option<u64>,
}

impl RenderConfig {
    /// A fixed-size config: 60 fps, 24pt font, black background.
    pub fn new(width: u32, height: u32, font: Font) -> Self {
        RenderConfig {
            width,
            height,
            auto_size: false,
            font_size: 24.0,
            padding_x: 0,
            padding_y: 0,
            cell_width: 0,
            cell_height: 0,
            fps: 60,
            background: Color::from_hex("000000").expect("valid hex"),
            font,
            seed: None,
        }
    }

    /// Auto-sized render config: the canvas is the input text bounding box
    /// plus `padding_x`/`padding_y` character cells, so the produced image
    /// wraps the animation tightly.
    ///
    /// `font_size` drives the character-cell size (line height and glyph
    /// advance); larger sizes produce a larger image at the same cell count.
    pub fn auto(font_size: f32, padding_x: u32, padding_y: u32, font: Font) -> Self {
        RenderConfig {
            width: 0,
            height: 0,
            auto_size: true,
            font_size: font_size.max(4.0),
            padding_x,
            padding_y,
            cell_width: 0,
            cell_height: 0,
            fps: 60,
            background: Color::from_hex("000000").expect("valid hex"),
            font,
            seed: None,
        }
    }

    /// Resolve auto cell dimensions against the font: cell height defaults to
    /// the font's line height at the configured font size, cell width to the
    /// glyph advance.
    pub fn resolved_cell_size(&self) -> (u32, u32) {
        let cell_h = if self.cell_height > 0 {
            self.cell_height
        } else {
            self.font.line_height(self.font_size).round().max(4.0) as u32
        };
        let cell_w = if self.cell_width > 0 {
            self.cell_width
        } else {
            self.font.advance(cell_h as f32).round().max(2.0) as u32
        };
        (cell_w, cell_h)
    }
}

/// Compute the engine's text bounding box `(columns, rows)` for `input`:
/// the widest line's character count (tabs expanded like the engine's
/// preprocessor, `tab_width = 4`) and the number of non-trailing-empty lines.
/// Mirrors the Preprocessor's screen layout so an auto-sized canvas never
/// clips the text.
fn text_bounding_box(input: &str) -> (i64, i64) {
    const TAB_WIDTH: i64 = 4;
    let effective = if input.is_empty() { "No Input." } else { input };
    let mut rows = 0i64;
    let mut max_cols = 0i64;
    for line in effective.lines() {
        let mut cols = 0i64;
        for c in line.chars() {
            if c == '\t' {
                cols += TAB_WIDTH - (cols % TAB_WIDTH);
            } else {
                cols += 1;
            }
        }
        rows += 1;
        max_cols = max_cols.max(cols);
    }
    if rows == 0 {
        rows = 1;
    }
    (max_cols.max(1), rows)
}

/// One rendered frame: the pixel buffer plus its 0-based sequence index.
#[derive(Debug, Clone)]
pub struct Frame {
    pub index: u32,
    pub image: RgbaImage,
}

impl Frame {
    /// Save this frame as a PNG file.
    pub fn save_png(&self, path: impl AsRef<Path>) -> Result<(), RenderError> {
        self.image
            .save_with_format(path.as_ref(), image::ImageFormat::Png)
            .map_err(RenderError::Image)
    }
}

/// Errors produced by the rendering pipeline.
#[derive(Debug)]
pub enum RenderError {
    Engine(EngineError),
    Image(image::ImageError),
    Io(std::io::Error),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Engine(e) => write!(f, "{e}"),
            RenderError::Image(e) => write!(f, "image error: {e}"),
            RenderError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<EngineError> for RenderError {
    fn from(e: EngineError) -> Self {
        RenderError::Engine(e)
    }
}

impl From<std::io::Error> for RenderError {
    fn from(e: std::io::Error) -> Self {
        RenderError::Io(e)
    }
}

/// Incremental renderer: owns the engine state and rasterizes one frame per
/// call to [`SequenceRenderer::next_frame`].
pub struct SequenceRenderer {
    ctx: EngineCtx,
    render: RenderConfig,
    cell_width: u32,
    cell_height: u32,
    index: u32,
}

impl SequenceRenderer {
    /// Build a renderer for `input` with the given output options. The canvas
    /// is derived from the pixel size (see [`RenderConfig`]) or, when
    /// `render.auto_size` is set, from the input text bounding box plus
    /// padding (see [`RenderConfig::auto`]). The terminal config can be
    /// overridden with [`SequenceRenderer::with_terminal_config`].
    pub fn new(input: &str, render: RenderConfig) -> Result<Self, RenderError> {
        let (cell_width, cell_height) = render.resolved_cell_size();
        let (cols, rows) = if render.auto_size {
            let (text_width, text_height) = text_bounding_box(input);
            (
                text_width + render.padding_x as i64 * 2,
                text_height + render.padding_y as i64 * 2,
            )
        } else {
            ((render.width / cell_width).max(1) as i64, (render.height / cell_height).max(1) as i64)
        };
        let config = TerminalConfig {
            canvas_width: cols,
            canvas_height: rows,
            ignore_terminal_dimensions: true,
            terminal_background_color: render.background,
            ..TerminalConfig::default()
        };
        SequenceRenderer::with_terminal_config(input, render, config)
    }

    /// Build a renderer with full control over the engine's terminal config
    /// (canvas in character cells, text anchoring, wrapping, ...). The output
    /// image is still `cell_width * canvas_columns` by `cell_height *
    /// canvas_rows` pixels; the `width`/`height` fields of `render` are
    /// ignored in this mode.
    pub fn with_terminal_config(
        input: &str,
        render: RenderConfig,
        terminal_config: TerminalConfig,
    ) -> Result<Self, RenderError> {
        let (cell_width, cell_height) = render.resolved_cell_size();
        let rng = match render.seed {
            Some(seed) => Rng::seeded(seed),
            None => Rng::from_entropy(),
        };
        let clock = Clock::virtual_with_frame_rate(render.fps.max(1) as i64);
        let ctx = EngineCtx::new(input, terminal_config, rng, clock)?;
        Ok(SequenceRenderer {
            ctx,
            render,
            cell_width,
            cell_height,
            index: 0,
        })
    }

    /// Run the effect's build phase (required once before the first
    /// [`SequenceRenderer::next_frame`] call).
    pub fn build_effect(&mut self, effect: &mut dyn Effect) -> Result<(), RenderError> {
        effect.build(&mut self.ctx)?;
        Ok(())
    }

    /// Advance the animation by one frame and rasterize it. Returns None when
    /// the effect reports completion.
    pub fn next_frame(&mut self, effect: &mut dyn Effect) -> Option<Frame> {
        let output = effect.next_frame(&mut self.ctx)?;
        self.ctx.terminal.recycle_output_string(output);
        let cols = self.ctx.terminal.canvas.right as u32;
        let rows = self.ctx.terminal.canvas.top as u32;
        let cells = self.ctx.terminal.frame_cells();
        let mut render = self.render.clone();
        render.width = self.cell_width * cols;
        render.height = self.cell_height * rows;
        let image = raster::rasterize(&cells, &render);
        let frame = Frame {
            index: self.index,
            image,
        };
        self.index += 1;
        Some(frame)
    }

    /// The engine context, for effects that need pre-frame setup beyond
    /// `build_effect` (e.g. chaining scenes before the first frame).
    pub fn ctx(&mut self) -> &mut EngineCtx {
        &mut self.ctx
    }
}

/// Render `effect` over `input` into `out_dir` as `frame_0000.png`,
/// `frame_0001.png`, ... until the effect completes (or `max_frames` is
/// reached). Returns the paths of the written files.
///
/// The effect's `build` phase is handled here; the caller only provides a
/// constructed effect (see [`crate::effects::EffectCommand::from_name`]).
pub fn render_to_pngs(
    input: &str,
    effect: &mut dyn Effect,
    render: &RenderConfig,
    out_dir: &Path,
    max_frames: Option<u32>,
) -> Result<Vec<PathBuf>, RenderError> {
    let mut renderer = SequenceRenderer::new(input, render.clone())?;
    renderer.build_effect(effect)?;

    std::fs::create_dir_all(out_dir)?;
    let mut paths = Vec::new();
    while let Some(frame) = renderer.next_frame(effect) {
        let path = out_dir.join(format!("frame_{:04}.png", frame.index));
        frame.save_png(&path)?;
        paths.push(path);
        if max_frames.is_some_and(|max| frame.index + 1 >= max) {
            break;
        }
    }
    Ok(paths)
}
