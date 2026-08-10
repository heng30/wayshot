use crate::{ProgressEvent, ProgressFn, Stage};
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use walkdir::WalkDir;

pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "heic", "heif", "gif", "bmp", "tiff", "tif", "webp", "raw", "cr2", "cr3",
    "nef", "arw", "dng", "orf", "rw2", "pef",
];

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    pub mtime: u64,
    pub ext: String,
}

pub fn collect_files(
    root: &Path,
    all_files: bool,
    skip_dir_names: &[String],
    progress: Option<&ProgressFn>,
) -> Vec<FileEntry> {
    let mut files = Vec::new();

    let walker = WalkDir::new(root).follow_links(false).into_iter();
    let filtered = walker.filter_entry(|e| {
        if e.depth() > 0 && e.file_type().is_dir() {
            let name = e.file_name().to_string_lossy();
            return !skip_dir_names.iter().any(|d| name == *d);
        }
        true
    });

    let mut done = 0u64;
    for entry in filtered
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path().to_owned();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if !all_files && !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        if let Ok(meta) = fs::metadata(&path) {
            let size = meta.len();
            if size == 0 {
                continue;
            }
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            files.push(FileEntry {
                path,
                size,
                mtime,
                ext,
            });
            done += 1;
            if let Some(p) = progress {
                p(ProgressEvent::ItemDone {
                    stage: Stage::Scan,
                    done,
                    total: 0,
                });
            }
        }
    }

    files
}
