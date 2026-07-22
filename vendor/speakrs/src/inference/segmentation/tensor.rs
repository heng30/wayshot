use super::SegmentationError;

pub(super) type OutputShape3 = (usize, usize, usize);

pub(super) struct SegmentationWindows<'a> {
    audio: &'a [f32],
    offsets: Vec<usize>,
    padded: Option<Vec<f32>>,
    window_samples: usize,
}

impl<'a> SegmentationWindows<'a> {
    pub(super) fn collect(audio: &'a [f32], window_samples: usize, step_samples: usize) -> Self {
        let mut offsets = Vec::new();
        let mut offset = 0;
        while offset + window_samples <= audio.len() {
            offsets.push(offset);
            offset += step_samples;
        }

        let padded = if offset < audio.len() && audio.len() > window_samples {
            let mut padded = vec![0.0f32; window_samples];
            let remaining = audio.len() - offset;
            padded[..remaining].copy_from_slice(&audio[offset..]);
            Some(padded)
        } else {
            None
        };

        Self {
            audio,
            offsets,
            padded,
            window_samples,
        }
    }

    pub(super) fn total_windows(&self) -> usize {
        self.offsets.len() + self.padded.is_some() as usize
    }

    pub(super) fn is_empty(&self) -> bool {
        self.total_windows() == 0
    }

    pub(super) fn window(
        &self,
        idx: usize,
        context: &'static str,
    ) -> Result<&[f32], SegmentationError> {
        if idx < self.offsets.len() {
            let start = self.offsets[idx];
            return Ok(&self.audio[start..start + self.window_samples]);
        }
        if idx == self.offsets.len() {
            return padded_window(&self.padded, context);
        }

        Err(SegmentationError::Invariant {
            context,
            message: format!(
                "window index {idx} exceeded total window count {}",
                self.total_windows()
            ),
        })
    }
}

pub(super) fn padded_window<'a>(
    padded: &'a Option<Vec<f32>>,
    context: &'static str,
) -> Result<&'a [f32], SegmentationError> {
    padded
        .as_deref()
        .ok_or_else(|| SegmentationError::Invariant {
            context,
            message: "missing padded window".to_owned(),
        })
}

pub(super) fn first_output<T>(
    outputs: impl IntoIterator<Item = T>,
    context: &'static str,
) -> Result<T, SegmentationError> {
    outputs
        .into_iter()
        .next()
        .ok_or_else(|| SegmentationError::MalformedOutput {
            context,
            message: "missing output tensor".to_owned(),
        })
}

pub(super) fn output_shape3(
    shape: &ort::value::Shape,
    context: &'static str,
) -> Result<OutputShape3, SegmentationError> {
    let [batch, frames, classes]: [i64; 3] =
        shape
            .as_ref()
            .try_into()
            .map_err(|_| SegmentationError::MalformedOutput {
                context,
                message: format!("expected rank 3 output, got shape {shape}"),
            })?;

    let dims = [batch, frames, classes];
    if dims.iter().any(|dim| *dim < 0) {
        return Err(SegmentationError::MalformedOutput {
            context,
            message: format!("expected non-negative output dimensions, got shape {shape}"),
        });
    }

    Ok((batch as usize, frames as usize, classes as usize))
}

#[cfg(test)]
mod tests {
    use super::{first_output, output_shape3};

    #[test]
    fn first_output_reports_missing_tensor() {
        let error = first_output(Vec::<()>::new(), "segmentation test").unwrap_err();

        assert_eq!(
            error.to_string(),
            "segmentation test: missing output tensor"
        );
    }

    #[test]
    fn output_shape3_reports_low_rank_tensor() {
        let shape = ort::value::Shape::from([10_i64, 3]);
        let error = output_shape3(&shape, "segmentation test").unwrap_err();

        assert_eq!(
            error.to_string(),
            "segmentation test: expected rank 3 output, got shape [10, 3]"
        );
    }
}
