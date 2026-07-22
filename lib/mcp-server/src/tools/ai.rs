use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct AiBgRemoverProcessParams {
    pub image_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct AiBgRemoverProcessOutput {
    pub result: serde_json::Value,
}

pub struct AiBgRemoverProcessTool;

impl ToolBase for AiBgRemoverProcessTool {
    type Parameter = AiBgRemoverProcessParams;
    type Output = AiBgRemoverProcessOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_ai_bg_remover_process".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Remove background from an image using AI (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for AiBgRemoverProcessTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::ai::bg_remover_process(params.image_path).map_err(ErrorData::from)?;
        Ok(AiBgRemoverProcessOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct AiSmartClipStartParams {}

#[derive(Serialize, JsonSchema)]
pub struct AiSmartClipStartOutput {
    pub result: serde_json::Value,
}

pub struct AiSmartClipStartTool;

impl ToolBase for AiSmartClipStartTool {
    type Parameter = AiSmartClipStartParams;
    type Output = AiSmartClipStartOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_ai_smart_clip_start".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Start AI smart clip detection (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for AiSmartClipStartTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::ai::smart_clip_start().map_err(ErrorData::from)?;
        Ok(AiSmartClipStartOutput { result })
    }
}

#[derive(Deserialize, Default, JsonSchema)]
pub struct AiSceneDetectParams {
    pub track_index: usize,
    pub segment_index: usize,
    pub algorithm: String,
    pub threshold: Option<f32>,
}
#[derive(Serialize, JsonSchema)]
pub struct AiSceneDetectOutput {
    pub result: serde_json::Value,
}

pub struct AiSceneDetectTool;

impl ToolBase for AiSceneDetectTool {
    type Parameter = AiSceneDetectParams;
    type Output = AiSceneDetectOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_ai_scene_detect".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Detect scene changes in a segment using AI (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for AiSceneDetectTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::ai::scene_detect(
            params.track_index,
            params.segment_index,
            params.algorithm,
            params.threshold,
        )
        .map_err(ErrorData::from)?;
        Ok(AiSceneDetectOutput { result })
    }
}

#[derive(Deserialize, Default, JsonSchema)]
pub struct AiDewatermarkProcessParams {
    pub image_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct AiDewatermarkProcessOutput {
    pub result: serde_json::Value,
}

pub struct AiDewatermarkProcessTool;

impl ToolBase for AiDewatermarkProcessTool {
    type Parameter = AiDewatermarkProcessParams;
    type Output = AiDewatermarkProcessOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_ai_dewatermark_process".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Remove watermark from an image using AI (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for AiDewatermarkProcessTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result =
            service::ai::dewatermark_process(params.image_path).map_err(ErrorData::from)?;
        Ok(AiDewatermarkProcessOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct AiCutoutProcessParams {
    pub image_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct AiCutoutProcessOutput {
    pub result: serde_json::Value,
}

pub struct AiCutoutProcessTool;

impl ToolBase for AiCutoutProcessTool {
    type Parameter = AiCutoutProcessParams;
    type Output = AiCutoutProcessOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_ai_cutout_process".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("AI cutout from an image (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for AiCutoutProcessTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::ai::cutout_process(params.image_path).map_err(ErrorData::from)?;
        Ok(AiCutoutProcessOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct AiChapterSummaryParams {}

#[derive(Serialize, JsonSchema)]
pub struct AiChapterSummaryOutput {
    pub result: serde_json::Value,
}

pub struct AiChapterSummaryTool;

impl ToolBase for AiChapterSummaryTool {
    type Parameter = AiChapterSummaryParams;
    type Output = AiChapterSummaryOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_ai_chapter_summary".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Generate chapter summary using AI (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for AiChapterSummaryTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::ai::chapter_summary().map_err(ErrorData::from)?;
        Ok(AiChapterSummaryOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct AiSpeakersProcessParams {
    pub audio_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct AiSpeakersProcessOutput {
    pub result: serde_json::Value,
}

pub struct AiSpeakersProcessTool;

impl ToolBase for AiSpeakersProcessTool {
    type Parameter = AiSpeakersProcessParams;
    type Output = AiSpeakersProcessOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_ai_speakers_process".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Speaker diarization on an audio file using AI (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for AiSpeakersProcessTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::ai::speakers_process(params.audio_path).map_err(ErrorData::from)?;
        Ok(AiSpeakersProcessOutput { result })
    }
}
