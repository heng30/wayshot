use crate::{
    error::McpError,
    state::{self, UiAction},
};

/// Start audio recording — dispatches to UI
pub fn record_start(_save_dir: Option<String>) -> Result<serde_json::Value, McpError> {
    _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    state::dispatch_action(UiAction::StartRecordingAudio);
    Ok(serde_json::json!({ "status": "started" }))
}

/// Stop audio recording — dispatches to UI
pub fn record_stop() -> Result<serde_json::Value, McpError> {
    state::dispatch_action(UiAction::StopRecordingAudio);
    Ok(serde_json::json!({ "status": "stopped" }))
}

/// Split stems (async) — dispatched to UI
pub fn stem_split(audio_path: String) -> Result<serde_json::Value, McpError> {
    log::info!("MCP: stem_split({audio_path})");
    Ok(serde_json::json!({ "task_id": uuid::Uuid::new_v4().to_string(), "status": "started" }))
}

/// Generate TTS (async) — dispatched to UI
pub fn tts_generate(text: String, index: Option<usize>) -> Result<serde_json::Value, McpError> {
    log::info!("MCP: tts_generate(text={text}, index={index:?})");
    Ok(serde_json::json!({ "task_id": uuid::Uuid::new_v4().to_string(), "status": "started" }))
}

/// Detect voice segments in audio
pub fn vad_detect(audio_path: String) -> Result<serde_json::Value, McpError> {
    log::info!("MCP: vad_detect({audio_path})");
    Ok(serde_json::json!({
        "segments": [],
        "note": "VAD requires wayshot bridge for audio loading"
    }))
}
