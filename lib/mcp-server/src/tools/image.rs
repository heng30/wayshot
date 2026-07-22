use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct ImgCodeGenerateParams {
    pub code: String,
    pub language: String,
    pub theme: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ImgCodeGenerateOutput {
    pub result: serde_json::Value,
}

pub struct ImgCodeGenerateTool;

impl ToolBase for ImgCodeGenerateTool {
    type Parameter = ImgCodeGenerateParams;
    type Output = ImgCodeGenerateOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_img_code_generate".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Generate a code syntax-highlighted image".into())
    }
}

impl AsyncTool<VideoEditorServer> for ImgCodeGenerateTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result =
            service::image::code_image_generate(params.code, params.language, params.theme)
                .map_err(ErrorData::from)?;
        Ok(ImgCodeGenerateOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, derivative::Derivative)]
#[derivative(Default)]
pub struct ImgPureColorGenerateParams {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[derivative(Default(value = "255"))]
    pub a: u8,
    #[derivative(Default(value = "1920"))]
    pub width: u32,
    #[derivative(Default(value = "1080"))]
    pub height: u32,
}

#[derive(Serialize, JsonSchema)]
pub struct ImgPureColorGenerateOutput {
    pub result: serde_json::Value,
}

pub struct ImgPureColorGenerateTool;

impl ToolBase for ImgPureColorGenerateTool {
    type Parameter = ImgPureColorGenerateParams;
    type Output = ImgPureColorGenerateOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_img_pure_color_generate".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Generate a solid color image with given dimensions".into())
    }
}

impl AsyncTool<VideoEditorServer> for ImgPureColorGenerateTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::image::pure_color_generate(
            params.r,
            params.g,
            params.b,
            params.a,
            params.width,
            params.height,
        )
        .map_err(ErrorData::from)?;
        Ok(ImgPureColorGenerateOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ImgLongScreenshotParams {
    pub track_index: usize,
    pub segment_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct ImgLongScreenshotOutput {
    pub result: serde_json::Value,
}

pub struct ImgLongScreenshotTool;

impl ToolBase for ImgLongScreenshotTool {
    type Parameter = ImgLongScreenshotParams;
    type Output = ImgLongScreenshotOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_img_long_screenshot".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Create a long screenshot from a segment (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for ImgLongScreenshotTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::image::long_screenshot(params.track_index, params.segment_index)
            .map_err(ErrorData::from)?;
        Ok(ImgLongScreenshotOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ImgAnimationPreviewParams {
    pub image_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ImgAnimationPreviewOutput {
    pub result: serde_json::Value,
}

pub struct ImgAnimationPreviewTool;

impl ToolBase for ImgAnimationPreviewTool {
    type Parameter = ImgAnimationPreviewParams;
    type Output = ImgAnimationPreviewOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_img_animation_preview".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Start image animation preview".into())
    }
}

impl AsyncTool<VideoEditorServer> for ImgAnimationPreviewTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result =
            service::image::img_animation_preview(params.image_path).map_err(ErrorData::from)?;
        Ok(ImgAnimationPreviewOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ImgBgAnimationParams {}

#[derive(Serialize, JsonSchema)]
pub struct ImgBgAnimationOutput {
    pub result: serde_json::Value,
}

pub struct ImgBgAnimationTool;

impl ToolBase for ImgBgAnimationTool {
    type Parameter = ImgBgAnimationParams;
    type Output = ImgBgAnimationOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_img_bg_animation".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Start background animation".into())
    }
}

impl AsyncTool<VideoEditorServer> for ImgBgAnimationTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::image::bg_animation_start().map_err(ErrorData::from)?;
        Ok(ImgBgAnimationOutput { result })
    }
}
