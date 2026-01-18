use fun_ast_nano::{FunASRModelConfig, FunAsrNanoGenerateModel, VadConfig};

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

    // Audio file path
    let audio_path = "./data/nejia.wav";

    println!("Loading model...");
    let mut model = FunAsrNanoGenerateModel::init(config, None, None)?;

    println!("Starting VAD-based segmented transcription...\n");
    let separator = "━".repeat(60);
    println!("{}", separator);

    // Configure VAD parameters
    let vad_config = VadConfig {
        sample_rate: 0,               // Will be auto-detected from audio
        min_speech_duration_ms: 250,  // Minimum speech segment: 250ms
        min_silence_duration_ms: 500, // Split on silence: 500ms
        speech_threshold: 0.01,       // Energy threshold (0.0 - 1.0)
        window_size_ms: 30,           // Analysis window: 30ms
    };

    let request = fun_ast_nano::TranscriptionRequest::default()
        .with_audio_path(audio_path.to_string())
        .with_prompt(Some("Transcribe the audio to text.".to_string()))
        .with_max_tokens(512);

    // Use VAD-based segmented transcription
    let response = model.generate_by_segments(request, Some(vad_config))?;

    println!("\n{}", separator);
    println!("✅ Transcription completed!");
    println!("   Total tokens: {}", response.num_tokens);
    println!(
        "   Total segments (timestamps): {}",
        response.timestamps.len()
    );

    println!("\n{}", separator);
    println!("📋 Full Transcription:");
    println!("{}", separator);
    println!("{}", response.text);
    println!("{}", separator);

    println!("\n⏱️  Timestamp Information (per sentence):");
    for (i, segment) in response.timestamps.iter().enumerate() {
        println!(
            "  [{}] {}ms → {}ms ({}ms)",
            i + 1,
            segment.start_ms,
            segment.end_ms,
            segment.end_ms - segment.start_ms
        );
        println!("      {}", segment.text);
    }

    Ok(())
}
