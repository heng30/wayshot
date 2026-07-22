use crate::{VideoEditorServer, service, types::ProjectStatus};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, JsonSchema, Default)]
pub struct ProjectStatusParams {}

#[derive(Serialize, JsonSchema)]
pub struct ProjectStatusOutput {
    #[serde(flatten)]
    pub status: ProjectStatus,
}

pub struct ProjectStatusTool;

impl ToolBase for ProjectStatusTool {
    type Parameter = ProjectStatusParams;
    type Output = ProjectStatusOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_project_status".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Get the current project status including path, track count, duration, undo/redo availability".into())
    }
}

impl AsyncTool<VideoEditorServer> for ProjectStatusTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let status = service::project::get_status().map_err(ErrorData::from)?;
        Ok(ProjectStatusOutput { status })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ProjectCreateParams {
    /// Project name (will be used as the filename without extension)
    pub name: String,
    /// Directory path where the project file will be created
    pub dir_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ProjectCreateOutput {
    pub result: serde_json::Value,
}

pub struct ProjectCreateTool;

impl ToolBase for ProjectCreateTool {
    type Parameter = ProjectCreateParams;
    type Output = ProjectCreateOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_project_create".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some(
            "Create a new video editor project — opens the project creation dialog in the UI"
                .into(),
        )
    }
}

impl AsyncTool<VideoEditorServer> for ProjectCreateTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::project::create_project(params.name, params.dir_path)
            .map_err(ErrorData::from)?;
        Ok(ProjectCreateOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ProjectOpenParams {
    /// Full path to the .wayshot project file
    pub path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ProjectOpenOutput {
    pub result: serde_json::Value,
}

pub struct ProjectOpenTool;

impl ToolBase for ProjectOpenTool {
    type Parameter = ProjectOpenParams;
    type Output = ProjectOpenOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_project_open".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some(
            "Open an existing video editor project — opens the project open dialog in the UI"
                .into(),
        )
    }
}

impl AsyncTool<VideoEditorServer> for ProjectOpenTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::project::open_project(params.path).map_err(ErrorData::from)?;
        Ok(ProjectOpenOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ProjectCloseParams {}

#[derive(Serialize, JsonSchema)]
pub struct ProjectCloseOutput {
    pub success: bool,
}

pub struct ProjectCloseTool;

impl ToolBase for ProjectCloseTool {
    type Parameter = ProjectCloseParams;
    type Output = ProjectCloseOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_project_close".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Close the current project, resetting all tracks and segments".into())
    }
}

impl AsyncTool<VideoEditorServer> for ProjectCloseTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        service::project::close_project().map_err(ErrorData::from)?;
        Ok(ProjectCloseOutput { success: true })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ProjectUndoParams {}

#[derive(Serialize, JsonSchema)]
pub struct ProjectUndoOutput {
    pub description: String,
}

pub struct ProjectUndoTool;

impl ToolBase for ProjectUndoTool {
    type Parameter = ProjectUndoParams;
    type Output = ProjectUndoOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_project_undo".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Undo the last operation in the video editor".into())
    }
}

impl AsyncTool<VideoEditorServer> for ProjectUndoTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let description = service::project::undo().map_err(ErrorData::from)?;
        Ok(ProjectUndoOutput { description })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct ProjectRedoParams {}

#[derive(Serialize, JsonSchema)]
pub struct ProjectRedoOutput {
    pub description: String,
}

pub struct ProjectRedoTool;

impl ToolBase for ProjectRedoTool {
    type Parameter = ProjectRedoParams;
    type Output = ProjectRedoOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_project_redo".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Redo the last undone operation in the video editor".into())
    }
}

impl AsyncTool<VideoEditorServer> for ProjectRedoTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let description = service::project::redo().map_err(ErrorData::from)?;
        Ok(ProjectRedoOutput { description })
    }
}
