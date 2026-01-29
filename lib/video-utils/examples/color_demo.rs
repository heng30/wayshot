//! Video color adjustment example
//!
//! This example demonstrates adjusting video brightness, contrast, and saturation.

use std::path::Path;
use video_utils::filters::color::{
    adjust_color, ColorAdjustConfig, adjust_brightness, adjust_contrast, adjust_saturation,
};
use video_utils::metadata::get_metadata;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              视频颜色调整功能测试                                  ║");
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

    // Test 1: Increase brightness
    println!("【测试1】增加亮度 (+30%)");
    println!("=========================================");

    match adjust_brightness(input_file, "tmp/color_bright.mp4", 30) {
        Ok(_) => println!("✓ 亮度调整完成"),
        Err(e) => println!("❌ 调整失败: {}", e),
    }

    verify_output("tmp/color_bright.mp4", "亮度调整")?;
    println!();

    // Test 2: Decrease brightness
    println!("【测试2】降低亮度 (-30%)");
    println!("=========================================");

    match adjust_brightness(input_file, "tmp/color_dark.mp4", -30) {
        Ok(_) => println!("✓ 亮度调整完成"),
        Err(e) => println!("❌ 调整失败: {}", e),
    }

    verify_output("tmp/color_dark.mp4", "降低亮度")?;
    println!();

    // Test 3: Increase contrast
    println!("【测试3】增加对比度 (+40%)");
    println!("=========================================");

    match adjust_contrast(input_file, "tmp/color_contrast.mp4", 40) {
        Ok(_) => println!("✓ 对比度调整完成"),
        Err(e) => println!("❌ 调整失败: {}", e),
    }

    verify_output("tmp/color_contrast.mp4", "对比度调整")?;
    println!();

    // Test 4: Grayscale (saturation -100)
    println!("【测试4】灰度化 (饱和度 -100%)");
    println!("=========================================");

    match adjust_saturation(input_file, "tmp/color_gray.mp4", -100) {
        Ok(_) => println!("✓ 饱和度调整完成"),
        Err(e) => println!("❌ 调整失败: {}", e),
    }

    verify_output("tmp/color_gray.mp4", "灰度化")?;
    println!();

    // Test 5: Combined adjustments
    println!("【测试5】组合调整 (亮度+20, 对比度+30, 饱和度+50)");
    println!("=========================================");

    let config = ColorAdjustConfig::new(input_file, "tmp/color_combined.mp4")
        .with_brightness(20)
        .with_contrast(30)
        .with_saturation(50);

    match adjust_color(config) {
        Ok(_) => println!("✓ 组合调整完成"),
        Err(e) => println!("❌ 调整失败: {}", e),
    }

    verify_output("tmp/color_combined.mp4", "组合调整")?;
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
