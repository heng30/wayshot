use crate::error::McpError;
use crate::state::{self, UiAction};

/// List playlist items
pub fn list_playlist() -> Result<serde_json::Value, McpError> {
    Ok(serde_json::json!({
        "items": [],
        "note": "Playlist access requires UI interaction"
    }))
}

/// Import a file to the playlist — opens the file picker dialog
pub fn import_to_playlist(_file_path: String) -> Result<(), McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    state::dispatch_action(UiAction::ImportToPlaylist);
    Ok(())
}

/// List library items
pub fn list_library() -> Result<serde_json::Value, McpError> {
    Ok(serde_json::json!({
        "items": [],
        "note": "Library access requires UI interaction"
    }))
}

/// Import a file to the library — opens the file picker dialog
pub fn import_to_library(_file_path: String) -> Result<(), McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    state::dispatch_action(UiAction::ImportToLibrary);
    Ok(())
}

/// Add a playlist item to a track by its index.
/// The item must already exist in the playlist.
/// If at_end is true, adds at the end of the track; otherwise at the current timeline position.
pub fn add_to_track(source: &str, index: usize, at_end: bool) -> Result<(), McpError> {
    let _ = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    let action = match source {
        "playlist" => if at_end {
            UiAction::PlaylistItemAddToTrackEnd { index }
        } else {
            UiAction::PlaylistItemAddToTrack { index }
        },
        "library" => if at_end {
            UiAction::LibraryItemAddToTrackEnd { index }
        } else {
            UiAction::LibraryItemAddToTrack { index }
        },
        _ => return Err(McpError::InvalidParameter(
            format!("Invalid source: '{source}'. Must be 'playlist' or 'library'")
        )),
    };
    state::dispatch_action(action);
    Ok(())
}
