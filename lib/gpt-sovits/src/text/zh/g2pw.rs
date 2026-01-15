use {
    crate::{
        error::GSVError,
        onnx_builder::create_onnx_cpu_session,
        text::{BERT_TOKENIZER, DICT_MONO_CHARS, DICT_POLY_CHARS, argmax_2d},
    },
    ndarray::Array,
    ort::value::Tensor,
    std::{
        fmt::Debug,
        path::Path,
        str::FromStr,
        sync::{Arc, LazyLock},
    },
    tokenizers::Tokenizer,
};

pub static LABELS: &str = include_str!("dict_poly_index_list.json");

pub static POLY_LABLES: LazyLock<Vec<String>> =
    LazyLock::new(|| {
        serde_json::from_str(LABELS)
            .expect("Failed to parse POLY_LABELS JSON")
    });

const POLYPHONIC_RATIO: usize = 10;

#[derive(Clone)]
pub enum G2PWOut {
    Pinyin(String),
    Yue(String),
    RawChar(char),
}

impl Debug for G2PWOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pinyin(s) => write!(f, "\"{}\"", s),
            Self::Yue(s) => write!(f, "\"{}\"", s),
            Self::RawChar(s) => write!(f, "\"{}\"", s),
        }
    }
}

#[derive(Debug)]
pub struct G2PW {
    model: Option<ort::session::Session>,
    tokenizers: Option<Arc<tokenizers::Tokenizer>>,
}

impl G2PW {
    pub fn new<P: AsRef<Path>>(g2pw_path: Option<P>) -> Result<Self, GSVError> {
        let (model, tokenizers) = match g2pw_path {
            Some(path) => {
                log::info!("G2PW model is loading...");
                let model = create_onnx_cpu_session(path)?;
                log::info!("G2PW model is loaded.");
                let tokenizer = Tokenizer::from_str(BERT_TOKENIZER)
                    .map_err(|e| GSVError::InternalError(format!("Failed to create G2PW tokenizer: {}", e)))?;
                (Some(model), Some(Arc::new(tokenizer)))
            }
            None => (None, None),
        };
        Ok(Self { model, tokenizers })
    }

    pub fn g2p(&mut self, text: &str) -> Vec<G2PWOut> {
        let has_ml = self.model.is_some() && self.tokenizers.is_some();
        if has_ml {
            self.get_pinyin_ml(text)
                .unwrap_or_else(|_| self.simple_get_pinyin(text))
        } else {
            self.simple_get_pinyin(text)
        }
    }

    /// Process a character to get its pronunciation
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

    pub fn simple_get_pinyin(&self, text: &str) -> Vec<G2PWOut> {
        Vec::from_iter(text.chars().map(Self::process_char))
    }

    fn get_pinyin_ml(&mut self, text: &str) -> Result<Vec<G2PWOut>, GSVError> {
        let tokenizer = self.tokenizers.as_ref()
            .ok_or_else(|| GSVError::InternalError("G2PW tokenizer not initialized".into()))?;
        let model = self.model.as_mut()
            .ok_or_else(|| GSVError::InternalError("G2PW model not initialized".into()))?;

        // Encode text
        let encoding = tokenizer.encode(text, true)
            .map_err(|e| GSVError::InternalError(format!("Failed to encode text for G2PW: {}", e)))?;
        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let token_type_ids = vec![0i64; input_ids.len()];
        let attention_mask = vec![1i64; input_ids.len()];

        // Helper to create tensor with error handling
        let create_tensor_f32 = |array: Array<f32, ndarray::Dim<[usize; 2]>>| -> Result<Tensor<f32>, GSVError> {
            Tensor::from_array(array)
                .map_err(|e| GSVError::InternalError(format!("Failed to create tensor: {}", e)))
        };

        let create_tensor_i64 = |array: Array<i64, ndarray::Dim<[usize; 2]>>| -> Result<Tensor<i64>, GSVError> {
            Tensor::from_array(array)
                .map_err(|e| GSVError::InternalError(format!("Failed to create tensor: {}", e)))
        };

        // Pre-allocate with capacity
        let char_count = text.chars().count();
        let mut poly_chars = Vec::with_capacity(char_count / POLYPHONIC_RATIO);
        let mut results: Vec<G2PWOut> = Vec::with_capacity(char_count);

        for (i, c) in text.chars().enumerate() {
            let result = Self::process_char(c);
            if let Some(poly) = DICT_POLY_CHARS.get(&c) {
                // Store polyphonic character info for ML processing
                let mut phoneme_mask = vec![0f32; POLY_LABLES.len()];
                for (_, idx) in &poly.phones {
                    phoneme_mask[*idx] = 1.0;
                }
                poly_chars.push((i + 1, poly.index, phoneme_mask));
                results.push(G2PWOut::Pinyin(String::new())); // Placeholder
            } else {
                results.push(result);
            }
        }

        // Process polyphonic characters with ML model
        for (position_id, char_id, phoneme_mask) in poly_chars {
            let phoneme_mask_tensor = create_tensor_f32(
                Array::from_shape_vec((1, phoneme_mask.len()), phoneme_mask)?)?;
            let position_id_tensor = create_tensor_i64(
                Array::from_shape_vec((1, 1), vec![position_id as i64])?)?;
            let char_id_tensor = create_tensor_i64(
                Array::from_shape_vec((1, 1), vec![char_id as i64])?)?;

            // Create input tensors (recreated each iteration to avoid clone issues)
            let input_ids_tensor = create_tensor_i64(
                Array::from_shape_vec((1, input_ids.len()), input_ids.clone())?)?;
            let token_type_ids_tensor = create_tensor_i64(
                Array::from_shape_vec((1, token_type_ids.len()), token_type_ids.clone())?)?;
            let attention_mask_tensor = create_tensor_i64(
                Array::from_shape_vec((1, attention_mask.len()), attention_mask.clone())?)?;

            // Run inference
            let model_output = model.run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "token_type_ids" => token_type_ids_tensor,
                "attention_mask" => attention_mask_tensor,
                "phoneme_mask" => phoneme_mask_tensor,
                "char_ids" => char_id_tensor,
                "position_ids" => position_id_tensor,
            ])?;

            // Extract and process results
            let probs = model_output["probs"].try_extract_array::<f32>()
                .map_err(|e| GSVError::InternalError(format!("Failed to extract probs array: {}", e)))?;

            let (_, label_idx) = argmax_2d(&probs.view());
            results[position_id - 1] = G2PWOut::Pinyin(POLY_LABLES[label_idx].clone());
        }

        Ok(results)
    }
}
