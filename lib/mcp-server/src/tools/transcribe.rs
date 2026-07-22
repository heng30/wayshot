use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct TranscribeStartParams {}

#[derive(Serialize, JsonSchema)]
pub struct TranscribeStartOutput {
    pub result: serde_json::Value,
}

pub struct TranscribeStartTool;

impl ToolBase for TranscribeStartTool {
    type Parameter = TranscribeStartParams;
    type Output = TranscribeStartOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_transcribe_start".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Start audio transcription (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for TranscribeStartTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::ai::transcribe_start().map_err(ErrorData::from)?;
        Ok(TranscribeStartOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct TranscribeCancelParams {}

#[derive(Serialize, JsonSchema)]
pub struct TranscribeCancelOutput {
    pub result: serde_json::Value,
}

pub struct TranscribeCancelTool;

impl ToolBase for TranscribeCancelTool {
    type Parameter = TranscribeCancelParams;
    type Output = TranscribeCancelOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_transcribe_cancel".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Cancel a running transcription task".into())
    }
}

impl AsyncTool<VideoEditorServer> for TranscribeCancelTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::ai::transcribe_cancel().map_err(ErrorData::from)?;
        Ok(TranscribeCancelOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}
