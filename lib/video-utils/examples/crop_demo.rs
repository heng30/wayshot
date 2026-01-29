//! Video crop example
//!
//! This example demonstrates cropping videos to extract regions.

use std::path::Path;
use video_utils::filters::crop::{crop_video, CropConfig, CropMode, crop_center, crop_to_aspect};
use video_utils::metadata::get_metadata;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              视频裁剪功能测试                                        ║");
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

    // Test 1: Center crop to 640x360
    println!("【测试1】中心裁剪到 640x360");
    println!("=========================================");

    let config1 = CropConfig::new(input_file, "tmp/crop_center.mp4", 640, 360)
        .with_mode(CropMode::Center);

    println!("配置: 640x360, 中心裁剪");

    match crop_video(config1) {
        Ok(_) => println!("✓ 裁剪完成"),
        Err(e) => println!("❌ 裁剪失败: {}", e),
    }

    verify_output("tmp/crop_center.mp4", "中心裁剪")?;
    println!();

    // Test 2: Crop from top-left to 320x240
    println!("【测试2】左上角裁剪到 320x240");
    println!("=========================================");

    let config2 = CropConfig::new(input_file, "tmp/crop_topleft.mp4", 320, 240)
        .with_mode(CropMode::TopLeft);

    println!("配置: 320x240, 左上角裁剪");

    match crop_video(config2) {
        Ok(_) => println!("✓ 裁剪完成"),
        Err(e) => println!("❌ 裁剪失败: {}", e),
    }

    verify_output("tmp/crop_topleft.mp4", "左上角裁剪")?;
    println!();

    // Test 3: Convenience function - crop center
    println!("【测试3】便捷函数 - 中心裁剪");
    println!("=========================================");

    println!("配置: 480x270, 中心裁剪");

    match crop_center(input_file, "tmp/crop_center_480.mp4", 480, 270) {
        Ok(_) => println!("✓ 裁剪完成"),
        Err(e) => println!("❌ 裁剪失败: {}", e),
    }

    verify_output("tmp/crop_center_480.mp4", "便捷中心裁剪")?;
    println!();

    // Test 4: Crop to 16:9 aspect ratio
    println!("【测试4】裁剪到 16:9 宽高比");
    println!("=========================================");

    println!("配置: 自动计算裁剪区域以获得 16:9 比例");

    match crop_to_aspect(input_file, "tmp/crop_16x9.mp4", 16, 9) {
        Ok(_) => println!("✓ 裁剪完成"),
        Err(e) => println!("❌ 裁剪失败: {}", e),
    }

    verify_output("tmp/crop_16x9.mp4", "16:9 裁剪")?;
    println!();

    // Test 5: Crop to 4:3 aspect ratio
    println!("【测试5】裁剪到 4:3 宽高比");
    println!("=========================================");

    println!("配置: 自动计算裁剪区域以获得 4:3 比例");

    match crop_to_aspect(input_file, "tmp/crop_4x3.mp4", 4, 3) {
        Ok(_) => println!("✓ 裁剪完成"),
        Err(e) => println!("❌ 裁剪失败: {}", e),
    }

    verify_output("tmp/crop_4x3.mp4", "4:3 裁剪")?;
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
            println!("  ✅ {} 验证通过:", test_name);
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
