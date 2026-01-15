use {
    gpt_sovits::{GSVError, GptSoVitsModel, LangId, SamplingParams, StreamExt},
    rodio::{OutputStreamBuilder, Sink, buffer::SamplesBuffer},
    std::{path::Path, time::Instant},
};

async fn synth<P, S>(
    tts: &mut GptSoVitsModel,
    ref_audio_path: P,
    ref_text: S,
    text: S,
    player: &Sink,
) -> Result<(), GSVError>
where
    P: AsRef<Path>,
    S: AsRef<str>,
{
    let time = Instant::now();
    let ref_data = tts
        .get_reference_data(ref_audio_path, ref_text, LangId::Auto)
        .await?;
    println!("{:?}", time.elapsed());
    let sampling_params = SamplingParams::builder()
        .top_k(4)
        .top_p(0.9)
        .temperature(1.0)
        .repetition_penalty(1.35)
        .build();
    let mut stream = tts
        .synthesize(text, ref_data, sampling_params, LangId::Auto)
        .await?;
    while let Some(item) = stream.next().await {
        let item = item?;
        println!("{}", item.len());
        player.append(SamplesBuffer::new(1, 32000, item));
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let output_stream = OutputStreamBuilder::from_default_device()?.open_stream()?;
    let player = Sink::connect_new(output_stream.mixer());
    player.set_volume(0.25);
    player.play();

    // 模型下载地址：
    // 1. https://huggingface.co/mikv39/gpt-sovits-onnx-custom
    // 2. https://huggingface.co/cisco-ai/mini-bart-g2p/tree/main/onnx
    let assets_dir = Path::new("assets");
    let mut tts = GptSoVitsModel::new(
        assets_dir.join("custom_vits.onnx"),
        assets_dir.join("ssl.onnx"),
        assets_dir.join("custom_t2s_encoder.onnx"),
        assets_dir.join("custom_t2s_fs_decoder.onnx"),
        assets_dir.join("custom_t2s_s_decoder.onnx"),
        Some(assets_dir.join("bert.onnx")),
        Some(assets_dir.join("g2pW.onnx")),
        Some(assets_dir.join("g2p_en")),
    )?;
    let text = "你好呀，我们是一群追逐梦想的人。\
            1.0版本什么时候发布？\
            Reference audio too short, must be at least 0.5 seconds.\
            随着时间推移，两者的代码库已大幅分化，XNNPACK 的 API 也不再与 QNNPACK 兼容。\
            面向移动端、服务器及Web的高效浮点神经网络推理算子。\
            在那广袤无垠的天元大陆，灵秀山川与繁华市井交相辉映，修仙者宛如星辰般闪耀，掌控着超凡脱俗的力量，于天地间纵横驰骋。\
            然而，在大陆一隅的偏远小村，生活着我们的主角，一个名叫林羽的农家少年………";
    synth(
        &mut tts,
        assets_dir.join("bajie.mp3"),
        "看你得意地，一听说炸妖怪，就跟见你外公似的你看！",
        text,
        &player,
    )
    .await?;
    synth(
        &mut tts,
        assets_dir.join("ref.wav"),
        "格式化，可以给自家的奶带来大量的。",
        text,
        &player,
    )
    .await?;
    synth(
        &mut tts,
        assets_dir.join("hello_in_cn.mp3"),
        "你好啊，我是智能语音助手。",
        text,
        &player,
    )
    .await?;
    player.sleep_until_end();

    Ok(())
}
