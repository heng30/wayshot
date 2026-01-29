//! Video scale/resize example
//!
//! This example demonstrates scaling videos to different resolutions.

use std::path::Path;
use video_utils::filters::scale::{scale_video, ScaleConfig, ScaleQuality, scale_to_fit, scale_to_exact};
use video_utils::metadata::get_metadata;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              视频缩放功能测试                                        ║");
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

    // Test 1: Scale down to 720p preserving aspect ratio
    println!("【测试1】缩放到 720p (保持宽高比)");
    println!("=========================================");
    let config1 = ScaleConfig::new(
        input_file,
        "tmp/scaled_720p.mp4",
        1280,
        720,
    )
    .with_quality(ScaleQuality::High);

    println!("配置: 1280x720, 高质量");
    match scale_video(config1) {
        Ok(_) => println!("✓ 缩放完成"),
        Err(e) => println!("❌ 缩放失败: {}", e),
    }

    // Verify with ffprobe
    verify_output("tmp/scaled_720p.mp4", "720p 缩放")?;
    println!();

    // Test 2: Scale to fit within 640x480
    println!("【测试2】缩放以适应 640x480 (保持宽高比)");
    println!("===============================================");
    match scale_to_fit(input_file, "tmp/scaled_fit.mp4", 640, 480) {
        Ok(_) => println!("✓ 缩放完成"),
        Err(e) => println!("❌ 缩放失败: {}", e),
    }

    verify_output("tmp/scaled_fit.mp4", "fit 缩放")?;
    println!();

    // Test 3: Scale to exact 320x240 (may stretch)
    println!("【测试3】强制缩放到 320x240 (不保持宽高比)");
    println!("===============================================");
    match scale_to_exact(input_file, "tmp/scaled_320x240.mp4", 320, 240) {
        Ok(_) => println!("✓ 缩放完成"),
        Err(e) => println!("❌ 缩放失败: {}", e),
    }

    verify_output("tmp/scaled_320x240.mp4", "320x240 强制缩放")?;
    println!();

    // Test 4: Fast scaling (for performance)
    println!("【测试4】快速缩放到 640x360");
    println!("===========================");
    let config4 = ScaleConfig::new(
        input_file,
        "tmp/scaled_fast.mp4",
        640,
        360,
    )
    .with_quality(ScaleQuality::Fast);

    println!("配置: 640x360, 快速质量 (最近邻)");
    match scale_video(config4) {
        Ok(_) => println!("✓ 快速缩放完成"),
        Err(e) => println!("❌ 缩放失败: {}", e),
    }

    verify_output("tmp/scaled_fast.mp4", "快速缩放")?;
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

        if lines.len() >= 2 {
            println!("  ✅ {} 输出验证通过:", test_name);
            println!("     宽度: {}", lines[0].trim());
            println!("     高度: {}", lines[1].trim());
            if lines.len() >= 3 {
                println!("     时长: {} 秒", lines[2].trim());
            }

            // Get file size
            if let Ok(metadata) = std::fs::metadata(file) {
                let size_kb = metadata.len() / 1024;
                println!("     大小: {} KB", size_kb);
            }
        }
    } else {
        println!("  ⚠️  ffprobe 验证失败");
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("     错误: {}", stderr);
    }

    Ok(())
}
