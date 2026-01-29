//! Video trim/cut example
//!
//! This example demonstrates extracting segments from videos.

use std::path::Path;
use std::time::Duration;
use video_utils::editor::trim::{trim_video, TrimConfig, extract_segment};
use video_utils::metadata::get_metadata;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              视频修剪功能测试                                        ║");
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

    // Test 1: Extract first 2 seconds
    println!("【测试1】提取前 2 秒");
    println!("=========================================");
    let config1 = TrimConfig::new(
        input_file,
        "tmp/trim_first_2s.mp4",
        Duration::ZERO,
    )
    .with_duration(Duration::from_secs(2));

    println!("配置: 从 0.00s 开始，持续 2.00 秒");
    match trim_video(config1) {
        Ok(_) => println!("✓ 修剪完成"),
        Err(e) => println!("❌ 修剪失败: {}", e),
    }

    verify_output("tmp/trim_first_2s.mp4", "前2秒")?;
    println!();

    // Test 2: Extract from 1s to 3s
    println!("【测试2】提取 1-3 秒片段");
    println!("=========================================");
    let config2 = TrimConfig::new(
        input_file,
        "tmp/trim_1_to_3s.mp4",
        Duration::from_secs(1),
    )
    .with_end(Duration::from_secs(3));

    println!("配置: 从 1.00s 开始，到 3.00s 结束");
    match trim_video(config2) {
        Ok(_) => println!("✓ 修剪完成"),
        Err(e) => println!("❌ 修剪失败: {}", e),
    }

    verify_output("tmp/trim_1_to_3s.mp4", "1-3秒")?;
    println!();

    // Test 3: Extract from 2s to end using convenience function
    println!("【测试3】提取从 2 秒到结尾");
    println!("=========================================");
    println!("配置: 从 2.00s 开始到视频结尾");

    // Get video duration
    let total_duration = metadata.duration;
    let start = 2.0;
    let duration = total_duration - start;

    match extract_segment(input_file, "tmp/trim_from_2s.mp4", start, duration) {
        Ok(_) => println!("✓ 修剪完成"),
        Err(e) => println!("❌ 修剪失败: {}", e),
    }

    verify_output("tmp/trim_from_2s.mp4", "从2秒到结尾")?;
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
        .arg("stream=width,height,r_frame_rate,duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(file)
        .output()?;

    if output.status.success() {
        let info = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = info.trim().split('\n').collect();

        println!("  ✅ {} 输出验证通过:", test_name);
        for line in lines.iter().take(4) {
            let parts: Vec<&str> = line.split('=').collect();
            if parts.len() == 2 {
                println!("     {}: {}", parts[0], parts[1]);
            } else {
                println!("     {}", line.trim());
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
