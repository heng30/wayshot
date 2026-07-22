use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct LibraryListParams {}

#[derive(Serialize, JsonSchema)]
pub struct LibraryListOutput {
    pub result: serde_json::Value,
}

pub struct LibraryListTool;

impl ToolBase for LibraryListTool {
    type Parameter = LibraryListParams;
    type Output = LibraryListOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_library_list".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("List all items in the media library".into())
    }
}

impl AsyncTool<VideoEditorServer> for LibraryListTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::media::list_library().map_err(ErrorData::from)?;
        Ok(LibraryListOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct LibraryImportParams {
    pub file_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct LibraryImportOutput {
    pub result: serde_json::Value,
}

pub struct LibraryImportTool;

impl ToolBase for LibraryImportTool {
    type Parameter = LibraryImportParams;
    type Output = LibraryImportOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_library_import".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Import a media file to the library".into())
    }
}

impl AsyncTool<VideoEditorServer> for LibraryImportTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::media::import_to_library(params.file_path).map_err(ErrorData::from)?;
        Ok(LibraryImportOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct LibraryAddToTrackParams {
    /// Index of the item in the media library
    pub index: usize,
    /// Whether to add at the end of the track (default: true)
    pub at_end: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
pub struct LibraryAddToTrackOutput {
    pub result: serde_json::Value,
}

pub struct LibraryAddToTrackTool;

impl ToolBase for LibraryAddToTrackTool {
    type Parameter = LibraryAddToTrackParams;
    type Output = LibraryAddToTrackOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_library_add_to_track".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Add a media library item to a track by its library index. The item must already exist in the library.".into())
    }
}

impl AsyncTool<VideoEditorServer> for LibraryAddToTrackTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let at_end = params.at_end.unwrap_or(true);
        service::media::add_to_track("library", params.index, at_end).map_err(ErrorData::from)?;
        Ok(LibraryAddToTrackOutput {
            result: serde_json::json!({"success": true}),
        })
    }
}
