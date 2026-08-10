use clap::Parser;
use dedup_photos::{
    CancellationToken, DedupOptions, DedupReason, KeepStrategy, ProgressEvent, SemanticConfig,
    dedup_directory_with,
};
use image::{Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const EXPECTED_RESULTS_FILE: &str = "expected_results.json";

#[derive(Parser)]
#[command(
    name = "dedup-photos",
    about = "Triple-detection photo deduplicator (SHA-256 / dHash / CLIP)"
)]
struct Cli {
    /// Directory to scan (not required with --generate-test-images or --verify)
    #[arg(required_unless_present_any = ["generate_test_images", "verify"])]
    root: Option<PathBuf>,

    /// Hamming distance threshold for perceptual (dHash) detection
    #[arg(long, default_value_t = dedup_photos::DEFAULT_THRESHOLD)]
    threshold: u32,

    /// Path to the CLIP vision ONNX model (enables semantic detection)
    #[arg(long)]
    semantic_model: Option<PathBuf>,

    /// Cosine similarity cutoff for semantic detection
    #[arg(long, default_value_t = dedup_photos::DEFAULT_SEMANTIC_THRESHOLD)]
    semantic_threshold: f32,

    /// Which file in each group is kept: largest, newest, or oldest
    #[arg(long, default_value = "largest")]
    keep: String,

    /// Scan all file types instead of only images
    #[arg(long)]
    all_files: bool,

    /// Name of the directory duplicates are moved into (under the scan root)
    #[arg(long, default_value = dedup_photos::DEFAULT_DUPLICATE_DIR)]
    duplicate_dir: String,

    /// Generate N test images (including exact and perceptual duplicates) plus
    /// an expected-results file into --test-image-dir, then exit
    #[arg(long, value_name = "COUNT")]
    generate_test_images: Option<usize>,

    /// Directory to save generated test images
    #[arg(long, default_value = "test-images")]
    test_image_dir: PathBuf,

    /// Run dedup on DIR and compare the report against its expected_results.json
    #[arg(long, value_name = "DIR")]
    verify: Option<PathBuf>,

    /// Print per-stage progress while scanning and deduplicating
    #[arg(long)]
    progress: bool,
}

#[derive(Serialize, Deserialize)]
struct ExpectedDuplicate {
    file: String,
    reason: String,
    kept: String,
}

#[derive(Serialize, Deserialize)]
struct ExpectedResults {
    scanned_files: usize,
    groups: usize,
    moved_files: usize,
    duplicates: Vec<ExpectedDuplicate>,
}

