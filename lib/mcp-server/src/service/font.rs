use crate::error::McpError;

/// List available fonts
pub fn list_fonts() -> Result<serde_json::Value, McpError> {
    log::info!("MCP: list_fonts");
    Ok(serde_json::json!({
        "fonts": [],
        "note": "Font listing requires wayshot bridge"
    }))
}

/// Import a font file
pub fn import_font(file_path: String) -> Result<(), McpError> {
    log::info!("MCP: import_font({file_path})");
    Ok(())
}

/// Search fonts
pub fn search_fonts(keyword: String) -> Result<serde_json::Value, McpError> {
    log::info!("MCP: search_fonts({keyword})");
    Ok(serde_json::json!({ "results": [] }))
}
