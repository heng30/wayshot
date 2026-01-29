//! Video speed change example
//!
//! This example demonstrates changing video playback speed.

use std::path::Path;
use video_utils::editor::speed::{change_speed, SpeedConfig, speed_up, slow_down};
use video_utils::metadata::get_metadata;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              视频速度控制测试                                        ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    // Check if test file exists
    let input_file = "data/test.mp4";
    if !Path::new(input_file).exists() {
        println!("❌ 测试文件不存在: {}", input_file);
        println!("请先确保有测试视频文件");
        return Ok(());
    }

    // Get original metadata
    println!("📹 原始视频信息:");
    let metadata = get_metadata(input_file)?;
    println!("  时长: {:.2} 秒", metadata.duration);
    println!("  视频流数: {}", metadata.video_streams_count);
    println!();

    // Test 1: Speed up to 2x
    println!("【测试1】2倍速播放");
    println!("=========================================");

    let expected_duration = metadata.duration / 2.0;
    println!("速度: 2x");
    println!("预期时长: {:.2} 秒 (原时长 / 2)", expected_duration);

    match speed_up(input_file, "tmp/speed_2x.mp4", 2.0) {
        Ok(_) => println!("✓ 速度调整完成"),
        Err(e) => println!("❌ 失败: {}", e),
    }

    verify_output("tmp/speed_2x.mp4", "2倍速")?;
    println!();

    // Test 2: Slow down to 0.5x
    println!("【测试2】0.5倍速播放 (慢动作)");
    println!("=========================================");

    let expected_duration = metadata.duration / 0.5;
    println!("速度: 0.5x");
    println!("预期时长: {:.2} 秒 (原时长 / 0.5)", expected_duration);

    match slow_down(input_file, "tmp/speed_05x.mp4", 0.5) {
        Ok(_) => println!("✓ 速度调整完成"),
        Err(e) => println!("❌ 失败: {}", e),
    }

    verify_output("tmp/speed_05x.mp4", "0.5倍速")?;
    println!();

    // Test 3: Speed up to 4x
    println!("【测试3】4倍速播放");
    println!("=========================================");

    let config = SpeedConfig::new(input_file, "tmp/speed_4x.mp4", 4.0);

    println!("配置: 4x 快速播放");
    println!("预期时长: {:.2} 秒", metadata.duration / 4.0);

    match change_speed(config) {
        Ok(_) => println!("✓ 速度调整完成"),
        Err(e) => println!("❌ 失败: {}", e),
    }

    verify_output("tmp/speed_4x.mp4", "4倍速")?;
    println!();

    // Test 4: Slow down to 0.25x (very slow motion)
    println!("【测试4】0.25倍速播放 (超慢动作)");
    println!("=========================================");

    let config = SpeedConfig::new(input_file, "tmp/speed_025x.mp4", 0.25);

    println!("配置: 0.25x 超慢动作播放");
    println!("预期时长: {:.2} 秒", metadata.duration / 0.25);

    match change_speed(config) {
        Ok(_) => println!("✓ 速度调整完成"),
        Err(e) => println!("❌ 失败: {}", e),
    }

    verify_output("tmp/speed_025x.mp4", "0.25倍速")?;
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

        println!("  ✅ {} 验证通过:", test_name);
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
            let size_kb = metadata.len() / 1024;
            println!("     大小: {} KB", size_kb);
        }
    } else {
        println!("  ⚠️  ffprobe 验证失败");
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("     错误: {}", stderr);
    }

    Ok(())
}
