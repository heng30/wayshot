use tokenizers::Tokenizer;

/// Wrapper around the HuggingFace tokenizer with chat template support.
pub struct LfmTokenizer {
    tokenizer: Tokenizer,
    bos_token: String,
    eos_token_id: u32,
    image_token_id: u32,
}

impl LfmTokenizer {
    /// Load the tokenizer from a `tokenizer.json` file.
    pub fn from_file(path: &std::path::Path) -> crate::Result<Self> {
        let tokenizer =
            Tokenizer::from_file(path).map_err(|e| crate::Error::Tokenizer(e.to_string()))?;
        Ok(Self::from_tokenizer(tokenizer))
    }

    /// Create from an already-loaded tokenizer.
    pub fn from_tokenizer(tokenizer: Tokenizer) -> Self {
        let vocab = tokenizer.get_vocab(true);
        let eos_token_id = vocab.get("<|im_end|>").copied().unwrap_or(7);
        let image_token_id = vocab.get("<image>").copied().unwrap_or(396);

        Self {
            tokenizer,
            bos_token: "<|startoftext|>".to_string(),
            eos_token_id,
            image_token_id,
        }
    }

    /// Format a user prompt with the chat template and expanded image tokens.
    pub fn format_chat_prompt(&self, user_text: &str, num_image_tokens: usize) -> String {
        let image_tokens = "<image>".repeat(num_image_tokens);
        format!(
            "{}<|im_start|>user\n<|image_start|>{}<|image_end|>{}<|im_end|>\n<|im_start|>assistant\n",
            self.bos_token, image_tokens, user_text
        )
    }

    /// Encode a text string into token IDs.
    pub fn encode(&self, text: &str) -> crate::Result<Vec<u32>> {
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|e| crate::Error::Tokenizer(e.to_string()))?;
        Ok(encoding.get_ids().to_vec())
    }

    /// Decode a sequence of token IDs into text.
    pub fn decode(&self, ids: &[u32], skip_special_tokens: bool) -> crate::Result<String> {
        let text = self
            .tokenizer
            .decode(ids, skip_special_tokens)
            .map_err(|e| crate::Error::Tokenizer(e.to_string()))?;
        Ok(text)
    }

    pub fn eos_token_id(&self) -> u32 {
        self.eos_token_id
    }

    pub fn image_token_id(&self) -> u32 {
        self.image_token_id
    }

    /// Find positions of image tokens in the input IDs.
    pub fn find_image_positions(&self, input_ids: &[u32]) -> Vec<usize> {
        input_ids
            .iter()
            .enumerate()
            .filter(|&(_, id)| *id == self.image_token_id)
            .map(|(i, _)| i)
            .collect()
    }
}
