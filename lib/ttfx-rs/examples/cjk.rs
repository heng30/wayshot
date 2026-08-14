//! Render Chinese text using a CJK fallback font.
//!
//! The ASCII font (monospace) drives cell metrics; cosmic-text layout pulls
//! missing glyphs (CJK, ...) from the non-ASCII font registered via
//! [`Font::from_files`] automatically.
//!
//! Run with: `cargo run --release --example cjk -- <ascii-font> <cjk-font>`

use std::error::Error;

use ttfx_rs::effects::EffectCommand;
use ttfx_rs::render::{render_to_pngs, Font, RenderConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let ascii_font = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/nix/store/c0asr1k4sg3i5xkzkcdnkywphyrw68qa-liberation-fonts-2.1.5/share/fonts/truetype/LiberationMono-Regular.ttf".to_string());
    let cjk_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "/nix/store/gh9ksha5bbf7a8si59qxdnwg6b3b3sqc-source-han-sans-2.005/share/fonts/opentype/source-han-sans/SourceHanSans.ttc".to_string());
    let font = Font::from_files(&ascii_font, &cjk_path)?;
    println!("ascii font: {ascii_font}\nCJK font: {cjk_path}");

    let input = "终端文字特效\n中文渲染测试\nTTFX-rs 混合 abc 123";
    let mut render = RenderConfig::auto(24.0, 1, 1, font);
    render.seed = Some(7);
    render.fps = 30;

    let out_dir = "out/cjk";
    let mut effect = EffectCommand::from_name("print")
        .expect("print effect")
        .build_effect();
    let paths = render_to_pngs(input, effect.as_mut(), &render, out_dir.as_ref(), Some(60))?;
    println!("wrote {} frames to {out_dir}", paths.len());
    println!(
        "last frame: {}",
        paths.last().expect("frames written").display()
    );
    Ok(())
}
