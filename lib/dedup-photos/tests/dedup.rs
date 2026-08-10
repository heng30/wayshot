use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use dedup_photos::{
    dedup_directory, dedup_directory_with, CancellationToken, DedupError, DedupOptions,
    DedupReason, KeepStrategy, ProgressEvent, Stage,
};

fn make_image(path: &Path, w: u32, h: u32, r: u8, g: u8, b: u8) {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb([r, g, b]));
    img.save(path).unwrap();
}

fn gradient(w: u32, h: u32) -> image::RgbImage {
    let mut img = image::RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let v = (x * 255 / w.max(1)) as u8;
            img.put_pixel(x, y, image::Rgb([v, v, v]));
        }
    }
    img
}

fn noise(w: u32, h: u32, seed: u64) -> image::RgbImage {
    let mut img = image::RgbImage::new(w, h);
    let mut x = seed;
    for y in 0..h {
        for xx in 0..w {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let v = (x >> 33) as u8;
            img.put_pixel(xx, y, image::Rgb([v, v, v]));
        }
    }
    img
}

#[test]
fn exact_duplicates_are_moved_with_report() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    make_image(&root.join("a.png"), 32, 32, 255, 0, 0);
    fs::copy(root.join("a.png"), root.join("b.png")).unwrap();
    make_image(&root.join("c.png"), 32, 32, 0, 0, 255);

    let result = dedup_directory(root, &DedupOptions::default()).unwrap();
    assert_eq!(result.summary.groups, 1);
    assert_eq!(result.summary.moved_files, 1);
    assert_eq!(result.duplicates.len(), 1);

    let d = &result.duplicates[0];
    assert!(matches!(d.reason, DedupReason::Exact));
    assert!(!d.path.exists());
    assert!(d.moved_to.exists());
    assert!(d.kept_path.exists());
    assert_eq!(d.moved_to.parent().unwrap().file_name().unwrap(), "duplicate");

    let report = fs::read_to_string(&result.report_path).unwrap();
    let lines: Vec<&str> = report.lines().collect();
    let detail_lines: Vec<&str> = lines
        .iter()
        .filter(|l| l.contains("duplicate of"))
        .copied()
        .collect();
    assert_eq!(detail_lines.len(), 1);
    assert!(detail_lines[0].contains("exact duplicate"));
    assert!(detail_lines[0].contains("SHA-256 identical"));
    assert!(detail_lines[0].contains("b.png"));
}

#[test]
fn perceptual_duplicates_are_moved() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    gradient(100, 100).save(root.join("a.png")).unwrap();
    gradient(50, 50).save(root.join("b.png")).unwrap();
    noise(64, 64, 1).save(root.join("c.png")).unwrap();

    let result = dedup_directory(root, &DedupOptions::default()).unwrap();
    assert_eq!(result.summary.moved_files, 1);
    let d = &result.duplicates[0];
    assert!(matches!(
        d.reason,
        DedupReason::Perceptual { hamming_distance } if hamming_distance <= 10
    ));
    assert!(d.moved_to.exists());
}

#[test]
fn unrelated_images_are_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    noise(64, 64, 1).save(root.join("a.png")).unwrap();
    noise(64, 64, 2).save(root.join("b.png")).unwrap();
    noise(64, 64, 3).save(root.join("c.png")).unwrap();

    let result = dedup_directory(root, &DedupOptions::default()).unwrap();
    assert_eq!(result.summary.groups, 0);
    assert_eq!(result.summary.moved_files, 0);
    assert!(root.join("a.png").exists());
    assert!(root.join("b.png").exists());
    assert!(root.join("c.png").exists());
}

#[test]
fn keep_strategy_picks_newest() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    make_image(&root.join("a.png"), 32, 32, 255, 0, 0);
    fs::copy(root.join("a.png"), root.join("b.png")).unwrap();

    let old = fs::File::options().write(true).open(root.join("a.png")).unwrap();
    old.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000))
        .unwrap();
    let new = fs::File::options().write(true).open(root.join("b.png")).unwrap();
    new.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(2_000_000))
        .unwrap();

    let result = dedup_directory(
        root,
        &DedupOptions {
            keep: KeepStrategy::Newest,
            ..DedupOptions::default()
        },
    )
    .unwrap();
    assert_eq!(result.duplicates.len(), 1);
    assert_eq!(result.duplicates[0].path, root.join("a.png"));
    assert_eq!(result.duplicates[0].kept_path, root.join("b.png"));
}

