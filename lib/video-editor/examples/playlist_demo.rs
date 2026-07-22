use std::{path::PathBuf, time::Duration};
use video_editor::media::{MediaType, playlist::Playlist};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::info!("=== Simple Playlist Demo ===");

    // Create playlist with cache configuration
    let cache_dir = PathBuf::from("tmp").join("cache");
    let mut playlist = Playlist::new("Videos".to_string()).with_cache_configured(
        cache_dir,
        160,
        90,
        Duration::from_secs(86400),
    )?;

    let video_files = vec![
        "data/test.mp4",
        "data/test.mkv",
        "data/test.wav",
        "data/test.png",
        "data/test.srt",
    ];

    for file_path in &video_files {
        match playlist.add_file(PathBuf::from(file_path)) {
            Ok(index) => {
                let item = &playlist.items[index];
                log::info!(
                    "  Added: {} ({}, {})",
                    item.name,
                    item.media_type.as_str(),
                    item.format_duration()
                );
            }
            Err(e) => {
                log::warn!("  Failed to add {}: {}", file_path, e);
            }
        }
    }

    log::info!(
        "  Created video playlist with {} items",
        playlist.item_count()
    );

    log::info!("Query and filter...");

    for (i, item) in playlist.items.iter().enumerate() {
        log::info!("  [{}] {} ({})", i, item.name, item.format_duration());
    }

    // Get audio items
    let audio_items = playlist.items_by_type(MediaType::Audio);
    log::info!("  Audio items: {}", audio_items.len());

    // Search items
    let results = playlist.search("song");
    log::info!("  Search results for 'song': {}", results.len());

    log::info!("Modifying playlist...");

    // Get item
    if let Some(item) = playlist.get_item(0) {
        log::info!("  First item: {}", item.name);
    }

    // Move an item
    if playlist.item_count() > 1 {
        playlist.move_item(0, 1)?;
        log::info!("  Moved item 0 to position 1");
    }

    // Remove an item
    if playlist.item_count() > 0 {
        let removed = playlist.remove_item(0)?;
        log::info!("  Removed item: {}", removed.name);
    }

    log::info!("  Remaining items: {}", playlist.item_count());

    log::info!("Serializing modified playlist...");
    let modified_json = playlist.to_json(true)?;
    log::info!("  Modified playlist JSON:\n{}", modified_json);

    // Test deserialization and cache rebuilding
    log::info!("Testing deserialization and cache rebuilding...");
    match Playlist::from_json(&modified_json) {
        Ok(restored_playlist) => {
            log::info!("  Successfully restored playlist from JSON");
            let count: usize = restored_playlist.item_count();
            log::info!(
                "  Restored playlist has {} items",
                count
            );
            log::info!("  Cache is configured: {}", restored_playlist.has_cache());

            // Verify thumbnails are still present
            for (i, item) in restored_playlist.items.iter().enumerate() {
                let thumb_status = if item.thumbnail_path.is_some() {
                    format!("has thumbnail: {:?}", item.thumbnail_path)
                } else {
                    "no thumbnail".to_string()
                };
                log::info!("    [{}] {} - {}", i, item.name, thumb_status);
            }
        }
        Err(e) => {
            log::warn!("  Failed to restore playlist: {}", e);
        }
    }

    log::info!("=== Demo Complete ===");

    Ok(())
}
