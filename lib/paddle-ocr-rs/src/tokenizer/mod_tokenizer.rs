use candle_core::{Device, Tensor};
use tokenizers::Tokenizer;

use crate::Error;

pub struct TokenizerModel {
    pub tokenizer: Tokenizer,
    pub eos_token_id: u32,
    pub image_token_id: u32,
    pub bos_token: u32,
}

impl TokenizerModel {
    pub fn init(model_path: &str) -> Result<Self, Error> {
        let tokenizer_path = std::path::Path::new(model_path).join("tokenizer.json");
        if !tokenizer_path.exists() {
            return Err(Error::TokenizerFileNotFound(
                tokenizer_path.to_string_lossy().to_string(),
            ));
        }
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| Error::Tokenizer(e.to_string()))?;

        let eos_token_id = tokenizer.token_to_id("</s>").unwrap_or(2);
        let image_token_id = tokenizer.token_to_id("<|IMAGE_PLACEHOLDER|>").unwrap_or(100296);
        let bos_token = tokenizer.token_to_id("<|begin_of_sentence|>").unwrap_or(100295);

        Ok(Self {
            tokenizer,
            eos_token_id,
            image_token_id,
            bos_token,
        })
    }

    pub fn text_encode(&self, text: String, device: &Device) -> Result<Tensor, Error> {
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|e| Error::Tokenizer(e.to_string()))?;
        let ids = encoding.get_ids();
        let ids_u32: Vec<u32> = ids.iter().map(|&x| x as u32).collect();
        let input_ids = Tensor::new(ids_u32.as_slice(), device)?.unsqueeze(0)?;
        Ok(input_ids)
    }

    pub fn token_decode(&self, tokens: Vec<u32>) -> Result<String, Error> {
        let text = self
            .tokenizer
            .decode(&tokens, true)
            .map_err(|e| Error::Tokenizer(e.to_string()))?;
        Ok(text)
    }

    /// Decode tokens with special tokens included (for spotting task)
    pub fn token_decode_with_special(&self, tokens: Vec<u32>) -> Result<String, Error> {
        let text = self
            .tokenizer
            .decode(&tokens, false)
            .map_err(|e| Error::Tokenizer(e.to_string()))?;
        Ok(text)
    }

    pub fn get_pad_id(&self) -> u32 {
        self.tokenizer.token_to_id("<|pad|>").unwrap_or(0)
    }
}