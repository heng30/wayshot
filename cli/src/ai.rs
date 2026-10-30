use anyhow::{Context, Result, anyhow};
use clap::{Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use crate::util;

#[derive(Clone, Copy, ValueEnum)]
pub enum BgModel {
    Modnet,
    Rmbg14,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum CutoutModel {
    U2net,
    U2netp,
    U2netClothSeg,
    U2netHumanSeg,
    IsnetAnime,
    IsnetGeneralUse,
    Silueta,
}

impl CutoutModel {
    fn to_model(self) -> cutout::model::Model {
        use cutout::model::Model;
        match self {
            CutoutModel::U2net => Model::U2Net,
            CutoutModel::U2netp => Model::U2NetP,
            CutoutModel::U2netClothSeg => Model::U2NetClothSeg,
            CutoutModel::U2netHumanSeg => Model::U2NetHumanSeg,
            CutoutModel::IsnetAnime => Model::IsnetAnime,
            CutoutModel::IsnetGeneralUse => Model::IsnetGeneralUse,
            CutoutModel::Silueta => Model::Silueta,
        }
    }
}

#[derive(Subcommand)]
pub enum Cmd {
    /// 图片超分放大（SwinIR x4）
    Upscale {
        input: PathBuf,
        output: PathBuf,
        /// ONNX 模型路径
        #[arg(long)]
        model: PathBuf,
    },

    /// 去背景，输出透明 PNG
    RemoveBg {
        input: PathBuf,
        output: PathBuf,
        /// ONNX 模型路径
        #[arg(long)]
        model: PathBuf,
        /// 模型类型，缺省按模型文件名推断
        #[arg(long, value_enum)]
        kind: Option<BgModel>,
        /// 同时输出灰度遮罩图
        #[arg(long)]
        mask: Option<PathBuf>,
    },

    /// 去水印（LaMa 修复）
    Dewatermark {
        input: PathBuf,
        output: PathBuf,
        /// ONNX 模型路径（lama_fp32.onnx）
        #[arg(long)]
        model: PathBuf,
        #[arg(long)]
        x: u32,
        #[arg(long)]
        y: u32,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
    },

    /// 抠图（U2Net / ISNet），输出透明 PNG
    Cutout {
        input: PathBuf,
        output: PathBuf,
        /// ONNX 模型路径
        #[arg(long)]
        model: PathBuf,
        /// 模型类型，缺省按模型文件名推断
        #[arg(long, value_enum)]
        kind: Option<CutoutModel>,
        /// alpha 阈值（0-255），越大抠得越狠
        #[arg(long, default_value_t = 160)]
        threshold: u8,
        /// 硬边抠图，不保留半透明边缘
        #[arg(long)]
        binary: bool,
        /// 贴纸描边颜色（rrggbb）
        #[arg(long)]
        sticker: Option<String>,
        /// 同时输出灰度遮罩
        #[arg(long)]
        mask: Option<PathBuf>,
    },

    /// 人声分离（Demucs），输出各声部 WAV
    SplitStems {
        /// 音频或视频文件
        input: PathBuf,
        /// 输出目录
        #[arg(short, long)]
        output_dir: PathBuf,
        /// ONNX 模型路径
        #[arg(long)]
        model: PathBuf,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Upscale {
            input,
            output,
            model,
        } => upscale(input, output, model),
        Cmd::RemoveBg {
            input,
            output,
            model,
            kind,
            mask,
        } => remove_bg(input, output, model, *kind, mask.as_deref()),
        Cmd::Dewatermark {
            input,
            output,
            model,
            x,
            y,
            width,
            height,
        } => dewatermark(
            input,
            output,
            model,
            dewatermark::WatermarkRegion {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
            },
        ),
        Cmd::Cutout {
            input,
            output,
            model,
            kind,
            threshold,
            binary,
            sticker,
            mask,
        } => do_cutout(
            input,
            output,
            model,
            *kind,
            *threshold,
            *binary,
            sticker.as_deref(),
            mask.as_deref(),
        ),
        Cmd::SplitStems {
            input,
            output_dir,
            model,
        } => split_stems(input, output_dir, model),
    }
}

fn upscale(input: &Path, output: &Path, model: &Path) -> Result<Value> {
    let image = image::open(input).with_context(|| format!("无法打开图片: {}", input.display()))?;
    let mut session =
        clear_vision::load_session(model).map_err(|e| anyhow!("加载模型失败: {e}"))?;

    let result = clear_vision::process(&image, &mut session, |_| {}, || false)
        .map_err(|e| anyhow!("超分失败: {e}"))?;

    let (w, h) = (result.width(), result.height());
    result
        .save(output)
        .with_context(|| format!("无法写入图片: {}", output.display()))?;

    Ok(json!({
        "path": output.display().to_string(),
        "width": w,
        "height": h,
    }))
}

fn remove_bg(
    input: &Path,
    output: &Path,
    model_path: &Path,
    kind: Option<BgModel>,
    mask: Option<&Path>,
) -> Result<Value> {
    use background_remover::Model;

    let model = match kind {
        Some(BgModel::Modnet) => Model::Modnet,
        Some(BgModel::Rmbg14) => Model::Rmbg14,
        None => {
            let filename = model_path
                .file_name()
                .and_then(|f| f.to_str())
                .ok_or_else(|| anyhow!("无法从模型路径推断类型，请用 --kind 指定"))?;
            Model::try_from_filename(filename).ok_or_else(|| {
                anyhow!("无法从 {filename} 推断模型类型，请用 --kind modnet|rmbg14")
            })?
        }
    };

    let image = image::open(input).with_context(|| format!("无法打开图片: {}", input.display()))?;
    let rgb = image.to_rgb8();

    let mut remover = background_remover::BackgroundRemover::new(model, model_path)
        .map_err(|e| anyhow!("加载模型失败: {e}"))?;
    let (result, gray) = remover
        .remove_with_mask(&rgb)
        .map_err(|e| anyhow!("去背景失败: {e}"))?;

    util::save_rgba(&result, output)?;
    if let Some(mask_path) = mask {
        gray.save(mask_path)
            .with_context(|| format!("无法写入遮罩: {}", mask_path.display()))?;
    }

    Ok(util::image_json(output, &result))
}

fn dewatermark(
    input: &Path,
    output: &Path,
    model: &Path,
    region: dewatermark::WatermarkRegion,
) -> Result<Value> {
    let image = image::open(input).with_context(|| format!("无法打开图片: {}", input.display()))?;
    let mut session = dewatermark::load_session(model).map_err(|e| anyhow!("加载模型失败: {e}"))?;

    let result = dewatermark::process(
        &image,
        &mut session,
        &dewatermark::MaskInput::Rect(region),
        |_| {},
    )
    .map_err(|e| anyhow!("去水印失败: {e}"))?;

    let (w, h) = (result.width(), result.height());
    result
        .save(output)
        .with_context(|| format!("无法写入图片: {}", output.display()))?;

    Ok(json!({
        "path": output.display().to_string(),
        "width": w,
        "height": h,
    }))
}

#[allow(clippy::too_many_arguments)]
fn do_cutout(
    input: &Path,
    output: &Path,
    model_path: &Path,
    kind: Option<CutoutModel>,
    threshold: u8,
    binary: bool,
    sticker: Option<&str>,
    mask: Option<&Path>,
) -> Result<Value> {
    let model = match kind {
        Some(kind) => kind.to_model(),
        None => {
            let filename = model_path
                .file_name()
                .and_then(|f| f.to_str())
                .ok_or_else(|| anyhow!("无法从模型路径推断类型，请用 --kind 指定"))?;
            cutout::model::Model::try_from_filename(filename)
                .ok_or_else(|| anyhow!("无法从 {filename} 推断模型类型，请用 --kind 指定"))?
        }
    };

    let image = image::open(input).with_context(|| format!("无法打开图片: {}", input.display()))?;
    let mut manager = cutout::manager::ModelManager::from_file(model_path)
        .map_err(|e| anyhow!("加载模型失败: {e}"))?;

    let mut options = cutout::cutout::CutoutOptions {
        threshold,
        binary,
        sticker: None,
        mask: None,
    };
    if let Some(hex) = sticker {
        options.sticker = Some(image::Rgba([
            parse_rgb(hex)?.0,
            parse_rgb(hex)?.1,
            parse_rgb(hex)?.2,
            255,
        ]));
    }

    let result = cutout::cutout::cutout(&mut manager, model, image, &options)
        .map_err(|e| anyhow!("抠图失败: {e}"))?;

    let (rgba, gray) = result.into_parts();
    util::save_rgba(&rgba, output)?;
    if let Some(mask_path) = mask {
        gray.save(mask_path)
            .with_context(|| format!("无法写入遮罩: {}", mask_path.display()))?;
    }

    Ok(util::image_json(output, &rgba))
}

fn parse_rgb(hex: &str) -> Result<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    let value = u32::from_str_radix(hex, 16).map_err(|_| anyhow!("颜色格式应为 rrggbb: {hex}"))?;
    if hex.len() != 6 {
        return Err(anyhow!("颜色格式应为 rrggbb: {hex}"));
    }
    Ok(((value >> 16) as u8, (value >> 8) as u8, value as u8))
}

fn split_stems(input: &Path, output_dir: &Path, model: &Path) -> Result<Value> {
    let audio = audio_utils::loader::load_audio_file_and_convert(input, 2, 44_100)
        .map_err(|e| anyhow!("无法读取音频 {}: {e}", input.display()))?;

    let data = stem_splitter::AudioData {
        samples: audio.samples,
        sample_rate: audio.sample_rate,
        channels: audio.channel,
    };

    let handle = stem_splitter::ModelHandle::from_embedded(model.to_path_buf())
        .map_err(|e| anyhow!("加载模型失败: {e}"))?;
    handle.reset_cancel();

    let result =
        stem_splitter::split(&data, &handle, None).map_err(|e| anyhow!("分离失败: {e}"))?;

    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("无法创建目录: {}", output_dir.display()))?;

    let mut stems = Vec::new();
    for (name, stem) in &result.stems {
        let path = output_dir.join(format!("{name}.wav"));
        util::write_wav(&path, &stem.samples, stem.sample_rate, stem.channels)?;
        stems.push(json!({
            "stem": name,
            "path": path.display().to_string(),
            "sample_rate": stem.sample_rate,
            "channels": stem.channels,
        }));
    }

    Ok(json!({ "stems": stems }))
}
