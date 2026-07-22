use std::path::PathBuf;
use video_editor::{
    Result,
    media::{ImportOptions, MediaImporter, MediaLibrary},
};

fn main() -> Result<()> {
    env_logger::init();

    log::info!("=== Media Library Demo ===");

    // Step 1: Create a media library with integrated cache
    log::info!("Step 1: Creating media library with cache...");
    let cache_dir = PathBuf::from("tmp").join("cache");
    let mut library = MediaLibrary::new().with_cache_configured(
        cache_dir.clone(),
        320,
        180,
        std::time::Duration::from_secs(86400),
    )?;
    log::info!("  Cache directory: {:?}", cache_dir);

    // Step 2: Demonstrate add_file (auto-detect media type and generate thumbnail)
    log::info!("Step 2: Adding files with auto-detection...");
    let test_files = vec!["data/video1.mp4", "data/video2.mp4", "data/song1.mp3"];

    for file_path in &test_files {
        match library.add_file(PathBuf::from(file_path)) {
            Ok(id) => {
                if let Some(item) = library.get_item(id) {
                    log::info!("  ✓ Added: {} (ID: {})", item.name, id);
                    log::info!("    Type: {}", item.media_type.as_str());
                    log::info!("    Duration: {}", item.format_duration());
                    if let Some(ref thumbnail) = item.thumbnail_path {
                        log::info!("    Thumbnail: {}", thumbnail.display());
                    }
                }
            }
            Err(e) => {
                log::warn!("  ✗ Failed to add {}: {}", file_path, e);
            }
        }
    }

    // Step 3: Configure import options for bulk import
    log::info!("Step 3: Configuring import options...");
    let import_options = ImportOptions::new()
        .with_import_thumbnails(true)
        .with_extract_metadata(true)
        .with_import_recursive(true);

    // Step 4: Import media files from the data directory
    log::info!("Step 4: Importing media files from data/ directory...");
    let data_dir = PathBuf::from("data");

    if data_dir.exists() {
        let mut importer = MediaImporter::new(import_options);
        // Note: importer doesn't need separate cache anymore, library has it
        let import_results = importer.import_directory(&data_dir, &mut library)?;

        // Display import results
        log::info!("Import Results:");
        log::info!("  Total files processed: {}", import_results.len());
        let successful = import_results.iter().filter(|r| r.success).count();
        let failed = import_results.iter().filter(|r| !r.success).count();
        log::info!("  Successful: {}", successful);
        log::info!("  Failed: {}", failed);
    } else {
        log::warn!("Data directory does not exist: {}", data_dir.display());
        log::info!(
            "Please add some media files (mp4, mkv, mp3, wav, jpg, png, srt) to the data/ directory"
        );
    }

    // Step 5: Display library statistics
    log::info!("=== Library Statistics ===");
    log::info!("Total items: {}", library.item_count());
    log::info!("Total size: {}", library.total_size());

    // Step 6: Display all media items grouped by type
    log::info!("=== Media Items by Type ===");

    for media_type in &[
        video_editor::media::MediaType::Video,
        video_editor::media::MediaType::Audio,
        video_editor::media::MediaType::Image,
        video_editor::media::MediaType::Subtitle,
    ] {
        let items = library.items_by_type(*media_type);
        log::info!("{} files ({}):", media_type.as_str(), items.len());

        for item in items {
            log::info!("  - {} [{}]", item.name, item.media_type.as_str());
            log::info!("    ID: {}", item.id);
            log::info!("    Path: {}", item.file_path.display());
            log::info!("    Size: {}", item.format_file_size());
            log::info!(
                "    Status: {}",
                if item.is_online() {
                    "Online"
                } else {
                    "Offline"
                }
            );

            if let Some(duration) = item.duration {
                log::info!("    Duration: {:.2}s", duration.as_secs_f64());
            }

            if let Some(thumbnail) = &item.thumbnail_path {
                log::info!("    Thumbnail: {}", thumbnail.display());
            }

            if !item.tags.is_empty() {
                log::info!("    Tags: {}", item.tags.join(", "));
            }
        }
    }

    // Step 7: Demonstrate search functionality
    log::info!("=== Search Demo ===");
    let query = "test";
    let search_results = library.search(query);
    log::info!(
        "Search results for '{}': {} items",
        query,
        search_results.len()
    );
    for item in search_results {
        log::info!("  - {}", item.name);
    }

    // Step 8: Demonstrate tag functionality
    log::info!("=== Tag Management Demo ===");
    let items = library.all_items();
    if let Some(first_item) = items.first() {
        let item_id = first_item.id;
        let first_item_name = first_item.name.clone();

        // Add tags to the first item
        let tags = vec!["demo".to_string(), "example".to_string()];
        if let Some(item) = library.get_item_mut(item_id) {
            *item = item.clone().with_tags(tags.clone());
            log::info!("Added tags to item '{}': {:?}", first_item_name, tags);
        }

        // Search by tag
        let tagged_items = library.items_by_tag("demo");
        log::info!("Items tagged with 'demo': {}", tagged_items.len());
    }

    // Step 9: Demonstrate offline/online status
    log::info!("=== File Status Demo ===");
    library.check_all_status();
    let offline_items = library.offline_items();
    if offline_items.is_empty() {
        log::info!("All items are online (files exist)");
    } else {
        log::info!("Offline items (missing files): {}", offline_items.len());
        for item in offline_items {
            log::info!("  - {}", item.name);
        }
    }

    // Step 11: Demonstrate custom filtering
    log::info!("=== Custom Filter Demo ===");
    let large_items = library.filter(|item| item.file_size > 1024 * 1024); // > 1MB
    log::info!("Items larger than 1MB: {}", large_items.len());
    for item in large_items {
        log::info!("  - {} ({} bytes)", item.name, item.file_size);
    }

    // Step 12: Cache management (now integrated into MediaLibrary)
    log::info!("=== Cache Management Demo ===");
    log::info!("Cache size: {} entries", library.cache_size());

    // Cleanup expired cache entries
    library.cleanup_cache()?;
    log::info!("Cache cleanup complete");

    // Step 13: Serialization and deserialization test
    log::info!("=== Serialization/Deserialization Demo ===");
    let json = library.to_json(true)?;
    log::info!("  Serialized library to JSON ({} bytes)", json.len());

    // Test deserialization and cache rebuilding
    match MediaLibrary::from_json(&json) {
        Ok(restored_library) => {
            log::info!("  Successfully restored library from JSON");
            log::info!(
                "  Restored library has {} items",
                restored_library.item_count()
            );
            log::info!("  Cache is configured: {}", restored_library.has_cache());

            // Verify thumbnails are preserved
            for item in restored_library.all_items().iter().take(3) {
                let thumb_status = if item.thumbnail_path.is_some() {
                    "has thumbnail".to_string()
                } else {
                    "no thumbnail".to_string()
                };
                log::info!("    {} - {}", item.name, thumb_status);
            }
        }
        Err(e) => {
            log::warn!("  Failed to restore library: {}", e);
        }
    }

    log::info!("=== Demo Complete ===");
    Ok(())
}
