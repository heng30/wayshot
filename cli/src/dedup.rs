use anyhow::{Result, anyhow};
use clap::{Subcommand, ValueEnum};
use dedup_photos::{DedupOptions, KeepStrategy, SemanticConfig, dedup_directory};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, ValueEnum)]
pub enum Keep {
    Largest,
    Newest,
    Oldest,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// 去重目录下的图片：完全重复（SHA-256）+ 感知重复（dHash），可选语义去重
    Run {
        /// 待扫描目录
        dir: PathBuf,
        /// dHash 汉明距离阈值，越小越严格
        #[arg(long, default_value_t = 10)]
        threshold: u32,
        /// 同组保留哪个文件
        #[arg(long, value_enum, default_value_t = Keep::Largest)]
        keep: Keep,
        /// 不按图片扩展名过滤，比较目录下所有文件
        #[arg(long)]
        all_files: bool,
        /// 重复文件移入的子目录名
        #[arg(long, default_value = "duplicate")]
        duplicate_dir: String,
        /// CLIP 语义去重模型路径（可选）
        #[arg(long)]
        model: Option<PathBuf>,
        #[arg(long, default_value_t = 0.85)]
        semantic_threshold: f32,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Run {
            dir,
            threshold,
            keep,
            all_files,
            duplicate_dir,
            model,
            semantic_threshold,
        } => dedup(
            dir,
            *threshold,
            *keep,
            *all_files,
            duplicate_dir,
            model.as_deref(),
            *semantic_threshold,
        ),
    }
}

fn dedup(
    dir: &Path,
    threshold: u32,
    keep: Keep,
    all_files: bool,
    duplicate_dir: &str,
    model: Option<&Path>,
    semantic_threshold: f32,
) -> Result<Value> {
    let mut options = DedupOptions {
        threshold,
        keep: match keep {
            Keep::Largest => KeepStrategy::Largest,
            Keep::Newest => KeepStrategy::Newest,
            Keep::Oldest => KeepStrategy::Oldest,
        },
        all_files,
        duplicate_dir_name: duplicate_dir.to_string(),
        semantic: None,
    };

    if let Some(model) = model {
        options.semantic = Some(SemanticConfig {
            model_path: model.to_path_buf(),
            threshold: semantic_threshold,
        });
    }

    let result = dedup_directory(dir, &options).map_err(|e| anyhow!("去重失败: {e}"))?;

    let duplicates: Vec<Value> = result
        .duplicates
        .iter()
        .map(|d| {
            json!({
                "path": d.path.display().to_string(),
                "moved_to": d.moved_to.display().to_string(),
                "kept": d.kept_path.display().to_string(),
                "reason": d.reason.label(),
                "detail": d.reason.detail(),
            })
        })
        .collect();

    Ok(json!({
        "scanned_files": result.summary.scanned_files,
        "groups": result.summary.groups,
        "moved_files": result.summary.moved_files,
        "moved_bytes": result.summary.moved_bytes,
        "warnings": result.summary.warnings,
        "report": result.report_path.display().to_string(),
        "duplicates": duplicates,
    }))
}
