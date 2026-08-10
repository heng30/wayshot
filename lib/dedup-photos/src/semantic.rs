//! CLIP 语义嵌入。
//!
//! 模型文件由调用方自行准备（本库不再自动下载），下载地址见
//! [`CLIP_MODEL_URL`]。

use ort::{session::Session, value::Tensor};
use std::{path::Path, sync::Mutex};

pub const CLIP_MODEL_FILE: &str = "vision_model_quantized.onnx";
pub const CLIP_MODEL_URL: &str = "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model_quantized.onnx";

pub struct Embedder {
    session: Mutex<Session>,
}

impl Embedder {
    pub fn new(model_path: &Path) -> Result<Self, String> {
        if !model_path.exists() {
            return Err(format!(
                "CLIP model not found at {} — download it manually from {}",
                model_path.display(),
                CLIP_MODEL_URL
            ));
        }
        let session = Session::builder()
            .map_err(|e| e.to_string())?
            .commit_from_file(model_path)
            .map_err(|e| e.to_string())?;
        Ok(Embedder {
            session: Mutex::new(session),
        })
    }

    fn preprocess(bytes: &[u8]) -> Option<Vec<f32>> {
        const SIZE: u32 = 224;
        const MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
        const STD: [f32; 3] = [0.268_63, 0.261_3, 0.275_78];

        let img = image::load_from_memory(bytes).ok()?;
        let (w, h) = (img.width(), img.height());
        if w == 0 || h == 0 {
            return None;
        }
        let scale = SIZE as f32 / w.min(h) as f32;
        let nw = ((w as f32 * scale).round() as u32).max(SIZE);
        let nh = ((h as f32 * scale).round() as u32).max(SIZE);
        let resized = img
            .resize_exact(nw, nh, image::imageops::FilterType::CatmullRom)
            .to_rgb8();
        let x0 = (nw - SIZE) / 2;
        let y0 = (nh - SIZE) / 2;

        let n = (SIZE * SIZE) as usize;
        let mut out = vec![0f32; 3 * n];
        for y in 0..SIZE {
            for x in 0..SIZE {
                let p = resized.get_pixel(x0 + x, y0 + y);
                let i = (y * SIZE + x) as usize;
                for c in 0..3 {
                    out[c * n + i] = (p[c] as f32 / 255.0 - MEAN[c]) / STD[c];
                }
            }
        }
        Some(out)
    }

    pub fn embed(&self, bytes: &[u8]) -> Option<Vec<f32>> {
        let input = Self::preprocess(bytes)?;
        let tensor = Tensor::from_array(([1usize, 3, 224, 224], input)).ok()?;
        let mut session = self.session.lock().ok()?;
        let outputs = session.run(ort::inputs!["pixel_values" => tensor]).ok()?;
        let mut v: Vec<f32> = if let Some(value) = outputs.get("image_embeds") {
            value.try_extract_tensor::<f32>().ok()?.1.to_vec()
        } else {
            let (_, value) = outputs.iter().next()?;
            value.try_extract_tensor::<f32>().ok()?.1.to_vec()
        };
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm == 0.0 {
            return None;
        }
        for x in v.iter_mut() {
            *x /= norm;
        }
        Some(v)
    }
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    dot
}
