use anyhow::{Result, anyhow};
use clap::Subcommand;
use serde_json::{Value, json};
use similar_video_segment::{SimilarVideoConfig, export_segments, scan_videos};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Subcommand)]
pub enum Cmd {
    /// 在一批视频里查找与参考图相似的片段
    Scan {
        /// 参考图片
        #[arg(long)]
        query: PathBuf,
        /// 待扫描的视频
        videos: Vec<PathBuf>,
        /// 余弦相似度阈值
        #[arg(long, default_value_t = 0.75)]
        threshold: f32,
        /// 每隔多少帧采样一次
        #[arg(long, default_value_t = 10)]
        sample_interval: u32,
        /// 合并相邻匹配的时间间隔（秒）
        #[arg(long, default_value_t = 5.0)]
        merge_gap: f64,
        /// 导出片段到该目录（不给则只输出匹配结果）
        #[arg(long)]
        export_dir: Option<PathBuf>,
        /// 导出片段保留匹配点之前的秒数
        #[arg(long, default_value_t = 5.0)]
        before: f64,
        /// 导出片段保留匹配点之后的秒数
        #[arg(long, default_value_t = 5.0)]
        after: f64,
        /// 导出时去掉音轨
        #[arg(long)]
        no_audio: bool,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Scan {
            query,
            videos,
            threshold,
            sample_interval,
            merge_gap,
            export_dir,
            before,
            after,
            no_audio,
        } => scan(
            query,
            videos,
            *threshold,
            *sample_interval,
            *merge_gap,
            export_dir.as_deref(),
            *before,
            *after,
            !no_audio,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn scan(
    query: &Path,
    videos: &[PathBuf],
    threshold: f32,
    sample_interval: u32,
    merge_gap: f64,
    export_dir: Option<&Path>,
    before: f64,
    after: f64,
    keep_audio: bool,
) -> Result<Value> {
    let config = SimilarVideoConfig {
        sample_interval: sample_interval.max(1),
        similarity_threshold: threshold,
        merge_gap_duration: Duration::from_secs_f64(merge_gap.max(0.0)),
        before_duration: Duration::from_secs_f64(before.max(0.0)),
        after_duration: Duration::from_secs_f64(after.max(0.0)),
        output_dir: export_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".")),
    };

    let matches =
        scan_videos(query, videos, &config, None, |_| {}).map_err(|e| anyhow!("扫描失败: {e}"))?;

    let items: Vec<Value> = matches
        .iter()
        .map(|m| {
            json!({
                "video": m.video_path.display().to_string(),
                "match_time_secs": m.match_time.as_secs_f64(),
                "similarity": m.similarity,
                "frame": m.frame_number,
            })
        })
        .collect();

    let mut exported = Vec::new();
    if export_dir.is_some() && !matches.is_empty() {
        let paths = export_segments(&matches, &config, None, |_| {}, keep_audio)
            .map_err(|e| anyhow!("导出片段失败: {e}"))?;
        exported = paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>();
    }

    Ok(json!({
        "count": items.len(),
        "matches": items,
        "exported": exported,
    }))
}
