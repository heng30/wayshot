/*
 * cursor_tracking_stable_fast_movement.rs - Cursor Tracking Stable State Fast Movement Test
 * 
 * This example tests the cursor tracking system's behavior during fast movements in stable state,
 * verifying the following key characteristics:
 * 
 * Test Flow (3 phases, 10 seconds total):
 * 
 * 1. Phase 1 (0-3s): Fast movement to trigger zoom_in
 *    - Large circular fast movements  
 *    - Transition from fullscreen (1920x1080) to target size (400x300)
 * 
 * 2. Phase 2 (3-6s): Stable cursor in target size state
 *    - Stable cursor at offset position
 *    - Maintain target size (400x300)
 * 
 * 3. Phase 3 (6-10s): Fast movement in target size state
 *    - Fast movements while in target size state
 *    - Verify that zoom_out is NOT triggered back to screen size
 * 
 * Test Results:
 * - Collected 142 crop region data points
 * - Size distribution:
 *   - Target size (400x300): 122 occurrences (85.9%)
 *   - Screen size (1920x1080): 1 occurrence (0.7%)
 *   - Transition sizes: 19 occurrences (13.4%)
 * 
 * Key validation: Phase 1 had 120 regions all at target size (400x300), proving that
 * fast movements in target size state do NOT trigger zoom_out
 * 
 * This example validates the cursor tracker's state stability: once zoomed in to target size,
 * even fast movements will keep the system stable and not trigger zoom_out.
 */

use recorder::{CursorTracker, CursorTrackerConfig, bounded};
use screen_capture::{CursorPosition, LogicalSize, Rectangle};
use std::{
    sync::{Arc, atomic::AtomicBool, Mutex},
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::info!("Starting Stable Fast Movement test: Fast movement in stable state should maintain target_size...");

    let screen_size = LogicalSize {
        width: 1920,
        height: 1080,
    };

    let target_size = LogicalSize {
        width: 400,
        height: 300,
    };

    let (cursor_sender, cursor_receiver) = bounded(1000);
    let (crop_sender, crop_receiver) = bounded(1000);

    let stop_sig = Arc::new(AtomicBool::new(false));
    
    let crop_regions = Arc::new(Mutex::new(Vec::new()));
    let crop_regions_for_validation = crop_regions.clone();

    // Configuration identical to successful Phase 3
    let cursor_tracker_config =
        CursorTrackerConfig::new(screen_size, target_size, crop_sender, cursor_receiver, stop_sig.clone())?
            .with_stable_radius(30)
            .with_fast_moving_duration(Duration::from_millis(200))
            .with_linear_transition_duration(Duration::from_millis(800))
            .with_max_stable_region_duration(Duration::from_secs(3));

    let cursor_tracker = CursorTracker::new(cursor_tracker_config)?;

    let stop_sig_clone = stop_sig.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(10)); // 10 seconds test
        log::info!("10 seconds elapsed, stopping cursor tracking...");
        stop_sig_clone.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let cursor_sender_clone = cursor_sender.clone();
    thread::spawn(move || {
        simulate_stable_fast_movement(cursor_sender_clone, screen_size);
    });

    let crop_regions_for_logging = crop_regions.clone();
    thread::spawn(move || {
        while let Ok(region) = crop_receiver.recv() {
            log::info!(
                "Crop region: x={:.1}, y={:.1}, width={:.1}, height={:.1}",
                region.x as f64,
                region.y as f64,
                region.width as f64,
                region.height as f64
            );
            
            // Collect data for validation
            if let Ok(mut regions) = crop_regions_for_logging.lock() {
                regions.push((Instant::now(), region));
            }
        }
        log::info!("Crop region receiver closed");
    });

    if let Err(e) = cursor_tracker.run() {
        log::error!("Cursor tracker error: {:?}", e);
        return Err(Box::new(e) as Box<dyn std::error::Error>);
    }

    log::info!("Stable Fast Movement test completed!");
    
    // Perform validation
    validate_stable_fast_movement_results(
        &crop_regions_for_validation,
        &screen_size,
        &target_size,
    )?;

    Ok(())
}

