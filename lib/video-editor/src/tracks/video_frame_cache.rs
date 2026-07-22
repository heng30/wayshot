use image::RgbaImage;
use lru::LruCache;
use resvg::tiny_skia::Pixmap;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};
use usvg::{Options, Tree as SvgTree, fontdb};

const SVG_CACHE_MAX_ENTRIES: usize = 50;

static GLOBAL_SVG_CACHE: OnceLock<GlobalSvgCache> = OnceLock::new();
static CONFIGURED_MAX_FRAMES: AtomicUsize = AtomicUsize::new(100);
static GLOBAL_FRAME_CACHE: OnceLock<GlobalFrameCache> = OnceLock::new();

#[derive(Clone, Debug)]
pub enum VideoImage {
    Image { buffer: RgbaImage },
    Empty,
}

impl VideoImage {
    pub fn image(buffer: RgbaImage) -> Self {
        Self::Image { buffer }
    }
}

pub fn set_global_cache_max_frames(max_frames: usize) {
    CONFIGURED_MAX_FRAMES.store(max_frames, Ordering::SeqCst);
}

pub fn get_global_cache_max_frames() -> usize {
    CONFIGURED_MAX_FRAMES.load(Ordering::SeqCst)
}

pub fn clear_global_cache() {
    get_global_video_cache().clear();
}

pub(crate) fn get_global_video_cache() -> &'static GlobalFrameCache {
    GLOBAL_FRAME_CACHE.get_or_init(|| {
        let max_frames = CONFIGURED_MAX_FRAMES.load(Ordering::SeqCst);
        GlobalFrameCache::new(max_frames)
    })
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(crate) struct FrameCacheKey {
    path_hash: u64,
    stream_index: usize,
    frame_index: usize,
}

impl FrameCacheKey {
    pub(crate) fn from_path(path: &Path, stream_index: usize, frame_index: usize) -> Self {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);

        Self {
            path_hash: hasher.finish(),
            stream_index,
            frame_index,
        }
    }
}

pub(crate) struct GlobalFrameCache {
    cache: Mutex<LruCache<FrameCacheKey, VideoImage>>,
}

impl GlobalFrameCache {
    pub(crate) fn new(max_frames: usize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(max_frames).unwrap_or_else(|| NonZeroUsize::new(100).unwrap()),
            )),
        }
    }

    pub(crate) fn get(&self, key: &FrameCacheKey) -> Option<VideoImage> {
        self.cache.lock().unwrap().get(key).cloned()
    }

    pub(crate) fn put(&self, key: FrameCacheKey, frame: VideoImage) {
        self.cache.lock().unwrap().put(key, frame);
    }

    #[allow(dead_code)]
    pub(crate) fn clear(&self) {
        self.cache.lock().unwrap().clear();
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SvgCacheKey {
    path: PathBuf,
    mtime_secs: u64,
}

impl SvgCacheKey {
    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        let mtime = std::fs::metadata(path)?
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(Self {
            path: path.to_path_buf(),
            mtime_secs: mtime.as_secs(),
        })
    }
}

pub struct GlobalSvgCache {
    cache: Mutex<LruCache<SvgCacheKey, RgbaImage>>,
}

impl GlobalSvgCache {
    fn new() -> Self {
        Self {
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(SVG_CACHE_MAX_ENTRIES).unwrap(),
            )),
        }
    }

    pub fn get(&self, key: &SvgCacheKey) -> Option<RgbaImage> {
        self.cache.lock().unwrap().get(key).cloned()
    }

    pub fn put(&self, key: SvgCacheKey, image: RgbaImage) {
        self.cache.lock().unwrap().put(key, image);
    }
}

pub fn get_global_svg_cache() -> &'static GlobalSvgCache {
    GLOBAL_SVG_CACHE.get_or_init(GlobalSvgCache::new)
}

/// Render an SVG file to RgbaImage, using the global in-memory cache.
/// On cache miss, the SVG is parsed and rendered via resvg, then cached.
pub fn render_svg_to_rgba(svg_path: &Path) -> crate::Result<RgbaImage> {
    let key = SvgCacheKey::from_path(svg_path).map_err(|e| {
        crate::Error::InvalidFile(format!(
            "Failed to stat SVG file {}: {}",
            svg_path.display(),
            e
        ))
    })?;

    let cache = get_global_svg_cache();
    if let Some(cached) = cache.get(&key) {
        return Ok(cached);
    }

    let svg_data = std::fs::read(svg_path).map_err(|e| {
        crate::Error::InvalidFile(format!(
            "Failed to read SVG file {}: {}",
            svg_path.display(),
            e
        ))
    })?;
    let svg_str = String::from_utf8(svg_data)
        .map_err(|e| crate::Error::InvalidFile(format!("SVG file is not valid UTF-8: {}", e)))?;

    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let opts = Options {
        fontdb: db.into(),
        ..Default::default()
    };
    let tree = SvgTree::from_str(&svg_str, &opts)
        .map_err(|e| crate::Error::InvalidFile(format!("Failed to parse SVG: {}", e)))?;

    let pixmap_size = tree.size().to_int_size();
    let width = pixmap_size.width();
    let height = pixmap_size.height();

    if width == 0 || height == 0 {
        return Err(crate::Error::InvalidFile(
            "SVG has zero dimensions".to_string(),
        ));
    }

    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| crate::Error::InvalidFile("Failed to create pixmap for SVG".to_string()))?;

    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );

    let rgba_image =
        RgbaImage::from_raw(width, height, pixmap.data().to_vec()).ok_or_else(|| {
            crate::Error::InvalidFile("Failed to create RgbaImage from SVG pixmap".to_string())
        })?;

    cache.put(key, rgba_image.clone());

    Ok(rgba_image)
}
