use crate::error::McpError;
use crate::state::{self, UiAction};
use crate::types::ProjectStatus;

/// Get the current project status
pub fn get_status() -> Result<ProjectStatus, McpError> {
    let project_path = state::get_project_path();
    let is_open = project_path.is_some();

    let (track_count, total_segments, duration_ms) =
        if let Some(manager) = state::get_tracks_manager() {
            let track_count = manager.len();
            let total_segments: usize = manager.tracks.iter().map(|t| t.segments_count()).sum();
            let duration_ms = manager.duration.as_millis() as u64;
            (track_count, total_segments, duration_ms)
        } else {
            (0, 0, 0)
        };

    Ok(ProjectStatus {
        is_open,
        project_path,
        is_unsaved: state::is_unsaved(),
        track_count,
        total_segments,
        duration_ms,
        can_undo: state::can_undo(),
        can_redo: state::can_redo(),
    })
}

/// Create a new project — dispatches CreateProject which creates the project file directly.
/// This does NOT open a dialog — the project is created at dir_path/name.wayshot.
pub fn create_project(name: String, dir_path: String) -> Result<serde_json::Value, McpError> {
    let mut path = std::path::PathBuf::from(&dir_path);
    path.push(&name);
    path.set_extension("wayshot");
    let path_str = path.to_string_lossy().to_string();

    state::dispatch_action(UiAction::CreateProject { name, dir_path });
    Ok(serde_json::json!({
        "success": true,
        "project_path": path_str
    }))
}

/// Open an existing project by path — dispatches OpenProjectPath which opens the file directly.
/// This does NOT open a dialog.
pub fn open_project(path: String) -> Result<serde_json::Value, McpError> {
    state::dispatch_action(UiAction::OpenProjectPath { path: path.clone() });
    Ok(serde_json::json!({
        "success": true,
        "project_path": path
    }))
}

/// Close the current project
pub fn close_project() -> Result<(), McpError> {
    state::dispatch_action(UiAction::CloseProject);
    Ok(())
}

/// Undo the last operation
pub fn undo() -> Result<String, McpError> {
    state::dispatch_action(UiAction::Undo);
    Ok("Undo dispatched".to_string())
}

/// Redo the last undone operation
pub fn redo() -> Result<String, McpError> {
    state::dispatch_action(UiAction::Redo);
    Ok("Redo dispatched".to_string())
}
