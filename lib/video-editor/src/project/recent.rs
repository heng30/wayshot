use crate::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFile {
    pub path: PathBuf,
    pub name: String,
    pub opened_at: SystemTime,
    pub last_modified: SystemTime,
    pub file_size: u64,
}

impl RecentFile {
    pub fn new(path: PathBuf) -> Result<Self> {
        let metadata = fs::metadata(&path)?;
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        Ok(Self {
            path,
            name,
            opened_at: SystemTime::now(),
            last_modified: metadata.modified()?,
            file_size: metadata.len(),
        })
    }

    pub fn age(&self) -> Option<Duration> {
        self.opened_at.elapsed().ok()
    }

    pub fn is_recent(&self, max_age: Duration) -> bool {
        if let Some(age) = self.age() {
            age <= max_age
        } else {
            false
        }
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn format_size(&self) -> String {
        cutil::str::pretty_size_string(self.file_size)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFilesManager {
    recent_files: Vec<RecentFile>,
    max_files: usize,
}

impl RecentFilesManager {
    pub fn new(max_files: usize) -> Self {
        Self {
            recent_files: Vec::new(),
            max_files,
        }
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn add_file(&mut self, path: PathBuf) -> Result<()> {
        self.recent_files.retain(|f| f.path != path);
        let recent_file = RecentFile::new(path.clone())?;
        self.recent_files.insert(0, recent_file);

        while self.recent_files.len() > self.max_files {
            self.recent_files.pop();
        }

        Ok(())
    }

    pub fn remove_file(&mut self, path: &PathBuf) {
        self.recent_files.retain(|f| f.path != *path);
    }

    pub fn get_recent_files(&self) -> Vec<&RecentFile> {
        self.recent_files.iter().collect()
    }

    pub fn get_existing_files(&self) -> Vec<&RecentFile> {
        self.recent_files.iter().filter(|f| f.exists()).collect()
    }

    pub fn clear_all(&mut self) {
        self.recent_files.clear();
    }

    pub fn clear_missing(&mut self) {
        self.recent_files.retain(|f| f.exists());
    }

    pub fn file_count(&self) -> usize {
        self.recent_files.len()
    }

    pub fn max_files(&self) -> usize {
        self.max_files
    }

    pub fn set_max_files(&mut self, max: usize) {
        self.max_files = max;
        while self.recent_files.len() > self.max_files {
            self.recent_files.pop();
        }
    }
}
