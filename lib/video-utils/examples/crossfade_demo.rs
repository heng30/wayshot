//! Video crossfade transition example
//!
//! This example demonstrates crossfade transitions between two videos.

use std::path::Path;
use video_utils::filters::crossfade::{crossfade_videos, CrossfadeConfig};
use video_utils::metadata::get_metadata;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              视频交叉淡化过渡功能测试                              ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    // Check if test files exist
    let input_file = "data/test.mp4";
    if !Path::new(input_file).exists() {
        println!("❌ 测试文件不存在: {}", input_file);
        println!("请先确保有测试视频文件");
        return Ok(());
    }

    // For demo purposes, use the same video twice (but in practice you'd use different videos)
    let video1 = input_file;
    let video2 = input_file;

    // Get original metadata
    println!("📹 视频信息:");
    let metadata1 = get_metadata(video1)?;
    println!("  视频1: {} ({:.2}秒)", video1, metadata1.duration);
    println!("  视频2: {} ({:.2}秒)", video2, metadata1.duration);
    println!();

    // Test 1: Short crossfade (1 second)
    println!("【测试1】短交叉淡化 (1秒重叠)");
    println!("=========================================");

    let config1 = CrossfadeConfig::new(video1, video2, "tmp/crossfade_1s.mp4", 1.0);

    println!("配置: 重叠时长 1.0 秒");
    match crossfade_videos(config1) {
        Ok(_) => println!("✓ 交叉淡化完成"),
        Err(e) => println!("❌ 交叉淡化失败: {}", e),
    }

    verify_output("tmp/crossfade_1s.mp4", "1秒交叉淡化")?;
    println!();

    // Test 2: Medium crossfade (2 seconds)
    println!("【测试2】中等交叉淡化 (2秒重叠)");
    println!("=========================================");

    let config2 = CrossfadeConfig::new(video1, video2, "tmp/crossfade_2s.mp4", 2.0);

    println!("配置: 重叠时长 2.0 秒");
    match crossfade_videos(config2) {
        Ok(_) => println!("✓ 交叉淡化完成"),
        Err(e) => println!("❌ 交叉淡化失败: {}", e),
    }

    verify_output("tmp/crossfade_2s.mp4", "2秒交叉淡化")?;
    println!();

    // Test 3: Long crossfade (3 seconds)
    println!("【测试3】长交叉淡化 (3秒重叠)");
    println!("=========================================");

    let config3 = CrossfadeConfig::new(video1, video2, "tmp/crossfade_3s.mp4", 3.0);

    println!("配置: 重叠时长 3.0 秒");
    match crossfade_videos(config3) {
        Ok(_) => println!("✓ 交叉淡化完成"),
        Err(e) => println!("❌ 交叉淡化失败: {}", e),
    }

    verify_output("tmp/crossfade_3s.mp4", "3秒交叉淡化")?;
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
        .arg("stream=width,height,duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(file)
        .output()?;

    if output.status.success() {
        let info = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = info.trim().split('\n').collect();

        println!("  ✅ {} 验证通过:", test_name);
        for line in lines.iter().take(3) {
            let label = match line.trim() {
                l if l.parse::<f32>().is_ok() => "时长",
                l if l.parse::<u32>().is_ok() && l.parse::<u32>().ok().unwrap_or(5000) < 5000 => "宽度/高度",
                _ => line,
            };
            println!("     {}: {}", label, line.trim());
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
