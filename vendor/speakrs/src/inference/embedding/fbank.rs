use ndarray::{Array2, s};
use ort::value::TensorRef;

use super::{EmbeddingModel, FBANK_BATCH_SIZE, array2_from_shape_vec, first_output};

impl EmbeddingModel {
    /// Compute fbank features for a single audio chunk via the split fbank model
    pub(crate) fn compute_chunk_fbank(&mut self, audio: &[f32]) -> Result<Array2<f32>, ort::Error> {
        let copy_len = audio.len().min(self.meta.window_samples);
        self.buffers
            .split_waveform_buffer
            .slice_mut(s![0, 0, ..copy_len])
            .assign(&ndarray::ArrayView1::from(&audio[..copy_len]));
        if copy_len < self.meta.window_samples {
            self.buffers
                .split_waveform_buffer
                .slice_mut(s![0, 0, copy_len..])
                .fill(0.0);
        }

        let waveform_tensor =
            TensorRef::from_array_view(self.buffers.split_waveform_buffer.view())?;
        let outputs = self
            .ort
            .split_fbank_session
            .as_mut()
            .ok_or_else(|| ort::Error::new("missing split fbank session"))?
            .run(ort::inputs!["waveform" => waveform_tensor])?;
        let output = first_output(outputs.values(), "chunk fbank output")?;
        let (shape, data) = output.try_extract_tensor::<f32>()?;
        let frames = shape[1] as usize;
        let features = shape[2] as usize;
        array2_from_shape_vec(frames, features, data.to_vec(), "chunk fbank output")
    }

    /// Compute fbank features for multiple audio chunks in a single batched call
    pub fn compute_chunk_fbanks_batch(
        &mut self,
        audios: &[&[f32]],
    ) -> Result<Vec<Array2<f32>>, ort::Error> {
        let has_batched = self.has_batched_fbank();
        if !has_batched {
            tracing::debug!(
                count = audios.len(),
                "fbank: no batched session, falling back to per-window"
            );
            return audios
                .iter()
                .map(|audio| self.compute_chunk_fbank(audio))
                .collect();
        }
        let mut results = Vec::with_capacity(audios.len());
        for batch_start in (0..audios.len()).step_by(FBANK_BATCH_SIZE) {
            let batch_end = (batch_start + FBANK_BATCH_SIZE).min(audios.len());
            let batch = &audios[batch_start..batch_end];

            if batch.len() == 1 {
                for audio in batch {
                    results.push(self.compute_chunk_fbank(audio)?);
                }
                continue;
            }

            self.fill_split_fbank_batch_buffer(batch);

            if batch.len() < FBANK_BATCH_SIZE {
                for audio in batch {
                    results.push(self.compute_chunk_fbank(audio)?);
                }
                continue;
            }

            let waveform_tensor =
                TensorRef::from_array_view(self.buffers.split_fbank_batch_buffer.view())?;
            let outputs = self
                .ort
                .split_fbank_batched_session
                .as_mut()
                .ok_or_else(|| ort::Error::new("missing split fbank batched session"))?
                .run(ort::inputs!["waveform" => waveform_tensor])?;
            let output = first_output(outputs.values(), "batched chunk fbank output")?;
            let (shape, data) = output.try_extract_tensor::<f32>()?;
            Self::push_fbank_batch_results(
                &mut results,
                data,
                shape[1] as usize,
                shape[2] as usize,
                batch.len(),
            )?;
        }

        Ok(results)
    }

    fn fill_split_fbank_batch_buffer(&mut self, audios: &[&[f32]]) {
        self.buffers.split_fbank_batch_buffer.fill(0.0);
        for (idx, audio) in audios.iter().enumerate() {
            let copy_len = audio.len().min(self.meta.window_samples);
            self.buffers
                .split_fbank_batch_buffer
                .slice_mut(s![idx, 0, ..copy_len])
                .assign(&ndarray::ArrayView1::from(&audio[..copy_len]));
        }
    }

    fn push_fbank_batch_results(
        results: &mut Vec<Array2<f32>>,
        data: &[f32],
        frames: usize,
        features: usize,
        count: usize,
    ) -> Result<(), ort::Error> {
        let stride = frames * features;
        for idx in 0..count {
            let start = idx * stride;
            let batch = array2_from_shape_vec(
                frames,
                features,
                data[start..start + stride].to_vec(),
                "batched fbank output",
            )?;
            results.push(batch);
        }
        Ok(())
    }
}
