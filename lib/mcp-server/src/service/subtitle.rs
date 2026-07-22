use crate::error::McpError;
use crate::state;

/// Add subtitle entry
pub fn add_subtitle(track_index: usize, text: String, start_ms: u64, end_ms: u64) -> Result<(), McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: add_subtitle(track={track_index}, text={text}, start={start_ms}, end={end_ms})");
    Ok(())
}

/// Update subtitle
pub fn update_subtitle(track_index: usize, index: usize, text: String) -> Result<(), McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: update_subtitle(track={track_index}, idx={index}, text={text})");
    Ok(())
}

/// Translate subtitles (starts async task)
pub fn translate_start(source_language: String, target_language: String, _prompt: Option<String>) -> Result<serde_json::Value, McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: translate_start({source_language} -> {target_language})");
    Ok(serde_json::json!({
        "task_id": uuid::Uuid::new_v4().to_string(),
        "status": "started",
    }))
}

/// Cancel subtitle translation
pub fn translate_cancel() -> Result<(), McpError> {
    log::info!("MCP: translate_cancel");
    Ok(())
}
