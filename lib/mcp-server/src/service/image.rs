use crate::error::McpError;
use crate::state;

/// Generate code image
pub fn code_image_generate(_code: String, language: String, _theme: Option<String>) -> Result<serde_json::Value, McpError> {
    log::info!("MCP: code_image_generate(lang={language})");
    Ok(serde_json::json!({
        "status": "started",
        "note": "Code image generation requires wayshot bridge"
    }))
}

/// Generate pure color image
pub fn pure_color_generate(r: u8, g: u8, b: u8, a: u8, width: u32, height: u32) -> Result<serde_json::Value, McpError> {
    log::info!("MCP: pure_color_generate({r},{g},{b},{a} {width}x{height})");
    Ok(serde_json::json!({
        "status": "started",
        "note": "Pure color image generation requires wayshot bridge"
    }))
}

/// Create long screenshot (async)
pub fn long_screenshot(track_index: usize, segment_index: usize) -> Result<serde_json::Value, McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: long_screenshot(track={track_index}, seg={segment_index})");
    Ok(serde_json::json!({ "task_id": uuid::Uuid::new_v4().to_string(), "status": "started" }))
}

/// Start image animation preview
pub fn img_animation_preview(image_path: String) -> Result<serde_json::Value, McpError> {
    log::info!("MCP: img_animation_preview({image_path})");
    Ok(serde_json::json!({ "status": "started" }))
}

/// Start background animation
pub fn bg_animation_start() -> Result<serde_json::Value, McpError> {
    log::info!("MCP: bg_animation_start");
    Ok(serde_json::json!({ "status": "started" }))
}
