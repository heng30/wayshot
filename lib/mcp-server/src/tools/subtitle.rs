use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct SubtitleAddParams {
    pub track_index: usize,
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Serialize, JsonSchema)]
pub struct SubtitleAddOutput {
    pub result: serde_json::Value,
}

pub struct SubtitleAddTool;

impl ToolBase for SubtitleAddTool {
    type Parameter = SubtitleAddParams;
    type Output = SubtitleAddOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_subtitle_add".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Add a subtitle entry to a track".into())
    }
}

impl AsyncTool<VideoEditorServer> for SubtitleAddTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::subtitle::add_subtitle(
            params.track_index,
            params.text,
            params.start_ms,
            params.end_ms,
        )
        .map_err(ErrorData::from)?;
        Ok(SubtitleAddOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SubtitleUpdateParams {
    pub track_index: usize,
    pub index: usize,
    pub text: String,
}

#[derive(Serialize, JsonSchema)]
pub struct SubtitleUpdateOutput {
    pub result: serde_json::Value,
}

pub struct SubtitleUpdateTool;

impl ToolBase for SubtitleUpdateTool {
    type Parameter = SubtitleUpdateParams;
    type Output = SubtitleUpdateOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_subtitle_update".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Update the text of an existing subtitle entry".into())
    }
}

impl AsyncTool<VideoEditorServer> for SubtitleUpdateTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::subtitle::update_subtitle(params.track_index, params.index, params.text)
            .map_err(ErrorData::from)?;
        Ok(SubtitleUpdateOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SubtitleTranslateParams {
    pub source_language: String,
    pub target_language: String,
    pub prompt: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct SubtitleTranslateOutput {
    pub result: serde_json::Value,
}

pub struct SubtitleTranslateTool;

impl ToolBase for SubtitleTranslateTool {
    type Parameter = SubtitleTranslateParams;
    type Output = SubtitleTranslateOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_subtitle_translate".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Start subtitle translation from source to target language (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for SubtitleTranslateTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::subtitle::translate_start(
            params.source_language,
            params.target_language,
            params.prompt,
        )
        .map_err(ErrorData::from)?;
        Ok(SubtitleTranslateOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct SubtitleTranslateCancelParams {}

#[derive(Serialize, JsonSchema)]
pub struct SubtitleTranslateCancelOutput {
    pub result: serde_json::Value,
}

pub struct SubtitleTranslateCancelTool;

impl ToolBase for SubtitleTranslateCancelTool {
    type Parameter = SubtitleTranslateCancelParams;
    type Output = SubtitleTranslateCancelOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_subtitle_translate_cancel".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Cancel a running subtitle translation task".into())
    }
}

impl AsyncTool<VideoEditorServer> for SubtitleTranslateCancelTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::subtitle::translate_cancel().map_err(ErrorData::from)?;
        Ok(SubtitleTranslateCancelOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}
