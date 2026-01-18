use strum::VariantArray as _;
use strum_macros::VariantArray;

const CUSTOM_VITS_FILENAME: &str = "custom_vits.onnx";
const SSL_FILENAME: &str = "ssl.onnx";
const CUSTOM_T2S_ENCODER_FILENAME: &str = "custom_t2s_encoder.onnx";
const CUSTOM_T2S_FS_DECODER_FILENAME: &str = "custom_t2s_fs_decoder.onnx";
const CUSTOM_T2S_S_DECODER_FILENAME: &str = "custom_t2s_s_decoder.onnx";
const BERT_FILENAME: &str = "bert.onnx";
const G2PW_FILENAME: &str = "g2pW.onnx";
const G2P_EN_ENCODER_FILENAME: &str = "g2p_en/encoder_model.onnx";
const G2P_EN_DECODER_FILENAME: &str = "g2p_en/decoder_model.onnx";

#[derive(VariantArray, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    CustomVits,
    Ssl,
    CustomT2sEncoder,
    CustomT2sFsDecoder,
    CustomT2sSDecoder,
    Bert,
    G2pW,
    G2pEnEncoderModel,
    G2pEnDecoderModel,
}

impl Model {
    pub fn all_models() -> Vec<Self> {
        Model::VARIANTS.to_vec()
    }

    pub fn to_filename(&self) -> &'static str {
        match self {
            Self::CustomVits => CUSTOM_VITS_FILENAME,
            Self::Ssl => SSL_FILENAME,
            Self::CustomT2sEncoder => CUSTOM_T2S_ENCODER_FILENAME,
            Self::CustomT2sFsDecoder => CUSTOM_T2S_FS_DECODER_FILENAME,
            Self::CustomT2sSDecoder => CUSTOM_T2S_S_DECODER_FILENAME,
            Self::Bert => BERT_FILENAME,
            Self::G2pW => G2PW_FILENAME,
            Self::G2pEnEncoderModel => G2P_EN_ENCODER_FILENAME,
            Self::G2pEnDecoderModel => G2P_EN_DECODER_FILENAME,
        }
    }

    pub fn try_from_filename(model: &str) -> Option<Self> {
        match model {
            CUSTOM_VITS_FILENAME => Some(Self::CustomVits),
            SSL_FILENAME => Some(Self::Ssl),
            CUSTOM_T2S_ENCODER_FILENAME => Some(Self::CustomT2sEncoder),
            CUSTOM_T2S_FS_DECODER_FILENAME => Some(Self::CustomT2sFsDecoder),
            CUSTOM_T2S_S_DECODER_FILENAME => Some(Self::CustomT2sSDecoder),
            BERT_FILENAME => Some(Self::Bert),
            G2PW_FILENAME => Some(Self::G2pW),
            G2P_EN_ENCODER_FILENAME => Some(Self::G2pEnEncoderModel),
            G2P_EN_DECODER_FILENAME => Some(Self::G2pEnDecoderModel),
            _ => None,
        }
    }

    pub fn download_url(&self) -> &'static str {
        match self {
            Self::CustomVits => {
                "https://huggingface.co/mikv39/gpt-sovits-onnx-custom/resolve/main/custom_vits.onnx"
            }
            Self::Ssl => {
                "https://huggingface.co/mikv39/gpt-sovits-onnx-custom/resolve/main/ssl.onnx"
            }
            Self::CustomT2sEncoder => {
                "https://huggingface.co/mikv39/gpt-sovits-onnx-custom/resolve/main/custom_t2s_encoder.onnx"
            }
            Self::CustomT2sFsDecoder => {
                "https://huggingface.co/mikv39/gpt-sovits-onnx-custom/resolve/main/custom_t2s_fs_decoder.onnx"
            }
            Self::CustomT2sSDecoder => {
                "https://huggingface.co/mikv39/gpt-sovits-onnx-custom/resolve/main/custom_t2s_s_decoder.onnx"
            }
            Self::Bert => {
                "https://huggingface.co/mikv39/gpt-sovits-onnx-custom/resolve/main/bert.onnx"
            }
            Self::G2pW => {
                "https://huggingface.co/mikv39/gpt-sovits-onnx-custom/resolve/main/g2pW.onnx"
            }
            Self::G2pEnEncoderModel => {
                "https://huggingface.co/cisco-ai/mini-bart-g2p/resolve/main/onnx/encoder_model.onnx"
            }
            Self::G2pEnDecoderModel => {
                "https://huggingface.co/cisco-ai/mini-bart-g2p/resolve/main/onnx/decoder_model.onnx"
            }
        }
    }
}
