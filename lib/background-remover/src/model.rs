#[derive(Clone, Copy, Debug)]
pub enum Model {
    Modnet,
    Rmbg14,
}

impl Model {
    pub fn all_models() -> Vec<Self> {
        vec![Self::Modnet, Self::Rmbg14]
    }

    pub fn to_input_size(&self) -> (u32, u32) {
        match self {
            Model::Modnet => (512, 512),
            Model::Rmbg14 => (1024, 1024),
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Modnet => "modnet_photographic_portrait_matting.onnx",
            Self::Rmbg14 => "rmbg-1.4.onnx",
        }
    }

    pub fn try_from(model: &str) -> Option<Self> {
        match model {
            "modnet_photographic_portrait_matting.onnx" => Some(Model::Modnet),
            "rmbg-1.4.onnx" => Some(Model::Rmbg14),
            _ => None,
        }
    }

    pub fn download_url(&self) -> &'static str {
        match self {
            Self::Modnet => {
                "https://huggingface.co/TheEeeeLin/HivisionIDPhotos_matting/resolve/034769305faf641ad94edfac654aba13be06e816/modnet_photographic_portrait_matting.onnx"
            }
            Self::Rmbg14 => "https://huggingface.co/briaai/RMBG-1.4/resolve/main/onnx/model.onnx",
        }
    }
}
