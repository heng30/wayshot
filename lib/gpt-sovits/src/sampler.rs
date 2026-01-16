use derivative::Derivative;
use derive_setters::Setters;
use rand::{
    SeedableRng,
    distr::{Distribution, weighted::WeightedIndex},
    rngs::StdRng,
};
use std::{cmp::Ordering, collections::HashSet};

#[derive(Debug, Clone, Copy, Derivative, Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct SamplingParams {
    #[derivative(Default(value = "1.0"))]
    pub temperature: f32, // greater than 0.0

    #[derivative(Default(value = "1.0"))]
    pub repetition_penalty: f32, // greater than 0.0

    pub top_k: Option<usize>,
    pub top_p: Option<f32>,
}

pub struct Sampler {
    rng: StdRng,
    probs: Vec<f32>,
}

impl Sampler {
    pub fn new(vocab_size: usize) -> Self {
        Self {
            rng: StdRng::from_os_rng(),
            probs: Vec::with_capacity(vocab_size),
        }
    }

    #[inline]
    fn apply_repetition_penalty(logits: &mut [f32], prev_tokens: &[i64], penalty: f32) {
        if penalty == 1.0 {
            return;
        }
        let prev_tokens_set: HashSet<_> = prev_tokens.iter().copied().collect();
        for (token_id, logit) in logits.iter_mut().enumerate() {
            if prev_tokens_set.contains(&(token_id as i64)) {
                *logit = if *logit >= 0.0 && penalty != 0.0 {
                    *logit / penalty
                } else {
                    *logit * penalty
                };
            }
        }
    }

    #[inline]
    fn apply_temperature(logits: &mut [f32], temperature: f32) {
        if temperature > 0.0 {
            let inv_temp = 1.0 / temperature;
            for logit in logits.iter_mut() {
                *logit *= inv_temp;
            }
        }
    }

    fn softmax(&mut self, logits: &[f32]) {
        self.probs.clear();
        if logits.is_empty() {
            return;
        }

        let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        let mut sum_exp = 0.0;
        self.probs.extend(logits.iter().map(|&logit| {
            let exp_val = (logit - max_logit).exp();
            sum_exp += exp_val;
            exp_val
        }));

        if sum_exp > 0.0 {
            let inv_sum_exp = 1.0 / sum_exp;
            for prob in self.probs.iter_mut() {
                *prob *= inv_sum_exp;
            }
        }
    }

    pub fn sample(
        &mut self,
        logits: &mut [f32],
        prev_tokens: &[i64],
        params: &SamplingParams,
    ) -> i64 {
        Self::apply_repetition_penalty(logits, prev_tokens, params.repetition_penalty);

        if params.temperature == 0.0 {
            return argmax(logits);
        }

        Self::apply_temperature(logits, params.temperature);
        self.softmax(logits);

        let mut candidates: Vec<(usize, f32)> = self.probs.iter().copied().enumerate().collect();
        if candidates.is_empty() {
            return argmax(logits);
        }

        if let Some(k) = params.top_k
            && k > 0
            && k < candidates.len()
        {
            candidates.select_nth_unstable_by(k - 1, |a, b| {
                b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal)
            });
            candidates.truncate(k);
        }

        if let Some(p) = params.top_p
            && p < 1.0
        {
            candidates.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
            let mut cum_prob = 0.0;
            let mut cutoff = candidates.len();
            for (i, &(_, prob)) in candidates.iter().enumerate() {
                cum_prob += prob;
                if cum_prob >= p {
                    cutoff = i + 1;
                    break;
                }
            }
            candidates.truncate(cutoff);
        }

        let weights = candidates.iter().map(|&(_, p)| p);
        let dist = match WeightedIndex::new(weights) {
            Ok(d) => d,
            Err(_) => {
                return candidates
                    .first()
                    .map_or_else(|| argmax(logits), |&(idx, _)| idx as i64);
            }
        };

        let sampled_candidate_index = dist.sample(&mut self.rng);
        candidates[sampled_candidate_index].0 as i64
    }
}

#[inline]
pub fn argmax(logits: &[f32]) -> i64 {
    logits
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, _)| idx as i64)
        .unwrap_or(0)
}
