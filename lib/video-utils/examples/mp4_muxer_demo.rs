use std::path::PathBuf;
use std::time::Duration;
use video_utils::mp4_muxer::{MP4Muxer, MP4MuxerConfig, AACConfig, FrameData, AudioData};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              MP4 封装器功能测试                                       ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    let config = MP4MuxerConfig {
        output_path: PathBuf::from("tmp/output_muxer.mp4"),
        frame_rate: 30,
        aac: AACConfig {
            bitrate: 128_000,
            sample_rate: 44_100,
            channels: 2,
        },
    };

    println!("配置:");
    println!("  输出: {:?}", config.output_path);
    println!("  帧率: {} fps", config.frame_rate);
    println!("  音频: {} kbps, {} Hz, {} 声道",
        config.aac.bitrate / 1000,
        config.aac.sample_rate,
        config.aac.channels
    );
    println!();

    let (muxer, video_tx, audio_tx) = MP4Muxer::start(config)?;

    println!("生成测试视频 (90帧, 约3秒)...");

    let width = 640;
    let height = 480;
    let frame_count = 90;

    for i in 0..frame_count {
        // 生成渐变色RGB帧
        let mut frame_data = vec![0u8; width as usize * height as usize * 3];

        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 3) as usize;

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

        video_tx.send(frame)?;

        // 每30帧生成一帧音频 (1秒)
        if i % 30 == 0 {
            let sample_rate = 44_100;
            let channels = 2;
            let samples_per_frame = sample_rate as usize / 30;

            // 生成正弦波音频
            let mut samples = vec![0.0f32; samples_per_frame * channels];
            let frequency = 440.0;

            for j in 0..samples_per_frame {
                let t = j as f32 / sample_rate as f32;
                let value = (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.3;

                samples[j * 2] = value;
                samples[j * 2 + 1] = value;
            }

            let audio = AudioData {
                samples,
                sample_rate,
                channels: channels as u8,
                timestamp: Duration::from_millis((i as u64 * 1000) / 30),
            };

            audio_tx.send(audio)?;
        }

        // 显示进度
        if (i + 1) % 30 == 0 {
            println!("  进度: {}/{} 帧 ({:.1}%)", i + 1, frame_count, ((i + 1) as f32 / frame_count as f32) * 100.0);
        }
    }

    println!("完成编码...");

    // 停止封装器
    muxer.stop()?;

    println!("✓ MP4 文件已生成: {:?}", config.output_path);

    println!();
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                     测试完成                                      ║");
    println!("╚════════════════════════════════════════════════════════════════╝");

    Ok(())
}
