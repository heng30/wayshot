use crate::{error::McpError, state};

/// Start background removal (async)
pub fn bg_remover_process(image_path: String) -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: bg_remover_process({image_path})");
    Ok(serde_json::json!({ "task_id": uuid::Uuid::new_v4().to_string(), "status": "started" }))
}

/// Start smart clip (async)
pub fn smart_clip_start() -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: smart_clip_start");
    Ok(serde_json::json!({ "task_id": uuid::Uuid::new_v4().to_string(), "status": "started" }))
}

/// Detect scenes (async)
pub fn scene_detect(
    track_index: usize,
    segment_index: usize,
    algorithm: String,
    _threshold: Option<f32>,
) -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: scene_detect(track={track_index}, seg={segment_index}, algo={algorithm})");
    Ok(serde_json::json!({ "task_id": uuid::Uuid::new_v4().to_string(), "status": "started" }))
}

/// Remove watermark (async)
pub fn dewatermark_process(image_path: String) -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: dewatermark_process({image_path})");
    Ok(serde_json::json!({ "task_id": uuid::Uuid::new_v4().to_string(), "status": "started" }))
}

/// Cutout from image (async)
pub fn cutout_process(image_path: String) -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: cutout_process({image_path})");
    Ok(serde_json::json!({ "task_id": uuid::Uuid::new_v4().to_string(), "status": "started" }))
}

/// Generate chapter summary (async)
pub fn chapter_summary() -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: chapter_summary");
    Ok(serde_json::json!({ "task_id": uuid::Uuid::new_v4().to_string(), "status": "started" }))
}

/// Speaker diarization (async)
pub fn speakers_process(audio_path: String) -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: speakers_process({audio_path})");
    Ok(serde_json::json!({ "task_id": uuid::Uuid::new_v4().to_string(), "status": "started" }))
}

/// OCR on image
pub fn ocr_process_image(
    image_path: String,
    task_mode: Option<String>,
) -> Result<serde_json::Value, McpError> {
    log::info!("MCP: ocr_process_image({image_path}, mode={task_mode:?})");
    Ok(serde_json::json!({ "status": "started", "note": "OCR requires wayshot bridge" }))
}

/// Start transcription (async)
pub fn transcribe_start() -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    log::info!("MCP: transcribe_start");
    Ok(serde_json::json!({ "task_id": uuid::Uuid::new_v4().to_string(), "status": "started" }))
}

/// Cancel transcription
pub fn transcribe_cancel() -> Result<(), McpError> {
    log::info!("MCP: transcribe_cancel");
    Ok(())
}
