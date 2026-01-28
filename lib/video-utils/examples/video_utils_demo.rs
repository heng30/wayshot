use std::path::PathBuf;
use std::time::Duration;
use video_utils::{get_metadata, extract_audio_interval, extract_all_audio, extract_frame_at_time, extract_frames_interval, save_frame_as_image};

fn main() {
    env_logger::init();

    let test_video = PathBuf::from("data/test.mp4");

    if !test_video.exists() {
        eprintln!("Test video not found: {:?}", test_video);
        eprintln!("Please place a test video file at data/test.mp4");
        std::process::exit(1);
    }

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║           Video Utils 功能测试                                        ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    // 功能1: 获取视频文件元信息
    println!("【功能1】获取视频文件元信息");
    println!("================================");
    match get_metadata(&test_video) {
        Ok(metadata) => {
            println!("✓ 成功获取元信息");
            println!("  文件: {}", metadata.path);
            println!("  格式: {}", metadata.format_name);
            println!("  时长: {:.2} 秒", metadata.duration);
            println!("  比特率: {} bps ({:.2} Mbps)", metadata.bitrate, metadata.bitrate as f64 / 1_000_000.0);
            println!("  大小: {} bytes ({:.2} MB)", metadata.size, metadata.size as f64 / 1_048_576.0);
            println!("  视频流: {} 个", metadata.video_streams_count);
            println!("  音频流: {} 个", metadata.audio_streams_count);
        }
        Err(e) => {
            eprintln!("✗ 获取元信息失败: {}", e);
        }
    }

    println!();
    println!();

    // 功能2: 获取视频中指定时间间隔音频数据
    println!("【功能2】获取指定时间间隔音频数据");
    println!("=====================================");
    match extract_audio_interval(&test_video, Duration::from_secs_f64(1.0), Duration::from_secs_f64(3.0)) {
        Ok(audio) => {
            println!("✓ 成功提取音频数据");
            println!("  采样率: {} Hz", audio.sample_rate);
            println!("  声道数: {}", audio.channels);
            println!("  样本格式: {}", audio.sample_format);
            println!("  开始时间: {:.2} 秒", audio.start_time.as_secs_f64());
            println!("  持续时间: {:.2} 秒", audio.duration.as_secs_f64());
            println!("  样本数: {}", audio.nb_samples);
        }
        Err(e) => {
            eprintln!("✗ 提取音频失败: {}", e);
        }
    }

    println!();
    println!();

    // 功能3: 获取视频指定时间点的图片
    println!("【功能3】获取指定时间点图片");
    println!("================================");
    match extract_frame_at_time(&test_video, Duration::from_secs_f64(2.5)) {
        Ok(frame) => {
            println!("✓ 成功提取帧");
            println!("  尺寸: {}x{}", frame.width, frame.height);
            println!("  像素格式: {}", frame.pixel_format);
            println!("  时间戳: {:.2} 秒", frame.pts.as_secs_f64());
            println!("  数据大小: {} bytes", frame.data.len());

            // Save the frame as an image
            let output_path = PathBuf::from("tmp/frame_at_2.5s.png");
            match save_frame_as_image(&frame, &output_path) {
                Ok(_) => println!("  已保存到: {:?}", output_path),
                Err(e) => eprintln!("  ✗ 保存图片失败: {}", e),
            }
        }
        Err(e) => {
            eprintln!("✗ 提取帧失败: {}", e);
        }
    }

    println!();
    println!();

    // 功能4: 获取视频指定时间间隔的所有图片
    println!("【功能4】获取指定时间间隔的所有图片");
    println!("=====================================");
    match extract_frames_interval(&test_video, Duration::from_secs_f64(1.0), Duration::from_secs_f64(4.0), Duration::from_secs_f64(1.0)) {
        Ok(frames) => {
            println!("✓ 成功提取 {} 帧", frames.len());
            for (i, frame) in frames.iter().take(3).enumerate() {
                println!("  帧 {}: {}x{}, 时间: {:.2}s, 大小: {} bytes",
                    i + 1, frame.width, frame.height, frame.pts.as_secs_f64(), frame.data.len());

                // Save first few frames as images
                if i < 3 {
                    let output_path = PathBuf::from(format!("tmp/frame_{}_at_{:.1}s.png", i + 1, frame.pts.as_secs_f64()));
                    match save_frame_as_image(frame, &output_path) {
                        Ok(_) => println!("    已保存到: {:?}", output_path),
                        Err(e) => eprintln!("    ✗ 保存失败: {}", e),
                    }
                }
            }
            if frames.len() > 3 {
                println!("  ... (还有 {} 帧)", frames.len() - 3);
            }
        }
        Err(e) => {
            eprintln!("✗ 提取帧失败: {}", e);
        }
    }

    println!();
    println!();

    // 测试整个视频音频
    println!("【附加测试】提取整个视频音频");
    println!("====================================");
    match extract_all_audio(&test_video) {
        Ok(audio) => {
            println!("✓ 成功提取完整音频");
            println!("  采样率: {} Hz", audio.sample_rate);
            println!("  声道数: {}", audio.channels);
            println!("  总时长: {:.2} 秒", audio.duration.as_secs_f64());
        }
        Err(e) => {
            eprintln!("✗ 提取音频失败: {}", e);
        }
    }

    println!();
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                     测试完成                                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}
