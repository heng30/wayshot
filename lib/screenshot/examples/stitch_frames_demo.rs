use screenshot::{Algorithm, StitchConfig, StitchOutcome, Stitcher};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <input_dir> <output.png> [algorithm] [limit]",
            args[0]
        );
        std::process::exit(1);
    }

    let input_dir = &args[1];
    let output_path = &args[2];
    let algo_name = args.get(3).map(|s| s.as_str()).unwrap_or("template");
    let limit: usize = args
        .get(4)
        .and_then(|s| s.parse().ok())
        .unwrap_or(usize::MAX);

    let algorithm = match algo_name {
        "template" => Algorithm::Template,
        "colsample" => Algorithm::ColSample,
        _ => {
            eprintln!(
                "Unknown algorithm: {}. Use: fast, template, colsample",
                algo_name
            );
            std::process::exit(1);
        }
    };

    let mut entries: Vec<_> = std::fs::read_dir(input_dir)
        .unwrap_or_else(|e| {
            eprintln!("Failed to read {}: {}", input_dir, e);
            std::process::exit(1);
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if entries.is_empty() {
        eprintln!("No .png frames found in {}", input_dir);
        std::process::exit(1);
    }

    let mut images = Vec::new();
    for entry in &entries {
        if images.len() >= limit {
            break;
        }
        let path = entry.path();
        let img = image::open(&path).unwrap_or_else(|e| {
            eprintln!("Failed to open {}: {}", path.display(), e);
            std::process::exit(1);
        });
        images.push(img.to_rgba8());
    }

    eprintln!(
        "Stitching {} frames with {} algorithm...",
        images.len(),
        algo_name
    );

    let config = StitchConfig {
        algorithm,
        min_overlap: 200,
        ..StitchConfig::default()
    };

    let mut stitcher = Stitcher::new(config);
    let mut appended_count = 0;
    let mut no_progress_count = 0;
    let mut no_match_count = 0;
    let mut total_added = 0u32;

    for (i, frame) in images.into_iter().enumerate() {
        let outcome = stitcher.push_frame(frame);
        match outcome {
            StitchOutcome::FirstFrame => eprintln!("  frame {}: FirstFrame", i),
            StitchOutcome::Appended { added } => {
                appended_count += 1;
                total_added += added;
                eprintln!("  frame {}: Appended, added={}", i, added);
            }
            StitchOutcome::NoProgress => {
                no_progress_count += 1;
            }
            StitchOutcome::NoMatch => {
                no_match_count += 1;
                eprintln!("  frame {}: NoMatch", i);
            }
        }
    }

    eprintln!(
        "\nSummary: appended={}, no_progress={}, no_match={}, total_added={}px",
        appended_count, no_progress_count, no_match_count, total_added
    );

    if let Some(image) = stitcher.into_image() {
        eprintln!("Result: {}x{}", image.width(), image.height());
        image.save(output_path).unwrap_or_else(|e| {
            eprintln!("Failed to save {}: {}", output_path, e);
            std::process::exit(1);
        });
        eprintln!("Saved to {}", output_path);
    } else {
        eprintln!("No output image");
    }
}
