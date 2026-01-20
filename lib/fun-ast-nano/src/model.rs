pub mod common;
pub mod fun_asr_nano;
pub mod qwen3;

pub use fun_asr_nano::generate::{
    FunAsrNanoGenerateModel, TranscriptionRequest, TranscriptionResponse,
};

use strum::VariantArray as _;
use strum_macros::VariantArray;

const FUN_ASR_NANO_FILENAME: &str = "model.pt";
const QWEN3_0_6B_TOKENIZER_FILENAME: &str = "qwen3_0.6B_tokenizer.json";

#[derive(VariantArray, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    FunAsrNano,
    Qwen306bTokenizer,
}

impl Model {
    pub fn all_models() -> Vec<Self> {
        Model::VARIANTS.to_vec()
    }

    pub fn to_filename(&self) -> &'static str {
        match self {
            Self::FunAsrNano => FUN_ASR_NANO_FILENAME,
            Self::Qwen306bTokenizer => QWEN3_0_6B_TOKENIZER_FILENAME,
        }
    }

    pub fn try_from_filename(model: &str) -> Option<Self> {
        match model {
            FUN_ASR_NANO_FILENAME => Some(Self::FunAsrNano),
            QWEN3_0_6B_TOKENIZER_FILENAME => Some(Self::Qwen306bTokenizer),
            _ => None,
        }
    }

    pub fn download_url(&self) -> &'static str {
        match self {
            Self::FunAsrNano => {
                "https://huggingface.co/FunAudioLLM/Fun-ASR-Nano-2512/resolve/main/model.pt"
            }
            Self::Qwen306bTokenizer => {
                "https://huggingface.co/FunAudioLLM/Fun-ASR-Nano-2512/resolve/main/Qwen3-0.6B/tokenizer.json"
            }
        }
    }
}
