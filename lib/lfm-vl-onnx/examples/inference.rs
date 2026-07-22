use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use lfm_vl_onnx::{generate, LfmVlModel, LfmTokenizer, Precision};

#[derive(Parser)]
#[command(name = "lfm-vl-inference", about = "LFM2.5-VL-450M ONNX inference")]
struct Args {
    /// Path to the model directory containing onnx/ and tokenizer.json
    #[arg(short, long)]
    model_dir: PathBuf,

    /// Path to the input image
    #[arg(short, long)]
    image: PathBuf,

    /// Text prompt (question about the image)
    #[arg(short, long, default_value = "What is in this image?")]
    prompt: String,

    /// Model precision variant
    #[arg(short, long, default_value = "fp16")]
    precision: String,

    /// Maximum number of tokens to generate
    #[arg(short, long, default_value_t = 512)]
    max_tokens: usize,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();

    let precision = match args.precision.to_lowercase().as_str() {
        "fp32" => Precision::Fp32,
        "fp16" => Precision::Fp16,
        "q4" => Precision::Q4,
        "q8" => Precision::Q8,
        other => anyhow::bail!("Unknown precision '{}'. Use: fp32, fp16, q4, q8", other),
    };

    println!("Loading model from {:?} with precision {:?}", args.model_dir, precision);
    let t0 = Instant::now();
    let mut model = LfmVlModel::load(&args.model_dir, precision)?;
    let model_load_time = t0.elapsed();
    println!("Model loaded in {:.2}s.", model_load_time.as_secs_f64());

    // Load tokenizer
    let tokenizer_path = args.model_dir.join("tokenizer.json");
    let tokenizer = LfmTokenizer::from_file(&tokenizer_path)?;
    println!("Tokenizer loaded.");

    // Load image
    let img = image::open(&args.image)
        .map_err(|e| anyhow::anyhow!("Failed to load image: {}", e))?;
    println!("Image loaded: {}x{}", img.width(), img.height());

    // Generate
    println!("\nPrompt: {}", args.prompt);
    println!("Generating...");
    let t1 = Instant::now();
    let result = generate(&mut model, &tokenizer, &img, &args.prompt, args.max_tokens)?;
    let inference_time = t1.elapsed();
    println!("\nResult: {}", result);
    println!("\nInference completed in {:.2}s.", inference_time.as_secs_f64());

    Ok(())
}
