use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct PreviewSeekParams {
    pub position_ms: u64,
}

#[derive(Serialize, JsonSchema)]
pub struct PreviewSeekOutput {
    pub result: serde_json::Value,
}

pub struct PreviewSeekTool;

impl ToolBase for PreviewSeekTool {
    type Parameter = PreviewSeekParams;
    type Output = PreviewSeekOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_preview_seek".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Seek the preview to a position (milliseconds)".into())
    }
}

impl AsyncTool<VideoEditorServer> for PreviewSeekTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::preview::seek(params.position_ms).map_err(ErrorData::from)?;
        Ok(PreviewSeekOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct PreviewInfoParams {}

#[derive(Serialize, JsonSchema)]
pub struct PreviewInfoOutput {
    pub result: serde_json::Value,
}

pub struct PreviewInfoTool;

impl ToolBase for PreviewInfoTool {
    type Parameter = PreviewInfoParams;
    type Output = PreviewInfoOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_preview_info".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Get the current preview info including duration and track count".into())
    }
}

impl AsyncTool<VideoEditorServer> for PreviewInfoTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::preview::get_preview_info().map_err(ErrorData::from)?;
        Ok(PreviewInfoOutput { result })
    }
}
