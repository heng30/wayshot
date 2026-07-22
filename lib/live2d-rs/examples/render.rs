use std::path::PathBuf;

use clap::Parser;
use live2d_rs::{Live2dRenderer, Options};

/// Headless Live2D renderer — renders a model to PNG image frames.
#[derive(Parser, Debug)]
#[command(name = "live2d-render", version, about)]
struct Args {
    /// Path to the .model3.json file
    #[arg(value_name = "MODEL")]
    model: PathBuf,

    /// Output directory for generated PNG frames
    #[arg(short, long, default_value = ".")]
    output: PathBuf,

    /// Output width in pixels
    #[arg(short, long, default_value_t = 512)]
    width: u32,

    /// Output height in pixels
    #[arg(long, default_value_t = 512)]
    height: u32,

    /// Frame rate (frames per second)
    #[arg(short, long, default_value_t = 30.0)]
    fps: f32,

    /// Duration in seconds (0 = render one static frame only)
    #[arg(short, long, default_value_t = 0.0)]
    duration: f32,

    /// Motion index to play (0-based, from the model's motion list)
    #[arg(short, long)]
    motion: Option<usize>,

    /// Expression index to play (0-based, from the model's expression list)
    #[arg(short, long)]
    expression: Option<usize>,

    /// Background color as hex (e.g. "FF0000FF" for opaque red, "00000000" for transparent)
    #[arg(long, default_value = "00000000")]
    background: String,

    /// Model view fill factor (controls how much of the canvas the model fills)
    #[arg(long, default_value_t = 1.85)]
    fill: f32,

    /// List available motions and expressions, then exit
    #[arg(long)]
    list: bool,
}

fn parse_hex_color(hex: &str) -> Option<[u8; 4]> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 8 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
    Some([r, g, b, a])
}

fn main() {
    let args = Args::parse();

    // Validate background color.
    let background = parse_hex_color(&args.background).unwrap_or_else(|| {
        eprintln!("Invalid background color '{}'. Expected 8-digit hex (e.g. FF0000FF).", args.background);
        std::process::exit(1);
    });

    // Validate model path.
    if !args.model.exists() {
        eprintln!("Model file not found: {}", args.model.display());
        std::process::exit(1);
    }

    // Create renderer.
    let options = Options {
        background,
        model_view_fill: args.fill,
    };
    let mut renderer = match Live2dRenderer::new_with_options(
        &args.model,
        args.width,
        args.height,
        options,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to load model: {e}");
            std::process::exit(1);
        }
    };

    // List mode: print motions and expressions, then exit.
    if args.list {
        println!("Motions ({}):", renderer.motion_paths().len());
        for (i, path) in renderer.motion_paths().iter().enumerate() {
            println!("  [{}] {}", i, path.display());
        }
        println!("Expressions ({}):", renderer.expression_paths().len());
        for (i, path) in renderer.expression_paths().iter().enumerate() {
            println!("  [{}] {}", i, path.display());
        }
        return;
    }

    // Play motion.
    if let Some(idx) = args.motion {
        let motion_path = renderer.motion_paths().get(idx).cloned();
        match motion_path {
            Some(path) => match renderer.play_motion(&path) {
                Ok(()) => eprintln!("Playing motion [{}]: {}", idx, path.display()),
                Err(e) => {
                    eprintln!("Failed to play motion: {e}");
                    std::process::exit(1);
                }
            },
            None => {
                eprintln!("Warning: motion index {idx} out of range (0..{}), skipping", renderer.motion_paths().len());
            }
        }
    }

    // Play expression.
    if let Some(idx) = args.expression {
        let expr_path = renderer.expression_paths().get(idx).cloned();
        match expr_path {
            Some(path) => match renderer.play_expression(&path) {
                Ok(()) => eprintln!("Playing expression [{}]: {}", idx, path.display()),
                Err(e) => {
                    eprintln!("Failed to play expression: {e}");
                    std::process::exit(1);
                }
            },
            None => {
                eprintln!("Warning: expression index {idx} out of range (0..{}), skipping", renderer.expression_paths().len());
            }
        }
    }

    // Create output directory.
    if let Err(e) = std::fs::create_dir_all(&args.output) {
        eprintln!("Failed to create output directory: {e}");
        std::process::exit(1);
    }

    // Render frames.
    if args.duration <= 0.0 {
        // Static mode: render one frame.
        let rgba = renderer.render_static();
        let path = args.output.join("frame_000.png");
        save_frame(&path, args.width, args.height, &rgba);
        eprintln!("Saved: {}", path.display());
    } else {
        // Animated mode: render multiple frames using idempotent render_at.
        let total_frames = (args.duration * args.fps).ceil() as usize;
        for i in 0..total_frames {
            let time_s = i as f32 / args.fps;
            let rgba = renderer.render_at(args.fps, time_s);
            let path = args.output.join(format!("frame_{:04}.png", i));
            save_frame(&path, args.width, args.height, &rgba);
            if i % 30 == 0 || i == total_frames - 1 {
                eprintln!("Frame {}/{}: {}", i + 1, total_frames, path.display());
            }
        }
    }

    eprintln!("Done.");
}

fn save_frame(path: &std::path::Path, width: u32, height: u32, rgba: &[u8]) {
    let img = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .expect("invalid frame buffer dimensions");
    img.save(path).unwrap_or_else(|e| {
        eprintln!("Failed to save {}: {e}", path.display());
        std::process::exit(1);
    });
}
