
use candle_core::{DType, Device, Tensor};
use candle_transformers::generation::LogitsProcessor;

use crate::{
    models::common::{
        InferenceModel, MultiModalData,
        sample::{get_logit_processor, use_repeat_penalty},
    },
    tokenizer::mod_tokenizer::TokenizerModel,
};

/// Generation context for text generation
pub struct GenerationContext {
    pub logit_processor: LogitsProcessor,
    pub repeat_penalty: f32,
    pub repeat_last_n: usize,
    pub seqlen_offset: usize,
    pub seq_len: usize,
    pub sample_len: u32,
    pub device: Device,
}

impl GenerationContext {
    pub fn new(
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<usize>,
        repeat_penalty: Option<f32>,
        repeat_last_n: Option<usize>,
        seed: u64,
        initial_seq_len: usize,
        max_tokens: u32,
        device: Device,
    ) -> Self {
        Self {
            logit_processor: get_logit_processor(temperature, top_p, top_k, seed),
            repeat_penalty: repeat_penalty.unwrap_or(1.0),
            repeat_last_n: repeat_last_n.unwrap_or(64),
            seqlen_offset: 0,
            seq_len: initial_seq_len,
            sample_len: max_tokens,
            device,
        }
    }

    pub fn prepare_for_next_token(&mut self, token: u32) -> Result<Tensor, crate::Error> {
        self.update_status();
        self.create_input_ids(token)
    }

    fn update_status(&mut self) {
        self.seqlen_offset += self.seq_len;
        self.seq_len = 1;
    }

    fn create_input_ids(&self, token: u32) -> Result<Tensor, crate::Error> {
        Ok(Tensor::from_vec(vec![token], (1, 1), &self.device)?)
    }
}

/// Sample and push token helper function
fn sample_and_push(
    ctx: &mut GenerationContext,
    logits: &Tensor,
    generated: &mut Vec<u32>,
) -> Result<u32, crate::Error> {
    let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;
    // Repeat penalty
    let logits = use_repeat_penalty(
        ctx.repeat_penalty,
        Some(ctx.repeat_last_n),
        &logits,
        generated,
    )?;
    let token = ctx.logit_processor.sample(&logits)?;
    generated.push(token);
    Ok(token)
}

/// Generate text using generic model
pub fn generate_generic_text<M: InferenceModel>(
    model: &mut M,
    tokenizer: &TokenizerModel,
    input_ids: Tensor,
    data: MultiModalData,
    ctx: &mut GenerationContext,
) -> Result<String, crate::Error> {
    let tokens = generate_generic_tokens(model, tokenizer, input_ids, data, ctx)?;
    tokenizer.token_decode(tokens)
}

/// Generate tokens using generic model, returning raw token IDs
///
/// This function returns the raw token IDs which can be used for
/// post-processing (e.g., extracting location information from LOC tokens)
pub fn generate_generic_tokens<M: InferenceModel>(
    model: &mut M,
    _tokenizer: &TokenizerModel,
    input_ids: Tensor,
    data: MultiModalData,
    ctx: &mut GenerationContext,
) -> Result<Vec<u32>, crate::Error> {
    let mut generated = Vec::new();
    let eos_ids = model.stop_token_ids();
    let logits = model.forward_initial(&input_ids, ctx.seqlen_offset, data)?;
    let next_token = sample_and_push(ctx, &logits, &mut generated)?;
    let mut input_ids = ctx.prepare_for_next_token(next_token)?;

    // Autoregressive loop
    for _ in 1..ctx.sample_len {
        let logits = model.forward_step(&input_ids, ctx.seqlen_offset)?;
        let next_token = sample_and_push(ctx, &logits, &mut generated)?;

        if eos_ids.contains(&next_token) {
            break;
        }
        input_ids = ctx.prepare_for_next_token(next_token)?;
    }
    model.clear_cache();
    Ok(generated)
}