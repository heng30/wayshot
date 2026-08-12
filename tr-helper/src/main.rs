use anyhow::Result;
use clap::Parser;
use regex::Regex;
use std::{collections::HashSet, ffi::OsStr, fs, path::Path};

mod tr;
use tr::tr;

#[derive(Parser, Debug)]
#[command(
    name = "tr-helper",
    version = "v1.0.0",
    about = "A tool to extract all sentences, needed to be translated, of current project.",
    long_about = None
)]

struct Args {
    /// Input directory
    #[arg(short, long, default_value = ".")]
    input_dir: String,

    /// Output file
    #[arg(short, long)]
    output_file: Option<String>,

    #[arg(short, long, default_value = "cn")]
    language: String,

    /// Directories to exclude from scanning, matched by name at any depth (comma-separated or repeated)
    #[arg(short = 'x', long)]
    exclude_dirs: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let translations = extract_tr(Path::new(&args.input_dir), &args.exclude_dirs)?;

    let mut final_translations = vec![];
    for item in translations {
        if tr(&item, args.language.clone()) == item {
            final_translations.push(item);
        }
    }

    final_translations.sort();

    if let Some(path) = args.output_file {
        let contents = final_translations.join("\n");
        fs::write(path, contents)?;
    } else {
        for item in final_translations {
            println!("{}", item);
        }
    }

    Ok(())
}

fn extract_tr(target_dir: &Path, exclude_dirs: &[String]) -> Result<Vec<String>> {
    let mut translations = HashSet::new();

    // Compile regex patterns
    let pattern =
        Regex::new(r#"(?:^|[^\w.])(Logic\.tr\("([^"\\]|\\.)*"\)|tr\("([^"\\]|\\.)*"\))"#)?;
    let extract_pattern = Regex::new(r#"(?:^|[^\w.])(?:Logic\.tr\("|tr\(")([^"]*)(?:"\))"#)?;

    // Walk through directory
    for entry in walkdir::WalkDir::new(target_dir)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), exclude_dirs))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Process only .slint and .rs files
        if let Some(ext) = path.extension()
            && (ext == "slint" || ext == "rs")
            && let Ok(content) = fs::read_to_string(path)
        {
            for cap in pattern.captures_iter(&content) {
                if let Some(matched) = cap.get(0)
                    && let Some(inner_cap) = extract_pattern.captures(matched.as_str())
                    && let Some(translation) = inner_cap.get(1)
                {
                    translations.insert(translation.as_str().to_string());
                }
            }
        }
    }

    Ok(translations.into_iter().collect())
}

fn is_excluded(path: &Path, exclude_dirs: &[String]) -> bool {
    if path
        .components()
        .any(|c| c.as_os_str() == ".git" || c.as_os_str() == "target")
    {
        return true;
    }
    exclude_dirs
        .iter()
        .any(|dir| path.components().any(|c| c.as_os_str() == OsStr::new(dir)))
}
