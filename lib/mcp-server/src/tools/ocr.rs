use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct OcrProcessImageParams {
    pub image_path: String,
    pub task_mode: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct OcrProcessImageOutput {
    pub result: serde_json::Value,
}

pub struct OcrProcessImageTool;

impl ToolBase for OcrProcessImageTool {
    type Parameter = OcrProcessImageParams;
    type Output = OcrProcessImageOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_ocr_process_image".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Run OCR on an image to extract text".into())
    }
}

impl AsyncTool<VideoEditorServer> for OcrProcessImageTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::ai::ocr_process_image(params.image_path, params.task_mode)
            .map_err(ErrorData::from)?;
        Ok(OcrProcessImageOutput { result })
    }
}
