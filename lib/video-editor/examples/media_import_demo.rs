use std::{path::PathBuf, time::Duration};
use video_editor::{
    Result,
    media::{ImportOptions, MediaImporter},
};

fn main() -> Result<()> {
    env_logger::init();

    log::info!("=== Media Import Demo ===");

    // Create library with integrated cache
    let cache_dir = PathBuf::from("tmp").join("cache");
    let mut library = video_editor::media::MediaLibrary::new().with_cache_configured(
        cache_dir.clone(),
        320,
        180,
        Duration::from_secs(86400),
    )?;
    log::info!("Cache directory: {:?}", cache_dir);

    // Configure import with custom options
    let import_options = ImportOptions::new()
        .with_import_thumbnails(true) // Generate thumbnails
        .with_extract_metadata(true) // Extract video/audio metadata
        .with_import_recursive(true); // Import from subdirectories

    // Create importer
    let mut importer = MediaImporter::new(import_options);

    // Import from data directory
    let data_dir = PathBuf::from("data");

    if !data_dir.exists() {
        log::warn!("Data directory not found: {}", data_dir.display());
        log::info!("Please create the data/ directory and add media files.");
        log::info!("Supported formats: mp4, mkv, mp3, wav, jpg, png, srt");
        return Ok(());
    }

    log::info!("Importing from: {}", data_dir.display());
    let _results = importer.import_directory(&data_dir, &mut library)?;

    // Show progress
    let progress = importer.progress();
    if let Ok(p) = progress.lock() {
        log::info!("Import Progress:");
        log::info!("  Total files: {}", p.total_files);
        log::info!("  Imported: {}", p.imported_files);
        log::info!("  Failed: {}", p.failed_files);
        log::info!("  Progress: {:.1}%", p.progress() * 100.0);
        if let Some(elapsed) = p.elapsed() {
            log::info!("  Time elapsed: {:.2}s", elapsed.as_secs_f64());
        }
    }

    // Show summary
    log::info!("Import Summary:");
    log::info!("  Total items in library: {}", library.item_count());
    log::info!("  Total size: {} bytes", library.total_size());

    // List imported items
    log::info!("Imported Items:");
    for item in library.all_items() {
        log::info!(
            "  [{}] {} - {}",
            item.media_type.as_str(),
            item.name,
            item.format_file_size()
        );

        if let Some(duration) = item.duration {
            log::info!("       Duration: {:.2}s", duration.as_secs_f64());
        }

        if item.thumbnail_path.is_some() {
            // Check if thumbnail was newly generated or loaded from cache
            if let Some(thumb_path) = &item.thumbnail_path {
                use std::fs;
                if let Ok(metadata) = fs::metadata(thumb_path) {
                    // If file was created in the last second, it was generated
                    if let Ok(modified) = metadata.modified() {
                        let age = modified.elapsed().unwrap_or_default().as_secs();
                        if age < 1 {
                            log::info!("       Thumbnail: Generated");
                        } else {
                            log::info!("       Thumbnail: Cached");
                        }
                    }
                }
            }
        }
    }

    log::info!("\n=== Import Complete ===");

    Ok(())
}
