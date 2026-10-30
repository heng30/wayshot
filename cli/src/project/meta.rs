use std::path::Path;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use video_editor::project::{load_project, save_project};

fn memo(project: &Path, set: Option<&str>) -> Result<Value> {
    let mut data = load_project(project)?;
    if let Some(text) = set {
        data.memo = text.to_string();
        save_project(&data, project).map_err(|e| anyhow!("保存工程失败: {e}"))?;
    }

    Ok(json!({
        "path": project.display().to_string(),
        "memo": data.memo,
        "saved": set.is_some(),
    }))
}
fn bookmarks(project: &Path) -> Result<Value> {
    let data = load_project(project)?;
    Ok(json!({
        "path": project.display().to_string(),
        "count": data.bookmarks.len(),
        "bookmarks": data.bookmarks.iter().enumerate().map(|(index, bookmark)| json!({
            "index": index,
            "time_ms": bookmark.time_ms,
            "time_secs": bookmark.time_ms as f64 / 1000.0,
        })).collect::<Vec<_>>(),
    }))
}
fn bookmark_add(project: &Path, time: f64) -> Result<Value> {
    if time < 0.0 {
        return Err(anyhow!("书签时间不能为负"));
    }
    let mut data = load_project(project)?;
    data.bookmarks.push(video_editor::project::BookmarkData {
        time_ms: (time * 1000.0) as u64,
    });
    data.bookmarks.sort_by_key(|bookmark| bookmark.time_ms);
    save_project(&data, project).map_err(|e| anyhow!("保存工程失败: {e}"))?;

    Ok(json!({ "path": project.display().to_string(), "count": data.bookmarks.len() }))
}
fn bookmark_remove(project: &Path, index: usize) -> Result<Value> {
    let mut data = load_project(project)?;
    if index >= data.bookmarks.len() {
        return Err(anyhow!("书签索引越界: {index}"));
    }
    data.bookmarks.remove(index);
    save_project(&data, project).map_err(|e| anyhow!("保存工程失败: {e}"))?;

    Ok(json!({ "path": project.display().to_string(), "count": data.bookmarks.len() }))
}

pub(super) fn run(cmd: &super::Cmd) -> Result<Value> {
    match cmd {
        super::Cmd::Memo { project, set } => memo(project, set.as_deref()),
        super::Cmd::Bookmarks { project } => bookmarks(project),
        super::Cmd::BookmarkAdd { project, time } => bookmark_add(project, *time),
        super::Cmd::BookmarkRemove { project, index } => bookmark_remove(project, *index),
        _ => unreachable!(),
    }
}
