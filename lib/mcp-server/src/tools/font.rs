use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct FontListParams {}

#[derive(Serialize, JsonSchema)]
pub struct FontListOutput {
    pub result: serde_json::Value,
}

pub struct FontListTool;

impl ToolBase for FontListTool {
    type Parameter = FontListParams;
    type Output = FontListOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_font_list".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("List all available fonts".into())
    }
}

impl AsyncTool<VideoEditorServer> for FontListTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::font::list_fonts().map_err(ErrorData::from)?;
        Ok(FontListOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct FontImportParams {
    pub file_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct FontImportOutput {
    pub result: serde_json::Value,
}

pub struct FontImportTool;

impl ToolBase for FontImportTool {
    type Parameter = FontImportParams;
    type Output = FontImportOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_font_import".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Import a font file".into())
    }
}

impl AsyncTool<VideoEditorServer> for FontImportTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::font::import_font(params.file_path).map_err(ErrorData::from)?;
        Ok(FontImportOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct FontSearchParams {
    pub keyword: String,
}

#[derive(Serialize, JsonSchema)]
pub struct FontSearchOutput {
    pub result: serde_json::Value,
}

pub struct FontSearchTool;

impl ToolBase for FontSearchTool {
    type Parameter = FontSearchParams;
    type Output = FontSearchOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_font_search".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Search fonts by keyword".into())
    }
}

impl AsyncTool<VideoEditorServer> for FontSearchTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::font::search_fonts(params.keyword).map_err(ErrorData::from)?;
        Ok(FontSearchOutput { result })
    }
}
