//! Video concatenation example
//!
//! This example demonstrates joining multiple videos end-to-end.

use std::path::Path;
use video_utils::editor::concat::{concat_videos, ConcatConfig, concat_videos_simple};
use video_utils::metadata::get_metadata;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              视频拼接功能测试                                        ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    // For testing, we'll use the same video 3 times to simulate 3 clips
    let input_file = "data/test.mp4";
    if !Path::new(input_file).exists() {
        println!("❌ 测试文件不存在: {}", input_file);
        println!("请先确保有测试视频文件");
        return Ok(());
    }

    println!("📹 输入视频信息:");
    let metadata = get_metadata(input_file)?;
    println!("  时长: {:.2} 秒", metadata.duration);
    println!("  视频流数: {}", metadata.video_streams_count);
    println!();

    // Test 1: Simple concatenation
    println!("【测试1】简单拼接（3个相同视频）");
    println!("=========================================");

    let inputs = vec![
        input_file.to_string(),
        input_file.to_string(),
        input_file.to_string(),
    ];

    println!("输入: 3 个视频文件（相同视频，仅用于测试）");
    println!("预期输出时长: {:.2} 秒 (3 x {:.2})", metadata.duration * 3.0, metadata.duration);

    match concat_videos_simple(inputs, "tmp/concat_simple.mp4") {
        Ok(_) => println!("✓ 拼接完成"),
        Err(e) => println!("❌ 拼接失败: {}", e),
    }

    verify_output("tmp/concat_simple.mp4", "简单拼接")?;
    println!();

    // Test 2: Concatenation with resolution normalization
    println!("【测试2】拼接并归一化分辨率");
    println!("=========================================");

    let config = ConcatConfig::new(
        vec![
            input_file.to_string(),
            input_file.to_string(),
        ],
        "tmp/concat_normalized.mp4".to_string(),
    )
    .with_resolution(1280, 720)
    .with_video_bitrate(3_000_000);

    println!("配置: 目标分辨率 1280x720");
    println!("      视频比特率: 3 Mbps");

    match concat_videos(config) {
        Ok(_) => println!("✓ 拼接完成"),
        Err(e) => println!("❌ 拼接失败: {}", e),
    }

    verify_output("tmp/concat_normalized.mp4", "归一化拼接")?;
    println!();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                     测试完成                                      ║");
    println!("╚════════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Verify output file using ffprobe
fn verify_output(file: &str, test_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(file).exists() {
        println!("  ⚠️  输出文件不存在: {}", file);
        return Ok(());
    }

    println!("  🔍 验证输出文件...");

    // Use ffprobe to get video info
    let output = std::process::Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=width,height,r_frame_rate,duration,bit_rate")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(file)
        .output()?;

    if output.status.success() {
        let info = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = info.trim().split('\n').collect();

        println!("  ✅ {} 输出验证通过:", test_name);
        for line in lines.iter().take(5) {
            let parts: Vec<&str> = line.split('=').collect();
            if parts.len() == 2 {
                let label = match parts[0] {
                    "r_frame_rate" => "帧率",
                    "width" => "宽度",
                    "height" => "高度",
                    "duration" => "时长",
                    "bit_rate" => "比特率",
                    _ => parts[0],
                };
                let value = if parts[0] == "duration" {
                    format!("{:.2} 秒", parts[1].parse::<f64>().unwrap_or(0.0))
                } else if parts[0] == "bit_rate" {
                    let bps = parts[1].parse::<u64>().unwrap_or(0);
                    format!("{:.2} Mbps", bps as f64 / 1_000_000.0)
                } else {
                    parts[1].to_string()
                };
                println!("     {}: {}", label, value);
            }
        }

        // Get file size
        if let Ok(metadata) = std::fs::metadata(file) {
            let size_mb = metadata.len() / 1_048_576;
            println!("     文件大小: {} MB", size_mb);
        }
    } else {
        println!("  ⚠️  ffprobe 验证失败");
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("     错误: {}", stderr);
    }

    Ok(())
}
