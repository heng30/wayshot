use anyhow::{Result, anyhow};
use image::Rgb;
use qrcode::QrCode;
use serde_json::Value;
use std::path::PathBuf;

use crate::util;

#[derive(clap::Args)]
pub struct Args {
    /// 二维码内容（网址、文本）
    pub text: String,

    #[arg(short, long)]
    pub output: PathBuf,

    /// 是否保留静默区（四周留白）
    #[arg(long, default_value_t = true)]
    pub quiet_zone: bool,

    /// 每个模块放大倍数
    #[arg(long, default_value_t = 8)]
    pub scale: u32,
}

pub fn run(args: &Args) -> Result<Value> {
    let code = QrCode::new(args.text.as_bytes()).map_err(|e| anyhow!("生成二维码失败: {e}"))?;

    let mut render = code.render::<Rgb<u8>>();
    render
        .quiet_zone(args.quiet_zone)
        .module_dimensions(args.scale.max(1), args.scale.max(1));
    let img = render.build();

    let rgba = image::RgbaImage::from_raw(
        img.width(),
        img.height(),
        img.as_raw()
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
    )
    .ok_or_else(|| anyhow!("二维码图片数据异常"))?;

    util::save_rgba(&rgba, &args.output)?;
    Ok(util::image_json(&args.output, &rgba))
}
