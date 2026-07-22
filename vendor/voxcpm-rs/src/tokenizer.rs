//! Tokenizer wrapper around [`tokenizers::Tokenizer`] implementing the
//! `mask_multichar_chinese_tokens` pre-processing behavior from the reference
//! Python implementation.
//!
//! Multi-character CJK tokens (len >= 2, all chars in U+4E00..=U+9FFF) are
//! split back into single characters before being encoded to ids.

use std::collections::HashSet;
use std::path::Path;

use tokenizers::tokenizer::Tokenizer;

use crate::{Error, Result};

#[derive(Debug)]
pub struct TextTokenizer {
    tokenizer: Tokenizer,
    multichar_tokens: HashSet<String>,
}

fn is_chinese_char(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c)
}

impl TextTokenizer {
    pub fn from_local(dir: impl AsRef<Path>) -> Result<Self> {
        let path = dir.as_ref().join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&path).map_err(|e| Error::Tokenizer(e.to_string()))?;
        let vocab = tokenizer.get_vocab(true);
        let multichar_tokens: HashSet<String> = vocab
            .keys()
            .filter(|tok| {
                let chars: Vec<char> = tok.chars().collect();
                chars.len() >= 2 && chars.iter().all(|c| is_chinese_char(*c))
            })
            .cloned()
            .collect();
        Ok(Self { tokenizer, multichar_tokens })
    }

    pub fn encode(&self, text: &str) -> Result<Vec<i64>> {
        let enc = self
            .tokenizer
            .encode(text, false)
            .map_err(|e| Error::Tokenizer(e.to_string()))?;
        let mut ids: Vec<i64> = Vec::with_capacity(enc.len());
        for tok in enc.get_tokens() {
            let clean = tok.replace('\u{2581}', "");
            if self.multichar_tokens.contains(&clean) {
                for c in clean.chars() {
                    let s = c.to_string();
                    let id = self
                        .tokenizer
                        .token_to_id(&s)
                        .ok_or_else(|| Error::Tokenizer(format!("unknown char token: {s}")))?;
                    ids.push(id as i64);
                }
            } else {
                let id = self
                    .tokenizer
                    .token_to_id(tok)
                    .ok_or_else(|| Error::Tokenizer(format!("unknown token: {tok}")))?;
                ids.push(id as i64);
            }
        }
        Ok(ids)
    }

    pub fn decode(&self, ids: &[i64]) -> Result<String> {
        let u: Vec<u32> = ids.iter().map(|i| *i as u32).collect();
        self.tokenizer
            .decode(&u, true)
            .map_err(|e| Error::Tokenizer(e.to_string()))
    }
}
