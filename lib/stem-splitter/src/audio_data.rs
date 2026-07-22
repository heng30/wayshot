#[derive(Clone, Debug)]
pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub fn audio_frame_count(samples: &[f32], channels: u16) -> usize {
    let channels = usize::from(channels.max(1));
    samples.len() / channels
}

pub fn fill_stereo_window(
    samples: &[f32],
    channels: u16,
    start_frame: usize,
    left_raw: &mut [f32],
    right_raw: &mut [f32],
) {
    let channels = usize::from(channels.max(1));

    for i in 0..left_raw.len() {
        let frame = start_frame + i;
        let base = frame * channels;
        if base >= samples.len() {
            left_raw[i] = 0.0;
            right_raw[i] = 0.0;
            continue;
        }

        let left = samples[base];
        let right = if channels == 1 {
            left
        } else {
            samples.get(base + 1).copied().unwrap_or(left)
        };

        left_raw[i] = left;
        right_raw[i] = right;
    }
}
