//! Video rotate/flip example
//!
//! This example demonstrates rotating and flipping videos.

use std::path::Path;
use video_utils::filters::transform::{
    rotate_video, flip_video, RotateConfig, FlipDirection, RotateAngle,
    rotate_90, rotate_180, flip_horizontal, flip_vertical,
};
use video_utils::metadata::get_metadata;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              视频旋转/翻转功能测试                                  ║");
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

    // Test 1: Rotate 90 degrees
    println!("【测试1】旋转 90 度顺时针");
    println!("=========================================");

    let config1 = RotateConfig::new(input_file, "tmp/rotate_90.mp4", RotateAngle::Degrees90);

    println!("配置: 90° 顺时针旋转");
    match rotate_video(config1) {
        Ok(_) => println!("✓ 旋转完成"),
        Err(e) => println!("❌ 旋转失败: {}", e),
    }

    verify_output("tmp/rotate_90.mp4", "90度旋转", Some((1080, 1920)))?;
    println!();

    // Test 2: Rotate 180 degrees
    println!("【测试2】旋转 180 度");
    println!("=========================================");

    match rotate_180(input_file, "tmp/rotate_180.mp4") {
        Ok(_) => println!("✓ 旋转完成"),
        Err(e) => println!("❌ 旋转失败: {}", e),
    }

    verify_output("tmp/rotate_180.mp4", "180度旋转", Some((1920, 1080)))?;
    println!();

    // Test 3: Rotate 270 degrees
    println!("【测试3】旋转 270 度顺时针 (90度逆时针)");
    println!("=========================================");

    match rotate_video(
        RotateConfig::new(input_file, "tmp/rotate_270.mp4", RotateAngle::Degrees270)
    ) {
        Ok(_) => println!("✓ 旋转完成"),
        Err(e) => println!("❌ 旋转失败: {}", e),
    }

    verify_output("tmp/rotate_270.mp4", "270度旋转", Some((1080, 1920)))?;
    println!();

    // Test 4: Flip horizontal
    println!("【测试4】水平翻转 (镜像左右)");
    println!("=========================================");

    match flip_horizontal(input_file, "tmp/flip_horizontal.mp4") {
        Ok(_) => println!("✓ 翻转完成"),
        Err(e) => println!("❌ 翻转失败: {}", e),
    }

    verify_output("tmp/flip_horizontal.mp4", "水平翻转", Some((1920, 1080)))?;
    println!();

    // Test 5: Flip vertical
    println!("【测试5】垂直翻转 (镜像上下)");
    println!("=========================================");

    match flip_vertical(input_file, "tmp/flip_vertical.mp4") {
        Ok(_) => println!("✓ 翻转完成"),
        Err(e) => println!("❌ 翻转失败: {}", e),
    }

    verify_output("tmp/flip_vertical.mp4", "垂直翻转", Some((1920, 1080)))?;
    println!();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                     测试完成                                      ║");
    println!("╚════════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Verify output file using ffprobe
fn verify_output(
    file: &str,
    test_name: &str,
    expected_size: Option<(u32, u32)>,
) -> Result<(), Box<dyn std::error::Error>> {
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

        // Check expected dimensions
        if let Some((exp_w, exp_h)) = expected_size {
            let width = lines[0].trim().parse::<u32>().unwrap_or(0);
            let height = lines[1].trim().parse::<u32>().unwrap_or(0);
            if width == exp_w && height == exp_h {
                println!("     ✓ 尺寸验证通过 ({}x{})", width, height);
            } else {
                println!("     ⚠️  尺寸不匹配: 预期 {}x{}, 实际 {}x{}", exp_w, exp_h, width, height);
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
