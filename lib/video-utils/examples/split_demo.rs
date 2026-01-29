//! Video split example
//!
//! This example demonstrates splitting videos into multiple segments.

use std::path::Path;
use video_utils::editor::split::{split_video, SplitConfig, split_equal, split_by_duration};
use video_utils::metadata::get_metadata;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              视频分割功能测试                                        ║");
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

    // Test 1: Split at specific points
    println!("【测试1】在指定时间点分割");
    println!("=========================================");

    let config1 = SplitConfig::new(
        input_file,
        "tmp/split_points",
        vec![1.5, 3.0, 4.5],
    )
    .with_name_pattern("segment_{index}_{start}s-{end}s.mp4");

    println!("分割点: 1.5s, 3.0s, 4.5s");
    println!("预期: 4个片段 (0-1.5s, 1.5-3.0s, 3.0-4.5s, 4.5s-结束)");

    match split_video(config1) {
        Ok(files) => {
            println!("✓ 分割完成，创建了 {} 个片段", files.len());
            for (idx, file) in files.iter().enumerate() {
                verify_output(file, &format!("片段{}", idx + 1))?;
            }
        },
        Err(e) => println!("❌ 分割失败: {}", e),
    }
    println!();

    // Test 2: Split into equal parts
    println!("【测试2】等分成3段");
    println!("=========================================");

    println!("分段数: 3");
    println!("预期: 每段约 {:.1} 秒", metadata.duration / 3.0);

    match split_equal(input_file, "tmp/split_equal", 3) {
        Ok(files) => {
            println!("✓ 分割完成，创建了 {} 个片段", files.len());
            for (idx, file) in files.iter().enumerate() {
                verify_output(file, &format!("等分段{}", idx + 1))?;
            }
        },
        Err(e) => println!("❌ 分割失败: {}", e),
    }
    println!();

    // Test 3: Split by duration
    println!("【测试3】按固定时长分割 (每段1.5秒)");
    println!("=========================================");

    let segment_duration = 1.5;
    let expected_count = (metadata.duration / segment_duration).ceil() as usize;

    println!("每段时长: {} 秒", segment_duration);
    println!("预期: {} 个片段", expected_count);

    match split_by_duration(input_file, "tmp/split_duration", segment_duration) {
        Ok(files) => {
            println!("✓ 分割完成，创建了 {} 个片段", files.len());
            for (idx, file) in files.iter().enumerate() {
                verify_output(file, &format!("时长分段{}", idx + 1))?;
            }
        },
        Err(e) => println!("❌ 分割失败: {}", e),
    }
    println!();

    // Test 4: Split with concat list generation
    println!("【测试4】分割并生成合并列表");
    println!("=========================================");

    let config4 = SplitConfig::new(
        input_file,
        "tmp/split_with_list",
        vec![2.0, 4.0],
    )
    .with_name_pattern("part_{index}.mp4")
    .with_concat_list(true);

    println!("分割点: 2.0s, 4.0s");
    println!("生成 concat_list.txt 用于重新合并");

    match split_video(config4) {
        Ok(files) => {
            println!("✓ 分割完成，创建了 {} 个片段", files.len());

            // Check if concat list was created
            let concat_list = Path::new("tmp/split_with_list/concat_list.txt");
            if concat_list.exists() {
                println!("✓ 合并列表已创建: concat_list.txt");

                // Show concat list content
                if let Ok(content) = std::fs::read_to_string(&concat_list) {
                    let lines: Vec<&str> = content.lines().collect();
                    println!("  内容 (共 {} 行):", lines.len());
                    for line in lines.iter().take(3) {
                        println!("    {}", line);
                    }
                    if lines.len() > 3 {
                        println!("    ...");
                    }
                }
            }

            for (idx, file) in files.iter().enumerate() {
                verify_output(file, &format!("带列表片段{}", idx + 1))?;
            }
        },
        Err(e) => println!("❌ 分割失败: {}", e),
    }
    println!();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                     测试完成                                      ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();
    println!("💡 提示: 可以使用 concat_list.txt 重新合并片段:");
    println!("   ffmpeg -f concat -i concat_list.txt -c copy merged.mp4");

    Ok(())
}

/// Verify output file using ffprobe
fn verify_output(file: &str, test_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(file).exists() {
        println!("  ⚠️  输出文件不存在: {}", file);
        return Ok(());
    }

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

        if lines.len() >= 3 {
            println!("  ✅ {} 验证通过:", test_name);
            println!("     分辨率: {}x{}", lines[0].trim(), lines[1].trim());
            println!("     时长: {} 秒", lines[2].trim());

            // Get file size
            if let Ok(metadata) = std::fs::metadata(file) {
                let size_kb = metadata.len() / 1024;
                println!("     大小: {} KB", size_kb);
            }
        }
    } else {
        println!("  ⚠️  ffprobe 验证失败");
    }

    Ok(())
}
