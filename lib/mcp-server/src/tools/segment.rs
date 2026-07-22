use crate::{VideoEditorServer, service, types::SegmentInfo};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentListParams {
    pub track_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentListOutput {
    pub segments: Vec<SegmentInfo>,
}

pub struct SegmentListTool;

impl ToolBase for SegmentListTool {
    type Parameter = SegmentListParams;
    type Output = SegmentListOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_list".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("List all segments in a track".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentListTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let segments =
            service::segment::list_segments(params.track_index).map_err(ErrorData::from)?;
        Ok(SegmentListOutput { segments })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentSplitParams {
    pub track_index: usize,
    pub segment_index: usize,
    pub position_ms: u64,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentSplitOutput {
    pub success: bool,
}

pub struct SegmentSplitTool;

impl ToolBase for SegmentSplitTool {
    type Parameter = SegmentSplitParams;
    type Output = SegmentSplitOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_split".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Split a segment at the given position (milliseconds)".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentSplitTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::segment::split_segment(
            params.track_index,
            params.segment_index,
            params.position_ms,
        )
        .map_err(ErrorData::from)?;
        Ok(SegmentSplitOutput { success: true })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentMoveParams {
    pub track_index: usize,
    pub segment_index: usize,
    pub offset_ms: u64,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentMoveOutput {
    pub success: bool,
}

pub struct SegmentMoveTool;

impl ToolBase for SegmentMoveTool {
    type Parameter = SegmentMoveParams;
    type Output = SegmentMoveOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_move".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Move a segment to a new timeline offset (milliseconds)".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentMoveTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::segment::move_segment(params.track_index, params.segment_index, params.offset_ms)
            .map_err(ErrorData::from)?;
        Ok(SegmentMoveOutput { success: true })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentDeleteParams {
    pub track_index: usize,
    pub segment_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentDeleteOutput {
    pub success: bool,
}

pub struct SegmentDeleteTool;

impl ToolBase for SegmentDeleteTool {
    type Parameter = SegmentDeleteParams;
    type Output = SegmentDeleteOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_delete".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Delete a segment from a track".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentDeleteTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::segment::delete_segment(params.track_index, params.segment_index)
            .map_err(ErrorData::from)?;
        Ok(SegmentDeleteOutput { success: true })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentToggleVisibleParams {
    pub track_index: usize,
    pub segment_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentToggleVisibleOutput {
    pub is_visible: bool,
}

pub struct SegmentToggleVisibleTool;

impl ToolBase for SegmentToggleVisibleTool {
    type Parameter = SegmentToggleVisibleParams;
    type Output = SegmentToggleVisibleOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_toggle_visible".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Toggle segment visibility on/off".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentToggleVisibleTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let is_visible = service::segment::toggle_visible(params.track_index, params.segment_index)
            .map_err(ErrorData::from)?;
        Ok(SegmentToggleVisibleOutput { is_visible })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentToggleAudioParams {
    pub track_index: usize,
    pub segment_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentToggleAudioOutput {
    pub is_muted: bool,
}

pub struct SegmentToggleAudioTool;

impl ToolBase for SegmentToggleAudioTool {
    type Parameter = SegmentToggleAudioParams;
    type Output = SegmentToggleAudioOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_toggle_audio".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Toggle segment audio mute on/off".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentToggleAudioTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let is_muted = service::segment::toggle_audio(params.track_index, params.segment_index)
            .map_err(ErrorData::from)?;
        Ok(SegmentToggleAudioOutput { is_muted })
    }
}

#[derive(Deserialize, JsonSchema, derivative::Derivative)]
#[derivative(Default)]
pub struct SegmentRemoveGapParams {
    pub track_index: usize,
    pub segment_index: usize,
    #[derivative(Default(value = "\"left\".to_string()"))]
    pub direction: String, // "left" or "right"
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentRemoveGapOutput {
    pub success: bool,
}

pub struct SegmentRemoveGapTool;

impl ToolBase for SegmentRemoveGapTool {
    type Parameter = SegmentRemoveGapParams;
    type Output = SegmentRemoveGapOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_remove_gap".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Remove gap before (left) or after (right) a segment".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentRemoveGapTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::segment::remove_gap(params.track_index, params.segment_index, &params.direction)
            .map_err(ErrorData::from)?;
        Ok(SegmentRemoveGapOutput { success: true })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentMetadataParams {
    pub track_index: usize,
    pub segment_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentMetadataOutput {
    pub metadata: serde_json::Value,
}

pub struct SegmentMetadataTool;

impl ToolBase for SegmentMetadataTool {
    type Parameter = SegmentMetadataParams;
    type Output = SegmentMetadataOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_metadata".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Get metadata for a segment including source file, resolution, audio info".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentMetadataTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let metadata = service::segment::get_metadata(params.track_index, params.segment_index)
            .map_err(ErrorData::from)?;
        Ok(SegmentMetadataOutput { metadata })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentAddParams {
    /// Track index to add the segment to
    pub track_index: usize,
    /// Path to the media file (video, audio, image, etc.)
    pub file_path: String,
    /// Timeline offset in milliseconds (default: 0, adds at the end of the track)
    pub timeline_offset_ms: Option<u64>,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentAddOutput {
    pub result: serde_json::Value,
}

pub struct SegmentAddTool;

impl ToolBase for SegmentAddTool {
    type Parameter = SegmentAddParams;
    type Output = SegmentAddOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_add".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Add a media file as a segment to a track. The file is probed for metadata and added using the command system (supports undo/redo).".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentAddTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::segment::add_segment(
            params.track_index,
            params.file_path,
            params.timeline_offset_ms,
        )
        .map_err(ErrorData::from)?;
        Ok(SegmentAddOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentResizeParams {
    pub track_index: usize,
    pub segment_index: usize,
    /// New duration in milliseconds
    pub duration_ms: u64,
    /// Whether to shift subsequent segments to maintain relative positions (default: false)
    pub shift_timeline: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentResizeOutput {
    pub result: serde_json::Value,
}

pub struct SegmentResizeTool;

impl ToolBase for SegmentResizeTool {
    type Parameter = SegmentResizeParams;
    type Output = SegmentResizeOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_resize".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Resize a segment by setting its duration in milliseconds. Uses the command system (supports undo/redo).".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentResizeTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let shift_timeline = params.shift_timeline.unwrap_or(false);
        let result = service::segment::resize_segment(
            params.track_index,
            params.segment_index,
            params.duration_ms,
            shift_timeline,
        )
        .map_err(ErrorData::from)?;
        Ok(SegmentResizeOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentShrinkParams {
    pub track_index: usize,
    pub segment_index: usize,
    /// Amount to shrink in milliseconds
    pub shrink_ms: u64,
    /// "left" or "right" — which side to shrink from
    pub direction: String,
    /// Whether to shift subsequent segments (default: false)
    pub shift_timeline: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentShrinkOutput {
    pub result: serde_json::Value,
}

pub struct SegmentShrinkTool;

impl ToolBase for SegmentShrinkTool {
    type Parameter = SegmentShrinkParams;
    type Output = SegmentShrinkOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_shrink".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Shrink a segment from the left or right side by a specified amount. Uses the command system (supports undo/redo).".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentShrinkTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let shift_timeline = params.shift_timeline.unwrap_or(false);
        let result = service::segment::shrink_segment(
            params.track_index,
            params.segment_index,
            params.shrink_ms,
            &params.direction,
            shift_timeline,
        )
        .map_err(ErrorData::from)?;
        Ok(SegmentShrinkOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentStretchParams {
    pub track_index: usize,
    pub segment_index: usize,
    /// Amount to stretch in milliseconds
    pub stretch_ms: u64,
    /// "left" or "right" — which side to stretch from
    pub direction: String,
    /// Whether to shift subsequent segments (default: false)
    pub shift_timeline: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentStretchOutput {
    pub result: serde_json::Value,
}

pub struct SegmentStretchTool;

impl ToolBase for SegmentStretchTool {
    type Parameter = SegmentStretchParams;
    type Output = SegmentStretchOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_stretch".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Stretch a segment from the left or right side by a specified amount. Uses the command system (supports undo/redo).".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentStretchTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let shift_timeline = params.shift_timeline.unwrap_or(false);
        let result = service::segment::stretch_segment(
            params.track_index,
            params.segment_index,
            params.stretch_ms,
            &params.direction,
            shift_timeline,
        )
        .map_err(ErrorData::from)?;
        Ok(SegmentStretchOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentDeleteCmdParams {
    pub track_index: usize,
    pub segment_index: usize,
    /// Whether to shift subsequent segments to close the gap (default: true)
    pub shift_timeline: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentDeleteCmdOutput {
    pub result: serde_json::Value,
}

pub struct SegmentDeleteCmdTool;

impl ToolBase for SegmentDeleteCmdTool {
    type Parameter = SegmentDeleteCmdParams;
    type Output = SegmentDeleteCmdOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_delete_cmd".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Delete a segment using the command system (supports undo/redo). Optionally shift subsequent segments to close the gap.".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentDeleteCmdTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let shift_timeline = params.shift_timeline.unwrap_or(true);
        let result = service::segment::delete_segment_cmd(
            params.track_index,
            params.segment_index,
            shift_timeline,
        )
        .map_err(ErrorData::from)?;
        Ok(SegmentDeleteCmdOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentMoveCmdParams {
    pub track_index: usize,
    pub segment_index: usize,
    /// New timeline offset in milliseconds
    pub new_timeline_offset_ms: u64,
    /// Whether to shift subsequent segments (default: false)
    pub shift_timeline: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentMoveCmdOutput {
    pub result: serde_json::Value,
}

pub struct SegmentMoveCmdTool;

impl ToolBase for SegmentMoveCmdTool {
    type Parameter = SegmentMoveCmdParams;
    type Output = SegmentMoveCmdOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_move_cmd".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Move a segment to a new timeline offset using the command system (supports undo/redo).".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentMoveCmdTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let shift_timeline = params.shift_timeline.unwrap_or(false);
        let result = service::segment::move_segment_cmd(
            params.track_index,
            params.segment_index,
            params.new_timeline_offset_ms,
            shift_timeline,
        )
        .map_err(ErrorData::from)?;
        Ok(SegmentMoveCmdOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SegmentCopyParams {
    pub track_index: usize,
    pub segment_index: usize,
    /// Target index to copy to (if not specified, copies to the end of the track)
    pub target_index: Option<usize>,
}

#[derive(Serialize, JsonSchema)]
pub struct SegmentCopyOutput {
    pub result: serde_json::Value,
}

pub struct SegmentCopyTool;

impl ToolBase for SegmentCopyTool {
    type Parameter = SegmentCopyParams;
    type Output = SegmentCopyOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_segment_copy".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Copy a segment to a new position in the same track. Uses the command system (supports undo/redo).".into())
    }
}

impl AsyncTool<VideoEditorServer> for SegmentCopyTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::segment::copy_segment(
            params.track_index,
            params.segment_index,
            params.target_index,
        )
        .map_err(ErrorData::from)?;
        Ok(SegmentCopyOutput { result })
    }
}
