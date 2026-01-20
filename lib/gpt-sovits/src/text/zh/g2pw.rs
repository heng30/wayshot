use crate::{
    GSVError, Result, create_session,
    text::{BERT_TOKENIZER, DICT_MONO_CHARS, DICT_POLY_CHARS, argmax_2d},
};
use ndarray::Array;
use ort::value::Tensor;
use std::{
    fmt::Debug,
    path::Path,
    str::FromStr,
    sync::{Arc, LazyLock},
};
use tokenizers::Tokenizer;

const POLYPHONIC_RATIO: usize = 10;
pub static LABELS: &str = include_str!("../../../asset/dict_poly_index_list.json");
pub static POLY_LABLES: LazyLock<Vec<String>> =
    LazyLock::new(|| serde_json::from_str(LABELS).expect("Failed to parse POLY_LABELS JSON"));

#[derive(Clone, Debug)]
pub enum G2PWOut {
    Pinyin(String),
    Yue(String),
    RawChar(char),
}

#[derive(Debug)]
pub struct G2PW {
    model: ort::session::Session,
    tokenizers: Arc<tokenizers::Tokenizer>,
}

impl G2PW {
    pub fn new<P: AsRef<Path>>(g2pw_path: P) -> Result<Self> {
        let tokenizer = Tokenizer::from_str(BERT_TOKENIZER).map_err(|e| {
            GSVError::InternalError(format!("Failed to create G2PW tokenizer: {}", e))
        })?;

        Ok(Self {
            model: create_session(g2pw_path)?,
            tokenizers: Arc::new(tokenizer),
        })
    }

    pub fn g2p(&mut self, text: &str) -> Vec<G2PWOut> {
        self.get_pinyin_ml(text)
            .unwrap_or_else(|_| Vec::from_iter(text.chars().map(Self::process_char)))
    }

    #[inline]
    fn process_char(c: char) -> G2PWOut {
        if let Some(mono) = DICT_MONO_CHARS.get(&c) {
            G2PWOut::Pinyin(mono.phone.clone())
        } else if let Some(poly) = DICT_POLY_CHARS.get(&c) {
            G2PWOut::Pinyin(poly.phones[0].0.clone())
        } else {
            G2PWOut::RawChar(c)
        }
    }

    fn get_pinyin_ml(&mut self, text: &str) -> Result<Vec<G2PWOut>> {
        let encoding = self.tokenizers.encode(text, true).map_err(|e| {
            GSVError::InternalError(format!("Failed to encode text for G2PW: {}", e))
        })?;
        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let token_type_ids = vec![0i64; input_ids.len()];
        let attention_mask = vec![1i64; input_ids.len()];

        let char_count = text.chars().count();
        let mut poly_chars = Vec::with_capacity(char_count / POLYPHONIC_RATIO);
        let mut results: Vec<G2PWOut> = Vec::with_capacity(char_count);

        for (i, c) in text.chars().enumerate() {
            let result = Self::process_char(c);

            if let Some(poly) = DICT_POLY_CHARS.get(&c) {
                let mut phoneme_mask = vec![0f32; POLY_LABLES.len()];
                for (_, idx) in &poly.phones {
                    phoneme_mask[*idx] = 1.0;
                }
                poly_chars.push((i + 1, poly.index, phoneme_mask));
                results.push(G2PWOut::Pinyin(String::new()));
            } else {
                results.push(result);
            }
        }

        for (position_id, char_id, phoneme_mask) in poly_chars {
            let phoneme_mask_tensor = Tensor::from_array(Array::from_shape_vec(
                (1, phoneme_mask.len()),
                phoneme_mask,
            )?)?;
            let position_id_tensor =
                Tensor::from_array(Array::from_shape_vec((1, 1), vec![position_id as i64])?)?;
            let char_id_tensor =
                Tensor::from_array(Array::from_shape_vec((1, 1), vec![char_id as i64])?)?;

            let input_ids_tensor = Tensor::from_array(Array::from_shape_vec(
                (1, input_ids.len()),
                input_ids.clone(),
            )?)?;
            let token_type_ids_tensor = Tensor::from_array(Array::from_shape_vec(
                (1, token_type_ids.len()),
                token_type_ids.clone(),
            )?)?;
            let attention_mask_tensor = Tensor::from_array(Array::from_shape_vec(
                (1, attention_mask.len()),
                attention_mask.clone(),
            )?)?;

            let model_output = self.model.run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "token_type_ids" => token_type_ids_tensor,
                "attention_mask" => attention_mask_tensor,
                "phoneme_mask" => phoneme_mask_tensor,
                "char_ids" => char_id_tensor,
                "position_ids" => position_id_tensor,
            ])?;

            let probs = model_output["probs"]
                .try_extract_array::<f32>()
                .map_err(|e| {
                    GSVError::InternalError(format!("Failed to extract probs array: {}", e))
                })?;

            let (_, label_idx) = argmax_2d(&probs.view());
            results[position_id - 1] = G2PWOut::Pinyin(POLY_LABLES[label_idx].clone());
        }

        Ok(results)
    }
}
