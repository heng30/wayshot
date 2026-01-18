const RMBG14_FILENAME: &str = "rmbg-1.4.onnx";
const MODNET_FILENAME: &str = "modnet_photographic_portrait_matting.onnx";

const RMBG14_URL: &str = "https://huggingface.co/briaai/RMBG-1.4/resolve/main/onnx/model.onnx";
const MODNET_URL: &str = "https://huggingface.co/TheEeeeLin/HivisionIDPhotos_matting/resolve/034769305faf641ad94edfac654aba13be06e816/modnet_photographic_portrait_matting.onnx";

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

    pub fn to_filename(&self) -> &'static str {
        match self {
            Self::Modnet => MODNET_FILENAME,
            Self::Rmbg14 => RMBG14_FILENAME,
        }
    }

    pub fn try_from_filename(model: &str) -> Option<Self> {
        match model {
            MODNET_FILENAME => Some(Model::Modnet),
            RMBG14_FILENAME => Some(Model::Rmbg14),
            _ => None,
        }
    }

    pub fn try_from_url(url: &str) -> Option<Self> {
        match url {
            MODNET_URL => Some(Model::Modnet),
            RMBG14_URL => Some(Model::Rmbg14),
            _ => None,
        }
    }

    pub fn download_url(&self) -> &'static str {
        match self {
            Self::Modnet => MODNET_URL,
            Self::Rmbg14 => RMBG14_URL,
        }
    }
}
