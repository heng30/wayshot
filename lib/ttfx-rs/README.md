# ttfx-rs

Terminal text effects, rendered to images instead of a terminal. Feed it text,
pick an effect, get a PNG sequence (or a GIF):

```rust
use ttfx_rs::effects::EffectCommand;
use ttfx_rs::render::{render_to_pngs, Font, RenderConfig};

let input = "Hello, ttfx-rs!";
let font = Font::from_files("mono.ttf", "cjk.otf")?;
let mut effect = EffectCommand::from_name("beams").unwrap().build_effect();
// auto 尺寸：画布 = 文本包围盒 + padding（2 列 x 1 行），刚好包裹动画
let render = RenderConfig::auto(24.0, 2, 1, font);
let paths = render_to_pngs(input, effect.as_mut(), &render, "out/beams".as_ref(), None)?;
```

## Credit where it's due

**This is a port of [TerminalTextEffects](https://github.com/ChrisBuilds/terminaltexteffects)
(TTE) by [ChrisBuilds](https://github.com/ChrisBuilds).** Every effect and the
animation engine are their design — this project translates that work to Rust
and adds nothing of its own to the art. If you like what you see here, star
the original.

TTE is MIT licensed and so is this port; the original copyright is preserved
in [LICENSE](LICENSE) and [NOTICE](NOTICE). Please file *effect* ideas
upstream, where they belong.

The original ttfx was a CLI that animated a real terminal. This fork keeps the
same engine and all 37 effects, but the output target is now a raster image:
each frame is drawn into a pixel buffer with caller-provided fonts.

## Library overview

```
ttfx_rs
├── effects      EffectCommand: 37 effects with clap-derived option structs
├── engine       The animation engine (terminal/canvas/motion/animation/events)
├── render       RenderConfig, SequenceRenderer, render_to_pngs, Font, rasterize
└── utils        easing, geometry, graphics (gradients), rng, pycompat
```

### The render pipeline

1. **`RenderConfig`** describes the output: image `width`/`height` in pixels,
   per-character `cell_width`/`cell_height` (0 = derive from the font), `fps`,
   `background`, `font`, and an optional `seed` for reproducible renders.
2. The canvas (a character grid) is derived from the pixel size:
   `columns = width / cell_width`, `rows = height / cell_height`; the produced
   image is exactly `columns * cell_width` by `rows * cell_height` pixels.
   For full control over the grid, build the renderer with
   [`SequenceRenderer::with_terminal_config`](src/render/mod.rs).
3. **Auto sizing**: [`RenderConfig::auto`](src/render/mod.rs) derives the canvas
   from the input text itself (the text bounding box plus horizontal/vertical
   padding in character cells), so the produced image wraps the animation
   tightly instead of leaving empty margins.
4. **`fps`** drives a virtual clock: time-gated effects (matrix,
   thunderstorm) advance `1/fps` seconds per frame, so a render takes
   `frames / fps` seconds of simulated time. Deterministic given a seed.
5. `SequenceRenderer` pulls one `Frame` (an `RgbaImage`) at a time; the
   convenience function `render_to_pngs` writes `frame_0000.png`, ... until
   the effect completes (or a `max_frames` cap).

### Fonts

The crate ships **no embedded font**. Callers must provide two font files:

- an ASCII (English) font — drives the character-cell metrics;
- a non-ASCII font (e.g. Source Han Sans CN / Noto Sans CJK) — used as a
  fallback for glyphs the ASCII font lacks.

```rust
let font = Font::from_files("path/to/mono.ttf", "path/to/cjk.otf")?;
```

Glyphs missing from every font render as nothing rather than tofu.

### Visual attributes

Terminal styling is honored per cell: reverse video swaps fg/bg, dim halves
the intensity, underline and strikethrough draw lines, hidden cells paint
only their background. Bold/italic are carried in the frame data but not
synthesized (use a bold/italic font file if you need them).

### Visual attributes

Terminal styling is honored per cell: reverse video swaps fg/bg, dim halves
the intensity, underline and strikethrough draw lines, hidden cells paint
only their background. Bold/italic are carried in the frame data but not
synthesized (use a bold/italic font file if you need them).

## Examples

| Example | What it shows |
|---|---|
| [`examples/basic.rs`](examples/basic.rs) | One effect to a PNG sequence (`cargo run --release --example basic`) |
| [`examples/gif.rs`](examples/gif.rs) | Incremental `SequenceRenderer` feeding a GIF encoder |
| [`examples/all_effects.rs`](examples/all_effects.rs) | Iterating the static registry: `EffectCommand::names()` + `from_name()` |
| [`examples/custom_font.rs`](examples/custom_font.rs) | Loading ASCII + non-ASCII fonts from disk (`-- <ascii.ttf> [<non-ascii.ttf>]`) |

## Effect options

Each effect takes its own options as clap-derived config structs. Build one
with defaults via `EffectCommand::from_name("beams")`, or construct the
config directly, e.g. `EffectCommand::Beams(BeamsConfig { beam_delay: 3, .. })`
— the `--help` text of the original CLI documented every field, and the
struct fields keep the same names and defaults as upstream `tte`.

## Building and testing

```sh
cargo build --release
cargo test --release
cargo clippy --all-targets -- -D warnings
```

The port is tested by unit tests, golden fixtures for easing/geometry/
gradient values (generated from CPython, `tools/goldens/`), engine event
traces, and render-pipeline tests that check cell extraction against the
engine's own state.

## Scope

Linux and macOS. The engine is dependency-light; `image` and `cosmic-text`
are the only runtime dependencies.

## License

MIT — see [LICENSE](LICENSE), which carries both this project's copyright and
the original TerminalTextEffects copyright, and [NOTICE](NOTICE) for the
attribution in full.
