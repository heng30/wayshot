use crate::{GSVError, Result, create_session, text::utils::en_word_dict};
use ndarray::{Array, s};
use ort::{inputs, session::Session, value::Tensor};
use std::{path::Path, str::FromStr};
use tokenizers::Tokenizer;

const DECODER_START_TOKEN_ID: u32 = 2;
const EOS_TOKEN_ID: u32 = 2;
const EOS_TOKEN_ID_I64: i64 = EOS_TOKEN_ID as i64;
const MAX_DECODER_STEPS: usize = 50;

static MINI_BART_G2P_TOKENIZER: &str = include_str!("../../../assert/tokenizer.mini-bart-g2p.json");

pub struct G2pEn {
    model: G2PEnModel,
}

impl G2pEn {
    pub fn new<P: AsRef<Path>>(encoder_path: P, decoder_path: P) -> Result<Self> {
        let model = G2PEnModel::new(encoder_path, decoder_path)?;
        Ok(Self { model })
    }

    pub fn g2p(&mut self, text: &str) -> Result<Vec<String>> {
        if let Some(result) = en_word_dict(text) {
            return Ok(result.to_owned());
        }

        let words: Vec<_> = text.split_whitespace().collect();
        let mut phonemes = Vec::with_capacity(words.len() * 2);
        for word in words {
            let phones = self.model.get_phoneme(word)?;
            phonemes.extend(phones);
        }
        Ok(phonemes)
    }
}

pub struct G2PEnModel {
    encoder_model: Session,
    decoder_model: Session,
    tokenizer: Tokenizer,
}

impl G2PEnModel {
    pub fn new<P: AsRef<Path>>(encoder_path: P, decoder_path: P) -> Result<Self> {
        Ok(Self {
            encoder_model: create_session(encoder_path)?,
            decoder_model: create_session(decoder_path)?,
            tokenizer: Tokenizer::from_str(MINI_BART_G2P_TOKENIZER)?,
        })
    }

    pub fn get_phoneme(&mut self, text: &str) -> Result<Vec<String>> {
        log::debug!("processing {:?}", text);

        let mut decoder_input_ids = vec![DECODER_START_TOKEN_ID as i64];
        let encoding = self.tokenizer.encode(text, true)?;
        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let seq_len = input_ids.len();

        let input_ids_tensor = Tensor::from_array(Array::from_shape_vec((1, seq_len), input_ids)?)?;
        let attention_mask_tensor = Tensor::from_array(Array::from_elem((1, seq_len), 1i64))?;

        let encoder_outputs = self.encoder_model.run(inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor
        ])?;

        for _ in 0..MAX_DECODER_STEPS {
            let encoder_output = encoder_outputs["last_hidden_state"].view();

            let decoder_input_ids_tensor = Tensor::from_array(Array::from_shape_vec(
                (1, decoder_input_ids.len()),
                decoder_input_ids.clone(),
            )?)?;

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

        let decoder_input_ids_u32: Vec<u32> = decoder_input_ids.iter().map(|&x| x as u32).collect();

        Ok(self
            .tokenizer
            .decode(&decoder_input_ids_u32, true)?
            .split_whitespace()
            .map(String::from)
            .collect())
    }
}
