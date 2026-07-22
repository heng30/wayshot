use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct ExportVideoParams {
    pub output_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ExportVideoOutput {
    pub result: serde_json::Value,
}

pub struct ExportVideoTool;

impl ToolBase for ExportVideoTool {
    type Parameter = ExportVideoParams;
    type Output = ExportVideoOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_export_video".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Export the project as a video file (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for ExportVideoTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::export::export_video(params.output_path).map_err(ErrorData::from)?;
        Ok(ExportVideoOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ExportAudioParams {
    pub output_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ExportAudioOutput {
    pub result: serde_json::Value,
}

pub struct ExportAudioTool;

impl ToolBase for ExportAudioTool {
    type Parameter = ExportAudioParams;
    type Output = ExportAudioOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_export_audio".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Export the project audio as a file (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for ExportAudioTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::export::export_audio(params.output_path).map_err(ErrorData::from)?;
        Ok(ExportAudioOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, derivative::Derivative)]
#[derivative(Default)]
pub struct ExportSubtitleParams {
    pub output_path: String,
    #[derivative(Default(value = "\"srt\".to_string()"))]
    pub format: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ExportSubtitleOutput {
    pub result: serde_json::Value,
}

pub struct ExportSubtitleTool;

impl ToolBase for ExportSubtitleTool {
    type Parameter = ExportSubtitleParams;
    type Output = ExportSubtitleOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_export_subtitle".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Export subtitles to a file in the specified format".into())
    }
}

impl AsyncTool<VideoEditorServer> for ExportSubtitleTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::export::export_subtitle(params.output_path, params.format)
            .map_err(ErrorData::from)?;
        Ok(ExportSubtitleOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ExportCancelParams {
    pub task_id: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ExportCancelOutput {
    pub result: serde_json::Value,
}

pub struct ExportCancelTool;

impl ToolBase for ExportCancelTool {
    type Parameter = ExportCancelParams;
    type Output = ExportCancelOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_export_cancel".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Cancel a running export task".into())
    }
}

impl AsyncTool<VideoEditorServer> for ExportCancelTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::export::cancel_export(params.task_id).map_err(ErrorData::from)?;
        Ok(ExportCancelOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ExportQueueParams {}

#[derive(Serialize, JsonSchema)]
pub struct ExportQueueOutput {
    pub result: serde_json::Value,
}

pub struct ExportQueueTool;

impl ToolBase for ExportQueueTool {
    type Parameter = ExportQueueParams;
    type Output = ExportQueueOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_export_queue".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("List all pending and active export tasks".into())
    }
}

impl AsyncTool<VideoEditorServer> for ExportQueueTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::export::list_export_queue().map_err(ErrorData::from)?;
        Ok(ExportQueueOutput { result })
    }
}
