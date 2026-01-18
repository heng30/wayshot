use fun_ast_nano::{FunASRModelConfig, FunAsrNanoGenerateModel};
use std::io::{self, Write};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let model_dir = "./Fun-ASR-Nano-2512";
    let config = FunASRModelConfig::default()
        .with_model_weights(format!("{}/model.pt", model_dir))
        .with_asr_config(format!("{}/config.yaml", model_dir))
        .with_llm_config(format!("{}/Qwen3-0.6B/config.json", model_dir))
        .with_generation_config(format!("{}/Qwen3-0.6B/generation_config.json", model_dir))
        .with_tokenizer_path(format!("{}/Qwen3-0.6B", model_dir));

    let audio_path = "./data/65s.wav";

    println!("Loading model...");
    let mut model = FunAsrNanoGenerateModel::init(config, None, None)?;

    println!("Starting real-time streaming transcription...\n");
    let separator = "━".repeat(60);
    println!("{}", separator);

    let request = fun_ast_nano::TranscriptionRequest::default()
        .with_audio_path(audio_path.to_string())
        .with_prompt(Some("Transcribe the audio to text.".to_string()))
        .with_max_tokens(512);

    // Use streaming callback for real-time output
    let response = model.generate_stream_callback(request, |chunk| {
        if chunk.is_finished {
            println!("\n{}", separator);
            println!("✅ Transcription completed!");
            println!("   Total tokens: {}", chunk.num_tokens);
            println!("   Progress: 100.0%");
        } else {
            // Print progress and current sentence
            let progress_clamped = chunk.progress.min(100.0).max(0.0);
            let progress_bar = "█".repeat((progress_clamped / 5.0) as usize);
            let progress_empty = "░".repeat(20_usize.saturating_sub(progress_bar.len()));

            let current_sent = chunk.current_sentence.as_deref().unwrap_or("");

            print!(
                "\r[{}{}] {:.1}% | {} tokens",
                progress_bar, progress_empty, chunk.progress, chunk.num_tokens
            );

            if !current_sent.is_empty() {
                print!(" | {}", current_sent);
            }

            io::stdout().flush()?;
        }
        Ok(())
    })?;

    println!("\n\n{}", separator);
    println!("📋 Final Result:");
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