fn main() {
    let cli = Cli::parse();

    let cancel = CancellationToken::new();
    let cancel_handler = cancel.clone();
    let _ = ctrlc::set_handler(move || {
        eprintln!("\nSIGINT received — cancelling...");
        cancel_handler.cancel();
    });

    if let Some(count) = cli.generate_test_images {
        match generate_test_images(&cli.test_image_dir, count) {
            Ok(n) => {
                println!(
                    "generated {n} test images in {}",
                    cli.test_image_dir.display()
                );
                if n > 0 {
                    match write_expected_results(&cli.test_image_dir, n, &cli.duplicate_dir) {
                        Ok(()) => println!(
                            "expected results written to {}",
                            cli.test_image_dir.join(EXPECTED_RESULTS_FILE).display()
                        ),
                        Err(e) => {
                            eprintln!("error: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                println!("next: dedup --verify {}", cli.test_image_dir.display());
            }
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let options = match build_options(&cli) {
        Ok(o) => o,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };

    if let Some(dir) = &cli.verify {
        match verify(dir, &options, &cancel) {
            Ok(pass) => std::process::exit(if pass { 0 } else { 1 }),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }

    let root = cli.root.as_deref().expect("root is required");
    let progress_cb = cli.progress.then_some(|event: ProgressEvent| match event {
        ProgressEvent::StageStarted { stage, total } => {
            if total > 0 {
                print!("\n[{stage:?}] 0/{total}");
                let _ = std::io::stdout().flush();
            }
        }
        ProgressEvent::ItemDone { stage, done, total } => {
            if total > 0 && (done % 25 == 0 || done == total) {
                print!("\r[{stage:?}] {done}/{total}   ");
                let _ = std::io::stdout().flush();
            }
        }
        ProgressEvent::StageFinished { stage, total } => {
            println!("\r[{stage:?}] {total}/{total} done");
        }
    });
    let progress_ref = progress_cb
        .as_ref()
        .map(|f| f as &(dyn Fn(ProgressEvent) + Sync));
    let result = dedup_directory_with(root, &options, progress_ref, Some(&cancel));
    match result {
        Ok(result) => {
            println!(
                "scanned {} files, {} duplicate group(s), moved {} file(s) ({} bytes)",
                result.summary.scanned_files,
                result.summary.groups,
                result.summary.moved_files,
                result.summary.moved_bytes,
            );
            println!("report: {}", result.report_path.display());
            for w in &result.summary.warnings {
                eprintln!("warning: {w}");
            }
        }
        Err(dedup_photos::DedupError::Cancelled) => {
            eprintln!("cancelled — already-moved files were left in place");
            std::process::exit(130);
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

fn build_options(cli: &Cli) -> Result<DedupOptions, String> {
    let keep = match cli.keep.as_str() {
        "largest" => KeepStrategy::Largest,
        "newest" => KeepStrategy::Newest,
        "oldest" => KeepStrategy::Oldest,
        other => {
            return Err(format!(
                "invalid --keep value '{other}' (expected largest, newest, oldest)"
            ));
        }
    };
    Ok(DedupOptions {
        threshold: cli.threshold,
        semantic: cli.semantic_model.clone().map(|model_path| SemanticConfig {
            model_path,
            threshold: cli.semantic_threshold,
        }),
        keep,
        all_files: cli.all_files,
        duplicate_dir_name: cli.duplicate_dir.clone(),
    })
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn reason_tag(reason: &DedupReason) -> String {
    match reason {
        DedupReason::Exact => "exact".to_string(),
        DedupReason::Perceptual { .. } => "perceptual".to_string(),
        DedupReason::Semantic { .. } => "semantic".to_string(),
    }
}

fn verify(
    dir: &Path,
    options: &DedupOptions,
    cancel: &CancellationToken,
) -> Result<bool, Box<dyn std::error::Error>> {
    let expected_path = dir.join(EXPECTED_RESULTS_FILE);
    if !expected_path.exists() {
        return Err(format!(
            "no expected results file at {} — generate test images first with --generate-test-images",
            expected_path.display()
        )
        .into());
    }
    let expected: ExpectedResults = serde_json::from_str(&fs::read_to_string(&expected_path)?)?;

    let result = match dedup_directory_with(dir, options, None, Some(cancel)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("dedup failed: {e}");
            return Ok(false);
        }
    };

    let mut problems: Vec<String> = Vec::new();

    if result.summary.scanned_files != expected.scanned_files {
        problems.push(format!(
            "scanned_files: expected {}, got {}",
            expected.scanned_files, result.summary.scanned_files
        ));
    }
    if result.summary.groups != expected.groups {
        problems.push(format!(
            "groups: expected {}, got {}",
            expected.groups, result.summary.groups
        ));
    }
    if result.summary.moved_files != expected.moved_files {
        problems.push(format!(
            "moved_files: expected {}, got {}",
            expected.moved_files, result.summary.moved_files
        ));
    }

    let mut actual: Vec<(String, String, String)> = result
        .duplicates
        .iter()
        .map(|d| {
            (
                file_name(&d.path),
                reason_tag(&d.reason),
                file_name(&d.kept_path),
            )
        })
        .collect();
    actual.sort();
    let mut exp: Vec<(String, String, String)> = expected
        .duplicates
        .iter()
        .map(|d| (d.file.clone(), d.reason.clone(), d.kept.clone()))
        .collect();
    exp.sort();

    if actual != exp {
        let actual_set: HashSet<&(String, String, String)> = actual.iter().collect();
        let exp_set: HashSet<&(String, String, String)> = exp.iter().collect();
        for e in exp_set.difference(&actual_set) {
            problems.push(format!("missing: {} ({}, kept {})", e.0, e.1, e.2));
        }
        for a in actual_set.difference(&exp_set) {
            problems.push(format!("unexpected: {} ({}, kept {})", a.0, a.1, a.2));
        }
    }

    if problems.is_empty() {
        println!(
            "PASS: report matches expected results ({} groups, {} files moved)",
            result.summary.groups, result.summary.moved_files
        );
        println!("report: {}", result.report_path.display());
        Ok(true)
    } else {
        println!("FAIL: report does not match expected results");
        for p in problems {
            println!("  - {p}");
        }
        Ok(false)
    }
}

fn gradient(w: u32, h: u32) -> RgbImage {
    let mut img = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let v = (x * 255 / w.max(1)) as u8;
            img.put_pixel(x, y, Rgb([v, v, v]));
        }
    }
    img
}

fn noise(w: u32, h: u32, seed: u64) -> RgbImage {
    let mut img = RgbImage::new(w, h);
    let mut x = seed;
    for y in 0..h {
        for xx in 0..w {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let v = (x >> 33) as u8;
            img.put_pixel(xx, y, Rgb([v, v, v]));
        }
    }
    img
}

fn generate_test_images(dir: &Path, count: usize) -> Result<usize, Box<dyn std::error::Error>> {
    fs::create_dir_all(dir)?;
    let _ = fs::remove_dir_all(dir.join("duplicate"));
    let mut generated = 0usize;

    if count >= 1 {
        gradient(320, 240).save(dir.join("gradient.png"))?;
        generated += 1;
    }
    if count >= 2 {
        fs::copy(dir.join("gradient.png"), dir.join("gradient_copy.png"))?;
        generated += 1;
    }
    if count >= 3 {
        gradient(160, 120).save(dir.join("gradient_small_a.jpg"))?;
        generated += 1;
    }
    if count >= 4 {
        gradient(80, 60).save(dir.join("gradient_small_b.jpg"))?;
        generated += 1;
    }
    let mut i = generated;
    while i < count {
        let w = 160 + ((i * 37) % 96) as u32;
        let h = 120 + ((i * 53) % 64) as u32;
        noise(w, h, i as u64).save(dir.join(format!("scene_{i}.png")))?;
        i += 1;
    }
    Ok(count)
}

fn write_expected_results(
    dir: &Path,
    count: usize,
    dup_dir_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = dir.join(EXPECTED_RESULTS_FILE);
    fs::write(&path, "{}")?;

    let files = dedup_photos::scan::collect_files(dir, false, &[dup_dir_name.to_string()], None);
    let mut expected = ExpectedResults {
        scanned_files: count,
        groups: 0,
        moved_files: 0,
        duplicates: Vec::new(),
    };

    if count >= 2 {
        expected.groups += 1;
        expected.moved_files += 1;
        let keep = files
            .iter()
            .find(|f| {
                let n = file_name(&f.path);
                n == "gradient.png" || n == "gradient_copy.png"
            })
            .map(|f| file_name(&f.path))
            .unwrap_or_else(|| "gradient.png".to_string());
        let moved = if keep == "gradient.png" {
            "gradient_copy.png"
        } else {
            "gradient.png"
        };
        expected.duplicates.push(ExpectedDuplicate {
            file: moved.to_string(),
            reason: "exact".to_string(),
            kept: keep,
        });
    }
    if count >= 4 {
        expected.groups += 1;
        expected.moved_files += 1;
        expected.duplicates.push(ExpectedDuplicate {
            file: "gradient_small_b.jpg".to_string(),
            reason: "perceptual".to_string(),
            kept: "gradient_small_a.jpg".to_string(),
        });
    }

    fs::write(&path, serde_json::to_string_pretty(&expected)?)?;
    Ok(())
}
