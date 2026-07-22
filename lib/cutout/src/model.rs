const ISNET_ANIME_FILENAME: &str = "isnet-anime.onnx";
const ISNET_GENERAL_USE_FILENAME: &str = "isnet-general-use.onnx";
const SILUETA_FILENAME: &str = "silueta.onnx";
const U2NET_FILENAME: &str = "u2net.onnx";
const U2NET_CLOTH_SEG_FILENAME: &str = "u2net_cloth_seg.onnx";
const U2NET_HUMAN_SEG_FILENAME: &str = "u2net_human_seg.onnx";
const U2NETP_FILENAME: &str = "u2netp.onnx";

const ISNET_ANIME_URL: &str =
    "https://huggingface.co/tomjackson2023/rembg/resolve/main/isnet-anime.onnx";
const ISNET_GENERAL_USE_URL: &str =
    "https://huggingface.co/tomjackson2023/rembg/resolve/main/isnet-general-use.onnx";
const SILUETA_URL: &str = "https://huggingface.co/tomjackson2023/rembg/resolve/main/silueta.onnx";
const U2NET_URL: &str = "https://huggingface.co/tomjackson2023/rembg/resolve/main/u2net.onnx";
const U2NET_CLOTH_SEG_URL: &str =
    "https://huggingface.co/tomjackson2023/rembg/resolve/main/u2net_cloth_seg.onnx";
const U2NET_HUMAN_SEG_URL: &str =
    "https://huggingface.co/tomjackson2023/rembg/resolve/main/u2net_human_seg.onnx";
const U2NETP_URL: &str = "https://huggingface.co/tomjackson2023/rembg/resolve/main/u2netp.onnx";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    U2Net,
    U2NetP,
    U2NetClothSeg,
    U2NetHumanSeg,
    IsnetAnime,
    IsnetGeneralUse,
    Silueta,
}

impl Model {
    pub fn all_models() -> Vec<Self> {
        vec![
            Self::U2Net,
            Self::U2NetP,
            Self::U2NetClothSeg,
            Self::U2NetHumanSeg,
            Self::IsnetAnime,
            Self::IsnetGeneralUse,
            Self::Silueta,
        ]
    }

    pub fn to_input_size(&self) -> (u32, u32) {
        match self {
            Model::IsnetAnime => (1024, 1024),
            Model::IsnetGeneralUse => (1024, 1024),
            Model::Silueta => (320, 320),
            Model::U2Net => (320, 320),
            Model::U2NetClothSeg => (768, 768),
            Model::U2NetHumanSeg => (320, 320),
            Model::U2NetP => (320, 320),
        }
    }

    pub fn to_filename(&self) -> &'static str {
        match self {
            Self::IsnetAnime => ISNET_ANIME_FILENAME,
            Self::IsnetGeneralUse => ISNET_GENERAL_USE_FILENAME,
            Self::Silueta => SILUETA_FILENAME,
            Self::U2Net => U2NET_FILENAME,
            Self::U2NetClothSeg => U2NET_CLOTH_SEG_FILENAME,
            Self::U2NetHumanSeg => U2NET_HUMAN_SEG_FILENAME,
            Self::U2NetP => U2NETP_FILENAME,
        }
    }

    pub fn try_from_filename(model: &str) -> Option<Self> {
        match model {
            ISNET_ANIME_FILENAME => Some(Model::IsnetAnime),
            ISNET_GENERAL_USE_FILENAME => Some(Model::IsnetGeneralUse),
            SILUETA_FILENAME => Some(Model::Silueta),
            U2NET_FILENAME => Some(Model::U2Net),
            U2NET_CLOTH_SEG_FILENAME => Some(Model::U2NetClothSeg),
            U2NET_HUMAN_SEG_FILENAME => Some(Model::U2NetHumanSeg),
            U2NETP_FILENAME => Some(Model::U2NetP),
            _ => None,
        }
    }

    pub fn try_from_url(url: &str) -> Option<Self> {
        match url {
            ISNET_ANIME_URL => Some(Model::IsnetAnime),
            ISNET_GENERAL_USE_URL => Some(Model::IsnetGeneralUse),
            SILUETA_URL => Some(Model::Silueta),
            U2NET_URL => Some(Model::U2Net),
            U2NET_CLOTH_SEG_URL => Some(Model::U2NetClothSeg),
            U2NET_HUMAN_SEG_URL => Some(Model::U2NetHumanSeg),
            U2NETP_URL => Some(Model::U2NetP),
            _ => None,
        }
    }

    pub fn download_url(&self) -> &'static str {
        match self {
            Self::IsnetAnime => ISNET_ANIME_URL,
            Self::IsnetGeneralUse => ISNET_GENERAL_USE_URL,
            Self::Silueta => SILUETA_URL,
            Self::U2Net => U2NET_URL,
            Self::U2NetClothSeg => U2NET_CLOTH_SEG_URL,
            Self::U2NetHumanSeg => U2NET_HUMAN_SEG_URL,
            Self::U2NetP => U2NETP_URL,
        }
    }

    pub fn mean(&self) -> (f32, f32, f32) {
        match self {
            Model::IsnetAnime => (0.485, 0.456, 0.406),
            Model::IsnetGeneralUse => (0.5, 0.5, 0.5),
            _ => (0.485, 0.456, 0.406),
        }
    }

    pub fn std(&self) -> (f32, f32, f32) {
        match self {
            Model::IsnetAnime | Model::IsnetGeneralUse => (1.0, 1.0, 1.0),
            _ => (0.229, 0.224, 0.225),
        }
    }
}
