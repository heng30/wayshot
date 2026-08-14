//! Minimal example: render one effect to a directory of PNG frames.
//!
//! Run with: `cargo run --release --example basic -- <ascii-font> <non-ascii-font>`

use std::error::Error;

use ttfx_rs::effects::EffectCommand;
use ttfx_rs::render::{render_to_pngs, Font, RenderConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let ascii_font = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/nix/store/c0asr1k4sg3i5xkzkcdnkywphyrw68qa-liberation-fonts-2.1.5/share/fonts/truetype/LiberationMono-Regular.ttf".to_string());
    let non_ascii_font = std::env::args().nth(2).unwrap_or_else(|| ascii_font.clone());
    let font = Font::from_files(&ascii_font, &non_ascii_font)?;
    println!("ascii font: {ascii_font}\nnon-ascii font: {non_ascii_font}");

    let input = "Hello, ttfx-rs!\nTerminal effects as images.";
    let effect = EffectCommand::from_name("beams").expect("beams effect exists");

    let render = RenderConfig::new(960, 360, font);
    // Pin a seed so the output is reproducible.
    let mut render = render;
    render.seed = Some(42);
    render.fps = 30;

    let out_dir = "out/basic";
    let mut effect = effect.build_effect();
    let paths = render_to_pngs(input, effect.as_mut(), &render, out_dir.as_ref(), None)?;
    println!("wrote {} frames to {out_dir}", paths.len());
    println!("first frame: {}", paths[0].display());
    Ok(())
}
