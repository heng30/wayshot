use crate::tools;
use rmcp::{
    RoleServer,
    handler::server::{ServerHandler, router::tool::ToolRouter, tool::ToolCallContext},
    model::{
        CallToolRequestParams, CallToolResponse, ListToolsResult, PaginatedRequestParams, ServerInfo,
    },
    service::RequestContext,
};
use std::future::Future;

/// The MCP server for the video editor.
///
/// Implements `ServerHandler` from rmcp and uses a `ToolRouter`
/// to dispatch tool calls to the appropriate handler.
pub struct VideoEditorServer {
    pub tool_router: ToolRouter<Self>,
}

impl VideoEditorServer {
    pub fn new() -> Self {
        Self {
            tool_router: tools::build_tool_router(),
        }
    }
}

impl Default for VideoEditorServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerHandler for VideoEditorServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "Wayshot Video Editor MCP Server. Use these tools to programmatically control \
             the video editor: manage projects, tracks, segments, filters, preview, \
             media library, export, and AI features."
                .into(),
        );
        info
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_ {
        let tools = self.tool_router.list_all();
        async move {
            Ok(ListToolsResult {
                tools,
                next_cursor: None,
                ..Default::default()
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResponse, rmcp::ErrorData>> + Send + '_ {
        let mut params = CallToolRequestParams::new(request.name.clone());
        if let Some(arguments) = request.arguments.clone() {
            params = params.with_arguments(arguments);
        }
        let tool_call_context = ToolCallContext::new(self, params, context);
        async move { self.tool_router.call(tool_call_context).await }
    }
}
