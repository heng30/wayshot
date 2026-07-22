use super::MediaList;
use crate::Result;
use stacksafe::stacksafe;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

#[derive(Debug, Clone, derivative::Derivative, derive_setters::Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct ImportOptions {
    #[derivative(Default(value = "true"))]
    pub import_thumbnails: bool,

    #[derivative(Default(value = "true"))]
    pub extract_metadata: bool,

    pub create_subfolders: bool,
    pub copy_files: bool,

    #[derivative(Default(value = "true"))]
    pub import_recursive: bool,

    pub supported_extensions: Vec<String>,
}

impl ImportOptions {
    pub fn new() -> Self {
        let mut opt = Self::default();

        opt.supported_extensions = vec![
            // Video containers (FFmpeg-supported)
            "mp4".to_string(),
            "mov".to_string(),
            "avi".to_string(),
            "mkv".to_string(),
            "webm".to_string(),
            "flv".to_string(),
            "ts".to_string(),
            "m2ts".to_string(),
            "wmv".to_string(),
            "asf".to_string(),
            "3gp".to_string(),
            "ogv".to_string(),
            "m4v".to_string(),
            "mpeg".to_string(),
            "mpg".to_string(),
            "mxf".to_string(),
            // Audio formats
            "mp3".to_string(),
            "wav".to_string(),
            "aac".to_string(),
            "flac".to_string(),
            "opus".to_string(),
            "ogg".to_string(),
            "m4a".to_string(),
            "wma".to_string(),
            "ape".to_string(),
            "aiff".to_string(),
            // Image formats
            "jpg".to_string(),
            "png".to_string(),
            "gif".to_string(),
            "svg".to_string(),
            // Subtitle formats
            "srt".to_string(),
            "vtt".to_string(),
        ];

        opt
    }

    pub fn is_supported(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                self.supported_extensions
                    .iter()
                    .any(|supported| supported.eq_ignore_ascii_case(ext))
            })
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, derivative::Derivative)]
#[derivative(Default)]
#[non_exhaustive]
pub struct ImportProgress {
    pub total_files: usize,
    pub imported_files: usize,
    pub failed_files: usize,
    pub current_file: Option<PathBuf>,

    #[derivative(Default(value = "SystemTime::now()"))]
    pub started_at: SystemTime,
}

impl ImportProgress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn progress(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            (self.imported_files + self.failed_files) as f64 / self.total_files as f64
        }
    }

    pub fn is_complete(&self) -> bool {
        self.imported_files + self.failed_files >= self.total_files
    }

    pub fn elapsed(&self) -> Option<std::time::Duration> {
        self.started_at.elapsed().ok()
    }
}

#[derive(Debug, Clone)]
pub struct ImportResult {
    pub file_path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
    pub item_id: Option<String>,
}

impl ImportResult {
    pub fn success(file_path: PathBuf, item_id: String) -> Self {
        Self {
            file_path,
            success: true,
            error: None,
            item_id: Some(item_id),
        }
    }

    pub fn failure(file_path: PathBuf, error: String) -> Self {
        Self {
            file_path,
            success: false,
            error: Some(error),
            item_id: None,
        }
    }
}

#[derive(Debug)]
pub struct MediaImporter {
    options: ImportOptions,
    progress: Arc<Mutex<ImportProgress>>,
    results: VecDeque<ImportResult>,
}

impl MediaImporter {
    pub fn new(options: ImportOptions) -> Self {
        Self {
            options,
            progress: Arc::new(Mutex::new(ImportProgress::new())),
            results: VecDeque::new(),
        }
    }

    pub fn progress(&self) -> &Arc<Mutex<ImportProgress>> {
        &self.progress
    }

    pub fn import_files<P: AsRef<Path>>(
        &mut self,
        files: Vec<P>,
        library: &mut MediaList,
    ) -> Result<Vec<ImportResult>> {
        let mut results = Vec::new();

        self.progress.lock().unwrap().total_files = files.len();

        for file_path in files {
            let result = self.import_file(file_path.as_ref(), library)?;
            let success = result.success;

            results.push(result);

            {
                let mut progress = self.progress.lock().unwrap();
                if success {
                    progress.imported_files += 1;
                } else {
                    progress.failed_files += 1;
                }
                progress.current_file = Some(file_path.as_ref().to_path_buf());
            }
        }

        Ok(results)
    }

    pub fn import_directory<P: AsRef<Path>>(
        &mut self,
        dir: P,
        library: &mut MediaList,
    ) -> Result<Vec<ImportResult>> {
        let mut files = Vec::new();
        self.collect_files(dir.as_ref(), &mut files)?;
        self.import_files(files, library)
    }

    #[stacksafe]
    fn collect_files(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if !dir.is_dir() {
            return Err(crate::Error::InvalidFile(format!(
                "Path is not a directory: {}",
                dir.display()
            )));
        }

        let entries = std::fs::read_dir(dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if self.options.import_recursive {
                    self.collect_files(&path, files)?;
                }
            } else if self.options.is_supported(&path) {
                files.push(path);
            }
        }

        Ok(())
    }

    fn import_file(&mut self, file_path: &Path, library: &mut MediaList) -> Result<ImportResult> {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        let item_id = library.add_file_with_name(file_path.to_path_buf(), Some(file_name), None)?;

        Ok(ImportResult::success(file_path.to_path_buf(), item_id))
    }

    pub fn results(&self) -> Vec<ImportResult> {
        self.results.iter().cloned().collect()
    }
}
