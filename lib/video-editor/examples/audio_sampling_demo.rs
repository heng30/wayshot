use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use video_editor::{metadata::get_metadata, tracks::segment::Segment};

fn main() {
    env_logger::init();

    let test_files = vec!["test.mp4", "test.wav"];
    for file_name in test_files {
        let file_path = PathBuf::from("data").join(file_name);

        log::info!("=== Testing audio_sampling for: {} ===", file_name);

        // 获取元数据
        let metadata = match get_metadata(&file_path) {
            Ok(meta) => Arc::new(meta),
            Err(e) => {
                log::warn!(
                    "Skipping `{}`: failed to get metadata: {}",
                    file_path.display(),
                    e
                );
                continue;
            }
        };

        // 检查是否有音频流
        let audio_meta = match metadata.audios.first() {
            Some(audio) => audio,
            None => {
                log::warn!("Skipping `{}`: no audio stream found", file_path.display());
                continue;
            }
        };

        log::info!("Source audio metadata:");
        log::info!("  - Channels: {}", audio_meta.channels);
        log::info!("  - Sample rate: {} Hz", audio_meta.sample_rate);
        log::info!("  - Duration: {:.2}s", metadata.duration.as_secs_f64());

        // 创建一个 segment (使用前 5 秒，如果有足够时长)
        let segment_duration = metadata.duration.min(Duration::from_secs(5));
        let segment = Segment::new_with_source_offset(
            Duration::ZERO,
            Duration::ZERO,
            segment_duration,
            1.0,
            1.0,
            metadata,
        );

        log::info!("Segment duration: {:.2}s", segment_duration.as_secs_f64());

        // 测试不同的采样数量
        let test_counts = vec![10, 50, 100];

        for count in test_counts {
            log::info!("\n--- Sampling {} points ---", count);

            let audio_samples = match segment.audio_sampling(count) {
                Ok(samples) => samples,
                Err(e) => {
                    log::warn!("Failed to sample audio: {:?}", e);
                    continue;
                }
            };

            // 验证计算的有效采样率
            let expected_sample_rate = (count as f64 / segment_duration.as_secs_f64()) as u32;

            log::info!("Result:");
            log::info!("  - Channels: {}", audio_samples.channels);
            log::info!(
                "  - Effective sample rate: {} Hz (expected: {} Hz)",
                audio_samples.sample_rate,
                expected_sample_rate
            );
            log::info!(
                "  - Sample count per channel: {}",
                audio_samples.samples.len() / audio_samples.channels as usize
            );
            log::info!(
                "  - Sample interval: {:.3} ms",
                1000.0 / audio_samples.sample_rate as f64
            );

            // 显示一些样本值
            if !audio_samples.samples.is_empty() {
                let show_count = 5.min(audio_samples.samples.len());
                log::info!("  - First {} samples (per-channel avg):", show_count);
                for (i, sample) in audio_samples.samples.iter().take(show_count).enumerate() {
                    log::info!("    [{}] {:.6}", i, sample);
                }

                // 计算统计信息
                let max_sample = audio_samples.samples.iter().fold(0.0_f32, |a, b| a.max(*b));
                let avg_sample: f32 =
                    audio_samples.samples.iter().sum::<f32>() / audio_samples.samples.len() as f32;

                log::info!("  - Max amplitude: {:.6}", max_sample);
                log::info!("  - Avg amplitude: {:.6}", avg_sample);
            }
        }

        log::info!("\n");
    }

    log::info!("=== Test completed ===");
}
