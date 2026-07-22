use serde::{Deserialize, Serialize};

/// MCP server configuration
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct McpConfig {
    /// Whether the MCP server is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Transport type
    #[serde(default)]
    pub transport: McpTransport,

    /// HTTP port (used when transport is Http or Both)
    #[serde(default = "default_mcp_port")]
    pub port: u16,
}

fn default_mcp_port() -> u16 {
    9527
}

/// MCP transport type
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub enum McpTransport {
    #[default]
    Stdio,
    Http,
    Both,
}
