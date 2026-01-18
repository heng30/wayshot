use fun_ast_nano::{FunASRModelConfig, FunAsrNanoGenerateModel};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Initialize logger
    env_logger::init();

    // Model directory
    let model_dir = "./Fun-ASR-Nano-2512";

    // Use builder pattern to configure model files
    let config = FunASRModelConfig::default()
        .with_model_weights(format!("{}/model.pt", model_dir))
        .with_asr_config(format!("{}/config.yaml", model_dir))
        .with_llm_config(format!("{}/Qwen3-0.6B/config.json", model_dir))
        .with_generation_config(format!("{}/Qwen3-0.6B/generation_config.json", model_dir))
        .with_tokenizer_path(format!("{}/Qwen3-0.6B", model_dir));

    // Verify all files exist before loading
    println!("Checking model files...");
    for (name, path) in [
        ("Model weights", &config.model_weights),
        ("ASR config", &config.asr_config),
        ("LLM config", &config.llm_config),
        ("Generation config", &config.generation_config),
        ("Tokenizer", &config.tokenizer_path),
    ] {
        if !Path::new(path).exists() {
            eprintln!("❌ {} not found: {}", name, path);
            std::process::exit(1);
        }
        println!("✓ Found: {}", name);
    }

    // Audio file path
    let audio_path = "./data/nejia.wav";
    if !Path::new(audio_path).exists() {
        eprintln!("Audio file not found: {}", audio_path);
        std::process::exit(1);
    }

    println!("\nLoading model...");
    let mut model = FunAsrNanoGenerateModel::init(config, None, None)?;

    println!("Transcribing audio: {}", audio_path);

    // Use builder pattern for request
    let request = fun_ast_nano::TranscriptionRequest::default()
        .with_audio_path(audio_path.to_string())
        .with_prompt(Some("Transcribe the audio to text.".to_string()))
        .with_max_tokens(512);

    let response = model.generate(request)?;

    println!("\n=== Transcription Result ===");
    println!("Text: {}", response.text);
    println!("Tokens: {}", response.num_tokens);

    println!("\n=== Timestamp Information ===");
    for (i, segment) in response.timestamps.iter().enumerate() {
        println!("Segment {}:", i + 1);
        println!("  Text: {}", segment.text);
        println!("  Time: {}ms -> {}ms", segment.start_ms, segment.end_ms);
        println!("  Duration: {}ms", segment.end_ms - segment.start_ms);
        println!(
            "  Tokens: [{}, {})",
            segment.token_range.0, segment.token_range.1
        );
    }

    Ok(())
}
