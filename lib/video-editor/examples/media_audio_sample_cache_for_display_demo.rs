use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use video_editor::{
    metadata::get_metadata,
    preview::cache::{AudioCacheKey, get_global_audio_display_cache},
    tracks::segment::Segment,
};

fn main() {
    env_logger::init();

    let test_files = vec!["test.mp4", "test.wav"];

    for file_name in test_files {
        let file_path = PathBuf::from("data").join(file_name);

        log::info!("=== Testing audio display cache for: {} ===", file_name);

        // Get metadata
        let metadata = match get_metadata(&file_path) {
            Ok(meta) => Arc::new(meta),
            Err(e) => {
                log::warn!(
                    "Skipping `{}`: failed to get metadata: {}",
                    file_path.display(),
                    e
                );
                continue;
            }
        };

        // Check if has audio stream
        let audio_meta = match metadata.audios.first() {
            Some(audio) => audio,
            None => {
                log::warn!("Skipping `{}`: no audio stream found", file_path.display());
                continue;
            }
        };

        log::info!("Source audio metadata:");
        log::info!("  - Channels: {}", audio_meta.channels);
        log::info!("  - Sample rate: {} Hz", audio_meta.sample_rate);
        log::info!("  - Duration: {:.2}s", metadata.duration.as_secs_f64());

        // Test 1: Load and cache (first load)
        log::info!("\n--- Test 1: First load (should decode) ---");
        let cache = get_global_audio_display_cache();
        let start = Instant::now();

        let cache_data = match cache.load_and_cache(&file_path, audio_meta.index, audio_meta) {
            Ok(data) => data,
            Err(e) => {
                log::warn!("Failed to load cache: {:?}", e);
                continue;
            }
        };

        log::info!("First load took: {:.2}ms", start.elapsed().as_millis());
        log::info!("Cached data:");
        log::info!("  - Samples: {}", cache_data.samples.len());
        log::info!("  - Cache sample rate: {} Hz", cache_data.cache_sample_rate);
        log::info!("  - Channels: {}", cache_data.channels);
        log::info!("  - Duration: {:.2}s", cache_data.duration.as_secs_f64());

        // Calculate memory usage
        let memory_kb = cache_data.samples.len() * 4 / 1024; // f32 = 4 bytes
        log::info!("  - Memory usage: ~{} KB", memory_kb);

        // Test 2: Load again (should return cached)
        log::info!("\n--- Test 2: Second load (should use cache) ---");
        let start = Instant::now();

        let cache_data2 = match cache.load_and_cache(&file_path, audio_meta.index, audio_meta) {
            Ok(data) => data,
            Err(e) => {
                log::warn!("Failed to load cache: {:?}", e);
                continue;
            }
        };

        log::info!(
            "Second load took: {:.2}ms (should be much faster)",
            start.elapsed().as_millis()
        );

        // Verify same data
        assert!(
            Arc::ptr_eq(&cache_data, &cache_data2),
            "Should return the same Arc"
        );
        log::info!("Verified: Same Arc returned (no duplicate data)");

        // Test 3: Get by path
        log::info!("\n--- Test 3: Get by path ---");
        let cached = cache.get_by_path(&file_path, audio_meta.index);
        assert!(cached.is_some(), "Should find cached data");
        log::info!("get_by_path() works correctly");

        // Test 4: Extract segments
        log::info!("\n--- Test 4: Extract segments from cache ---");
        let segment_duration = metadata.duration.min(Duration::from_secs(5));
        let segment = Segment::new_with_source_offset(
            Duration::ZERO,
            Duration::ZERO,
            segment_duration,
            1.0,
            1.0,
            metadata.clone(),
        );

        // Test different sample counts
        let sample_counts = vec![50, 100, 200];

        for samples_per_channel in sample_counts {
            let start = Instant::now();
            let (channels, samples) = segment.audio_resampling_for_display(samples_per_channel);

            log::info!(
                "Extract {} samples: {:.2}ms, channels={}, actual samples={}",
                samples_per_channel,
                start.elapsed().as_millis(),
                channels,
                samples.len()
            );

            // Show some samples
            if !samples.is_empty() {
                let max = samples.iter().fold(0.0_f32, |a, b| a.max(*b));
                let avg: f32 = samples.iter().sum::<f32>() / samples.len() as f32;
                log::info!("  - Max: {:.4}, Avg: {:.4}", max, avg);
            }
        }

        // Test 5: Extract with source offset
        log::info!("\n--- Test 5: Extract with source offset ---");
        if metadata.duration > Duration::from_secs(2) {
            let segment_with_offset = Segment::new_with_source_offset(
                Duration::ZERO,
                Duration::from_secs(1), // Start from 1 second
                Duration::from_secs(2), // Duration 2 seconds
                1.0,
                1.0,
                metadata.clone(),
            );

            let start = Instant::now();
            let (channels, samples) = segment_with_offset.audio_resampling_for_display(100);

            log::info!(
                "Extract from 1s-3s: {:.2}ms, channels={}, samples={}",
                start.elapsed().as_millis(),
                channels,
                samples.len()
            );
        }

        // Test 6: Concurrent loading simulation
        log::info!("\n--- Test 6: Concurrent loading (same key) ---");
        // Clear cache first
        let audio_index = audio_meta.index;
        cache.remove_by_path(&file_path, audio_index);

        let start = Instant::now();

        // Spawn multiple threads trying to load the same file
        let handles: Vec<_> = (0..3)
            .map(|i| {
                let file_path = file_path.clone();
                let audio_meta = audio_meta.clone();
                std::thread::spawn(move || {
                    let cache = get_global_audio_display_cache();
                    let thread_start = Instant::now();
                    let result = cache.load_and_cache(&file_path, audio_meta.index, &audio_meta);
                    log::info!(
                        "  Thread {}: completed in {:.2}ms",
                        i,
                        thread_start.elapsed().as_millis()
                    );
                    result
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            let _ = handle.join();
        }

        log::info!(
            "All threads completed in {:.2}ms (only one should decode)",
            start.elapsed().as_millis()
        );

        log::info!("\n");
    }

    // Test 7: Check if loading flag works
    log::info!("=== Test 7: Loading state check ===");
    let file_path = PathBuf::from("data/test.mp4");
    if let Ok(metadata) = get_metadata(&file_path) {
        if let Some(audio_meta) = metadata.audios.first() {
            let cache = get_global_audio_display_cache();
            let key = AudioCacheKey::from_path(&file_path, audio_meta.index);

            // Clear cache
            cache.remove(&key);

            log::info!("Before load: is_loading = {}", cache.is_loading(&key));

            // Start loading in a separate thread
            let file_path_clone = file_path.clone();
            let audio_meta_clone = audio_meta.clone();
            let handle = std::thread::spawn(move || {
                let cache = get_global_audio_display_cache();
                cache.load_and_cache(&file_path_clone, audio_meta_clone.index, &audio_meta_clone)
            });

            // Small delay to let the other thread start loading
            std::thread::sleep(Duration::from_millis(10));

            log::info!("During load: is_loading = {}", cache.is_loading(&key));

            // Wait for completion
            let _ = handle.join();

            log::info!("After load: is_loading = {}", cache.is_loading(&key));
        }
    }

    log::info!("\n=== All tests completed ===");
}

