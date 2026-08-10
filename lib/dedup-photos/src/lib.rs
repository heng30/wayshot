//! dedup-photos — 照片去重库。
//!
//! 三重检测管线，按优先级分层执行：
//! 1. **SHA-256 精确去重**：字节级完全相同的文件。
//! 2. **感知相似（dHash）**：127-bit 双梯度感知哈希，汉明距离低于阈值的文件。
//! 3. **语义相似（CLIP）**：本地 CLIP 视觉模型嵌入的余弦相似度（可选）。
//!
//! 每组保留一张（见 [`KeepStrategy`]），其余重复图片移动到
//! `<root>/<duplicate_dir_name>/` 下，并生成去重报告。
//!
//! 长任务支持进度回调与协作式取消（见 [`ProgressEvent`] 与 [`CancellationToken`]）。

pub mod hash;
pub mod report;
pub mod scan;
pub mod semantic;

use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

pub use scan::IMAGE_EXTENSIONS;

pub const DEFAULT_DUPLICATE_DIR: &str = "duplicate";
pub const DEFAULT_REPORT_FILE: &str = "dedup_report.txt";
pub const DEFAULT_THRESHOLD: u32 = 10;
pub const DEFAULT_SEMANTIC_THRESHOLD: f32 = 0.85;

#[derive(Debug, thiserror::Error)]
pub enum DedupError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("no files found under {0}")]
    NoFiles(PathBuf),
    #[error("semantic detection unavailable: {0}")]
    Semantic(String),
    #[error("operation cancelled")]
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeepStrategy {
    #[default]
    Largest,
    Newest,
    Oldest,
}

