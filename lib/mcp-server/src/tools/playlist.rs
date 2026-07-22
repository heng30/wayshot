use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct PlaylistListParams {}

#[derive(Serialize, JsonSchema)]
pub struct PlaylistListOutput {
    pub result: serde_json::Value,
}

pub struct PlaylistListTool;

impl ToolBase for PlaylistListTool {
    type Parameter = PlaylistListParams;
    type Output = PlaylistListOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_playlist_list".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("List all items in the playlist".into())
    }
}

impl AsyncTool<VideoEditorServer> for PlaylistListTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::media::list_playlist().map_err(ErrorData::from)?;
        Ok(PlaylistListOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct PlaylistImportParams {
    pub file_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct PlaylistImportOutput {
    pub result: serde_json::Value,
}

pub struct PlaylistImportTool;

impl ToolBase for PlaylistImportTool {
    type Parameter = PlaylistImportParams;
    type Output = PlaylistImportOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_playlist_import".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Import a media file to the playlist".into())
    }
}

impl AsyncTool<VideoEditorServer> for PlaylistImportTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::media::import_to_playlist(params.file_path).map_err(ErrorData::from)?;
        Ok(PlaylistImportOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct PlaylistAddToTrackParams {
    /// Index of the item in the playlist
    pub index: usize,
    /// Whether to add at the end of the track (default: true)
    pub at_end: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct PlaylistAddToTrackOutput {
    pub result: serde_json::Value,
}

pub struct PlaylistAddToTrackTool;

impl ToolBase for PlaylistAddToTrackTool {
    type Parameter = PlaylistAddToTrackParams;
    type Output = PlaylistAddToTrackOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_playlist_add_to_track".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Add a playlist item to a track by its playlist index. The item must already exist in the playlist.".into())
    }
}

impl AsyncTool<VideoEditorServer> for PlaylistAddToTrackTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let at_end = params.at_end.unwrap_or(true);
        service::media::add_to_track("playlist", params.index, at_end).map_err(ErrorData::from)?;
        Ok(PlaylistAddToTrackOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}
