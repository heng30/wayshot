use half::f16;
use ndarray::{Array, Array2, Axis, Ix2, Ix3, IxDyn, s};
use num_traits::FloatConst;
use ort::value::DynValue;
use rand::{RngExt, distr::weighted::WeightedIndex};
use std::{
    fmt::{Debug, Formatter},
    ops::{Deref, DerefMut},
};

pub struct Logits(Array2<f32>);

impl TryFrom<DynValue> for Logits {
    type Error = ort::Error;

    fn try_from(value: DynValue) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&DynValue> for Logits {
    type Error = ort::Error;

    fn try_from(value: &DynValue) -> Result<Self, Self::Error> {
        let arr = value.try_extract_array::<f32>()?.into_owned();
        let arr = arr.into_dimensionality::<Ix2>().expect("Expected dim 2");
        Ok(Self(arr))
    }
}

impl From<Array<f32, IxDyn>> for Logits {
    fn from(value: Array<f32, IxDyn>) -> Self {
        let arr = value.into_dimensionality::<Ix2>().expect("Expected dim 2");
        Self(arr)
    }
}

impl Deref for Logits {
    type Target = Array2<f32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Logits {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Debug for Logits {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Logits {
    pub fn from_3d_dyn_value(value: &DynValue) -> ort::Result<Self> {
        let arr = if let Ok(res) = value.try_extract_array::<f32>() {
            res.into_owned()
        } else {
            let arr = value.try_extract_array::<f16>()?;
            arr.into_owned().mapv(f32::from)
        };

        let arr = arr
            .into_dimensionality::<Ix3>()
            .expect("Expected 3 dimensions");
        // logits come in the following shape float32[batch_size,decoder_sequence_length,2048]
        // based on transformers.js we can assume that decoder_sequence_length is going
        // to be 1, so we can just remove it.
        let arr = arr.remove_axis(Axis(1));
        Ok(Self(arr))
    }

    pub fn apply_free_guidance(self, guidance_scale: usize) -> Self {
        if self.0.dim().0 % 2 != 0 {
            panic!(
                "In order to apply free guidance to the logits, the first size of the first dimension must be even"
            )
        }

        let unguided_bsz = self.0.dim().0 / 2;
        let cond_logits = self.0.slice(s![0..unguided_bsz, ..]);
        let uncond_logits = self.0.slice(s![unguided_bsz.., ..]);

        // Based on transformers.js, src/generation/logits_process.js#L603:
        // scores = uncond_logits + (cond_logits - uncond_logits) * guidance_scale
        Self((cond_logits.into_owned() - uncond_logits) * guidance_scale as f32 + uncond_logits)
    }

    /// Samples the logits across the batch dimension (the first one), and returns a vector
    /// of length equal to the batch size, with the sampled index and the log probability for
    /// that batch entry.
    ///
    /// # Arguments
    ///
    /// * `k`: Take into account only top k logits in each batch
    ///
    /// returns: Vec<(i64, f32), Global> the per-batch sample
    pub fn sample(&self, k: usize) -> Vec<(i64, f32)> {
        let mut result = vec![];
        let softmax_logits = ndarray_softmax(&self.0);
        for batch in softmax_logits.axis_iter(Axis(0)) {
            let k = k.min(batch.len());

            // Vec<(token_id, softmax_prob)>
            let mut softmax_logits_batch = batch
                .iter()
                .enumerate()
                .map(|(i, e)| (i as i64, *e))
                .collect::<Vec<_>>();

            // Sort based on softmax_prob in order to bring the most probable tokens to the front.
            softmax_logits_batch.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .expect("Could not compare two numbers in order to sort them")
            });
            // Trim based on provided k.
            softmax_logits_batch = softmax_logits_batch[0..k].to_vec();
            // Create a distribution based on the softmax probabilities.
            let distribution = WeightedIndex::new(softmax_logits_batch.iter().map(|e| e.1))
                .expect("Could not create WeightedIndex distribution");
            // Sample a random index based on the softmax probabilities.
            let (idx, softmax_prob) = softmax_logits_batch[rand::rng().sample(distribution)];
            // based on JS implementation:
            //  Math.log(probabilities[sampledIndex])
            // In JS, Math.log uses euler's number base.
            result.push((idx, softmax_prob.log(f32::E())))
        }
        result
    }
}

/// Compute softmax of a 2D array along the last axis (vocabulary dimension).
///
/// For a `[batch_size, vocab_size]` logits array, softmax must be applied
/// independently to each row so that each row sums to 1.0.
fn ndarray_softmax(arr: &Array2<f32>) -> Array2<f32> {
    let mut result = arr.clone();
    for mut row in result.rows_mut() {
        let max = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mut sum = 0.0f32;
        for val in row.iter_mut() {
            *val = (*val - max).exp();
            sum += *val;
        }
        for val in row.iter_mut() {
            *val /= sum;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_guidance() {
        let logits = Logits::from(Array::from(vec![[10., -1., 3.], [-1., 1., 11.]]).into_dyn());
        let logits = logits.apply_free_guidance(3);
        assert_eq!(logits.shape(), &[1, 3]);
    }

    #[test]
    fn softmax_rows_sum_to_one() {
        let data = Array2::from_shape_vec(
            (4, 8),
            vec![
                1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0,
            ],
        )
        .unwrap();
        let result = ndarray_softmax(&data);
        for i in 0..4 {
            let row_sum: f32 = result.row(i).sum();
            assert!(
                (row_sum - 1.0).abs() < 1e-5,
                "Row {i} sum is {row_sum}, expected 1.0"
            );
        }
    }

    #[test]
    fn softmax_uniform_input() {
        let data = Array2::from_shape_vec((2, 4), vec![5.0, 5.0, 5.0, 5.0, -3.0, -3.0, -3.0, -3.0])
            .unwrap();
        let result = ndarray_softmax(&data);
        for i in 0..2 {
            for j in 0..4 {
                assert!(
                    (result[[i, j]] - 0.25).abs() < 1e-5,
                    "Uniform input should give 0.25 for each element, got {} at [{i},{j}]",
                    result[[i, j]]
                );
            }
        }
    }

    #[test]
    fn softmax_dominant_token() {
        let data = Array2::from_shape_vec((1, 4), vec![-100.0, -100.0, 100.0, -100.0]).unwrap();
        let result = ndarray_softmax(&data);
        assert!(
            result[[0, 2]] > 0.99,
            "Dominant token should have >0.99 probability"
        );
        assert!(
            result[[0, 0]] < 0.01,
            "Non-dominant tokens should have <0.01 probability"
        );
    }
}
