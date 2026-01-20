use fun_ast_nano::{FunASRModelConfig, FunAsrNanoGenerateModel, VadConfig, load_wav};
use std::io::{self, Write};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let model_dir = "./Fun-ASR-Nano-2512";
    let config = FunASRModelConfig::default()
        .with_model_weights(format!("{}/model.pt", model_dir))
        .with_tokenizer_path(format!("{}/Qwen3-0.6B/tokenizer.json", model_dir));

    // let audio_path = "./data/nejia.wav";
    let audio_path = "./data/65s.wav";
    // let audio_path = "./data/long.wav";

    let input_audio_config = load_wav(audio_path, hound::SampleFormat::Int)?;

    log::debug!("Loading model...");
    let mut model = FunAsrNanoGenerateModel::new(config, None, None)?;
    let separator = "━".repeat(70);
    let mut total_tokens = 0;

    let vad_config = VadConfig::default()
        .with_min_speech_duration_ms(250)
        .with_min_silence_duration_ms(200);

    let request = fun_ast_nano::TranscriptionRequest::default()
        .with_audio_config(input_audio_config)
        .with_prompt(Some("Transcribe the audio to text.".to_string()))
        .with_max_tokens(512);

    let response = model.generate(request, Some(vad_config), |chunk| {
        if !chunk.is_finished {
            total_tokens += chunk.num_tokens;

            if let Some(seg_info) = chunk.segment_info {
                log::debug!(
                    "[Segment {}/{} | {}ms-{}ms] {:.1}% | {}/{} tokens",
                    seg_info.current_segment,
                    seg_info.total_segments,
                    seg_info.segment_start_ms,
                    seg_info.segment_end_ms,
                    chunk.progress * 100.0,
                    chunk.num_tokens,
                    total_tokens
                );

                if !chunk.text.is_empty() {
                    log::debug!("\"{}\"\n", chunk.text);
                }
            } else {
                log::debug!(
                    "\r {:.1}% | {} tokens",
                    chunk.progress * 100.0,
                    chunk.num_tokens
                );
            }
        }

        io::stdout().flush()?;
        Ok(())
    })?;

    log::debug!("📋 Full Transcription:");
    log::debug!("{}", separator);
    log::debug!("{}", response.text);
    log::debug!("{}", separator);

    Ok(())
}
