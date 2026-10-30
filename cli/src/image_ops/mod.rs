use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use clap::{Subcommand, ValueEnum};
use image::imageops;
use serde_json::{Value, json};

use crate::util;

mod effect;

#[derive(Subcommand)]
pub enum Cmd {
    /// 转换图片格式（按输出扩展名）
    Convert { input: PathBuf, output: PathBuf },

    /// 缩放图片
    Resize {
        input: PathBuf,
        output: PathBuf,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        /// stretch=拉伸铺满，contain=等比缩放并留白
        #[arg(long, value_enum, default_value_t = Fit::Contain)]
        fit: Fit,
    },

    /// 裁剪图片
    Crop {
        input: PathBuf,
        output: PathBuf,
        #[arg(long)]
        x: u32,
        #[arg(long)]
        y: u32,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
    },

    /// 旋转图片（顺时针）
    Rotate {
        input: PathBuf,
        output: PathBuf,
        /// 90 / 180 / 270
        #[arg(long)]
        degrees: u32,
    },

    /// 翻转图片
    Flip {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, value_enum)]
        axis: Axis,
    },

    /// 生成缩略图（等比缩放，长边不超过 size）
    Thumbnail {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, default_value_t = 320)]
        size: u32,
    },

    /// 应用图像特效
    Effect {
        input: PathBuf,
        output: PathBuf,
        /// 特效名，见 `image effect --help` 的列表，或 `image filters`
        #[arg(long)]
        name: String,
        /// 主参数：模糊半径 / 亮度 / 饱和度 / 角度 / 强度 ...
        #[arg(long, allow_hyphen_values = true)]
        value: Option<f64>,
        /// 次参数：仅部分特效使用（vignette 半径、tint 的 g 通道 ...）
        #[arg(long, allow_hyphen_values = true)]
        value2: Option<f64>,
        /// 次参数：tint 的 b 通道 / preset 预设名请用 --name preset --text
        #[arg(long, allow_hyphen_values = true)]
        value3: Option<f64>,
        /// preset 预设滤镜名，或 tint 的颜色名（r,g,b 用 --value* 传）
        #[arg(long)]
        text: Option<String>,
    },

    /// 列出可用特效
    Filters,

    /// 长截图拼接：把多张有重叠区域的滚动截图合成一张
    Stitch {
        /// 按从上到下的顺序给出截图
        inputs: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, value_enum, default_value_t = StitchAlgorithm::Template)]
        algorithm: StitchAlgorithm,
        #[arg(long, default_value_t = 100)]
        min_overlap: u32,
        #[arg(long, default_value_t = 3.5)]
        accept_diff: f32,
    },

    /// 生成纯色图片（做背景/占位）
    Solid {
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        /// 颜色，格式 rrggbb 或 rrggbbaa
        #[arg(long, default_value = "000000")]
        color: String,
    },

    /// 查看图片基本信息
    Info { input: PathBuf },
}
#[derive(Clone, Copy, ValueEnum)]
pub enum Fit {
    Stretch,
    Contain,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum Axis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum StitchAlgorithm {
    Template,
    ColSample,
}
pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Convert { input, output } => convert(input, output),
        Cmd::Resize {
            input,
            output,
            width,
            height,
            fit,
        } => resize(input, output, *width, *height, *fit),
        Cmd::Crop {
            input,
            output,
            x,
            y,
            width,
            height,
        } => crop(input, output, *x, *y, *width, *height),
        Cmd::Rotate {
            input,
            output,
            degrees,
        } => rotate(input, output, *degrees),
        Cmd::Flip {
            input,
            output,
            axis,
        } => flip(input, output, *axis),
        Cmd::Thumbnail {
            input,
            output,
            size,
        } => thumbnail(input, output, *size),
        Cmd::Effect {
            input,
            output,
            name,
            value,
            value2,
            value3,
            text,
        } => effect::effect(
            input,
            output,
            name,
            *value,
            *value2,
            *value3,
            text.as_deref(),
        ),
        Cmd::Filters => Ok(effect::catalog()),
        Cmd::Stitch {
            inputs,
            output,
            algorithm,
            min_overlap,
            accept_diff,
        } => stitch(inputs, output, *algorithm, *min_overlap, *accept_diff),
        Cmd::Solid {
            output,
            width,
            height,
            color,
        } => solid(output, *width, *height, color),
        Cmd::Info { input } => info(input),
    }
}
fn convert(input: &Path, output: &Path) -> Result<Value> {
    let img = util::load_rgba(input)?;
    util::save_rgba(&img, output)?;
    Ok(util::image_json(output, &img))
}
fn resize(input: &Path, output: &Path, width: u32, height: u32, fit: Fit) -> Result<Value> {
    let img = util::load_rgba(input)?;
    let img = match fit {
        Fit::Stretch => video_utils::convert::resize_rgba_image(img, width, height),
        Fit::Contain => video_utils::convert::resize_rgba_image_contain(img, width, height, true),
    }
    .map_err(|e| anyhow!("缩放失败: {e}"))?;

    util::save_rgba(&img, output)?;
    Ok(util::image_json(output, &img))
}
fn crop(input: &Path, output: &Path, x: u32, y: u32, width: u32, height: u32) -> Result<Value> {
    let img = util::load_rgba(input)?;
    let (iw, ih) = img.dimensions();
    if x + width > iw || y + height > ih {
        return Err(anyhow!(
            "裁剪区域超出图片范围: 图片 {iw}x{ih}，请求 x={x} y={y} w={width} h={height}"
        ));
    }
    let img = imageops::crop_imm(&img, x, y, width, height).to_image();
    util::save_rgba(&img, output)?;
    Ok(util::image_json(output, &img))
}
fn rotate(input: &Path, output: &Path, degrees: u32) -> Result<Value> {
    let img = util::load_rgba(input)?;
    let img = match degrees {
        90 => imageops::rotate90(&img),
        180 => imageops::rotate180(&img),
        270 => imageops::rotate270(&img),
        other => return Err(anyhow!("不支持的旋转角度: {other}")),
    };
    util::save_rgba(&img, output)?;
    Ok(util::image_json(output, &img))
}
fn flip(input: &Path, output: &Path, axis: Axis) -> Result<Value> {
    let img = util::load_rgba(input)?;
    let img = match axis {
        Axis::Horizontal => imageops::flip_horizontal(&img),
        Axis::Vertical => imageops::flip_vertical(&img),
    };
    util::save_rgba(&img, output)?;
    Ok(util::image_json(output, &img))
}
fn thumbnail(input: &Path, output: &Path, size: u32) -> Result<Value> {
    let img = util::load_rgba(input)?;
    let (w, h) = img.dimensions();
    let ratio = (size as f64 / w.max(h) as f64).min(1.0);
    let img = imageops::resize(
        &img,
        ((w as f64 * ratio) as u32).max(1),
        ((h as f64 * ratio) as u32).max(1),
        imageops::FilterType::Lanczos3,
    );
    util::save_rgba(&img, output)?;
    Ok(util::image_json(output, &img))
}
fn solid(output: &Path, width: u32, height: u32, color: &str) -> Result<Value> {
    let [r, g, b, a] = parse_rgba(color)?;
    let img = image::RgbaImage::from_pixel(width, height, image::Rgba([r, g, b, a]));
    util::save_rgba(&img, output)?;
    Ok(util::image_json(output, &img))
}
pub fn parse_rgba(hex: &str) -> Result<[u8; 4]> {
    let hex = hex.trim_start_matches('#');
    let value = u32::from_str_radix(hex, 16).map_err(|_| anyhow!("颜色格式应为 rrggbb: {hex}"))?;
    Ok(match hex.len() {
        6 => [(value >> 16) as u8, (value >> 8) as u8, value as u8, 255],
        8 => [
            (value >> 24) as u8,
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        ],
        _ => return Err(anyhow!("颜色格式应为 rrggbb 或 rrggbbaa: {hex}")),
    })
}
fn info(input: &Path) -> Result<Value> {
    let img = util::load_rgba(input)?;
    Ok(util::image_json(input, &img))
}
fn stitch(
    inputs: &[PathBuf],
    output: &Path,
    algorithm: StitchAlgorithm,
    min_overlap: u32,
    accept_diff: f32,
) -> Result<Value> {
    let images = inputs
        .iter()
        .map(|p| util::load_rgba(p))
        .collect::<Result<Vec<_>>>()?;

    let config = screenshot::StitchConfig {
        algorithm: match algorithm {
            StitchAlgorithm::Template => screenshot::Algorithm::Template,
            StitchAlgorithm::ColSample => screenshot::Algorithm::ColSample,
        },
        min_overlap,
        accept_diff,
        ..Default::default()
    };

    let result = screenshot::stitch(images, config).map_err(|e| anyhow!("拼接失败: {e}"))?;
    util::save_rgba(&result.image, output)?;

    let outcomes: Vec<Value> = result
        .outcomes
        .iter()
        .map(|o| {
            let (kind, added) = match o {
                screenshot::StitchOutcome::FirstFrame => ("first-frame", 0),
                screenshot::StitchOutcome::Appended { added } => ("appended", *added),
                screenshot::StitchOutcome::NoProgress => ("no-progress", 0),
                screenshot::StitchOutcome::NoMatch => ("no-match", 0),
            };
            json!({ "outcome": kind, "added_rows": added })
        })
        .collect();

    let mut value = util::image_json(output, &result.image);
    value["frames"] = json!(outcomes);
    Ok(value)
}
