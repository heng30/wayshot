//! Render an effect into a single animated GIF.
//!
//! Uses the incremental [`SequenceRenderer`] API to pull raw frames and hand
//! them to the GIF encoder instead of writing individual PNGs.
//!
//! Run with: `cargo run --release --example gif -- <ascii-font> [<non-ascii-font>]`

use std::error::Error;
use std::fs::File;

use image::codecs::gif::GifEncoder;
use image::Frame as GifFrame;
use ttfx_rs::effects::EffectCommand;
use ttfx_rs::render::{Font, RenderConfig, SequenceRenderer};

fn main() -> Result<(), Box<dyn Error>> {
    let ascii_font = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/nix/store/c0asr1k4sg3i5xkzkcdnkywphyrw68qa-liberation-fonts-2.1.5/share/fonts/truetype/LiberationMono-Regular.ttf".to_string());
    let non_ascii_font = std::env::args().nth(2).unwrap_or_else(|| ascii_font.clone());
    let font = Font::from_files(&ascii_font, &non_ascii_font)?;

    let input = "GIF output";
    let mut render = RenderConfig::new(640, 160, font);
    render.seed = Some(99);
    render.fps = 30;

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
    while let Some(frame) = renderer.next_frame(effect.as_mut()) {
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
    Ok(())
}
