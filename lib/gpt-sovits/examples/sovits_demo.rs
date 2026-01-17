// https://huggingface.co/mikv39/gpt-sovits-onnx-custom
// https://huggingface.co/cisco-ai/mini-bart-g2p/tree/main/onnx

use gpt_sovits::{
    GSVError, GptSoVitsModel, GptSoVitsModelConfig, LangId, OUTPUT_AUDIO_CHANNEL,
    OUTPUT_AUDIO_SAMPLE_RATE, SamplingParams, StreamExt,
};
use hound::{WavSpec, WavWriter};
use rodio::{OutputStreamBuilder, Sink, buffer::SamplesBuffer};
use std::path::Path;

const TEXT: &str = "Liquid 模板语言是一种开源的、安全的模板语言。最新版本为1.12.3。最初由 Shopify 用 Ruby 3.2 编写并广泛用于其电子商务平台。它的核心设计理念是将业务逻辑与展示层分离，允许非开发者（如设计师、内容管理者）安全地修改界面而不影响后端代码。\nThis is a cross-platform library for interacting with the clipboard. It allows to copy and paste both text and image data in a platform independent way on Linux, Mac, and Windows.";

async fn synth<P>(
    tts: &mut GptSoVitsModel,
    ref_audio_path: P,
    ref_text: &str,
    text: &str,
    player: &Sink,
    output_wav: Option<P>,
) -> Result<(), GSVError>
where
    P: AsRef<Path>,
{
    let ref_data = tts
        .get_reference_data(ref_audio_path, ref_text, LangId::Auto)
        .await?;

    let sampling_params = SamplingParams::default()
        .with_top_k(Some(4))
        .with_top_p(Some(0.9))
        .with_temperature(1.0)
        .with_repetition_penalty(1.35);

    let mut stream = tts
        .synthesize(text, ref_data, sampling_params, LangId::Auto)
        .await?;

    let mut wav_writer =
        if let Some(ref wav_path) = output_wav {
            let spec = WavSpec {
                channels: 1,
                sample_rate: 32000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            Some(WavWriter::create(&wav_path, spec).map_err(|e| {
                GSVError::InternalError(format!("Failed to create WAV file: {}", e))
            })?)
        } else {
            None
        };

    log::info!("Starting streaming synthesis...");

    let mut total_samples = 0;
    while let Some(item) = stream.next().await {
        let audio_chunk = item?;
        let chunk_len = audio_chunk.len();

        log::info!("Received chunk: {} samples", chunk_len);

        player.append(SamplesBuffer::new(
            OUTPUT_AUDIO_CHANNEL,
            OUTPUT_AUDIO_SAMPLE_RATE,
            audio_chunk.clone(),
        ));

        if let Some(ref mut writer) = wav_writer {
            for &sample in &audio_chunk {
                let sample_i16 = (sample * i16::MAX as f32) as i16;
                writer.write_sample(sample_i16).map_err(|e| {
                    GSVError::InternalError(format!("Failed to write WAV sample: {}", e))
                })?;
            }
        }

        total_samples += chunk_len;
    }

    log::info!("Total samples: {}", total_samples);

    if let Some(writer) = wav_writer {
        writer
            .finalize()
            .map_err(|e| GSVError::InternalError(format!("Failed to finalize WAV file: {}", e)))?;
        if let Some(wav_path) = output_wav {
            log::info!("Audio saved to: {}", wav_path.as_ref().display());
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let model_dir = Path::new("model");

    let output_stream = OutputStreamBuilder::from_default_device()?.open_stream()?;
    let player = Sink::connect_new(output_stream.mixer());
    player.set_volume(0.25);
    player.play();

    let config = GptSoVitsModelConfig::default()
        .with_sovits_path(model_dir.join("custom_vits.onnx"))
        .with_ssl_path(model_dir.join("ssl.onnx"))
        .with_t2s_encoder_path(model_dir.join("custom_t2s_encoder.onnx"))
        .with_t2s_fs_decoder_path(model_dir.join("custom_t2s_fs_decoder.onnx"))
        .with_t2s_s_decoder_path(model_dir.join("custom_t2s_s_decoder.onnx"))
        .with_bert_path(model_dir.join("bert.onnx"))
        .with_g2pw_path(model_dir.join("g2pW.onnx"))
        .with_g2p_en_encoder_path(model_dir.join("g2p_en").join("encoder_model.onnx"))
        .with_g2p_en_decoder_path(model_dir.join("g2p_en").join("decoder_model.onnx"));

    let mut tts = GptSoVitsModel::new(config)?;

    let items = vec![
        ("ai.mp3", "你好啊，我是智能语音助手。"),
        // (
        //     "bajie.mp3",
        //     "看你得意地，一听说炸妖怪，就跟见你外公似的你看！",
        // ),
    ];

    for item in items {
        let name = item.0.split('.').into_iter().next().unwrap();

        synth(
            &mut tts,
            Path::new("data").join(item.0),
            item.1,
            TEXT,
            &player,
            Some(
                Path::new("tmp")
                    .join(format!("output-{name}"))
                    .with_extension("wav"),
            ),
        )
        .await?;
    }

    player.sleep_until_end();

    Ok(())
}
