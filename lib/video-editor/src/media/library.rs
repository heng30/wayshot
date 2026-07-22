use crate::{
    Result,
    media::{cache::MediaCache, media_type::MediaType},
    metadata::{Metadata, get_metadata},
    project::metadata::resolve_relative_path,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

pub const SUPPORT_EXT: &[&'static str] = &[
    "mp4", "mov", "avi", "mkv", "webm", "flv", "ts", "m2ts", "wmv", "asf", "3gp", "ogv", "m4v",
    "mpeg", "mpg", "mxf", // Video containers (FFmpeg-supported)
    "mp3", "wav", "aac", "flac", "opus", "ogg", "m4a", "wma", "ape", "aiff", // Audio formats
    "png", "jpg", "jpeg", "gif", "svg", "webp", "tiff", "tif", "bmp", "ico", "tga", "ppm", "pgm",
    "pnm", "hdr", "qoi", // Image formats
    "srt", "vtt", "lrc", // Subtitle formats
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaItemStatus {
    Online,  // File is available and accessible
    Offline, // File is missing or inaccessible
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: String,
    pub file_path: PathBuf,
    pub name: String,
    pub media_type: MediaType,
    pub status: MediaItemStatus,
    pub thumbnail_path: Option<PathBuf>,
    pub tags: Vec<String>,
    pub duration: Option<Duration>,
    pub file_size: u64,
    pub is_marked: bool,
    #[serde(default)]
    pub parent_id: Option<String>,

    #[serde(skip)]
    pub metadata: Option<Arc<Metadata>>,
}

impl MediaItem {
    pub fn new(file_path: PathBuf, name: String, media_type: MediaType) -> Self {
        let absolute_path = file_path
            .canonicalize()
            .unwrap_or_else(|_| std::path::absolute(&file_path).unwrap_or(file_path.clone()));

        let file_size = fs::metadata(&absolute_path).map(|m| m.len()).unwrap_or(0);

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            file_path: absolute_path,
            name,
            media_type,
            status: MediaItemStatus::Online,
            metadata: None,
            thumbnail_path: None,
            tags: Vec::new(),
            duration: None,
            file_size,
            is_marked: false,
            parent_id: None,
        }
    }

    pub fn from_path(file_path: PathBuf, name: String, media_type: MediaType) -> Self {
        Self::new(file_path, name, media_type)
    }

    pub fn with_metadata(mut self, metadata: Arc<Metadata>) -> Self {
        self.duration = Some(metadata.duration);
        self.media_type = MediaType::from_metadata(&metadata);
        self.metadata = Some(metadata);
        self
    }

    pub fn with_thumbnail(mut self, thumbnail_path: PathBuf) -> Self {
        self.thumbnail_path = Some(thumbnail_path);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn is_online(&self) -> bool {
        self.status == MediaItemStatus::Online
    }

    pub fn is_offline(&self) -> bool {
        self.status == MediaItemStatus::Offline
    }

    pub fn format_duration(&self) -> String {
        if let Some(duration) = self.duration {
            let secs = duration.as_secs();
            let minutes = secs / 60;
            let seconds = secs % 60;
            format!("{:02}:{:02}", minutes, seconds)
        } else {
            "--:--".to_string()
        }
    }

    pub fn format_file_size(&self) -> String {
        cutil::str::pretty_size_string(self.file_size)
    }

    pub fn check_status(&mut self) {
        self.status = if self.file_path.exists() {
            MediaItemStatus::Online
        } else {
            MediaItemStatus::Offline
        };
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryFolder {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    /// If this folder was imported from a directory on disk, this stores that path.
    /// Used for "sync" functionality to re-scan the directory.
    #[serde(default)]
    pub source_path: Option<PathBuf>,
    #[serde(default)]
    pub is_marked: bool,
    #[serde(default = "SystemTime::now")]
    pub created_at: SystemTime,
}

impl LibraryFolder {
    pub fn new(id: String, name: String, parent_id: Option<String>) -> Self {
        Self {
            id,
            name,
            parent_id,
            source_path: None,
            is_marked: false,
            created_at: SystemTime::now(),
        }
    }

    pub fn from_name(name: String, parent_id: Option<String>) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        Self::new(id, name, parent_id)
    }

    pub fn from_source(source_path: PathBuf, parent_id: Option<String>) -> Self {
        let name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();
        let id = uuid::Uuid::new_v4().to_string();
        Self {
            id,
            name,
            parent_id,
            source_path: Some(source_path),
            is_marked: false,
            created_at: SystemTime::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LibraryNode {
    File(MediaItem),
    Folder(LibraryFolder),
}

impl LibraryNode {
    pub fn id(&self) -> &str {
        match self {
            LibraryNode::File(item) => &item.id,
            LibraryNode::Folder(folder) => &folder.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            LibraryNode::File(item) => &item.name,
            LibraryNode::Folder(folder) => &folder.name,
        }
    }

    pub fn parent_id(&self) -> Option<&str> {
        match self {
            LibraryNode::File(item) => item.parent_id.as_deref(),
            LibraryNode::Folder(folder) => folder.parent_id.as_deref(),
        }
    }

    pub fn is_folder(&self) -> bool {
        matches!(self, LibraryNode::Folder(_))
    }

    pub fn is_file(&self) -> bool {
        matches!(self, LibraryNode::File(_))
    }

    pub fn as_file(&self) -> Option<&MediaItem> {
        match self {
            LibraryNode::File(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_file_mut(&mut self) -> Option<&mut MediaItem> {
        match self {
            LibraryNode::File(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_folder(&self) -> Option<&LibraryFolder> {
        match self {
            LibraryNode::Folder(folder) => Some(folder),
            _ => None,
        }
    }

    pub fn as_folder_mut(&mut self) -> Option<&mut LibraryFolder> {
        match self {
            LibraryNode::Folder(folder) => Some(folder),
            _ => None,
        }
    }

    pub fn media_type(&self) -> Option<MediaType> {
        match self {
            LibraryNode::File(item) => Some(item.media_type),
            LibraryNode::Folder(_) => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct SyncResult {
    pub removed: Vec<String>, // names of removed items
    pub added: Vec<String>,   // names of added items
}

#[derive(Debug, Clone, derivative::Derivative)]
#[derivative(Default)]
pub struct MediaList {
    pub name: String,
    pub description: Option<String>,

    #[derivative(Default(value = "IndexMap::new()"))]
    nodes: IndexMap<String, LibraryNode>,

    #[derivative(Default(value = "SystemTime::now()"))]
    created_at: SystemTime,
    #[derivative(Default(value = "SystemTime::now()"))]
    modified_at: SystemTime,

    cache: Option<MediaCache>,

    cache_dir: Option<PathBuf>,
    thumbnail_size: Option<(u32, u32)>,
    max_cache_age: Option<Duration>,
}

impl Serialize for MediaList {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct MediaListSerde<'a> {
            name: &'a String,
            description: &'a Option<String>,
            nodes: &'a IndexMap<String, LibraryNode>,
            created_at: &'a SystemTime,
            modified_at: &'a SystemTime,
            cache_dir: &'a Option<PathBuf>,
            thumbnail_size: &'a Option<(u32, u32)>,
            max_cache_age: &'a Option<Duration>,
        }

        MediaListSerde {
            name: &self.name,
            description: &self.description,
            nodes: &self.nodes,
            created_at: &self.created_at,
            modified_at: &self.modified_at,
            cache_dir: &self.cache_dir,
            thumbnail_size: &self.thumbnail_size,
            max_cache_age: &self.max_cache_age,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MediaList {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct MediaListData {
            #[serde(default)]
            name: String,
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            nodes: IndexMap<String, LibraryNode>,
            #[serde(default = "SystemTime::now")]
            created_at: SystemTime,
            #[serde(default = "SystemTime::now")]
            modified_at: SystemTime,
            cache_dir: Option<PathBuf>,
            thumbnail_size: Option<(u32, u32)>,
            max_cache_age: Option<Duration>,
        }

        let data: MediaListData = serde_json::Value::deserialize(deserializer)
            .and_then(|v| serde_json::from_value(v).map_err(serde::de::Error::custom))?;

        Ok(MediaList {
            name: data.name,
            description: data.description,
            nodes: data.nodes,
            created_at: data.created_at,
            modified_at: data.modified_at,
            cache: None,
            cache_dir: data.cache_dir,
            thumbnail_size: data.thumbnail_size,
            max_cache_age: data.max_cache_age,
        })
    }
}

impl MediaList {
    pub fn new(name: String) -> Self {
        let now = SystemTime::now();
        Self {
            name,
            description: None,
            nodes: IndexMap::new(),
            created_at: now,
            modified_at: now,
            cache: None,
            cache_dir: None,
            thumbnail_size: None,
            max_cache_age: None,
        }
    }

    pub fn resolve_relative_paths(&mut self, base_dir: Option<&PathBuf>) {
        for node in self.nodes.values_mut() {
            if let LibraryNode::File(item) = node {
                let resolved_path = resolve_relative_path(&item.file_path, base_dir);
                if resolved_path != item.file_path {
                    log::debug!(
                        "Resolved path: {} -> {}",
                        item.file_path.display(),
                        resolved_path.display()
                    );
                    item.file_path = resolved_path;
                }
            }
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_cache(mut self, cache_dir: PathBuf) -> Result<Self> {
        self.cache_dir = Some(cache_dir.clone());
        self.cache = Some(MediaCache::new(cache_dir)?);
        Ok(self)
    }

    pub fn with_cache_configured(
        mut self,
        cache_dir: PathBuf,
        thumbnail_width: u32,
        thumbnail_height: u32,
        max_age: Duration,
    ) -> Result<Self> {
        self.cache_dir = Some(cache_dir.clone());
        self.thumbnail_size = Some((thumbnail_width, thumbnail_height));
        self.max_cache_age = Some(max_age);
        let cache = MediaCache::new(cache_dir)?
            .with_thumbnail_size(thumbnail_width, thumbnail_height)
            .with_max_age(max_age);
        self.cache = Some(cache);
        Ok(self)
    }

    pub fn get_thumbnail(&self, file_path: &Path) -> Option<PathBuf> {
        self.cache
            .as_ref()?
            .get_thumbnail(file_path)
            .map(|t| t.path)
    }

    pub fn generate_thumbnail(&mut self, file_path: &Path) -> Result<PathBuf> {
        let cache = self
            .cache
            .as_mut()
            .ok_or_else(|| crate::Error::InvalidConfig("Cache not initialized".to_string()))?;
        let thumbnail = cache.generate_thumbnail(file_path)?;
        Ok(thumbnail.path)
    }

    pub fn get_or_generate_thumbnail(&mut self, file_path: &Path) -> Result<PathBuf> {
        let cache = self
            .cache
            .as_mut()
            .ok_or_else(|| crate::Error::InvalidConfig("Cache not initialized".to_string()))?;
        let thumbnail = cache.get_or_generate_thumbnail(file_path)?;
        Ok(thumbnail.path)
    }

    /// Refresh thumbnails for all file items, regenerating if the source file
    /// has been modified since the thumbnail was created.
    pub fn refresh_thumbnails(&mut self) {
        let cache = match self.cache.as_mut() {
            Some(c) => c,
            None => return,
        };

        for node in self.nodes.values_mut() {
            if let LibraryNode::File(item) = node {
                let expected_path = cache.get_current_thumbnail_path(&item.file_path);
                let needs_refresh = item
                    .thumbnail_path
                    .as_ref()
                    .map(|p| p != &expected_path || !p.exists())
                    .unwrap_or(true);

                if needs_refresh
                    && let Ok(thumbnail) = cache.get_or_generate_thumbnail(&item.file_path)
                {
                    item.thumbnail_path = Some(thumbnail.path);
                }
            }
        }
    }

    pub fn is_thumbnail_current(&self, file_path: &Path, cached_path: &Path) -> bool {
        self.cache
            .as_ref()
            .map(|c| c.is_thumbnail_current(file_path, cached_path))
            .unwrap_or(false)
    }

    pub fn clear_cache(&mut self) {
        if let Some(cache) = &mut self.cache {
            cache.clear_thumbnail_cache();
        }
    }

    pub fn cleanup_cache(&mut self) -> Result<()> {
        if let Some(cache) = &mut self.cache {
            cache.cleanup_cache()?;
        }
        Ok(())
    }

    pub fn cache_size(&self) -> u64 {
        self.cache.as_ref().map_or(0, |c| c.cache_size())
    }

    pub fn has_cache(&self) -> bool {
        self.cache.is_some()
    }

    pub fn get_node(&self, id: &str) -> Option<&LibraryNode> {
        self.nodes.get(id)
    }

    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut LibraryNode> {
        self.nodes.get_mut(id)
    }

    /// Add a file to the list under the given parent folder.
    /// `parent_id = None` means root level.
    pub fn add_file(&mut self, file_path: PathBuf, parent_id: Option<String>) -> Result<String> {
        self.add_file_with_name(file_path, None, parent_id)
    }

    pub fn add_file_with_name(
        &mut self,
        file_path: PathBuf,
        name: Option<String>,
        parent_id: Option<String>,
    ) -> Result<String> {
        let absolute_path = file_path
            .canonicalize()
            .unwrap_or_else(|_| std::path::absolute(&file_path).unwrap_or(file_path.clone()));

        // Check duplicate: same path in the same folder
        let is_duplicate = self
            .file_children(parent_id.as_deref())
            .iter()
            .any(|item| item.file_path == absolute_path);
        if is_duplicate {
            return Err(crate::Error::DuplicateEntry(
                absolute_path.display().to_string(),
            ));
        }

        // Validate parent folder exists
        if let Some(ref pid) = parent_id {
            match self.nodes.get(pid.as_str()) {
                Some(LibraryNode::Folder(_)) => {}
                Some(LibraryNode::File(_)) => {
                    return Err(crate::Error::InvalidConfig(
                        "Parent ID refers to a file, not a folder".to_string(),
                    ));
                }
                None => {
                    return Err(crate::Error::InvalidConfig(format!(
                        "Parent folder not found: {}",
                        pid
                    )));
                }
            }
        }

        let metadata = get_metadata(&file_path)?;
        let media_type = MediaType::from_metadata(&metadata);

        let item_name = name.unwrap_or_else(|| {
            file_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

        let mut item =
            MediaItem::new(file_path, item_name, media_type).with_metadata(Arc::new(metadata));

        item.parent_id = parent_id.clone();
        item.check_status();

        if self.cache.is_some()
            && let Ok(thumbnail_path) = self.get_or_generate_thumbnail(&item.file_path)
        {
            item.thumbnail_path = Some(thumbnail_path);
        }

        let id = item.id.clone();
        self.nodes.insert(id.clone(), LibraryNode::File(item));
        self.modified_at = SystemTime::now();

        Ok(id)
    }

    pub fn create_folder(&mut self, name: String, parent_id: Option<String>) -> Result<String> {
        if let Some(ref pid) = parent_id {
            match self.nodes.get(pid.as_str()) {
                Some(LibraryNode::Folder(_)) => {}
                Some(LibraryNode::File(_)) => {
                    return Err(crate::Error::InvalidConfig(
                        "Parent ID refers to a file, not a folder".to_string(),
                    ));
                }
                None => {
                    return Err(crate::Error::InvalidConfig(format!(
                        "Parent folder not found: {}",
                        pid
                    )));
                }
            }
        }

        let folder = LibraryFolder::from_name(name, parent_id);
        let id = folder.id.clone();
        if self.nodes.contains_key(&id) {
            return Err(crate::Error::DuplicateEntry(folder.name.clone()));
        }
        self.nodes.insert(id.clone(), LibraryNode::Folder(folder));
        self.modified_at = SystemTime::now();
        Ok(id)
    }

    /// Create a folder from a source directory path, setting `source_path` for sync.
    pub fn create_folder_from_source(
        &mut self,
        source_path: PathBuf,
        parent_id: Option<String>,
    ) -> Result<String> {
        if let Some(ref pid) = parent_id {
            match self.nodes.get(pid.as_str()) {
                Some(LibraryNode::Folder(_)) => {}
                Some(LibraryNode::File(_)) => {
                    return Err(crate::Error::InvalidConfig(
                        "Parent ID refers to a file, not a folder".to_string(),
                    ));
                }
                None => {
                    return Err(crate::Error::InvalidConfig(format!(
                        "Parent folder not found: {}",
                        pid
                    )));
                }
            }
        }

        let folder = LibraryFolder::from_source(source_path, parent_id);
        let id = folder.id.clone();
        if self.nodes.contains_key(&id) {
            return Err(crate::Error::DuplicateEntry(folder.name.clone()));
        }
        self.nodes.insert(id.clone(), LibraryNode::Folder(folder));
        self.modified_at = SystemTime::now();
        Ok(id)
    }

    pub fn rename_folder(&mut self, id: &str, new_name: String) -> Result<()> {
        let node = self
            .nodes
            .get_mut(id)
            .ok_or_else(|| crate::Error::InvalidConfig(format!("Node not found: {}", id)))?;
        match node {
            LibraryNode::Folder(folder) => folder.name = new_name,
            LibraryNode::File(_) => {
                return Err(crate::Error::InvalidConfig(format!(
                    "Node {} is not a folder",
                    id
                )));
            }
        }
        self.modified_at = SystemTime::now();
        Ok(())
    }

    /// Remove a node. If it's a folder, recursively removes all children first.
    pub fn remove_node(&mut self, id: &str) -> Result<Option<LibraryNode>> {
        if !self.nodes.contains_key(id) {
            return Err(crate::Error::InvalidConfig(format!(
                "Node not found: {}",
                id
            )));
        }

        let is_folder = matches!(self.nodes.get(id), Some(LibraryNode::Folder(_)));
        if is_folder {
            let child_ids: Vec<String> = self
                .nodes
                .values()
                .filter(|n| n.parent_id() == Some(id))
                .map(|n| n.id().to_string())
                .collect();
            for child_id in child_ids {
                self.remove_node(&child_id)?;
            }
        }

        self.modified_at = SystemTime::now();
        Ok(self.nodes.shift_remove(id))
    }

    /// Move a node (file or folder) to a new parent folder.
    /// `new_parent_id = None` means root level.
    pub fn move_node(&mut self, node_id: &str, new_parent_id: Option<String>) -> Result<()> {
        if !self.nodes.contains_key(node_id) {
            return Err(crate::Error::InvalidConfig(format!(
                "Node not found: {}",
                node_id
            )));
        }

        // Validate new parent exists and is a folder (or None = root)
        if let Some(ref pid) = new_parent_id {
            match self.nodes.get(pid.as_str()) {
                Some(LibraryNode::Folder(_)) => {}
                Some(LibraryNode::File(_)) => {
                    return Err(crate::Error::InvalidConfig(
                        "Parent ID refers to a file, not a folder".to_string(),
                    ));
                }
                None => {
                    return Err(crate::Error::InvalidConfig(format!(
                        "Parent folder not found: {}",
                        pid
                    )));
                }
            }
        }

        // Prevent moving folder into itself or a descendant
        if new_parent_id.as_deref() == Some(node_id) {
            return Err(crate::Error::InvalidConfig(
                "Cannot move node into itself".to_string(),
            ));
        }
        if self.is_descendant(node_id, new_parent_id.as_deref()) {
            return Err(crate::Error::InvalidConfig(
                "Cannot move folder into its own descendant".to_string(),
            ));
        }

        let node = self.nodes.get_mut(node_id).unwrap();
        match node {
            LibraryNode::File(item) => item.parent_id = new_parent_id,
            LibraryNode::Folder(folder) => folder.parent_id = new_parent_id,
        }
        self.modified_at = SystemTime::now();
        Ok(())
    }

    /// Check if `possible_descendant_id` is a descendant of `ancestor_id`
    pub fn is_descendant(&self, ancestor_id: &str, possible_descendant_id: Option<&str>) -> bool {
        let Some(mut current_id) = possible_descendant_id else {
            return false;
        };
        let mut visited = std::collections::HashSet::new();
        while let Some(node) = self.nodes.get(current_id) {
            if current_id == ancestor_id {
                return true;
            }
            if !visited.insert(current_id) {
                break; // cycle detection
            }
            current_id = match node.parent_id() {
                Some(pid) => pid,
                None => break,
            };
        }
        false
    }

    /// Get all nodes (files + folders) that are direct children of the given parent.
    pub fn children(&self, parent_id: Option<&str>) -> Vec<&LibraryNode> {
        self.nodes
            .values()
            .filter(|node| node.parent_id() == parent_id)
            .collect()
    }

    /// Get folder children only.
    pub fn folder_children(&self, parent_id: Option<&str>) -> Vec<&LibraryFolder> {
        self.nodes
            .values()
            .filter_map(|node| match node {
                LibraryNode::Folder(f) if f.parent_id.as_deref() == parent_id => Some(f),
                _ => None,
            })
            .collect()
    }

    /// Get file children only.
    pub fn file_children(&self, parent_id: Option<&str>) -> Vec<&MediaItem> {
        self.nodes
            .values()
            .filter_map(|node| match node {
                LibraryNode::File(item) if item.parent_id.as_deref() == parent_id => Some(item),
                _ => None,
            })
            .collect()
    }

    /// Returns the path from root to the given folder (inclusive), used for breadcrumb navigation.
    pub fn folder_path(&self, folder_id: &str) -> Vec<&LibraryFolder> {
        let mut path = Vec::new();
        let mut current_id: Option<&str> = Some(folder_id);
        let mut visited = std::collections::HashSet::new();

        while let Some(id) = current_id {
            if !visited.insert(id) {
                break; // cycle detection
            }
            if let Some(LibraryNode::Folder(folder)) = self.nodes.get(id) {
                path.push(folder);
                current_id = folder.parent_id.as_deref();
            } else {
                break;
            }
        }

        path.reverse();
        path
    }

    pub fn add_item(&mut self, item: MediaItem) {
        let id = item.id.clone();
        self.nodes.insert(id, LibraryNode::File(item));
        self.modified_at = SystemTime::now();
    }

    pub fn add_node(&mut self, node: LibraryNode) {
        let id = node.id().to_string();
        self.nodes.insert(id, node);
        self.modified_at = SystemTime::now();
    }

    pub fn nodes(&self) -> impl Iterator<Item = &LibraryNode> {
        self.nodes.values()
    }

    pub fn items(&self) -> Vec<&MediaItem> {
        self.nodes.values().filter_map(|n| n.as_file()).collect()
    }

    pub fn get_item(&self, id: &str) -> Option<&MediaItem> {
        self.nodes.get(id).and_then(|n| n.as_file())
    }

    pub fn get_item_mut(&mut self, id: &str) -> Option<&mut MediaItem> {
        self.nodes.get_mut(id).and_then(|n| n.as_file_mut())
    }

    pub fn update_item(&mut self, id: &str, item: MediaItem) -> Result<()> {
        if !self.nodes.contains_key(id) {
            return Err(crate::Error::InvalidConfig(format!(
                "Media item not found: {}",
                id
            )));
        }
        self.nodes.insert(id.to_string(), LibraryNode::File(item));
        self.modified_at = SystemTime::now();
        Ok(())
    }

    pub fn all_items(&self) -> Vec<&MediaItem> {
        self.nodes.values().filter_map(|n| n.as_file()).collect()
    }

    pub fn all_folders(&self) -> Vec<&LibraryFolder> {
        self.nodes.values().filter_map(|n| n.as_folder()).collect()
    }

    pub fn folder_count(&self) -> usize {
        self.nodes.values().filter(|n| n.is_folder()).count()
    }

    pub fn items_by_type(&self, media_type: MediaType) -> Vec<&MediaItem> {
        self.nodes
            .values()
            .filter_map(|n| match n {
                LibraryNode::File(item) if item.media_type == media_type => Some(item),
                _ => None,
            })
            .collect()
    }

    pub fn items_by_type_in_folder(
        &self,
        media_type: MediaType,
        parent_id: Option<&str>,
    ) -> Vec<&MediaItem> {
        self.nodes
            .values()
            .filter_map(|n| match n {
                LibraryNode::File(item)
                    if item.media_type == media_type && item.parent_id.as_deref() == parent_id =>
                {
                    Some(item)
                }
                _ => None,
            })
            .collect()
    }

    pub fn items_by_tag(&self, tag: &str) -> Vec<&MediaItem> {
        self.nodes
            .values()
            .filter_map(|n| match n {
                LibraryNode::File(item) if item.tags.iter().any(|t| t == tag) => Some(item),
                _ => None,
            })
            .collect()
    }

    pub fn offline_items(&self) -> Vec<&MediaItem> {
        self.nodes
            .values()
            .filter_map(|n| match n {
                LibraryNode::File(item) if item.is_offline() => Some(item),
                _ => None,
            })
            .collect()
    }

    pub fn search(&self, query: &str) -> Vec<&MediaItem> {
        let query_lower = query.to_lowercase();
        self.nodes
            .values()
            .filter_map(|n| match n {
                LibraryNode::File(item) if item.name.to_lowercase().contains(&query_lower) => {
                    Some(item)
                }
                _ => None,
            })
            .collect()
    }

    pub fn filter<F>(&self, predicate: F) -> Vec<&MediaItem>
    where
        F: Fn(&&MediaItem) -> bool,
    {
        self.nodes
            .values()
            .filter_map(|n| n.as_file())
            .filter(predicate)
            .collect()
    }

    pub fn check_all_status(&mut self) {
        for node in self.nodes.values_mut() {
            if let LibraryNode::File(item) = node {
                item.check_status();
            }
        }
    }

    pub fn item_count(&self) -> usize {
        self.nodes.values().filter(|n| n.is_file()).count()
    }

    pub fn total_size(&self) -> u64 {
        self.nodes
            .values()
            .filter_map(|n| n.as_file())
            .map(|item| item.file_size)
            .sum()
    }

    /// Synchronize a folder with its source directory on disk.
    /// - Removes file children whose file_path no longer exists
    /// - Adds new files from the source directory that aren't already in the folder
    /// - Recursively syncs sub-folders that also have a source_path
    /// Returns the sync result with counts of removed and added items.
    #[stacksafe::stacksafe]
    pub fn sync_folder(&mut self, folder_id: &str) -> Result<SyncResult> {
        let source_path = {
            let node = self.nodes.get(folder_id).ok_or_else(|| {
                crate::Error::InvalidConfig(format!("Node not found: {}", folder_id))
            })?;
            let folder = node.as_folder().ok_or_else(|| {
                crate::Error::InvalidConfig(format!("Node {} is not a folder", folder_id))
            })?;
            folder.source_path.clone()
        };

        let Some(ref source_dir) = source_path else {
            return Err(crate::Error::InvalidConfig(format!(
                "Folder {} has no source path",
                folder_id
            )));
        };

        if !source_dir.exists() {
            return Err(crate::Error::InvalidConfig(format!(
                "Source directory no longer exists: {}",
                source_dir.display()
            )));
        }

        let mut result = SyncResult::default();

        // 1. Remove file children whose path no longer exists on disk
        let file_children: Vec<(String, PathBuf)> = self
            .file_children(Some(folder_id))
            .iter()
            .map(|item| (item.id.clone(), item.file_path.clone()))
            .collect();

        for (item_id, file_path) in &file_children {
            if !file_path.exists()
                && let Ok(Some(removed)) = self.remove_node(item_id)
            {
                result.removed.push(removed.name().to_string());
            }
        }

        // 1b. Remove sub-folders whose source_path no longer exists on disk
        let sub_folder_info: Vec<(String, Option<PathBuf>)> = self
            .folder_children(Some(folder_id))
            .iter()
            .map(|f| (f.id.clone(), f.source_path.clone()))
            .collect();

        for (sub_id, sub_source_path) in &sub_folder_info {
            if let Some(sp) = sub_source_path
                && !sp.exists()
                && let Ok(Some(removed)) = self.remove_node(sub_id)
            {
                result.removed.push(removed.name().to_string());
            }
        }

        // 2. Collect existing file paths in the folder to detect new files
        let existing_paths: Vec<PathBuf> = self
            .file_children(Some(folder_id))
            .iter()
            .map(|item| item.file_path.clone())
            .collect();

        // 3. Scan source directory for new files and new subdirectories (skip symlinks)
        if let Ok(entries) = std::fs::read_dir(source_dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Skip symlinks to prevent circular recursion
                if path.is_symlink() {
                    continue;
                }

                if path.is_dir() {
                    // Check if this subdirectory already has a corresponding folder node
                    let existing_sub_folders: Vec<(String, Option<PathBuf>)> = self
                        .folder_children(Some(folder_id))
                        .iter()
                        .map(|f| (f.name.clone(), f.source_path.clone()))
                        .collect();

                    let dir_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    let already_exists = existing_sub_folders.iter().any(|(name, sp)| {
                        name == &dir_name || sp.as_ref().map(|p| p.clone()) == Some(path.clone())
                    });

                    // Create a new sub-folder for this directory
                    if !already_exists
                        && let Ok(sub_id) =
                            self.create_folder_from_source(path, Some(folder_id.to_string()))
                        && let Some(name) = self.get_node(&sub_id).map(|n| n.name().to_string())
                    {
                        result.added.push(name);
                    }
                } else if path.is_file() {
                    // Check if supported extension
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if !SUPPORT_EXT.contains(&ext.as_str()) {
                        continue;
                    }

                    // Check if already exists
                    let absolute_path = path
                        .canonicalize()
                        .unwrap_or_else(|_| std::path::absolute(&path).unwrap_or(path.clone()));
                    if existing_paths.contains(&absolute_path) {
                        continue;
                    }

                    // Add new file
                    if let Ok(item_id) = self.add_file(path.clone(), Some(folder_id.to_string()))
                        && let Some(name) = self.get_node(&item_id).map(|n| n.name().to_string())
                    {
                        result.added.push(name);
                    }
                }
            }
        }

        // 4. Recursively sync sub-folders that have a source_path
        let sub_folder_ids: Vec<String> = self
            .folder_children(Some(folder_id))
            .iter()
            .filter(|f| f.source_path.is_some())
            .map(|f| f.id.clone())
            .collect();

        for sub_id in sub_folder_ids {
            if let Ok(sub_result) = self.sync_folder(&sub_id) {
                result.removed.extend(sub_result.removed);
                result.added.extend(sub_result.added);
            }
        }

        self.modified_at = SystemTime::now();
        Ok(result)
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.modified_at = SystemTime::now();
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn to_json(&self, pretty: bool) -> Result<String> {
        if pretty {
            serde_json::to_string_pretty(self).map_err(|e| crate::Error::Json(e))
        } else {
            serde_json::to_string(self).map_err(|e| crate::Error::Json(e))
        }
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let mut list: MediaList = serde_json::from_str(json).map_err(|e| crate::Error::Json(e))?;
        list.rebuild_cache()?;
        Ok(list)
    }

    fn rebuild_cache(&mut self) -> Result<()> {
        let Some(ref cache_dir) = self.cache_dir else {
            return Ok(());
        };

        let thumbnail_size = self.thumbnail_size.unwrap_or((160, 90));
        let max_age = self.max_cache_age.unwrap_or(Duration::from_secs(86400));

        let mut cache = MediaCache::new(cache_dir.clone())?
            .with_thumbnail_size(thumbnail_size.0, thumbnail_size.1)
            .with_max_age(max_age);

        for node in self.nodes.values_mut() {
            if let LibraryNode::File(item) = node {
                if let Ok(thumbnail) = cache.get_or_generate_thumbnail(&item.file_path) {
                    item.thumbnail_path = Some(thumbnail.path);
                }
            }
        }

        self.cache = Some(cache);

        Ok(())
    }
}
