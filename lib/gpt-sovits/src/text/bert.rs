use {
    crate::{GSVError, create_session, text::utils::BERT_TOKENIZER},
    log::{debug, warn},
    ndarray::Array2,
    ort::{inputs, value::Tensor},
    std::{path::Path, str::FromStr, sync::Arc},
    tokenizers::Tokenizer,
};

const BERT_FEATURE_SIZE: usize = 1024;

#[derive(Debug)]
pub struct BertModel {
    model: Option<ort::session::Session>,
    tokenizers: Option<Arc<tokenizers::Tokenizer>>,
}

/// Helper function to create an i64 tensor with proper error handling
fn create_i64_tensor(data: Vec<i64>, cols: usize) -> Result<Tensor<i64>, GSVError> {
    Tensor::from_array(Array2::from_shape_vec((1, cols), data)?)
        .map_err(|e| GSVError::InternalError(format!("Failed to create tensor: {}", e)))
}

impl BertModel {
    pub fn new<P: AsRef<Path>>(path: Option<P>) -> Result<Self, GSVError> {
        let model = path.map(create_session).transpose()?;
        let tokenizer = Tokenizer::from_str(BERT_TOKENIZER)
            .map_err(|e| GSVError::InternalError(format!("Failed to create BERT tokenizer: {}", e)))?;

        Ok(Self {
            model,
            tokenizers: Some(Arc::new(tokenizer)),
        })
    }

    pub fn get_bert(
        &mut self,
        text: &str,
        word2ph: &[i32],
        total_phones: usize,
    ) -> Result<Array2<f32>, GSVError> {
        let has_models = self.model.is_some() && self.tokenizers.is_some();

        if has_models {
            match self.get_real_bert(text, word2ph) {
                Ok(bert_features) => {
                    debug!("use real bert, {}", text);
                    if bert_features.nrows() != total_phones {
                        warn!(
                            "bert_features.shape()[0]: {} != total_phones: {}, use empty",
                            bert_features.nrows(),
                            total_phones
                        );
                        return Ok(self.get_fake_bert(total_phones));
                    }
                    Ok(bert_features)
                }
                Err(e) => {
                    warn!("Failed to get real bert for '{}': {}, using fake bert", text, e);
                    Ok(self.get_fake_bert(total_phones))
                }
            }
        } else {
            debug!("use empty bert, {}", text);
            Ok(self.get_fake_bert(total_phones))
        }
    }

    fn get_real_bert(&mut self, text: &str, word2ph: &[i32]) -> Result<Array2<f32>, GSVError> {
        let tokenizer = self.tokenizers.as_ref()
            .ok_or_else(|| GSVError::InternalError("BERT tokenizer not initialized".into()))?;
        let session = self.model.as_mut()
            .ok_or_else(|| GSVError::InternalError("BERT model not initialized".into()))?;

        let encoding = tokenizer.encode(text, true)
            .map_err(|e| GSVError::InternalError(format!("Failed to encode text for BERT: {}", e)))?;

        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&m| m as i64).collect();
        let token_type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&t| t as i64).collect();

        let inputs = inputs![
            "input_ids" => create_i64_tensor(input_ids.clone(), input_ids.len())?,
            "attention_mask" => create_i64_tensor(attention_mask.clone(), attention_mask.len())?,
            "token_type_ids" => create_i64_tensor(token_type_ids.clone(), token_type_ids.len())?
        ];

        let bert_out = session.run(inputs)?;
        let bert_feature = bert_out["bert_feature"]
            .try_extract_array::<f32>()?
            .to_owned();

        build_phone_level_feature(bert_feature.into_dimensionality()?, word2ph)
    }

    fn get_fake_bert(&self, total_phones: usize) -> Array2<f32> {
        Array2::<f32>::zeros((total_phones, BERT_FEATURE_SIZE))
    }
}

/// Helper function to expand word-level features to phone-level features.
fn build_phone_level_feature(res: Array2<f32>, word2ph: &[i32]) -> Result<Array2<f32>, GSVError> {
    let num_rows = res.nrows();
    let num_cols = res.ncols();
    let total_phones: usize = word2ph.iter().map(|&c| c as usize).sum();

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
