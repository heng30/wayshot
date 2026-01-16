use {
    crate::{GSVError, create_session, text::dict},
    arpabet::Arpabet,
    log::debug,
    ndarray::{Array, s},
    ort::{inputs, session::Session, value::Tensor},
    std::{path::Path, str::FromStr},
    tokenizers::Tokenizer,
};

static MINI_BART_G2P_TOKENIZER: &str = include_str!("tokenizer.mini-bart-g2p.json");

const DECODER_START_TOKEN_ID: u32 = 2;
const EOS_TOKEN_ID: u32 = 2;
const EOS_TOKEN_ID_I64: i64 = EOS_TOKEN_ID as i64;
const MAX_DECODER_STEPS: usize = 50;

#[allow(unused)]
const BOS_TOKEN: &str = "<s>";
#[allow(unused)]
const EOS_TOKEN: &str = "</s>";
#[allow(unused)]
const BOS_TOKEN_ID: u32 = 0;

pub struct G2PEnModel {
    encoder_model: Session,
    decoder_model: Session,
    tokenizer: Tokenizer,
}

impl G2PEnModel {
    pub fn new<P: AsRef<Path>>(encoder_path: P, decoder_path: P) -> Result<Self, GSVError> {
        let encoder_model = create_session(encoder_path)?;
        let decoder_model = create_session(decoder_path)?;
        let tokenizer = Tokenizer::from_str(MINI_BART_G2P_TOKENIZER)?;

        Ok(Self {
            encoder_model,
            decoder_model,
            tokenizer,
        })
    }

    pub fn get_phoneme(&mut self, text: &str) -> Result<Vec<String>, GSVError> {
        debug!("processing {:?}", text);

        let encoding = self.tokenizer.encode(text, true)?;
        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let seq_len = input_ids.len();
        let mut decoder_input_ids = vec![DECODER_START_TOKEN_ID as i64];

        // Helper to create tensor with error handling
        let create_tensor = |array: Array<i64, ndarray::Dim<[usize; 2]>>| -> Result<Tensor<i64>, GSVError> {
            Tensor::from_array(array)
                .map_err(|e| GSVError::InternalError(format!("Failed to create tensor: {}", e)))
        };

        let input_ids_tensor = create_tensor(Array::from_shape_vec((1, seq_len), input_ids.clone())?)?;
        let attention_mask_tensor = create_tensor(Array::from_elem((1, seq_len), 1i64))?;

        let encoder_outputs = self.encoder_model.run(inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor
        ])?;

        // Decode loop
        for _ in 0..MAX_DECODER_STEPS {
            let encoder_output = encoder_outputs["last_hidden_state"].view();

            let decoder_input_ids_tensor = create_tensor(
                Array::from_shape_vec((1, decoder_input_ids.len()), decoder_input_ids.clone())?
            )?;

            let outputs = self.decoder_model.run(inputs![
                "input_ids" => decoder_input_ids_tensor,
                "encoder_attention_mask" => Tensor::from_array(Array::from_elem((1, seq_len), 1i64))?,
                "encoder_hidden_states" => encoder_output,
            ])?;

            let output_array = outputs["logits"].try_extract_array::<f32>()?;
            let last_token_logits = &output_array.slice(s![0, output_array.shape()[1] - 1, ..]);

            let next_token_id = last_token_logits
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i as i64)
                .ok_or(GSVError::DecodeTokenFailed)?;

            decoder_input_ids.push(next_token_id);

            if next_token_id == EOS_TOKEN_ID_I64 {
                break;
            }
        }

        let decoder_input_ids_u32: Vec<u32> = decoder_input_ids
            .iter()
            .map(|&x| x as u32)
            .collect();

        Ok(self
            .tokenizer
            .decode(&decoder_input_ids_u32, true)?
            .split_whitespace()
            .map(String::from)
            .collect())
    }
}

pub struct G2pEn {
    model: Option<G2PEnModel>,
    arpabet: Arpabet,
}

impl G2pEn {
    pub fn new<P: AsRef<Path>>(path: Option<P>) -> Result<Self, GSVError> {
        let arpabet = arpabet::load_cmudict().clone();
        let model = path
            .and_then(|p| {
                let p = p.as_ref();
                G2PEnModel::new(p.join("encoder_model.onnx"), p.join("decoder_model.onnx")).ok()
            });

        Ok(Self { model, arpabet })
    }

    pub fn g2p(&mut self, text: &str) -> Result<Vec<String>, GSVError> {
        if let Some(result) = dict::en_word_dict(text) {
            return Ok(result.to_owned());
        }

        match &mut self.model {
            Some(model) => {
                let words: Vec<_> = text.split_whitespace().collect();
                let mut phonemes = Vec::with_capacity(words.len() * 2);
                for word in words {
                    let phones = model.get_phoneme(word)?;
                    phonemes.extend(phones);
                }
                Ok(phonemes)
            }
            None => self.fallback_to_arpabet(text),
        }
    }

    #[inline]
    fn fallback_to_arpabet(&self, text: &str) -> Result<Vec<String>, GSVError> {
        // First, try to split by hyphens to handle compound words like "cross-platform"
        let hyphen_parts: Vec<&str> = text.split('-').collect();

        if hyphen_parts.len() > 1 {
            // Handle compound word: process each part separately
            let mut result = Vec::new();
            for (i, part) in hyphen_parts.iter().enumerate() {
                if !part.is_empty() {
                    // Process each part
                    if let Some(phones) = self.arpabet.get_polyphone_str(part) {
                        result.extend(phones.iter().map(|&s| s.to_string()));
                    } else {
                        // Fallback to character-by-character for this part
                        for c in part.chars() {
                            let c_str = c.to_string();
                            if let Some(phones) = self.arpabet.get_polyphone_str(&c_str) {
                                result.extend(phones.iter().map(|&s| s.to_string()));
                            } else {
                                result.push(c_str);
                            }
                        }
                    }
                }
                // Add hyphen between parts (except after the last part)
                if i < hyphen_parts.len() - 1 && !hyphen_parts[i + 1].is_empty() {
                    result.push("-".to_string());
                }
            }
            Ok(result)
        } else {
            // Original logic for non-hyphenated words
            let words: Vec<_> = text.split_whitespace().collect();
            let mut result = Vec::with_capacity(words.len() * 2);

            for word in words {
                if let Some(phones) = self.arpabet.get_polyphone_str(word) {
                    result.extend(phones.iter().map(|&s| s.to_string()));
                } else {
                    for c in word.chars() {
                        let c_str = c.to_string();
                        if let Some(phones) = self.arpabet.get_polyphone_str(&c_str) {
                            result.extend(phones.iter().map(|&s| s.to_string()));
                        } else {
                            result.push(c_str);
                        }
                    }
                }
            }
            Ok(result)
        }
    }
}
