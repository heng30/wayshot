//! Render with fonts loaded from disk (ASCII font + non-ASCII fallback).
//!
//! The crate ships no embedded font; pass an ASCII (English) font and a
//! non-ASCII font (e.g. CJK) as file paths. Any TrueType/OpenType font works.
//! A monospace ASCII font keeps the character grid aligned; with a
//! proportional font the per-cell centering still looks reasonable, just less
//! uniform.
//!
//! Run with: `cargo run --release --example custom_font -- <ascii.ttf> [<non-ascii.ttf>]`

use std::error::Error;

use ttfx_rs::effects::EffectCommand;
use ttfx_rs::render::{render_to_pngs, Font, RenderConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let ascii_path = std::env::args()
        .nth(1)
        .ok_or("usage: custom_font <ascii-font> [<non-ascii-font>]")?;
    let non_ascii_path = std::env::args().nth(2).unwrap_or_else(|| ascii_path.clone());
    let font = Font::from_files(&ascii_path, &non_ascii_path)?;
    println!("ascii font: {ascii_path}\nnon-ascii font: {non_ascii_path}");

    let mut render = RenderConfig::new(800, 200, font);
    render.seed = Some(3);
    render.fps = 30;

    let out_dir = "out/custom_font";
    let mut effect = EffectCommand::from_name("waves")
        .expect("waves effect")
        .build_effect();
    let paths = render_to_pngs(
        "Custom font rendering",
        effect.as_mut(),
        &render,
        out_dir.as_ref(),
        None,
    )?;
    println!("wrote {} frames to {out_dir}", paths.len());
    Ok(())
}
