use anyhow::{Context, Result, anyhow};
use candle_core::{DType, Device};
use clap::{Subcommand, ValueEnum};
use paddle_ocr_rs::{OcrTask, PaddleOCR};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Clone, Copy, ValueEnum)]
pub enum Task {
    /// 纯文本识别
    Text,
    /// 文本 + 位置框
    Spotting,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum DeviceKind {
    Cpu,
    Cuda,
    Metal,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Dtype {
    F32,
    F16,
    Bf16,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// 图片文字识别（PaddleOCR-VL）
    Run {
        input: PathBuf,
        /// 模型目录
        #[arg(long)]
        model: PathBuf,
        #[arg(long, value_enum, default_value_t = Task::Text)]
        task: Task,
        /// 自定义提示词，缺省用任务默认提示词
        #[arg(long)]
        prompt: Option<String>,
        #[arg(long, value_enum, default_value_t = DeviceKind::Cpu)]
        device: DeviceKind,
        /// 推理精度，缺省由模型配置决定
        #[arg(long, value_enum)]
        dtype: Option<Dtype>,
        /// 图片宽高，用于换算像素坐标
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Run {
            input,
            model,
            task,
            prompt,
            device,
            dtype,
            width,
            height,
        } => ocr(
            input,
            model,
            *task,
            prompt.as_deref(),
            *device,
            *dtype,
            *width,
            *height,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn ocr(
    input: &std::path::Path,
    model: &std::path::Path,
    task: Task,
    prompt: Option<&str>,
    device: DeviceKind,
    dtype: Option<Dtype>,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<Value> {
    let device = match device {
        DeviceKind::Cpu => Device::Cpu,
        DeviceKind::Cuda => Device::new_cuda(0).map_err(|e| anyhow!("初始化 CUDA 失败: {e}"))?,
        DeviceKind::Metal => Device::new_metal(0).map_err(|e| anyhow!("初始化 Metal 失败: {e}"))?,
    };
    let dtype = dtype.map(|dtype| match dtype {
        Dtype::F32 => DType::F32,
        Dtype::F16 => DType::F16,
        Dtype::Bf16 => DType::BF16,
    });

    let model_path = model
        .to_str()
        .ok_or_else(|| anyhow!("模型路径不是有效 UTF-8: {}", model.display()))?;
    let input_path = input
        .to_str()
        .ok_or_else(|| anyhow!("图片路径不是有效 UTF-8: {}", input.display()))?;

    let mut ocr = PaddleOCR::new_with_options(model_path, Some(device), dtype)
        .map_err(|e| anyhow!("加载模型失败: {e}"))?;

    let result = match (task, prompt) {
        (_, Some(prompt)) => ocr
            .ocr_with_prompt(input_path, prompt)
            .map_err(|e| anyhow!("识别失败: {e}"))
            .map(|text| paddle_ocr_rs::OcrResult {
                text,
                blocks: Vec::new(),
            })?,
        (Task::Text, None) => ocr
            .ocr_with_task(input_path, OcrTask::Text)
            .map_err(|e| anyhow!("识别失败: {e}"))?,
        (Task::Spotting, None) => ocr
            .ocr_with_task(input_path, OcrTask::Spotting)
            .map_err(|e| anyhow!("识别失败: {e}"))?,
    };

    let blocks: Vec<Value> = result
        .blocks
        .iter()
        .map(|block| {
            let mut item = json!({ "text": block.text });
            if let (Some(w), Some(h)) = (width, height)
                && let Some((x1, y1, x2, y2)) = block.bbox_pixels(w, h)
            {
                item["bbox_pixels"] = json!([x1, y1, x2, y2]);
            }
            if let Some((x1, y1, x2, y2)) = block.bbox_normalized() {
                item["bbox_normalized"] = json!([x1, y1, x2, y2]);
            }
            item
        })
        .collect();

    Ok(json!({
        "path": input.display().to_string(),
        "text": result.text,
        "blocks": blocks,
    }))
}

#[allow(dead_code)]
fn need_utf8(path: &std::path::Path) -> Result<&str> {
    path.to_str()
        .with_context(|| format!("路径不是有效 UTF-8: {}", path.display()))
}