#[test]
fn name_collision_in_duplicate_dir_gets_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    make_image(&root.join("a.png"), 32, 32, 255, 0, 0);
    fs::copy(root.join("a.png"), root.join("b.png")).unwrap();

    let dup = root.join("duplicate");
    fs::create_dir_all(&dup).unwrap();
    make_image(&dup.join("b.png"), 8, 8, 0, 255, 0);

    let result = dedup_directory(root, &DedupOptions::default()).unwrap();
    assert_eq!(result.duplicates.len(), 1);
    let moved = &result.duplicates[0].moved_to;
    assert_eq!(moved.file_name().unwrap(), "b_1.png");
    assert!(moved.exists());
    assert!(dup.join("b.png").exists());
}

#[test]
fn duplicate_dir_is_skipped_on_rescan() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    make_image(&root.join("a.png"), 32, 32, 255, 0, 0);
    fs::copy(root.join("a.png"), root.join("b.png")).unwrap();

    let first = dedup_directory(root, &DedupOptions::default()).unwrap();
    assert_eq!(first.summary.moved_files, 1);

    let second = dedup_directory(root, &DedupOptions::default()).unwrap();
    assert_eq!(second.summary.groups, 0);
    assert_eq!(second.summary.moved_files, 0);
}

#[test]
fn semantic_requires_model_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    make_image(&root.join("a.png"), 32, 32, 255, 0, 0);

    let options = DedupOptions {
        semantic: Some(dedup_photos::SemanticConfig {
            model_path: root.join("missing.onnx"),
            ..dedup_photos::SemanticConfig::default()
        }),
        ..DedupOptions::default()
    };
    let err = dedup_directory(root, &options).unwrap_err();
    assert!(matches!(err, DedupError::Semantic(_)));
}

#[test]
fn progress_events_cover_all_stages() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    make_image(&root.join("a.png"), 32, 32, 255, 0, 0);
    fs::copy(root.join("a.png"), root.join("b.png")).unwrap();
    noise(64, 64, 1).save(root.join("c.png")).unwrap();

    let events = Arc::new(Mutex::new(Vec::new()));
    {
        let events = events.clone();
        let cb = move |e: ProgressEvent| events.lock().unwrap().push(e);
        dedup_photos::dedup_directory_with(root, &DedupOptions::default(), Some(&cb), None)
            .unwrap();
    }

    let events = events.lock().unwrap();
    let stages: Vec<Stage> = events
        .iter()
        .filter_map(|e| match e {
            ProgressEvent::StageStarted { stage, .. } => Some(*stage),
            _ => None,
        })
        .collect();
    assert!(stages.contains(&Stage::Scan));
    assert!(stages.contains(&Stage::Exact));
    assert!(stages.contains(&Stage::Perceptual));
    assert!(stages.contains(&Stage::Move));
    assert!(stages.contains(&Stage::Report));

    let exact_done: Vec<(u64, u64)> = events
        .iter()
        .filter_map(|e| match e {
            ProgressEvent::ItemDone {
                stage: Stage::Exact,
                done,
                total,
            } => Some((*done, *total)),
            _ => None,
        })
        .collect();
    assert_eq!(exact_done.last(), Some(&(2, 2)));

    let move_total: u64 = events
        .iter()
        .filter_map(|e| match e {
            ProgressEvent::StageStarted {
                stage: Stage::Move,
                total,
            } => Some(*total),
            _ => None,
        })
        .next()
        .unwrap();
    assert_eq!(move_total, 1);
}

#[test]
fn pre_cancelled_token_aborts_immediately() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    make_image(&root.join("a.png"), 32, 32, 255, 0, 0);

    let token = CancellationToken::new();
    token.cancel();
    let err =
        dedup_directory_with(root, &DedupOptions::default(), None, Some(&token)).unwrap_err();
    assert!(matches!(err, DedupError::Cancelled));
}

#[test]
fn cancel_during_scan_stops_before_move() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    make_image(&root.join("a.png"), 32, 32, 255, 0, 0);
    fs::copy(root.join("a.png"), root.join("b.png")).unwrap();

    let token = CancellationToken::new();
    let token2 = token.clone();
    let cb = move |e: ProgressEvent| {
        if let ProgressEvent::ItemDone {
            stage: Stage::Scan, ..
        } = e
        {
            token2.cancel();
        }
    };
    let err = dedup_directory_with(root, &DedupOptions::default(), Some(&cb), Some(&token))
        .unwrap_err();
    assert!(matches!(err, DedupError::Cancelled));
    assert!(root.join("a.png").exists());
    assert!(root.join("b.png").exists());
}
