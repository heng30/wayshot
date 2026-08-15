//! Render an effect into a single animated GIF.
//!
//! Uses the incremental [`SequenceRenderer`] API to pull raw frames and hand
//! them to the GIF encoder instead of writing individual PNGs.
//!
//! Run with: `cargo run --release --example gif -- <ascii-font> [<non-ascii-font>] [--transparent]`
//!
//! `--transparent` 用黑色全透明背景渲染，并统计暗色不透明像素数量，
//! 用于验证透明背景 GIF 的文字黑边 bug 是否修复。

use std::error::Error;
use std::fs::File;

use image::codecs::gif::GifEncoder;
use image::Frame as GifFrame;
use ttfx_rs::effects::EffectCommand;
use ttfx_rs::render::{Font, RenderConfig, SequenceRenderer};
use ttfx_rs::utils::graphics::Color;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let transparent = args.iter().any(|a| a == "--transparent");
    let font_args: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let ascii_font = font_args
        .first()
        .map(|s| s.as_str())
        .unwrap_or("/nix/store/c0asr1k4sg3i5xkzkcdnkywphyrw68qa-liberation-fonts-2.1.5/share/fonts/truetype/LiberationMono-Regular.ttf");
    let non_ascii_font = font_args.get(1).map(|s| s.as_str()).unwrap_or(ascii_font);
    let font = Font::from_files(ascii_font, non_ascii_font)?;

    let input = "GIF output";
    let mut render = RenderConfig::new(640, 160, font);
    render.seed = Some(99);
    render.fps = 30;
    if transparent {
        // 黑色全透明背景，与用户复现黑色描边的配置一致。
        render.background = Color::from_hex("000000")?;
        render.background_alpha = 0;
    }

    let fps = render.fps;
    let mut renderer = SequenceRenderer::new(input, render)?;
    let mut effect = EffectCommand::from_name("decrypt")
        .expect("decrypt effect")
        .build_effect();
    renderer.build_effect(effect.as_mut())?;

    let out_path = "out/gif/decrypt.gif";
    std::fs::create_dir_all("out/gif")?;
    let mut encoder = GifEncoder::new(File::create(out_path)?);
    encoder.set_repeat(image::codecs::gif::Repeat::Infinite)?;
    let delay_ms = 1000 / fps;

    let mut frames = 0;
    let mut dark_opaque_total = 0usize;
    while let Some(frame) = renderer.next_frame(effect.as_mut()) {
        if transparent {
            // 统计不透明暗色像素（旧 bug 的黑色描边特征）。
            let dark_opaque = frame
                .image
                .pixels()
                .filter(|px| px[3] == 255 && px[0] < 60 && px[1] < 60 && px[2] < 60)
                .count();
            dark_opaque_total += dark_opaque;
            if dark_opaque > 0 {
                println!("frame {}: dark_opaque={dark_opaque}", frame.index);
            }
        }
        // Take every 2nd frame to keep the GIF small.
        if frame.index % 2 == 0 {
            let gif_frame = GifFrame::from_parts(
                frame.image.clone(),
                0,
                0,
                image::Delay::from_numer_denom_ms(delay_ms, 1),
            );
            encoder.encode_frame(gif_frame)?;
        }
        frames += 1;
    }
    println!("rendered {frames} frames, encoded to {out_path}");
    if transparent {
        println!(
            "透明背景黑边检查: 暗色不透明像素总数 = {dark_opaque_total} -> {}",
            if dark_opaque_total == 0 {
                "通过，无黑色描边"
            } else {
                "失败，仍有黑色描边"
            }
        );
    }
    Ok(())
}
