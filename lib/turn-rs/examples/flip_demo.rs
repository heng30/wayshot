//! Page flip animation generator CLI.
//!
//! Usage:
//!   cargo run --example flip -F animation -- <image> [options]
//!   cargo run --example flip -F animation -- gen-test [path]
//!
//! Examples:
//!   cargo run --example flip -F animation -- photo.png
//!   cargo run --example flip -F animation -- photo.png --corner bl --axis vertical --direction roundtrip
//!   cargo run --example flip -F animation -- photo.png --output out.webp --png-dir frames/

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use image::{Rgba, RgbaImage};
use turn_rs::{
    Corner, FlipAxis, FlipConfig, FlipDirection,
    generate_flip_to_webp, generate_flip_to_pngs,
};

#[derive(Clone, clap::ValueEnum)]
enum CliCorner {
    Br,
    Bl,
    Tr,
    Tl,
}

#[derive(Clone, clap::ValueEnum)]
enum CliAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, clap::ValueEnum)]
enum CliDirection {
    Forward,
    Backward,
    Roundtrip,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a solid-color test image (400×600 blue)
    GenTest {
        /// Output path [default: output/test.png]
        path: Option<PathBuf>,
    },
    /// Generate a page flip animation
    Flip {
        /// Input image path
        image: PathBuf,

        /// Corner: br, bl, tr, tl
        #[arg(short, long, default_value = "br")]
        corner: CliCorner,

        /// Axis: horizontal, vertical
        #[arg(short, long, default_value = "horizontal")]
        axis: CliAxis,

        /// Direction: forward, backward, roundtrip
        #[arg(short, long, default_value = "forward")]
        direction: CliDirection,

        /// Output WebP path
        #[arg(short, long, default_value = "output/flip.webp")]
        output: PathBuf,

        /// Also output PNG frames to this directory
        #[arg(long)]
        png_dir: Option<PathBuf>,

        /// Animation duration in milliseconds
        #[arg(long, default_value = "800")]
        duration: u32,

        /// Number of frames
        #[arg(long, default_value = "60")]
        frames: u32,

        /// Disable shadow/highlight
        #[arg(long)]
        no_shadow: bool,
    },
}

#[derive(Parser)]
#[command(name = "flip", about = "Generate page flip animations")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::GenTest { path } => {
            let path = path.unwrap_or_else(|| PathBuf::from("output/test.png"));
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            let img = RgbaImage::from_pixel(400, 600, Rgba([70, 130, 220, 255]));
            img.save(&path).unwrap();
            println!("Generated test image: {}", path.display());
        }
        Command::Flip {
            image,
            corner,
            axis,
            direction,
            output,
            png_dir,
            duration,
            frames,
            no_shadow,
        } => {
            let front = image::open(&image)
                .expect("Failed to load image")
                .to_rgba8();

            let (w, h) = (front.width(), front.height());
            println!("Loaded image: {}x{}", w, h);

            let back = RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, 255]));

            let corner = match corner {
                CliCorner::Br => Corner::BottomRight,
                CliCorner::Bl => Corner::BottomLeft,
                CliCorner::Tr => Corner::TopRight,
                CliCorner::Tl => Corner::TopLeft,
            };

            let axis = match axis {
                CliAxis::Horizontal => FlipAxis::Horizontal,
                CliAxis::Vertical => FlipAxis::Vertical,
            };

            let direction = match direction {
                CliDirection::Forward => FlipDirection::Forward,
                CliDirection::Backward => FlipDirection::Backward,
                CliDirection::Roundtrip => FlipDirection::RoundTrip,
            };

            let config = FlipConfig {
                corner,
                duration_ms: duration,
                time_ms: 0,
                shadow: !no_shadow,
                direction,
                axis,
                flip_extent: 1.0,
            };

            // Ensure output directory exists
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }

            // Generate WebP
            println!(
                "Generating WebP: {} ({}ms, {} frames)...",
                output.display(),
                config.duration_ms,
                frames
            );
            generate_flip_to_webp(&front, &back, &config, frames, &output)
                .expect("Failed to generate WebP");
            println!("Saved: {}", output.display());

            // Optionally generate PNG frames
            if let Some(ref png_dir) = png_dir {
                println!("Generating PNG frames: {}/", png_dir.display());
                generate_flip_to_pngs(&front, &back, &config, frames, png_dir)
                    .expect("Failed to generate PNG frames");
                println!("Saved: {}/", png_dir.display());
            }
        }
    }
}
