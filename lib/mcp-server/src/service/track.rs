use crate::error::McpError;
use crate::state::{self, UiAction};
use crate::types::TrackInfo;

/// Parse track type string into canonical form
pub fn parse_track_type(track_type: &str) -> Result<&'static str, McpError> {
    match track_type.to_lowercase().as_str() {
        "video" => Ok("video"),
        "audio" => Ok("audio"),
        "subtitle" => Ok("subtitle"),
        "image" => Ok("image"),
        "text" => Ok("text"),
        _ => Err(McpError::InvalidTrackType(track_type.to_string())),
    }
}

/// Get track type string from Track enum
fn track_type_str(track: &video_editor::tracks::track::Track) -> &'static str {
    use video_editor::tracks::track::Track;
    match track {
        Track::Video(_) => "video",
        Track::Audio(_) => "audio",
        Track::Subtitle(_) => "subtitle",
        Track::Image(_) => "image",
        Track::Text(_) => "text",
    }
}

/// Extract track properties: (name, hiding, locked, muted)
fn track_props(track: &video_editor::tracks::track::Track) -> (String, bool, bool, bool) {
    use video_editor::tracks::track::Track;
    match track {
        Track::Video(t) => (t.name.clone(), t.hiding, t.locked, t.muted),
        Track::Audio(t) => (t.name.clone(), t.hiding, t.locked, false),
        Track::Subtitle(t) => (t.name.clone(), t.hiding, t.locked, false),
        Track::Image(t) => (t.name.clone(), t.hiding, t.locked, false),
        Track::Text(t) => (t.name.clone(), t.hiding, t.locked, false),
    }
}

/// List all tracks in the project
pub fn list_tracks() -> Result<Vec<TrackInfo>, McpError> {
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;

    let tracks: Vec<TrackInfo> = manager
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| {
            let (name, hidden, locked, muted) = track_props(track);
            TrackInfo {
                index,
                name,
                track_type: track_type_str(track).to_string(),
                locked,
                hidden,
                muted,
                segment_count: track.segments_count(),
                duration_ms: track.duration().as_millis() as u64,
            }
        })
        .collect();

    Ok(tracks)
}

/// Add a new track — dispatches to UI Logic callback
pub fn add_track(track_type: String, _name: Option<String>) -> Result<(usize, String), McpError> {
    let action = match track_type.to_lowercase().as_str() {
        "video" => UiAction::AddEmptyVideoTrack,
        "audio" => UiAction::AddEmptyAudioTrack,
        "subtitle" => UiAction::AddEmptySubtitleTrack,
        "image" => UiAction::AddEmptyImageTrack,
        "text" => UiAction::AddEmptyTextTrack,
        _ => return Err(McpError::InvalidTrackType(track_type)),
    };
    state::dispatch_action(action);

    // Return the track info from the current state
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    let track_index = manager.len().saturating_sub(1);
    let track_name = manager.get(track_index)
        .map(|t| track_props(t).0)
        .unwrap_or_else(|| track_type.to_uppercase());
    Ok((track_index, track_name))
}

/// Insert a track at a specific index
pub fn insert_track(track_type: String, index: usize, _name: Option<String>) -> Result<usize, McpError> {
    let action = match track_type.to_lowercase().as_str() {
        "video" => UiAction::InsertVideoTrack { index },
        "audio" => UiAction::InsertAudioTrack { index },
        "subtitle" => UiAction::InsertSubtitleTrack { index },
        "image" => UiAction::InsertImageTrack { index },
        "text" => UiAction::InsertTextTrack { index },
        _ => return Err(McpError::InvalidTrackType(track_type)),
    };
    state::dispatch_action(action);
    Ok(index)
}

/// Remove a track by index — selects the track first, then removes selected tracks
pub fn remove_track(track_index: usize) -> Result<(), McpError> {
    state::dispatch_action(UiAction::AddSelectedTrack { index: track_index });
    state::dispatch_action(UiAction::RemoveTracks);
    Ok(())
}

/// Move a track from one index to another
pub fn move_track(from_index: usize, to_index: usize) -> Result<(), McpError> {
    state::dispatch_action(UiAction::MoveTrackByDrag { from_index, to_index });
    Ok(())
}

/// Toggle track locked state
pub fn toggle_locked(track_index: usize) -> Result<bool, McpError> {
    state::dispatch_action(UiAction::ToggleLockedTrack { index: track_index });

    // Return the new state from the manager
    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    let is_locked = manager.get(track_index)
        .map(|t| match t {
            video_editor::tracks::track::Track::Video(vt) => vt.locked,
            video_editor::tracks::track::Track::Audio(at) => at.locked,
            video_editor::tracks::track::Track::Subtitle(st) => st.locked,
            video_editor::tracks::track::Track::Image(it) => it.locked,
            video_editor::tracks::track::Track::Text(tt) => tt.locked,
        })
        .unwrap_or(false);
    Ok(is_locked)
}

/// Toggle track visibility
pub fn toggle_hidden(track_index: usize) -> Result<bool, McpError> {
    state::dispatch_action(UiAction::ToggleHidingTrack { index: track_index });

    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    let is_hidden = manager.get(track_index)
        .map(|t| match t {
            video_editor::tracks::track::Track::Video(vt) => vt.hiding,
            video_editor::tracks::track::Track::Audio(at) => at.hiding,
            video_editor::tracks::track::Track::Subtitle(st) => st.hiding,
            video_editor::tracks::track::Track::Image(it) => it.hiding,
            video_editor::tracks::track::Track::Text(tt) => tt.hiding,
        })
        .unwrap_or(false);
    Ok(is_hidden)
}

/// Toggle track muted state
pub fn toggle_muted(track_index: usize) -> Result<bool, McpError> {
    state::dispatch_action(UiAction::ToggleMutedTrack { index: track_index });

    let manager = state::get_tracks_manager().ok_or(McpError::ProjectNotOpen)?;
    let is_muted = manager.get(track_index)
        .map(|t| match t {
            video_editor::tracks::track::Track::Video(vt) => vt.muted,
            _ => false,
        })
        .unwrap_or(false);
    Ok(is_muted)
}
