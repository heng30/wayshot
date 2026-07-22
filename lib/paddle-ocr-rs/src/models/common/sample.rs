
use candle_core::Tensor;
use candle_transformers::generation::{LogitsProcessor, Sampling};
use rand::distr::Distribution;

/// Get logits processor with sampling parameters
pub fn get_logit_processor(
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<usize>,
    seed: u64,
) -> LogitsProcessor {
    let temperature = temperature.and_then(|v| if v < 1e-7 { None } else { Some(v) });
    match top_k {
        None => LogitsProcessor::new(
            seed,
            temperature.map(|temp| temp as f64),
            top_p.map(|tp| tp as f64),
        ),
        Some(k) => {
            let sampling = match temperature {
                None => Sampling::ArgMax,
                Some(temperature) => match top_p {
                    None => Sampling::TopK {
                        k,
                        temperature: temperature as f64,
                    },
                    Some(p) => Sampling::TopKThenTopP {
                        k,
                        p: p as f64,
                        temperature: temperature as f64,
                    },
                },
            };
            LogitsProcessor::from_sampling(seed, sampling)
        }
    }
}

/// Apply repeat penalty to logits
pub fn use_repeat_penalty(
    repeat_penalty: f32,
    repeat_last_n: Option<usize>,
    logits: &Tensor,
    context: &[u32],
) -> Result<Tensor, crate::Error> {
    if repeat_penalty == 1.0 || repeat_last_n == Some(0) {
        Ok(logits.clone())
    } else {
        let start_at = if let Some(last_n) = repeat_last_n {
            context.len().saturating_sub(last_n)
        } else {
            0
        };
        Ok(candle_transformers::utils::apply_repeat_penalty(
            logits,
            repeat_penalty,
            &context[start_at..],
        )?)
    }
}

/// Sample weighted from probability distribution
pub fn sample_weighted(prs: &[f32]) -> Result<u32, crate::Error> {
    let mut rng = rand::rng();
    let dist = rand::distr::weighted::WeightedIndex::new(prs)
        .map_err(|e| crate::Error::Sampling(e.to_string()))?;
    Ok(dist.sample(&mut rng) as u32)
}