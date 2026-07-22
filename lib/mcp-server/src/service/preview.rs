use crate::error::McpError;
use crate::state::{self, UiAction};

/// Seek the preview to a position
pub fn seek(position_ms: u64) -> Result<(), McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    state::dispatch_action(UiAction::PreviewSeek { position_ms: position_ms as i32 });
    Ok(())
}

/// Get the current preview info
pub fn get_preview_info() -> Result<serde_json::Value, McpError> {
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    Ok(serde_json::json!({
        "duration_ms": manager.duration.as_millis() as u64,
        "track_count": manager.len(),
    }))
}
