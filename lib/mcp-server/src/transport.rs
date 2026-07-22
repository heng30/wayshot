use crate::{
    config::{McpConfig, McpTransport},
    server::VideoEditorServer,
};
use rmcp::{
    ServiceExt,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Start the MCP server with the configured transport
pub async fn start(config: McpConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match config.transport {
        McpTransport::Stdio => {
            log::info!("MCP server starting on stdio");
            let transport = rmcp::transport::io::stdio();
            let ct = CancellationToken::new();
            let server = VideoEditorServer::new();
            let running = server.serve_with_ct(transport, ct).await?;
            running.waiting().await?;
            Ok(())
        }
        McpTransport::Http => {
            log::info!(
                "MCP server starting on HTTP port {} (Streamable HTTP)",
                config.port
            );
            start_http_server(config.port).await
        }
        McpTransport::Both => {
            log::info!(
                "MCP server starting on both stdio and HTTP port {}",
                config.port
            );

            let ct = CancellationToken::new();
            let ct_stdio = ct.clone();
            let ct_http = ct.clone();

            let stdio_handle = tokio::spawn(async move {
                let transport = rmcp::transport::io::stdio();
                let server = VideoEditorServer::new();
                match server.serve_with_ct(transport, ct_stdio).await {
                    Ok(running) => _ = running.waiting().await,
                    Err(e) => log::error!("MCP stdio server error: {e}"),
                }
            });

            let http_handle = tokio::spawn(async move {
                if let Err(e) = start_http_server_with_ct(config.port, ct_http).await {
                    log::error!("MCP HTTP server error: {e}");
                }
            });

            tokio::select! {
                _ = stdio_handle => {},
                _ = http_handle => {},
            }
            Ok(())
        }
    }
}

/// Start a Streamable HTTP MCP server using rmcp's tower integration with axum.
async fn start_http_server(port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ct = CancellationToken::new();
    start_http_server_with_ct(port, ct).await
}

async fn start_http_server_with_ct(
    port: u16,
    ct: CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let service = StreamableHttpService::new(
        || Ok(VideoEditorServer::new()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().with_cancellation_token(ct.child_token()),
    );

    // Mount the MCP service at /mcp using axum's nest_service
    let app = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    log::info!("MCP Streamable HTTP server listening on 0.0.0.0:{port}/mcp");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move { ct.cancelled_owned().await })
        .await?;

    Ok(())
}
