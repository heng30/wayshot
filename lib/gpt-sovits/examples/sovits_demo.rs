// https://huggingface.co/mikv39/gpt-sovits-onnx-custom
// https://huggingface.co/cisco-ai/mini-bart-g2p/tree/main/onnx

use gpt_sovits::{
    GSVError, GptSoVitsModel, GptSoVitsModelConfig, LangId, OUTPUT_AUDIO_CHANNEL,
    OUTPUT_AUDIO_SAMPLE_RATE, SamplingParams, StreamExt,
};
use hound::{WavSpec, WavWriter};
use rodio::{OutputStreamBuilder, Sink, buffer::SamplesBuffer};
use std::path::Path;

const TEXT: &str = "你好呀，我们是一群追逐梦想的人。1.0版本什么时候发布？Reference audio too short, must be at least 0.5 seconds. 随着时间推移，两者的代码库已大幅分化，XNNPACK的API也不再与QNNPACK兼容。面向移动端、服务器及Web的高效浮点神经网络推理算子。";

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
        println!("Received chunk: {} samples", chunk_len);

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

    let data_dir = Path::new("data");
    let output_dir = Path::new("tmp");
    let assets_dir = Path::new("assets");

    let output_stream = OutputStreamBuilder::from_default_device()?.open_stream()?;
    let player = Sink::connect_new(output_stream.mixer());
    player.set_volume(0.25);
    player.play();

    let config = GptSoVitsModelConfig::default()
        .with_sovits_path(assets_dir.join("custom_vits.onnx"))
        .with_ssl_path(assets_dir.join("ssl.onnx"))
        .with_t2s_encoder_path(assets_dir.join("custom_t2s_encoder.onnx"))
        .with_t2s_fs_decoder_path(assets_dir.join("custom_t2s_fs_decoder.onnx"))
        .with_t2s_s_decoder_path(assets_dir.join("custom_t2s_s_decoder.onnx"))
        .with_bert_path(Some(assets_dir.join("bert.onnx")))
        .with_g2pw_path(Some(assets_dir.join("g2pW.onnx")))
        .with_g2p_en_path(Some(assets_dir.join("g2p_en")));

    let mut tts = GptSoVitsModel::new(config)?;

    synth(
        &mut tts,
        data_dir.join("me.wav"),
        "这里是这个库支持的编解码器",
        TEXT,
        &player,
        Some(output_dir.join("output.wav")),
    )
    .await?;
    // synth(
    //     &mut tts,
    //     assets_dir.join("bajie.mp3"),
    //     "看你得意地，一听说炸妖怪，就跟见你外公似的你看！",
    //     text,
    //     &player,
    // )
    // .await?;
    // synth(
    //     &mut tts,
    //     assets_dir.join("ref.wav"),
    //     "格式化，可以给自家的奶带来大量的。",
    //     text,
    //     &player,
    // )
    // .await?;
    // synth(
    //     &mut tts,
    //     assets_dir.join("hello_in_cn.mp3"),
    //     "你好啊，我是智能语音助手。",
    //     text,
    //     &player,
    // )
    // .await?;
    player.sleep_until_end();

    Ok(())
}
