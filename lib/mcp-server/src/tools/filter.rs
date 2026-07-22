use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct FilterListSegmentParams {
    pub track_index: usize,
    pub segment_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct FilterListSegmentOutput {
    pub result: serde_json::Value,
}

pub struct FilterListSegmentTool;

impl ToolBase for FilterListSegmentTool {
    type Parameter = FilterListSegmentParams;
    type Output = FilterListSegmentOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_filter_list_segment".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("List all filters on a segment".into())
    }
}

impl AsyncTool<VideoEditorServer> for FilterListSegmentTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let filters =
            service::filter::list_segment_filters(params.track_index, params.segment_index)
                .map_err(ErrorData::from)?;
        let result = serde_json::to_value(filters)
            .unwrap_or(serde_json::json!({"error": "serialization failed"}));
        Ok(FilterListSegmentOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct FilterRemoveParams {
    pub track_index: usize,
    pub segment_index: usize,
    pub filter_type: String,
    pub filter_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct FilterRemoveOutput {
    pub result: serde_json::Value,
}

pub struct FilterRemoveTool;

impl ToolBase for FilterRemoveTool {
    type Parameter = FilterRemoveParams;
    type Output = FilterRemoveOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_filter_remove".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Remove a filter from a segment".into())
    }
}

impl AsyncTool<VideoEditorServer> for FilterRemoveTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::filter::remove_filter(
            params.track_index,
            params.segment_index,
            &params.filter_type,
            params.filter_index,
        )
        .map_err(ErrorData::from)?;
        Ok(FilterRemoveOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct FilterToggleParams {
    pub track_index: usize,
    pub segment_index: usize,
    pub filter_type: String,
    pub filter_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct FilterToggleOutput {
    pub result: serde_json::Value,
}

pub struct FilterToggleTool;

impl ToolBase for FilterToggleTool {
    type Parameter = FilterToggleParams;
    type Output = FilterToggleOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_filter_toggle".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Toggle a filter enabled/disabled".into())
    }
}

impl AsyncTool<VideoEditorServer> for FilterToggleTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let enabled = service::filter::toggle_filter(
            params.track_index,
            params.segment_index,
            &params.filter_type,
            params.filter_index,
        )
        .map_err(ErrorData::from)?;
        Ok(FilterToggleOutput {
            result: serde_json::json!({"enabled": enabled}),
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct FilterClearParams {
    pub track_index: usize,
    pub segment_index: usize,
    pub filter_type: String,
}

#[derive(Serialize, JsonSchema)]
pub struct FilterClearOutput {
    pub result: serde_json::Value,
}

pub struct FilterClearTool;

impl ToolBase for FilterClearTool {
    type Parameter = FilterClearParams;
    type Output = FilterClearOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_filter_clear".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Clear all filters of a given type from a segment".into())
    }
}

impl AsyncTool<VideoEditorServer> for FilterClearTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::filter::clear_filters(
            params.track_index,
            params.segment_index,
            &params.filter_type,
        )
        .map_err(ErrorData::from)?;
        Ok(FilterClearOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct FilterAddParams {
    /// Track index
    pub track_index: usize,
    /// Segment index within the track
    pub segment_index: usize,
    /// Filter type: "video" or "audio"
    pub filter_type: String,
    /// Filter name (e.g., "grayscale", "opacity", "blur", "sharpen", "gain")
    pub filter_name: String,
}

#[derive(Serialize, JsonSchema)]
pub struct FilterAddOutput {
    pub result: serde_json::Value,
}

pub struct FilterAddTool;

impl ToolBase for FilterAddTool {
    type Parameter = FilterAddParams;
    type Output = FilterAddOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_filter_add".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Add a filter to a segment by name. Creates a default instance with default parameters. Supports undo/redo.".into())
    }
}

impl AsyncTool<VideoEditorServer> for FilterAddTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::filter::add_filter(
            params.track_index,
            params.segment_index,
            &params.filter_type,
            &params.filter_name,
        )
        .map_err(ErrorData::from)?;
        Ok(FilterAddOutput { result })
    }
}
