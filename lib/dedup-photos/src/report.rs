use crate::{DedupOptions, DedupSummary, Duplicate};
use std::path::Path;

pub fn render(
    root: &Path,
    options: &DedupOptions,
    duplicates: &[Duplicate],
    summary: &DedupSummary,
) -> String {
    let mut s = String::new();
    s.push_str("dedup-photos report\n");
    s.push_str("===================\n");
    s.push_str(&format!("scanned root     : {}\n", root.display()));
    s.push_str(&format!("scanned files    : {}\n", summary.scanned_files));
    s.push_str(&format!("duplicate groups : {}\n", summary.groups));
    s.push_str(&format!(
        "files moved to {} : {}\n",
        options.duplicate_dir_name, summary.moved_files
    ));
    s.push_str(&format!(
        "space moved      : {} bytes\n",
        summary.moved_bytes
    ));
    s.push_str(&format!("keep strategy    : {}\n", options.keep));
    s.push_str(&format!(
        "dHash threshold  : {} bits (of {})\n",
        options.threshold,
        crate::hash::DHASH_BITS
    ));
    match &options.semantic {
        Some(sem) => s.push_str(&format!(
            "semantic model   : {}\n",
            sem.model_path.display()
        )),
        None => s.push_str("semantic model   : disabled\n"),
    }
    if !summary.warnings.is_empty() {
        s.push_str("warnings         :\n");
        for w in &summary.warnings {
            s.push_str(&format!("  - {w}\n"));
        }
    }
    s.push('\n');

    for d in duplicates {
        s.push_str(&format!(
            "{} -> {}: {} of {} {}\n",
            d.path.display(),
            d.moved_to.display(),
            d.reason.label(),
            d.kept_path.display(),
            d.reason.detail(),
        ));
    }
    s
}
