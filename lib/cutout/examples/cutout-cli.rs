use clap::Parser;
use cutout::{
    cutout::{CutoutOptions, cutout},
    manager::ModelManager,
    model::Model,
};
use image::Rgba;
use std::{path::PathBuf, process};

#[derive(Parser)]
#[command(
    name = "cutout",
    about = "Remove image backgrounds using neural networks"
)]
struct Args {
    /// Input image file path
    #[arg(short = 'i', long = "input")]
    input: PathBuf,

    /// Output image file path
    #[arg(short = 'o', long = "output")]
    output: PathBuf,

    /// ONNX model file path
    #[arg(short = 'm', long = "model", default_value = "u2net.onnx")]
    model: PathBuf,

    /// Alpha matting threshold (0-255)
    #[arg(short = 't', long = "threshold", default_value = "160")]
    threshold: u8,

    /// Enable binary mask mode
    #[arg(short = 'b', long = "binary")]
    binary: bool,

    /// Save mask as separate file
    #[arg(short = 's', long = "save-mask")]
    save_mask: bool,

    /// Sticker border color as R,G,B,A (e.g. "0,0,0,255" for black)
    #[arg(long = "sticker", value_name = "R,G,B,A")]
    sticker: Option<String>,

    /// Optional region mask image (white=process, black=keep background)
    #[arg(long = "mask")]
    mask: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    println!("cutout - Background Removal Tool");
    println!("Input: {:?}", args.input);
    println!("Output: {:?}", args.output);
    println!("Model: {:?}", args.model);
    println!();

    let mut manager = match ModelManager::from_file(&args.model) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load model: {}", e);
            process::exit(1);
        }
    };

    println!("Model loaded\n");

    println!("Loading image...");
    let img = match image::open(&args.input) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("Failed to load image: {}", e);
            process::exit(1);
        }
    };

    // Load user mask if provided
    let user_mask = args.mask.as_ref().map(|path| {
        image::open(path).map(|img| img.to_luma8()).unwrap_or_else(|e| {
            eprintln!("Failed to load mask: {}", e);
            process::exit(1);
        })
    });

    let sticker_color = args.sticker.as_ref().and_then(|s| {
        let parts: Vec<u8> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if parts.len() == 4 {
            Some(Rgba([parts[0], parts[1], parts[2], parts[3]]))
        } else {
            eprintln!("Invalid sticker color format, expected R,G,B,A (e.g. \"0,0,0,255\")");
            process::exit(1);
        }
    });

    let options = CutoutOptions::default()
        .with_threshold(args.threshold)
        .with_binary(args.binary)
        .with_sticker(sticker_color)
        .with_mask(user_mask);

    println!("Processing image...");

    let model_enum = args
        .model
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(Model::try_from_filename)
        .unwrap_or(Model::U2NetP);

    let result = match cutout(&mut manager, model_enum, img, &options) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    println!("Saving result...");
    let result_img = image::DynamicImage::ImageRgba8(result.image().clone());
    if let Err(e) = result_img.save(&args.output) {
        eprintln!("Failed to save result: {}", e);
        process::exit(1);
    }

    if args.save_mask {
        let mask_path = generate_mask_path(&args.output);
        println!("Saving mask to: {:?}", mask_path);

        let mask_img = result.mask();
        if let Err(e) = mask_img.save(&mask_path) {
            eprintln!("Failed to save mask: {}", e);
        }
    }

    println!();
    println!("Background removed successfully!");
    println!("Output saved to: {:?}", args.output);
}

fn generate_mask_path(output_path: &std::path::Path) -> std::path::PathBuf {
    let file_stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let extension = output_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png");

    let parent = output_path.parent().unwrap_or(std::path::Path::new("."));

    parent.join(format!("{}_mask.{}", file_stem, extension))
}
