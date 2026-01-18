use fun_ast_nano::{FunASRModelConfig, FunAsrNanoGenerateModel, VadConfig};
use std::io::{self, Write};

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
    // let audio_path = "./data/65s.wav";
    let audio_path = "./data/long.wav";

    println!("Loading model...");
    let mut model = FunAsrNanoGenerateModel::init(config, None, None)?;

    println!("Starting VAD-based segmented transcription with streaming...\n");
    let separator = "━".repeat(70);
    println!("{}", separator);

    // Configure VAD parameters
    let vad_config = VadConfig::default()
        .with_min_speech_duration_ms(250) // Minimum speech segment: 250ms
        .with_min_silence_duration_ms(500); // Split on silence: 500ms

    let request = fun_ast_nano::TranscriptionRequest::default()
        .with_audio_path(audio_path.to_string())
        .with_prompt(Some("Transcribe the audio to text.".to_string()))
        .with_max_tokens(512);

    // Use VAD-based segmented transcription with streaming callback
    let response =
        model.generate_by_segments_stream_callback(request, Some(vad_config), |chunk| {
            if chunk.is_finished {
                println!("\n{}", separator);
                println!("✅ Transcription completed!");
                println!("   Total tokens: {}", chunk.num_tokens);
                println!("   Progress: 100.0%");
            } else {
                // Print progress with segment info
                let progress_clamped = chunk.progress.min(100.0).max(0.0);
                let progress_bar = "█".repeat((progress_clamped / 5.0) as usize);
                let progress_empty = "░".repeat(20_usize.saturating_sub(progress_bar.len()));

                if let Some(seg_info) = chunk.segment_info {
                    print!(
                        "\r[Segment {}/{} | {}ms-{}ms] [{}{}] {:.1}% | {} tokens",
                        seg_info.current_segment,
                        seg_info.total_segments,
                        seg_info.segment_start_ms,
                        seg_info.segment_end_ms,
                        progress_bar,
                        progress_empty,
                        chunk.progress,
                        chunk.num_tokens
                    );

                    // Print current sentence if available
                    if let Some(ref sentence) = chunk.current_sentence {
                        if !sentence.is_empty() {
                            print!(" | \"{}\"", sentence);
                        }
                    }
                } else {
                    print!(
                        "\r[{}{}] {:.1}% | {} tokens",
                        progress_bar, progress_empty, chunk.progress, chunk.num_tokens
                    );
                }

                io::stdout().flush()?;
            }
            Ok(())
        })?;

    println!("\n\n{}", separator);
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
