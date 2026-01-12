#[derive(Clone, Copy, Debug)]
pub enum Model {
    Modnet,
    Rmbg14,
    Rmbg14Fp16,
    Rmbg14Quantized,
    U2NET,
    U2NETP,
}

impl Model {
    pub fn all_models() -> Vec<Self> {
        vec![
            Self::Modnet,
            Self::Rmbg14,
            Self::Rmbg14Fp16,
            Self::Rmbg14Quantized,
            Self::U2NET,
            Self::U2NETP,
        ]
    }

    pub fn to_input_size(&self) -> (u32, u32) {
        match self {
            Model::Modnet => (512, 512),
            Model::Rmbg14 | Model::Rmbg14Fp16 | Model::Rmbg14Quantized => (1024, 1024),
            Model::U2NET | Model::U2NETP => (320, 320),
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Modnet => "modnet_photographic_portrait_matting.onnx",
            Self::Rmbg14 => "rmbg-1.4.onnx",
            Self::Rmbg14Fp16 => "rmbg-1.4_fp16.onnx",
            Self::Rmbg14Quantized => "rmbg-1.4_quantized.onnx",
            Self::U2NET => "u2net.onnx",
            Self::U2NETP => "u2netp.onnx",
        }
    }

    pub fn try_from(model: &str) -> Option<Self> {
        match model {
            "modnet_photographic_portrait_matting.onnx" => Some(Model::Modnet),
            "rmbg-1.4.onnx" => Some(Model::Rmbg14),
            "rmbg-1.4_fp16.onnx" => Some(Model::Rmbg14Fp16),
            "rmbg-1.4_quantized.onnx" => Some(Model::Rmbg14Quantized),
            "u2net.onnx" => Some(Model::U2NET),
            "u2netp.onnx" => Some(Model::U2NETP),
            _ => None,
        }
    }

    pub fn download_url(&self) -> &'static str {
        match self {
            Self::Modnet => {
                "https://huggingface.co/TheEeeeLin/HivisionIDPhotos_matting/resolve/034769305faf641ad94edfac654aba13be06e816/modnet_photographic_portrait_matting.onnx"
            }
            Self::Rmbg14 => "https://huggingface.co/briaai/RMBG-1.4/resolve/main/onnx/model.onnx",
            Self::Rmbg14Fp16 => {
                "https://huggingface.co/briaai/RMBG-1.4/resolve/main/onnx/model_fp16.onnx"
            }
            Self::Rmbg14Quantized => {
                "https://huggingface.co/briaai/RMBG-1.4/resolve/main/onnx/model_quantized.onnx"
            }
            Self::U2NET => "https://huggingface.co/AlenZeng/u2netonnxmodel/resolve/main/u2net.onnx",
            Self::U2NETP => {
                "https://huggingface.co/martintomov/comfy/resolve/1b0c3477e152d8a2dea8e4e418a6dba32de56fda/rembg/u2netp.onnx"
            }
        }
    }
}
