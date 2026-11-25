use std::net::SocketAddr;

use clap::{Parser, ValueEnum};
use serenade_rangular_mcp::{
    server::{DocsServer, Profile},
    transport,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Transport {
    Stdio,
    Http,
}

#[derive(Debug, Parser)]
#[command(version, about = "Serenade and Rangular documentation MCP")]
struct Args {
    #[arg(long, value_enum, default_value_t = Transport::Stdio)]
    transport: Transport,
    #[arg(long, value_enum, default_value_t = Profile::All)]
    profile: Profile,
    #[arg(long, env = "MCP_BIND", default_value = "127.0.0.1")]
    bind: std::net::IpAddr,
    #[arg(long, env = "PORT", default_value_t = 8765)]
    port: u16,
    /// Public hostnames accepted by HTTP, in addition to loopback.
    #[arg(long, env = "MCP_ALLOWED_HOSTS", value_delimiter = ',')]
    allowed_hosts: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let args = Args::parse();
    let server = DocsServer::new(args.profile);
    match args.transport {
        Transport::Stdio => transport::stdio(server).await,
        Transport::Http => {
            if !args.bind.is_loopback() && args.allowed_hosts.is_empty() {
                anyhow::bail!("non-loopback HTTP requires --allowed-hosts or MCP_ALLOWED_HOSTS");
            }
            let mut hosts = vec![
                "localhost".to_owned(),
                "127.0.0.1".to_owned(),
                "::1".to_owned(),
            ];
            hosts.extend(args.allowed_hosts);
            transport::http(server, SocketAddr::new(args.bind, args.port), hosts).await
        }
    }
}
