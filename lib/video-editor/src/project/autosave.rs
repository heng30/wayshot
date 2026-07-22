use crate::{Result, project::ProjectFile};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, SystemTime},
};

#[derive(Debug, Clone, derivative::Derivative, derive_setters::Setters)]
#[derivative(Default)]
#[setters(prefix = "with_")]
#[non_exhaustive]
pub struct AutoSaveConfig {
    #[derivative(Default(value = "true"))]
    pub enabled: bool,

    #[derivative(Default(value = "Duration::from_secs(60)"))]
    pub interval: Duration,

    #[derivative(Default(value = "5"))]
    pub max_temp_files: usize,

    #[derivative(Default(value = "PathBuf::from(\".autosaves\")"))]
    pub temp_location: PathBuf,
}

impl AutoSaveConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

// Information about a recovery file found on startup
#[derive(Debug, Clone)]
pub struct RecoveryInfo {
    // Path to the temporary save file
    pub temp_file_path: PathBuf,

    // Original project path if available
    pub original_project_path: Option<PathBuf>,

    // When the temp file was saved
    pub saved_at: SystemTime,

    // Size of the temp file in bytes
    pub file_size: u64,
}

#[derive(Debug)]
pub struct AutoSaveHandle {
    stop_signal: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl AutoSaveHandle {
    pub fn stop(mut self) {
        self.stop_signal.store(true, Ordering::Release);
        if let Some(handle) = self.thread_handle.take() {
            _ = handle.join();
        }
    }

    pub fn is_running(&self) -> bool {
        !self.stop_signal.load(Ordering::Acquire)
    }
}

impl Drop for AutoSaveHandle {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::Release);
    }
}

#[derive(Debug)]
pub struct AutoSaveManager {
    config: AutoSaveConfig,
    #[allow(dead_code)]
    project_path: Option<PathBuf>,
    project_hash: u64,
    dirty: Arc<AtomicBool>,
    last_save: Arc<Mutex<SystemTime>>,
}

impl AutoSaveManager {
    pub fn new(config: AutoSaveConfig, project_path: Option<&Path>) -> Result<Self> {
        fs::create_dir_all(&config.temp_location)?;

        let project_hash = project_path
            .map(|p| {
                let mut hasher = DefaultHasher::new();
                p.hash(&mut hasher);
                hasher.finish()
            })
            .unwrap_or(0);

        Ok(Self {
            config,
            project_path: project_path.map(|p| p.to_path_buf()),
            project_hash,
            dirty: Arc::new(AtomicBool::new(false)),
            last_save: Arc::new(Mutex::new(SystemTime::UNIX_EPOCH)),
        })
    }

    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Release);
    }

    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Release);
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }

    pub fn should_autosave(&self) -> bool {
        if !self.config.enabled || !self.is_dirty() {
            return false;
        }

        let last_save = *self.last_save.lock().unwrap();
        if let Ok(elapsed) = last_save.elapsed() {
            elapsed >= self.config.interval
        } else {
            true
        }
    }

    pub fn save_temp(&mut self, project_file: &ProjectFile) -> Result<PathBuf> {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!(".tmp_{:016x}_{}.json", self.project_hash, timestamp);
        let temp_path = self.config.temp_location.join(filename);

        let json = serde_json::to_string_pretty(project_file)?;
        fs::write(&temp_path, json)?;

        *self.last_save.lock().unwrap() = SystemTime::now();
        self.dirty.store(false, Ordering::Release);

        self.cleanup_old_temp_files()?;

        log::debug!("Auto-saved project to: {}", temp_path.display());

        Ok(temp_path)
    }

    fn cleanup_old_temp_files(&mut self) -> Result<()> {
        let mut temp_files = Self::scan_temp_files(&self.config.temp_location, self.project_hash)?;

        while temp_files.len() > self.config.max_temp_files {
            let old_file = temp_files.remove(0);
            if old_file.exists() {
                _ = fs::remove_file(&old_file);
                log::debug!("Removed old temp file: {}", old_file.display());
            }
        }
        Ok(())
    }

    fn scan_temp_files(dir: &Path, project_hash: u64) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let prefix = format!(".tmp_{:016x}_", project_hash);

        if !dir.exists() {
            return Ok(files);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.starts_with(&prefix) && name_str.ends_with(".json") {
                    files.push(path);
                }
            }
        }

        // Sort by modification time, newest first
        files.sort_by_key(|path| {
            fs::metadata(path)
                .and_then(|m| m.modified())
                .map(|t| std::cmp::Reverse(t))
                .unwrap_or(std::cmp::Reverse(SystemTime::UNIX_EPOCH))
        });

        Ok(files)
    }

    pub fn get_temp_files(&self) -> Result<Vec<PathBuf>> {
        Self::scan_temp_files(&self.config.temp_location, self.project_hash)
    }

    pub fn cleanup_temp_files(&mut self) -> Result<()> {
        let temp_files = Self::scan_temp_files(&self.config.temp_location, self.project_hash)?;

        for file in temp_files {
            if file.exists() {
                fs::remove_file(&file)?;
                log::info!("Cleaned up temp file: {}", file.display());
            }
        }
        Ok(())
    }

    pub fn dirty_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.dirty)
    }

    pub fn last_save_time(&self) -> SystemTime {
        *self.last_save.lock().unwrap()
    }

    pub fn update_save_time(&self) {
        *self.last_save.lock().unwrap() = SystemTime::now();
    }

    pub fn config(&self) -> &AutoSaveConfig {
        &self.config
    }

    pub fn update_config(&mut self, config: AutoSaveConfig) -> Result<()> {
        if config.temp_location != self.config.temp_location {
            fs::create_dir_all(&config.temp_location)?;
        }
        self.config = config;
        Ok(())
    }

    pub fn start_autosave_thread<F>(&self, get_project: F) -> AutoSaveHandle
    where
        F: Fn() -> Option<ProjectFile> + Send + 'static,
    {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let dirty = Arc::clone(&self.dirty);
        let last_save = Arc::clone(&self.last_save);
        let interval = self.config.interval;
        let temp_location = self.config.temp_location.clone();
        let project_hash = self.project_hash;
        let enabled = self.config.enabled;

        let stop_signal_clone = Arc::clone(&stop_signal);
        let handle = std::thread::spawn(move || {
            while !stop_signal_clone.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_secs(1));

                if !enabled || !dirty.load(Ordering::Acquire) {
                    continue;
                }

                let should_save = {
                    let last = *last_save.lock().unwrap();
                    last.elapsed().map(|e| e >= interval).unwrap_or(true)
                };

                if should_save && let Some(project) = get_project() {
                    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                    let filename = format!(".tmp_{:016x}_{}.json", project_hash, timestamp);
                    let temp_path = temp_location.join(filename);

                    match serde_json::to_string_pretty(&project) {
                        Ok(json) => match fs::write(&temp_path, &json) {
                            Ok(_) => {
                                log::info!("Auto-saved to: {}", temp_path.display());
                                dirty.store(false, Ordering::Release);
                                *last_save.lock().unwrap() = SystemTime::now();
                            }
                            Err(e) => log::error!("Failed to write temp file: {}", e),
                        },
                        Err(e) => log::error!("Failed to serialize project: {}", e),
                    }
                }
            }
        });

        AutoSaveHandle {
            stop_signal,
            thread_handle: Some(handle),
        }
    }
}