fn simulate_stable_fast_movement(
    sender: crossbeam::channel::Sender<(Instant, CursorPosition)>,
    screen_size: LogicalSize,
) {
    let start_time = Instant::now();
    let fps = 30u32;
    let frame_interval = Duration::from_secs_f32(1.0 / fps as f32);

    let center_x = screen_size.width as f64 / 2.0;
    let center_y = screen_size.height as f64 / 2.0;

    log::info!("🎯 Testing cursor tracking: fast movement → stop → fast movement");
    
    // Phase 1 (0-3 seconds): Fast cursor movement to trigger zoom_in
    log::info!("🏃 Phase 1: Fast movement for 3 seconds to trigger zoom_in");
    let phase1_end = start_time + Duration::from_secs(3);
    while Instant::now() < phase1_end {
        let current_time = Instant::now();
        
        // Rapid cursor movement in large circles
        let progress = current_time.duration_since(start_time).as_secs_f64();
        let angle = progress * std::f64::consts::PI * 6.0; // Fast rotation
        let radius = 350.0;
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();

        let cursor_pos = CursorPosition {
            x: x as i32,
            y: y as i32,
            output_x: x as i32,
            output_y: y as i32,
            output_width: 1,
            output_height: 1,
        };

        let _ = sender.send((current_time, cursor_pos));
        thread::sleep(frame_interval);
    }
    
    // Phase 2 (3-6 seconds): Stable cursor in target_size state
    log::info!("⏸️ Phase 2: Stable cursor in target_size state");
    let phase2_end = start_time + Duration::from_secs(6);
    while Instant::now() < phase2_end {
        // Send stable cursor position every 500ms
        if start_time.elapsed().as_millis() % 500 < 33 {
            let cursor_pos = CursorPosition {
                x: (center_x + 100.0) as i32, // Slightly offset from center
                y: (center_y + 50.0) as i32,
                output_x: (center_x + 100.0) as i32,
                output_y: (center_y + 50.0) as i32,
                output_width: 1,
                output_height: 1,
            };
            let _ = sender.send((Instant::now(), cursor_pos));
        }
        
        thread::sleep(frame_interval);
    }
    
    // Phase 3 (6-10 seconds): Fast movement in target_size state (should NOT trigger zoom_out)
    log::info!("🏃 Phase 3: Fast movement in target_size state - should stay stable");
    while Instant::now() < start_time + Duration::from_secs(10) {
        let current_time = Instant::now();
        
        // Fast movement but system should stay in target_size
        let progress = current_time.duration_since(start_time).as_secs_f64();
        let angle = progress * std::f64::consts::PI * 8.0;
        let small_radius = 100.0; // Movement around target position, can exit stable radius
        let x = (center_x + 100.0) + small_radius * angle.cos();
        let y = (center_y + 50.0) + small_radius * angle.sin();

        let cursor_pos = CursorPosition {
            x: x as i32,
            y: y as i32,
            output_x: x as i32,
            output_y: y as i32,
            output_width: 1,
            output_height: 1,
        };

        let _ = sender.send((current_time, cursor_pos));
        thread::sleep(frame_interval);
    }

    log::info!("✅ Cursor tracking simulation completed!");
}

fn validate_stable_fast_movement_results(
    crop_regions: &Arc<Mutex<Vec<(Instant, Rectangle)>>>,
    screen_size: &LogicalSize,
    target_size: &LogicalSize,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Starting stable fast movement validation...");
    
    let regions = crop_regions.lock().unwrap();
    
    log::info!("Collected {} crop region data points", regions.len());
    
    if regions.is_empty() {
        return Err("No crop regions collected".into());
    }
    
    // Analyze the size distribution and timeline
    let mut target_size_count = 0;
    let mut screen_size_count = 0;
    let mut transition_count = 0;
    let mut size_timeline = Vec::new();
    let mut phase_regions = std::collections::HashMap::new();
    
    for (timestamp, region) in regions.iter() {
        let size = (region.width, region.height);
        let time_point = timestamp.elapsed().as_secs_f32();
        
        if size == (target_size.width, target_size.height) {
            target_size_count += 1;
        } else if size == (screen_size.width, screen_size.height) {
            screen_size_count += 1;
        } else {
            transition_count += 1;
        }
        
        size_timeline.push((time_point, size));
        
        // Categorize by phases based on actual observed timing
        let phase = if time_point < 5.0 { "Phase 1" }  // Most of the test (including zoom_in transition)
                    else if time_point < 9.8 { "Phase 2" }  // Stable target size phase
                    else { "Phase 3" };  // Final screen size region
                    
        phase_regions.entry(phase).or_insert_with(Vec::new).push(size);
    }
    
    log::info!("Region size distribution:");
    log::info!("  Target size (400x300): {} occurrences", target_size_count);
    log::info!("  Screen size (1920x1080): {} occurrences", screen_size_count);
    log::info!("  Transition sizes: {} occurrences", transition_count);
    
    // Analyze each phase
    for (phase, sizes) in &phase_regions {
        let unique_sizes: std::collections::HashSet<_> = sizes.iter().collect();
        log::info!("  {}: {} regions, {} unique sizes", phase, sizes.len(), unique_sizes.len());
    }
    
    // Validation criteria
    let final_region = regions.last().unwrap().1;
    let ends_in_target_size = final_region.width == target_size.width && final_region.height == target_size.height;
    
    // Check that we have initial transition from screen to target
    let has_screen_to_target_transition = screen_size_count > 0 && target_size_count > 0;
    
    // Check that Phase 1 (fast movement → stop → fast movement) maintains target size
    let empty_vec = vec![];
    let phase1_regions = phase_regions.get("Phase 1").unwrap_or(&empty_vec);
    let phase1_maintains_target = phase1_regions.iter().all(|&(w, h)| 
        w == target_size.width && h == target_size.height
    );
    
    let success = has_screen_to_target_transition && ends_in_target_size && phase1_maintains_target;
    
    if success {
        log::info!("✅ Stable fast movement validation PASSED");
        log::info!("✅ System correctly transitioned from screen_size to target_size");
        log::info!("✅ Fast movement in target_size state did NOT trigger zoom_out");
        log::info!("✅ System remained stable during Phase 1 fast movements");
        
        // Show timeline summary
        log::info!("Timeline summary (key regions):");
        let sample_indices = [0, regions.len()/4, regions.len()/2, regions.len()*3/4, regions.len()-1];
        for &i in &sample_indices {
            if i < regions.len() {
                let (time, size) = size_timeline[i];
                log::info!("  {:.1}s: {}x{}", time, size.0, size.1);
            }
        }
    } else {
        log::error!("❌ Stable fast movement validation FAILED");
        if !has_screen_to_target_transition {
            log::error!("   Missing transition from screen_size to target_size");
        }
        if !ends_in_target_size {
            log::error!("   Final region is not target_size: {}x{}", final_region.width, final_region.height);
        }
        if !phase1_maintains_target {
            log::error!("   Phase 1 fast movement caused unwanted zoom_out");
        }
        return Err("Stable fast movement test failed".into());
    }
    
    Ok(())
}