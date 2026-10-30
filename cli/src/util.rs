use anyhow::{Context, Result};
use image::RgbaImage;
use serde_json::{Value, json};
use std::path::Path;

/// 成功结果统一带 `ok: true`，方便 LLM 判定。
pub fn ok(mut value: Value) -> Value {
    if let Value::Object(ref mut map) = value {
        map.insert("ok".to_string(), Value::Bool(true));
    }
    value
}

pub fn load_rgba(path: &Path) -> Result<RgbaImage> {
    image::open(path)
        .with_context(|| format!("无法打开图片: {}", path.display()))
        .map(|img| img.to_rgba8())
}

pub fn save_rgba(img: &RgbaImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建目录: {}", parent.display()))?;
    }
    let jpeg = matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .as_deref(),
        Some("jpg" | "jpeg")
    );
    if jpeg {
        // jpeg 不支持 alpha 通道
        image::DynamicImage::ImageRgb8(video_utils::convert::rgba_to_rgb(img))
            .save(path)
            .with_context(|| format!("无法写入图片: {}", path.display()))?;
    } else {
        img.save(path)
            .with_context(|| format!("无法写入图片: {}", path.display()))?;
    }
    Ok(())
}

pub fn image_json(path: &Path, img: &RgbaImage) -> Value {
    json!({
        "path": path.display().to_string(),
        "width": img.width(),
        "height": img.height(),
    })
}

pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32, channels: u16) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("无法写入 WAV: {}", path.display()))?;
    for sample in samples {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    Ok(())
}