pub fn check_for_recovery(temp_dir: &Path, project_path: &Path) -> Option<RecoveryInfo> {
    if !temp_dir.exists() {
        return None;
    }

    let project_hash = {
        let mut hasher = DefaultHasher::new();
        project_path.hash(&mut hasher);
        hasher.finish()
    };

    let prefix = format!(".tmp_{:016x}_", project_hash);
    let mut recovery_files: Vec<RecoveryInfo> = Vec::new();

    if let Ok(entries) = fs::read_dir(&temp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.starts_with(&prefix)
                    && name_str.ends_with(".json")
                    && let Ok(metadata) = fs::metadata(&path)
                    && let Ok(modified) = metadata.modified()
                {
                    recovery_files.push(RecoveryInfo {
                        temp_file_path: path,
                        original_project_path: Some(project_path.to_path_buf()),
                        saved_at: modified,
                        file_size: metadata.len(),
                    });
                }
            }
        }
    }

    // Return the most recent one
    recovery_files.sort_by_key(|r| std::cmp::Reverse(r.saved_at));
    recovery_files.into_iter().next()
}

pub fn get_all_recovery_files(temp_dir: &Path) -> Vec<RecoveryInfo> {
    if !temp_dir.exists() {
        return Vec::new();
    }

    let mut recovery_files: Vec<RecoveryInfo> = Vec::new();

    if let Ok(entries) = fs::read_dir(&temp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.starts_with(".tmp_")
                    && name_str.ends_with(".json")
                    && let Ok(metadata) = fs::metadata(&path)
                    && let Ok(modified) = metadata.modified()
                {
                    recovery_files.push(RecoveryInfo {
                        temp_file_path: path,
                        original_project_path: None,
                        saved_at: modified,
                        file_size: metadata.len(),
                    });
                }
            }
        }
    }

    // Sort by modification time, newest first
    recovery_files.sort_by_key(|r| std::cmp::Reverse(r.saved_at));
    recovery_files
}

pub fn restore_from_recovery(recovery_info: &RecoveryInfo) -> Result<ProjectFile> {
    let json = fs::read_to_string(&recovery_info.temp_file_path)?;
    let project: ProjectFile = serde_json::from_str(&json)?;

    log::info!(
        "Restored project from recovery file: {}",
        recovery_info.temp_file_path.display()
    );

    Ok(project)
}

pub fn cleanup_recovery_file(recovery_info: &RecoveryInfo) -> Result<()> {
    if recovery_info.temp_file_path.exists() {
        fs::remove_file(&recovery_info.temp_file_path)?;
        log::info!(
            "Cleaned up recovery file: {}",
            recovery_info.temp_file_path.display()
        );
    }
    Ok(())
}

pub fn check_recovery_on_startup(
    temp_dir: &Path,
    project_path: Option<&Path>,
) -> Option<RecoveryInfo> {
    match project_path {
        Some(path) => check_for_recovery(temp_dir, path),
        None => get_all_recovery_files(temp_dir).into_iter().next(),
    }
}

pub fn cleanup_temp_files_by_path(temp_dir: &Path, project_path: &Path) -> Result<()> {
    let project_hash = {
        let mut hasher = DefaultHasher::new();
        project_path.hash(&mut hasher);
        hasher.finish()
    };

    let prefix = format!(".tmp_{:016x}_", project_hash);

    if !temp_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(temp_dir)?.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();
            if name_str.starts_with(&prefix) && name_str.ends_with(".json") {
                fs::remove_file(&path)?;
                log::info!("Cleaned up temp file by path: {}", path.display());
            }
        }
    }

    Ok(())
}
