//! 相似画面查找与导出示例。
//!
//! 支持指定查询图片和视频文件，在视频中查找相似画面，
//! 并将匹配点前后的视频片段导出到指定目录。
//!
//! 用法:
//!   cargo run --example find_similar -- -q test.png -v test.mp4
//!   cargo run --example find_similar -- -q test.png -v a.mp4 -v b.mp4 --keep-audio
//!   cargo run --example find_similar -- -q test.png -v test.mp4 -o tmp/output --threshold 0.5

use similar_video_segment::{
    CancellationToken, ExportProgress, ScanProgress, SimilarVideoConfig, export_segments, scan_videos,
};
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

/// 相似画面查找与导出工具
#[derive(Parser, Debug)]
#[command(name = "find_similar", about = "在视频中查找与查询图片相似的画面并导出片段")]
struct Args {
    /// 查询图片路径
    #[arg(short = 'q', long = "query")]
    query_image: PathBuf,

    /// 视频文件路径（可指定多个）
    #[arg(short = 'v', long = "video")]
    video_paths: Vec<PathBuf>,

    /// 输出目录
    #[arg(short = 'o', long = "output", default_value = "tmp/output")]
    output_dir: PathBuf,

    /// 相似度阈值（0.0~1.0，越低匹配越多）
    #[arg(long = "threshold", default_value_t = 0.5)]
    similarity_threshold: f32,

    /// 采样间隔（每 N 帧采样一次）
    #[arg(long = "sample-interval", default_value_t = 10)]
    sample_interval: u32,

    /// 匹配点前保留时长（秒）
    #[arg(long = "before", default_value_t = 2)]
    before_duration: u64,

    /// 匹配点后保留时长（秒）
    #[arg(long = "after", default_value_t = 2)]
    after_duration: u64,

    /// 连续匹配帧合并间隔（秒）
    #[arg(long = "merge-gap", default_value_t = 5)]
    merge_gap_duration: u64,

    /// 导出时保留原始音频
    #[arg(long = "keep-audio", default_value_t = false)]
    keep_audio: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args = Args::parse();

    // 检查输入文件是否存在
    if !args.query_image.exists() {
        eprintln!("查询图片不存在: {}", args.query_image.display());
        return Ok(());
    }
    if args.video_paths.is_empty() {
        eprintln!("请至少指定一个视频文件（-v <path>）");
        return Ok(());
    }
    for v in &args.video_paths {
        if !v.exists() {
            eprintln!("视频文件不存在: {}", v.display());
            return Ok(());
        }
    }

    println!("=== 相似画面查找 ===");
    println!("查询图片: {}", args.query_image.display());
    println!("视频列表: {:?}", args.video_paths);
    println!("保留音频: {}", args.keep_audio);
    println!();

    // 配置
    let config = SimilarVideoConfig {
        sample_interval: args.sample_interval,
        similarity_threshold: args.similarity_threshold,
        merge_gap_duration: Duration::from_secs(args.merge_gap_duration),
        before_duration: Duration::from_secs(args.before_duration),
        after_duration: Duration::from_secs(args.after_duration),
        output_dir: args.output_dir.clone(),
    };

    // 创建取消令牌（可用于 Ctrl+C 等场景）
    let cancellation_token = CancellationToken::new();

    // 第一步：扫描视频，查找相似画面
    println!("--- 扫描视频中 ---");
    let matches = scan_videos(
        &args.query_image,
        &args.video_paths,
        &config,
        Some(cancellation_token.clone()),
        |progress: ScanProgress| {
            print!(
                "\r  视频 {}/{} | 帧进度: {}/{} ({:.1}%) | 最佳相似度: {:.3}",
                progress.video_index + 1,
                progress.total_videos,
                progress.frames_processed,
                progress.total_frames,
                progress.fraction() * 100.0,
                progress.best_similarity,
            );
            use std::io::Write;
            std::io::stdout().flush().ok();
        },
    )?;

    println!(); // 换行

    if matches.is_empty() {
        println!(
            "\n未找到相似画面。可尝试降低 similarity_threshold（当前: {:.2}）",
            config.similarity_threshold
        );
        return Ok(());
    }

    println!("\n找到 {} 个匹配:", matches.len());
    for (i, m) in matches.iter().enumerate() {
        println!(
            "  #{} | 时间: {:.3}s | 帧号: {} | 相似度: {:.4} | 视频: {}",
            i + 1,
            m.match_time.as_secs_f64(),
            m.frame_number,
            m.similarity,
            m.video_path.display(),
        );
    }

    // 第二步：导出匹配点前后的视频片段
    println!("\n--- 导出视频片段 ---");
    let exported = export_segments(
        &matches,
        &config,
        None,
        |progress: ExportProgress| {
            print!(
                "\r  片段 {}/{} | 帧进度: {:.1}%",
                progress.segment_index + 1,
                progress.total_segments,
                progress.fraction() * 100.0,
            );
            use std::io::Write;
            std::io::stdout().flush().ok();
        },
        args.keep_audio,
    )?;

    println!(); // 换行

    if exported.is_empty() {
        println!("没有成功导出的片段");
    } else {
        println!("\n导出完成！共 {} 个片段:", exported.len());
        for path in &exported {
            println!("  {}", path.display());
        }
    }

    Ok(())
}
