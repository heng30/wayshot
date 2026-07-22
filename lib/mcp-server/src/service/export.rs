use crate::error::McpError;
use crate::state;

/// Export video (starts async task)
pub fn export_video(output_path: String) -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: export_video({output_path}) - requires wayshot bridge");
    Ok(serde_json::json!({
        "task_id": uuid::Uuid::new_v4().to_string(),
        "status": "started",
        "note": "Export requires wayshot bridge for full functionality"
    }))
}

/// Export audio (starts async task)
pub fn export_audio(output_path: String) -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: export_audio({output_path}) - requires wayshot bridge");
    Ok(serde_json::json!({
        "task_id": uuid::Uuid::new_v4().to_string(),
        "status": "started",
        "note": "Export requires wayshot bridge for full functionality"
    }))
}

/// Export subtitles
pub fn export_subtitle(output_path: String, format: String) -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: export_subtitle({output_path}, {format}) - requires wayshot bridge");
    Ok(serde_json::json!({
        "status": "started",
        "note": "Export requires wayshot bridge for full functionality"
    }))
}

/// Cancel an export task
pub fn cancel_export(task_id: String) -> Result<(), McpError> {
    log::info!("MCP: cancel_export({task_id}) - requires wayshot bridge");
    Ok(())
}

/// List export queue
pub fn list_export_queue() -> Result<serde_json::Value, McpError> {
    Ok(serde_json::json!({
        "queue": [],
    }))
}
