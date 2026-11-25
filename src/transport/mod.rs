//! Stdio and Streamable HTTP transports.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    routing::{any_service, get},
};
use rmcp::{
    ServiceExt,
    transport::{
        self,
        streamable_http_server::{
            StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
        },
    },
};

use crate::server::DocsServer;

pub async fn stdio(server: DocsServer) -> anyhow::Result<()> {
    server.serve(transport::stdio()).await?.waiting().await?;
    Ok(())
}

pub async fn http(
    server: DocsServer,
    address: SocketAddr,
    hosts: Vec<String>,
) -> anyhow::Result<()> {
    let mut config = StreamableHttpServerConfig::default()
        .with_allowed_hosts(hosts)
        .enforce_origin_validation();
    config.max_request_body_bytes = 128_000;
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(LocalSessionManager::default()),
        config,
    );
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/mcp", any_service(service.clone()))
        .route("/mcp/", any_service(service))
        .layer(tower::limit::ConcurrencyLimitLayer::new(32));
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address, "MCP HTTP server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