impl std::fmt::Display for KeepStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeepStrategy::Largest => write!(f, "largest"),
            KeepStrategy::Newest => write!(f, "newest"),
            KeepStrategy::Oldest => write!(f, "oldest"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SemanticConfig {
    pub model_path: PathBuf,
    pub threshold: f32,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        SemanticConfig {
            model_path: PathBuf::new(),
            threshold: DEFAULT_SEMANTIC_THRESHOLD,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DedupOptions {
    pub threshold: u32,
    pub semantic: Option<SemanticConfig>,
    pub keep: KeepStrategy,
    pub all_files: bool,
    pub duplicate_dir_name: String,
}

impl Default for DedupOptions {
    fn default() -> Self {
        DedupOptions {
            threshold: DEFAULT_THRESHOLD,
            semantic: None,
            keep: KeepStrategy::Largest,
            all_files: false,
            duplicate_dir_name: DEFAULT_DUPLICATE_DIR.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum DedupReason {
    Exact,
    Perceptual { hamming_distance: u32 },
    Semantic { cosine_similarity: f32 },
}

impl DedupReason {
    pub fn label(&self) -> &'static str {
        match self {
            DedupReason::Exact => "exact duplicate",
            DedupReason::Perceptual { .. } => "perceptual duplicate",
            DedupReason::Semantic { .. } => "semantic duplicate",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            DedupReason::Exact => "(SHA-256 identical)".to_string(),
            DedupReason::Perceptual { hamming_distance } => {
                format!("(Hamming distance {hamming_distance}/{})", hash::DHASH_BITS)
            }
            DedupReason::Semantic { cosine_similarity } => {
                format!("(cosine similarity {cosine_similarity:.3})")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Duplicate {
    pub path: PathBuf,
    pub moved_to: PathBuf,
    pub kept_path: PathBuf,
    pub reason: DedupReason,
}

#[derive(Clone, Debug, Default)]
pub struct DedupSummary {
    pub scanned_files: usize,
    pub groups: usize,
    pub moved_files: usize,
    pub moved_bytes: u64,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DedupResult {
    pub duplicates: Vec<Duplicate>,
    pub summary: DedupSummary,
    pub report_path: PathBuf,
}

/// 检测/处理阶段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Scan,
    Exact,
    Perceptual,
    Semantic,
    Move,
    Report,
}

/// 进度事件。回调可能从 rayon 工作线程并发调用。
#[derive(Clone, Debug)]
pub enum ProgressEvent {
    StageStarted { stage: Stage, total: u64 },
    ItemDone { stage: Stage, done: u64, total: u64 },
    StageFinished { stage: Stage, total: u64 },
}

pub type ProgressFn = dyn Fn(ProgressEvent) + Sync;

/// 协作式取消令牌：跨线程共享，调用 [`CancellationToken::cancel`] 后，
/// 检测点会尽快让 [`dedup_directory_with`] 返回 [`DedupError::Cancelled`]。
#[derive(Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

fn emit(progress: Option<&ProgressFn>, event: ProgressEvent) {
    if let Some(p) = progress {
        p(event);
    }
}

fn check_cancel(cancel: Option<&CancellationToken>) -> Result<(), DedupError> {
    if cancel.is_some_and(|c| c.is_cancelled()) {
        Err(DedupError::Cancelled)
    } else {
        Ok(())
    }
}

/// 便捷入口：不带进度回调与取消令牌。
pub fn dedup_directory(root: &Path, options: &DedupOptions) -> Result<DedupResult, DedupError> {
    dedup_directory_with(root, options, None, None)
}

/// 完整入口。`progress` 为可选的进度回调，`cancel` 为可选的取消令牌。
///
/// 取消是协作式的：正在运行的并行批次会跑完当前文件，随后立即返回
/// [`DedupError::Cancelled`]；已经移动的文件不会回滚。
pub fn dedup_directory_with(
    root: &Path,
    options: &DedupOptions,
    progress: Option<&ProgressFn>,
    cancel: Option<&CancellationToken>,
) -> Result<DedupResult, DedupError> {
    check_cancel(cancel)?;

    if !root.is_dir() {
        return Err(DedupError::NotADirectory(root.to_path_buf()));
    }

    emit(
        progress,
        ProgressEvent::StageStarted {
            stage: Stage::Scan,
            total: 0,
        },
    );
    let files = scan::collect_files(
        root,
        options.all_files,
        std::slice::from_ref(&options.duplicate_dir_name),
        progress,
    );
    emit(
        progress,
        ProgressEvent::StageFinished {
            stage: Stage::Scan,
            total: files.len() as u64,
        },
    );
    check_cancel(cancel)?;

    if files.is_empty() {
        return Err(DedupError::NoFiles(root.to_path_buf()));
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(16)
        .build()
        .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap());

    let mut groups: Vec<RawGroup> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let (exact, assigned1, errs) = detect_exact(&files, &pool, progress, cancel)?;
    groups.extend(exact.into_iter().map(|members| RawGroup {
        members,
        stage: Stage::Exact,
    }));
    let mut assigned = assigned1;
    warnings.extend(errs);

    let (perceptual, assigned2, hashes, errs) = detect_perceptual(
        &files,
        &groups,
        &assigned,
        options.threshold,
        &pool,
        progress,
        cancel,
    )?;
    groups.extend(perceptual.into_iter().map(|members| RawGroup {
        members,
        stage: Stage::Perceptual,
    }));
    assigned.extend(assigned2);
    warnings.extend(errs);

    let mut embs: HashMap<usize, Vec<f32>> = HashMap::new();
    if let Some(sem) = &options.semantic {
        let embedder = semantic::Embedder::new(&sem.model_path).map_err(DedupError::Semantic)?;
        let (sem_groups, assigned3, embeddings, errs) = detect_semantic(
            &files,
            &groups,
            &assigned,
            &embedder,
            sem.threshold,
            &pool,
            progress,
            cancel,
        )?;
        groups.extend(sem_groups.into_iter().map(|members| RawGroup {
            members,
            stage: Stage::Semantic,
        }));
        assigned.extend(assigned3);
        embs = embeddings;
        warnings.extend(errs);
    }

    let to_move = build_duplicates(&files, &groups, options.keep, &hashes, &embs);

    let dup_dir = root.join(&options.duplicate_dir_name);
    let total = to_move.len() as u64;
    emit(
        progress,
        ProgressEvent::StageStarted {
            stage: Stage::Move,
            total,
        },
    );
    fs::create_dir_all(&dup_dir)?;

    let mut duplicates = Vec::new();
    let mut moved_bytes = 0u64;
    let mut done = 0u64;
    for (m, reason, keep) in to_move {
        check_cancel(cancel)?;
        let target = unique_dest(&dup_dir, &files[m].path);
        match move_file(&files[m].path, &target) {
            Ok(()) => {
                moved_bytes += files[m].size;
                duplicates.push(Duplicate {
                    path: files[m].path.clone(),
                    moved_to: target,
                    kept_path: files[keep].path.clone(),
                    reason,
                });
            }
            Err(e) => warnings.push(format!("failed to move {}: {}", files[m].path.display(), e)),
        }
        done += 1;
        emit(
            progress,
            ProgressEvent::ItemDone {
                stage: Stage::Move,
                done,
                total,
            },
        );
    }
    emit(
        progress,
        ProgressEvent::StageFinished {
            stage: Stage::Move,
            total: done,
        },
    );

    let summary = DedupSummary {
        scanned_files: files.len(),
        groups: groups.len(),
        moved_files: duplicates.len(),
        moved_bytes,
        warnings,
    };

    emit(
        progress,
        ProgressEvent::StageStarted {
            stage: Stage::Report,
            total: 0,
        },
    );
    let report_path = dup_dir.join(DEFAULT_REPORT_FILE);
    fs::write(
        &report_path,
        report::render(root, options, &duplicates, &summary),
    )?;
    emit(
        progress,
        ProgressEvent::StageFinished {
            stage: Stage::Report,
            total: 0,
        },
    );

    Ok(DedupResult {
        duplicates,
        summary,
        report_path,
    })
}

#[derive(Clone, Debug)]
struct RawGroup {
    members: Vec<usize>,
    stage: Stage,
}

type ExactOut = (Vec<Vec<usize>>, HashSet<usize>, Vec<String>);
type PerceptualOut = (
    Vec<Vec<usize>>,
    HashSet<usize>,
    HashMap<usize, u128>,
    Vec<String>,
);
type SemanticOut = (
    Vec<Vec<usize>>,
    HashSet<usize>,
    HashMap<usize, Vec<f32>>,
    Vec<String>,
);
type HashOut = (Vec<(usize, u128)>, usize);
type EmbedOut = (Vec<(usize, Vec<f32>)>, usize);

fn detect_exact(
    files: &[scan::FileEntry],
    pool: &rayon::ThreadPool,
    progress: Option<&ProgressFn>,
    cancel: Option<&CancellationToken>,
) -> Result<ExactOut, DedupError> {
    check_cancel(cancel)?;

    let mut size_groups: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        size_groups.entry(f.size).or_default().push(i);
    }
    let candidates: Vec<Vec<usize>> = size_groups.into_values().filter(|v| v.len() > 1).collect();
    let total = candidates.iter().map(|v| v.len()).sum::<usize>() as u64;

    emit(
        progress,
        ProgressEvent::StageStarted {
            stage: Stage::Exact,
            total,
        },
    );

    let counter = AtomicUsize::new(0);
    let results: Vec<(usize, Option<String>)> = pool.install(|| {
        candidates
            .par_iter()
            .flatten()
            .map(|&i| {
                let cancelled = cancel.is_some_and(|c| c.is_cancelled());
                let h = if cancelled {
                    None
                } else {
                    hash::sha256_file(&files[i].path).ok()
                };
                let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
                emit(
                    progress,
                    ProgressEvent::ItemDone {
                        stage: Stage::Exact,
                        done: done as u64,
                        total,
                    },
                );
                (i, h)
            })
            .collect()
    });
    check_cancel(cancel)?;
    emit(
        progress,
        ProgressEvent::StageFinished {
            stage: Stage::Exact,
            total,
        },
    );

    let mut hash_groups: HashMap<String, Vec<usize>> = HashMap::new();
    let mut errors = 0usize;
    for (i, h) in results {
        match h {
            Some(h) => hash_groups.entry(h).or_default().push(i),
            None => errors += 1,
        }
    }

    let mut groups = Vec::new();
    let mut assigned = HashSet::new();
    for g in hash_groups.into_values() {
        if g.len() > 1 {
            assigned.extend(g.iter().copied());
            groups.push(g);
        }
    }

    let warnings = if errors > 0 {
        vec![format!(
            "{errors} file(s) skipped in exact detection (hash failed)"
        )]
    } else {
        Vec::new()
    };
    Ok((groups, assigned, warnings))
}

fn detect_perceptual(
    files: &[scan::FileEntry],
    existing: &[RawGroup],
    assigned: &HashSet<usize>,
    threshold: u32,
    pool: &rayon::ThreadPool,
    progress: Option<&ProgressFn>,
    cancel: Option<&CancellationToken>,
) -> Result<PerceptualOut, DedupError> {
    check_cancel(cancel)?;

    let mut cand: Vec<usize> = (0..files.len()).filter(|i| !assigned.contains(i)).collect();
    cand.extend(existing.iter().map(|g| g.members[0]));
    cand.sort_unstable();
    cand.dedup();
    let total = cand.len() as u64;

    emit(
        progress,
        ProgressEvent::StageStarted {
            stage: Stage::Perceptual,
            total,
        },
    );

    let (pairs, errors) = compute_hashes(files, &cand, pool, progress, cancel, Stage::Perceptual)?;
    let map: HashMap<usize, u128> = pairs.into_iter().collect();

    let mut in_group: HashSet<usize> = assigned.clone();
    let mut groups = Vec::new();
    let mut new_assigned = HashSet::new();
    for &i in &cand {
        check_cancel(cancel)?;
        if in_group.contains(&i) {
            continue;
        }
        let Some(h1) = map.get(&i) else { continue };
        let mut g = vec![i];
        in_group.insert(i);
        for &j in &cand {
            if in_group.contains(&j) {
                continue;
            }
            let Some(h2) = map.get(&j) else { continue };
            if hash::hamming_distance(*h1, *h2) <= threshold {
                g.push(j);
                in_group.insert(j);
            }
        }
        if g.len() > 1 {
            new_assigned.extend(g.iter().copied());
            groups.push(g);
        }
    }
    emit(
        progress,
        ProgressEvent::StageFinished {
            stage: Stage::Perceptual,
            total,
        },
    );

    let warnings = if errors > 0 {
        vec![format!(
            "{errors} file(s) skipped in perceptual detection (dHash failed)"
        )]
    } else {
        Vec::new()
    };
    Ok((groups, new_assigned, map, warnings))
}

#[allow(clippy::too_many_arguments)]
fn detect_semantic(
    files: &[scan::FileEntry],
    existing: &[RawGroup],
    assigned: &HashSet<usize>,
    embedder: &semantic::Embedder,
    threshold: f32,
    pool: &rayon::ThreadPool,
    progress: Option<&ProgressFn>,
    cancel: Option<&CancellationToken>,
) -> Result<SemanticOut, DedupError> {
    check_cancel(cancel)?;

    let mut cand: Vec<usize> = (0..files.len()).filter(|i| !assigned.contains(i)).collect();
    cand.extend(existing.iter().map(|g| g.members[0]));
    cand.sort_unstable();
    cand.dedup();
    let total = cand.len() as u64;

    emit(
        progress,
        ProgressEvent::StageStarted {
            stage: Stage::Semantic,
            total,
        },
    );

    let (pairs, errors) = compute_embeddings(files, &cand, embedder, pool, progress, cancel)?;
    let map: HashMap<usize, Vec<f32>> = pairs.into_iter().collect();

    let mut in_group: HashSet<usize> = assigned.clone();
    let mut groups = Vec::new();
    let mut new_assigned = HashSet::new();
    for &i in &cand {
        check_cancel(cancel)?;
        if in_group.contains(&i) {
            continue;
        }
        let Some(e1) = map.get(&i) else { continue };
        let mut g = vec![i];
        in_group.insert(i);
        for &j in &cand {
            if in_group.contains(&j) {
                continue;
            }
            let Some(e2) = map.get(&j) else { continue };
            if semantic::cosine(e1, e2) >= threshold {
                g.push(j);
                in_group.insert(j);
            }
        }
        if g.len() > 1 {
            new_assigned.extend(g.iter().copied());
            groups.push(g);
        }
    }
    emit(
        progress,
        ProgressEvent::StageFinished {
            stage: Stage::Semantic,
            total,
        },
    );

    let warnings = if errors > 0 {
        vec![format!(
            "{errors} file(s) skipped in semantic detection (embedding failed)"
        )]
    } else {
        Vec::new()
    };
    Ok((groups, new_assigned, map, warnings))
}

fn compute_hashes(
    files: &[scan::FileEntry],
    indices: &[usize],
    pool: &rayon::ThreadPool,
    progress: Option<&ProgressFn>,
    cancel: Option<&CancellationToken>,
    stage: Stage,
) -> Result<HashOut, DedupError> {
    let total = indices.len() as u64;
    let counter = AtomicUsize::new(0);
    let results: Vec<Option<(usize, u128)>> = pool.install(|| {
        indices
            .par_iter()
            .map(|&i| {
                if cancel.is_some_and(|c| c.is_cancelled()) {
                    return None;
                }
                let h = hash::dhash_from_file(&files[i].path);
                let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
                emit(
                    progress,
                    ProgressEvent::ItemDone {
                        stage,
                        done: done as u64,
                        total,
                    },
                );
                h.map(|h| (i, h))
            })
            .collect()
    });
    check_cancel(cancel)?;
    let mut pairs = Vec::new();
    let mut errors = 0usize;
    for r in results {
        match r {
            Some(p) => pairs.push(p),
            None => errors += 1,
        }
    }
    Ok((pairs, errors))
}

fn compute_embeddings(
    files: &[scan::FileEntry],
    indices: &[usize],
    embedder: &semantic::Embedder,
    pool: &rayon::ThreadPool,
    progress: Option<&ProgressFn>,
    cancel: Option<&CancellationToken>,
) -> Result<EmbedOut, DedupError> {
    let total = indices.len() as u64;
    let counter = AtomicUsize::new(0);
    let results: Vec<Option<(usize, Vec<f32>)>> = pool.install(|| {
        indices
            .par_iter()
            .map(|&i| {
                if cancel.is_some_and(|c| c.is_cancelled()) {
                    return None;
                }
                let emb = std::fs::read(&files[i].path)
                    .ok()
                    .and_then(|b| embedder.embed(&b));
                let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
                emit(
                    progress,
                    ProgressEvent::ItemDone {
                        stage: Stage::Semantic,
                        done: done as u64,
                        total,
                    },
                );
                emb.map(|e| (i, e))
            })
            .collect()
    });
    check_cancel(cancel)?;
    let mut pairs = Vec::new();
    let mut errors = 0usize;
    for r in results {
        match r {
            Some(p) => pairs.push(p),
            None => errors += 1,
        }
    }
    Ok((pairs, errors))
}

fn build_duplicates(
    files: &[scan::FileEntry],
    groups: &[RawGroup],
    keep: KeepStrategy,
    hashes: &HashMap<usize, u128>,
    embs: &HashMap<usize, Vec<f32>>,
) -> Vec<(usize, DedupReason, usize)> {
    let mut out = Vec::new();
    for g in groups {
        if g.members.len() < 2 {
            continue;
        }
        let mut members = g.members.clone();
        match keep {
            KeepStrategy::Newest => members.sort_by(|&a, &b| files[b].mtime.cmp(&files[a].mtime)),
            KeepStrategy::Oldest => members.sort_by(|&a, &b| files[a].mtime.cmp(&files[b].mtime)),
            KeepStrategy::Largest => members.sort_by(|&a, &b| files[b].size.cmp(&files[a].size)),
        }
        let keep_idx = members[0];
        for &m in members.iter().skip(1) {
            let reason = match g.stage {
                Stage::Exact => DedupReason::Exact,
                Stage::Perceptual => {
                    let d = hashes
                        .get(&keep_idx)
                        .and_then(|hk| hashes.get(&m).map(|hm| hash::hamming_distance(*hk, *hm)))
                        .unwrap_or(0);
                    DedupReason::Perceptual {
                        hamming_distance: d,
                    }
                }
                Stage::Semantic => {
                    let c = embs
                        .get(&keep_idx)
                        .and_then(|ek| embs.get(&m).map(|em| semantic::cosine(ek, em)))
                        .unwrap_or(0.0);
                    DedupReason::Semantic {
                        cosine_similarity: c,
                    }
                }
                Stage::Scan | Stage::Move | Stage::Report => unreachable!("not a detection group"),
            };
            out.push((m, reason, keep_idx));
        }
    }
    out
}

fn unique_dest(dir: &Path, src: &Path) -> PathBuf {
    let base = src
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut candidate = dir.join(&base);
    if !candidate.exists() {
        return candidate;
    }
    let mut n = 1u32;
    loop {
        let named = match (src.file_stem(), src.extension()) {
            (Some(s), Some(e)) => format!("{}_{}.{}", s.to_string_lossy(), n, e.to_string_lossy()),
            (Some(s), None) => format!("{}_{}", s.to_string_lossy(), n),
            _ => format!("file_{n}"),
        };
        candidate = dir.join(named);
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

fn move_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    fs::copy(src, dst)?;
    fs::remove_file(src)
}
