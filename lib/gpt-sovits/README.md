# GPT-SoVITS的Rust推理实现

[![Crates.io](https://img.shields.io/crates/v/gpt-sovits)](https://crates.io/crates/gpt-sovits)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![GitHub](https://img.shields.io/badge/github-mzdk100/GPT--SoVITS-8da0cb?logo=github)](https://github.com/mzdk100/GPT-SoVITS)

本库是使用Rust实现的GPT-SoVITS模型推理引擎，GPT-SoVITS是一个强大的语音合成和语音克隆模型。通过Rust实现，本项目提供了高效、跨平台的部署能力，并对模型进行了深度优化。

## 特性

- ✅ 高性能Rust实现，低资源占用
- ✅ 跨平台支持：Windows、macOS、Linux、Android（理论支持iOS）
- ✅ 流式合成API，支持实时语音生成
- ✅ 语音克隆能力，可基于参考音频复制说话风格
- ✅ 多语言支持，自动语言识别，并支持粤语
- ✅ 完全本地化推理，无需网络连接

## 快速开始

### 安装

直接从crates.io上获取：
```shell
cargo add gpt-sovits
```

### 基本用法

```rust
use gpt_sovits::{GptSoVitsModel, LangId, SamplingParams, StreamExt};

async fn example() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化模型
    let assets_dir = std::path::Path::new("assets");
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
    
    // 获取参考音频数据
    let ref_data = tts
        .get_reference_data("assets/ref.wav", "参考音频的文本", LangId::Auto)
        .await?;
    
    // 设置采样参数
    let sampling_params = SamplingParams::builder()
        .top_k(4)
        .top_p(0.9)
        .temperature(1.0)
        .repetition_penalty(1.35)
        .build();
    
    // 合成语音
    let mut stream = tts
        .synthesize("要合成的文本内容", ref_data, sampling_params, LangId::Auto)
        .await?;
    
    // 处理合成的音频流
    while let Some(item) = stream.next().await {
        let audio_samples = item?;
        // 处理音频样本...
    }
    
    Ok(())
}
```

## 示例程序

项目提供了两个示例应用：

1. **桌面应用** - 位于`desktop`文件夹，支持Windows和macOS平台
   ```shell
   cargo run --release -p gpt-sovits-desktop
   ```

2. **移动应用** - 位于`mobile`文件夹，支持Android平台
   ```shell
   # 需要先安装cargo-apk2
   cargo install cargo-apk2
   # 然后构建并运行
   cargo apk2 run -p gpt-sovits-mobile --release
   ```

详细使用方法请查看各示例文件夹中的README文件。

## 模型下载

使用本库需要下载以下模型文件：

1. **主模型下载地址**：[huggingface.co/mikv39/gpt-sovits-onnx-custom](https://huggingface.co/mikv39/gpt-sovits-onnx-custom)
2. **g2p_en模型下载**：[cisco-ai/mini-bart-g2p](https://huggingface.co/cisco-ai/mini-bart-g2p/tree/main/onnx)

下载后，将模型文件放置在`assets`目录中。

## API文档

详细的API文档可通过以下命令生成：
```shell
cargo doc --open
```

## 性能优化

本项目对GPT-SoVITS模型进行了多项优化：
- ONNX格式转换，提高推理速度
- 流式处理架构，减少延迟
- 内存使用优化，降低资源占用

## 贡献指南

欢迎提交Pull Request或Issue来改进本项目。贡献前请先查看项目的Issue列表，确保不与现有工作重复。

## 致谢

感谢[gpt-sovits-onnx-rs](https://github.com/null-define/gpt-sovits-onnx-rs)对模型优化做出的巨大工作。

## 许可证

本项目采用[Apache 2.0许可证](LICENSE)。