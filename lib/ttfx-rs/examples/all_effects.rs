//! Render every effect to its own directory of PNG frames.
//!
//! Demonstrates the static effect registry: [`EffectCommand::names`] lists
//! every available effect, [`EffectCommand::from_name`] builds one with
//! default options. Each effect here is limited to 24 frames so the example
//! finishes quickly.
//!
//! Run with: `cargo run --release --example all_effects -- <ascii-font> <non-ascii-font>`

use std::error::Error;

use ttfx_rs::effects::EffectCommand;
use ttfx_rs::render::{render_to_pngs, Font, RenderConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let ascii_font = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/nix/store/c0asr1k4sg3i5xkzkcdnkywphyrw68qa-liberation-fonts-2.1.5/share/fonts/truetype/LiberationMono-Regular.ttf".to_string());
    let non_ascii_font = std::env::args().nth(2).unwrap_or_else(|| ascii_font.clone());
    let font = Font::from_files(&ascii_font, &non_ascii_font)?;

    let input = "ttfx-rs\nAll effects\nrender to images";
    let names = EffectCommand::names();

    let mut render = RenderConfig::new(640, 240, font);
    render.seed = Some(7);
    render.fps = 30;

    let mut total = 0;
    for name in names {
        let effect = EffectCommand::from_name(name).expect("registered name builds");
        let out_dir = format!("out/all/{name}");
        let mut effect = effect.build_effect();
        let paths = render_to_pngs(input, effect.as_mut(), &render, out_dir.as_ref(), Some(24))?;
        total += paths.len();
        println!("{name:>16}: {} frames -> {out_dir}", paths.len());
    }
    println!("total: {total} frames");
    Ok(())
}
