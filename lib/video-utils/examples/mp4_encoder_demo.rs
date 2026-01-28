use std::path::PathBuf;
use std::time::Duration;
use video_utils::{
    MP4Encoder, MP4EncoderConfig, H264Config, AACConfig, H264Preset, FrameData, AudioData,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              MP4编码器功能测试                                        ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    // 测试1: 高质量配置 (CRF 20, Slow preset)
    test_high_quality()?;

    // 测试2: 中等质量配置 (CRF 23, Medium preset)
    test_medium_quality()?;

    // 测试3: 快速编码 (CRF 28, Ultrafast preset)
    test_fast_encoding()?;

    println!();
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                     测试完成                                      ║");
    println!("╚════════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// 测试1: 高质量配置
fn test_high_quality() -> Result<(), Box<dyn std::error::Error>> {
    println!("【测试1】高质量编码 (CRF 20, Slow preset)");
    println!("=========================================");

    let config = MP4EncoderConfig {
        output_path: PathBuf::from("tmp/output_high_quality.mp4"),
        frame_rate: 30,
        h264: H264Config {
            bitrate: 5_000_000, // 5 Mbps
            preset: H264Preset::Slow, // 高质量，编码慢
            crf: Some(20), // 高质量 (0-51, 越低越好)
        },
        aac: AACConfig {
            bitrate: 192_000, // 192 kbps
            sample_rate: 48_000,
            channels: 2,
        },
    };

    println!("配置:");
    println!("  视频: CRF={}, preset={:?}, 5Mbps", config.h264.crf.unwrap(), config.h264.preset);
    println!("  音频: 192kbps, 48kHz, 立体声");
    println!();

    encode_test_video(config, 90)?;

    println!("✓ 高质量测试完成");
    println!();
    println!();

    Ok(())
}

/// 测试2: 中等质量配置
fn test_medium_quality() -> Result<(), Box<dyn std::error::Error>> {
    println!("【测试2】中等质量编码 (CRF 23, Medium preset)");
    println!("==============================================");

    let config = MP4EncoderConfig {
        output_path: PathBuf::from("tmp/output_medium_quality.mp4"),
        frame_rate: 30,
        h264: H264Config {
            bitrate: 2_000_000, // 2 Mbps
            preset: H264Preset::Medium, // 默认
            crf: Some(23), // 中等质量
        },
        aac: AACConfig {
            bitrate: 128_000, // 128 kbps
            sample_rate: 44_100,
            channels: 2,
        },
    };

    println!("配置:");
    println!("  视频: CRF={}, preset={:?}, 2Mbps", config.h264.crf.unwrap(), config.h264.preset);
    println!("  音频: 128kbps, 44.1kHz, 立体声");
    println!();

    encode_test_video(config, 90)?;

    println!("✓ 中等质量测试完成");
    println!();
    println!();

    Ok(())
}

/// 测试3: 快速编码
fn test_fast_encoding() -> Result<(), Box<dyn std::error::Error>> {
    println!("【测试3】快速编码 (CRF 28, Ultrafast preset)");
    println!("============================================");

    let config = MP4EncoderConfig {
        output_path: PathBuf::from("tmp/output_fast_encoding.mp4"),
        frame_rate: 30,
        h264: H264Config {
            bitrate: 1_000_000, // 1 Mbps
            preset: H264Preset::Ultrafast, // 编码最快
            crf: Some(28), // 较低质量
        },
        aac: AACConfig {
            bitrate: 96_000, // 96 kbps
            sample_rate: 44_100,
            channels: 2,
        },
    };

    println!("配置:");
    println!("  视频: CRF={}, preset={:?}, 1Mbps", config.h264.crf.unwrap(), config.h264.preset);
    println!("  音频: 96kbps, 44.1kHz, 立体声");
    println!();

    encode_test_video(config, 90)?;

    println!("✓ 快速编码测试完成");
    println!();
    println!();

    Ok(())
}

/// 生成测试视频
fn encode_test_video(config: MP4EncoderConfig, frame_count: usize) -> Result<(), Box<dyn std::error::Error>> {
    let (encoder, video_tx, audio_tx) = MP4Encoder::start(config)?;

    println!("生成测试视频 ({}帧, 约 {:.1} 秒)...", frame_count, frame_count as f32 / 30.0);

    let width = 1280;
    let height = 720;

    // 生成测试帧
    for i in 0..frame_count {
        // 创建一个渐变色的RGB帧
        let mut frame_data = vec![0u8; width as usize * height as usize * 3];

        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 3) as usize;

                // 生成渐变色
                let r = ((x as f32 / width as f32) * 255.0) as u8;
                let g = ((y as f32 / height as f32) * 255.0) as u8;
                let b = ((i as f32 / frame_count as f32) * 255.0) as u8;

                frame_data[idx] = r;
                frame_data[idx + 1] = g;
                frame_data[idx + 2] = b;
            }
        }

        let frame = FrameData {
            width,
            height,
            data: frame_data,
            timestamp: Duration::from_millis((i as u64 * 1000) / 30),
        };

        video_tx.send(frame).map_err(|e| format!("Failed to send video frame: {}", e))?;

        // 每30帧生成一帧音频 (1秒)
        if i % 30 == 0 {
            let sample_rate = 48000;
            let channels = 2;
            let samples_per_frame = sample_rate as usize / 30; // 30 fps

            // 生成正弦波音频
            let mut samples = vec![0.0f32; samples_per_frame * channels];
            let frequency = 440.0; // A4音符

            for j in 0..samples_per_frame {
                let t = j as f32 / sample_rate as f32;
                let value = (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.3;

                // 立体声
                samples[j * 2] = value;
                samples[j * 2 + 1] = value;
            }

            let audio = AudioData {
                samples,
                sample_rate,
                channels: channels as u8,
                timestamp: Duration::from_millis((i as u64 * 1000) / 30),
            };

            audio_tx.send(audio).map_err(|e| format!("Failed to send audio: {}", e))?;
        }

        // 显示进度
        if (i + 1) % 30 == 0 {
            println!("  进度: {}/{} 帧 ({:.1}%)", i + 1, frame_count, ((i + 1) as f32 / frame_count as f32) * 100.0);
        }
    }

    println!("编码中...");

    // 停止编码器
    encoder.stop()?;

    println!("✓ 编码完成");

    Ok(())
}
