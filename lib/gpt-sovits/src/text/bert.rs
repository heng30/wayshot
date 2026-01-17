use crate::{GSVError, create_session, text::utils::BERT_TOKENIZER};
use ndarray::Array2;
use ort::{inputs, value::Tensor};
use std::{path::Path, str::FromStr, sync::Arc};
use tokenizers::Tokenizer;

const BERT_FEATURE_SIZE: usize = 1024;

#[derive(Debug)]
pub struct BertModel {
    model: ort::session::Session,
    tokenizers: Arc<tokenizers::Tokenizer>,
}

impl BertModel {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, GSVError> {
        let tokenizer = Tokenizer::from_str(BERT_TOKENIZER).map_err(|e| {
            GSVError::InternalError(format!("Failed to create BERT tokenizer: {}", e))
        })?;

        Ok(Self {
            model: create_session(path)?,
            tokenizers: Arc::new(tokenizer),
        })
    }

    pub fn get_bert(
        &mut self,
        text: &str,
        word2ph: &[i32],
        total_phones: usize,
    ) -> Result<Array2<f32>, GSVError> {
        match self.get_bert_internal(text, word2ph) {
            Ok(mut bert_features) => {
                if bert_features.nrows() != total_phones {
                    log::warn!(
                        "bert_features.nrows({}) != total_phones({total_phones}), using zeros bert",
                        bert_features.nrows()
                    );

                    bert_features = Array2::<f32>::zeros((total_phones, BERT_FEATURE_SIZE));
                }
                Ok(bert_features)
            }
            Err(e) => {
                log::warn!("Failed to get real bert for '{text}': {e}, using zeros bert",);
                Ok(Array2::<f32>::zeros((total_phones, BERT_FEATURE_SIZE)))
            }
        }
    }

    fn get_bert_internal(&mut self, text: &str, word2ph: &[i32]) -> Result<Array2<f32>, GSVError> {
        let encoding = self.tokenizers.encode(text, true).map_err(|e| {
            GSVError::InternalError(format!("Failed to encode text for BERT: {}", e))
        })?;

        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let token_type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&t| t as i64).collect();
        let attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&m| m as i64)
            .collect();

        let input_ids_len = input_ids.len();
        let token_type_ids_len = token_type_ids.len();
        let attention_mask_len = attention_mask.len();

        let inputs = inputs![
            "input_ids" => create_i64_tensor(input_ids, input_ids_len)?,
            "attention_mask" => create_i64_tensor(attention_mask, attention_mask_len)?,
            "token_type_ids" => create_i64_tensor(token_type_ids, token_type_ids_len)?
        ];

        let bert_out = self.model.run(inputs)?;
        let bert_feature = bert_out["bert_feature"]
            .try_extract_array::<f32>()?
            .to_owned();

        build_phone_level_feature(bert_feature.into_dimensionality()?, word2ph)
    }
}

fn build_phone_level_feature(res: Array2<f32>, word2ph: &[i32]) -> Result<Array2<f32>, GSVError> {
    let (num_rows, num_cols) = (res.nrows(), res.ncols());
    let total_phones = word2ph.iter().map(|&c| c as usize).sum();

    let mut result = Array2::zeros((total_phones, num_cols));
    let mut row_offset = 0;

    for (i, &count) in word2ph.iter().enumerate() {
        let src_row = if i < num_rows {
            res.row(i)
        } else {
            res.row(num_rows - 1)
        };

        let count = count as usize;
        for j in 0..num_cols {
            for k in 0..count {
                result[[row_offset + k, j]] = src_row[j];
            }
        }
        row_offset += count;
    }

    Ok(result)
}

#[inline]
fn create_i64_tensor(data: Vec<i64>, cols: usize) -> Result<Tensor<i64>, GSVError> {
    Tensor::from_array(Array2::from_shape_vec((1, cols), data)?)
        .map_err(|e| GSVError::InternalError(format!("Failed to create tensor: {}", e)))
}
