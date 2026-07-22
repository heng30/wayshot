//! Tokenizer wrapper using the `tokenizers` crate.

use crate::error::{LfmError, Result};
use candle_core::{DType, Device, Tensor};
use tokenizers::Tokenizer;

pub struct TokenizerModel {
    pub tokenizer: Tokenizer,
}

impl TokenizerModel {
    /// Load tokenizer from a model directory.
    pub fn init(path: &str) -> Result<Self> {
        let tokenizer_path = path.to_string() + "/tokenizer.json";
        if !std::path::Path::new(&tokenizer_path).exists() {
            return Err(LfmError::FileNotFound(tokenizer_path.into()));
        }
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| LfmError::Tokenizer(format!("load error: {e}")))?;
        Ok(Self { tokenizer })
    }

    /// Encode text into a `(1, seq_len)` tensor of token IDs.
    ///
    /// `add_special_tokens` controls whether BOS/EOS tokens configured in the
    /// tokenizer are automatically prepended/appended. For LFM2.5-VL the chat
    /// template already includes the BOS token (`<|startoftext|>`), so this
    /// should typically be `false` to avoid a doubled BOS.
    pub fn encode(&self, text: &str, device: &Device, add_special_tokens: bool) -> Result<Tensor> {
        let encoding = self
            .tokenizer
            .encode(text, add_special_tokens)
            .map_err(|e| LfmError::Tokenizer(format!("encode error: {e}")))?;
        let ids = encoding.get_ids();
        let ids: Vec<u32> = ids.to_vec();
        let tensor = Tensor::new(ids.as_slice(), device)?
            .reshape((1, ids.len()))?
            .to_dtype(DType::U32)?;
        Ok(tensor)
    }

    /// Decode a list of token IDs back to text.
    pub fn decode(&self, ids: Vec<u32>) -> Result<String> {
        let text = self
            .tokenizer
            .decode(&ids, true)
            .map_err(|e| LfmError::Tokenizer(format!("decode error: {e}")))?;
        Ok(text)
    }
}
