use crate::{VideoEditorServer, service, types::TrackInfo};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct TrackListParams {}

#[derive(Serialize, JsonSchema)]
pub struct TrackListOutput {
    pub tracks: Vec<TrackInfo>,
}

pub struct TrackListTool;

impl ToolBase for TrackListTool {
    type Parameter = TrackListParams;
    type Output = TrackListOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_track_list".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("List all tracks in the current project".into())
    }
}

impl AsyncTool<VideoEditorServer> for TrackListTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let tracks = service::track::list_tracks().map_err(ErrorData::from)?;
        Ok(TrackListOutput { tracks })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct TrackAddParams {
    /// Track type: "video", "audio", "subtitle", "image", "text"
    pub track_type: String,
    /// Optional custom track name
    pub name: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct TrackAddOutput {
    pub track_index: usize,
    pub track_name: String,
}

pub struct TrackAddTool;

impl ToolBase for TrackAddTool {
    type Parameter = TrackAddParams;
    type Output = TrackAddOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_track_add".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Add a new empty track to the project".into())
    }
}

impl AsyncTool<VideoEditorServer> for TrackAddTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let (track_index, track_name) =
            service::track::add_track(params.track_type, params.name).map_err(ErrorData::from)?;
        Ok(TrackAddOutput {
            track_index,
            track_name,
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct TrackInsertParams {
    pub track_type: String,
    pub index: usize,
    pub name: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct TrackInsertOutput {
    pub actual_index: usize,
}

pub struct TrackInsertTool;

impl ToolBase for TrackInsertTool {
    type Parameter = TrackInsertParams;
    type Output = TrackInsertOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_track_insert".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Insert an empty track at a specific index".into())
    }
}

impl AsyncTool<VideoEditorServer> for TrackInsertTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let actual_index =
            service::track::insert_track(params.track_type, params.index, params.name)
                .map_err(ErrorData::from)?;
        Ok(TrackInsertOutput { actual_index })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct TrackRemoveParams {
    pub track_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct TrackRemoveOutput {
    pub success: bool,
}

pub struct TrackRemoveTool;

impl ToolBase for TrackRemoveTool {
    type Parameter = TrackRemoveParams;
    type Output = TrackRemoveOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_track_remove".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Remove a track by index".into())
    }
}

impl AsyncTool<VideoEditorServer> for TrackRemoveTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::track::remove_track(params.track_index).map_err(ErrorData::from)?;
        Ok(TrackRemoveOutput { success: true })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct TrackMoveParams {
    pub from_index: usize,
    pub to_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct TrackMoveOutput {
    pub success: bool,
}

pub struct TrackMoveTool;

impl ToolBase for TrackMoveTool {
    type Parameter = TrackMoveParams;
    type Output = TrackMoveOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_track_move".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Move a track from one index to another".into())
    }
}

impl AsyncTool<VideoEditorServer> for TrackMoveTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::track::move_track(params.from_index, params.to_index).map_err(ErrorData::from)?;
        Ok(TrackMoveOutput { success: true })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct TrackToggleLockedParams {
    pub track_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct TrackToggleLockedOutput {
    pub is_locked: bool,
}

pub struct TrackToggleLockedTool;

impl ToolBase for TrackToggleLockedTool {
    type Parameter = TrackToggleLockedParams;
    type Output = TrackToggleLockedOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_track_toggle_locked".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Toggle track lock state".into())
    }
}

impl AsyncTool<VideoEditorServer> for TrackToggleLockedTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let is_locked =
            service::track::toggle_locked(params.track_index).map_err(ErrorData::from)?;
        Ok(TrackToggleLockedOutput { is_locked })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct TrackToggleHiddenParams {
    pub track_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct TrackToggleHiddenOutput {
    pub is_hidden: bool,
}

pub struct TrackToggleHiddenTool;

impl ToolBase for TrackToggleHiddenTool {
    type Parameter = TrackToggleHiddenParams;
    type Output = TrackToggleHiddenOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_track_toggle_hidden".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Toggle track visibility".into())
    }
}

impl AsyncTool<VideoEditorServer> for TrackToggleHiddenTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let is_hidden =
            service::track::toggle_hidden(params.track_index).map_err(ErrorData::from)?;
        Ok(TrackToggleHiddenOutput { is_hidden })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct TrackToggleMutedParams {
    pub track_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct TrackToggleMutedOutput {
    pub is_muted: bool,
}

pub struct TrackToggleMutedTool;

impl ToolBase for TrackToggleMutedTool {
    type Parameter = TrackToggleMutedParams;
    type Output = TrackToggleMutedOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_track_toggle_muted".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Toggle track audio mute state".into())
    }
}

impl AsyncTool<VideoEditorServer> for TrackToggleMutedTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let is_muted = service::track::toggle_muted(params.track_index).map_err(ErrorData::from)?;
        Ok(TrackToggleMutedOutput { is_muted })
    }
}
