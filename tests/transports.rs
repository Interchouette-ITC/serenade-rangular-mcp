use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream as StdTcpStream},
    time::Duration,
};

use anyhow::{Context, bail};
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, ReadResourceRequestParams},
    service::Peer,
    transport::{StreamableHttpClientTransport, TokioChildProcess},
};
use serde_json::{Value, json};
use tokio::{net::TcpStream, process::Command, time::sleep};

async fn call(client: &Peer<RoleClient>, name: &str, args: Value) -> anyhow::Result<Value> {
    let response = client
        .call_tool(
            CallToolRequestParams::new(name.to_owned()).with_arguments(
                args.as_object()
                    .context("arguments must be an object")?
                    .clone(),
            ),
        )
        .await?;
    if response.is_error == Some(true) {
        bail!("tool returned an error: {response:?}");
    }
    let text = response
        .content
        .first()
        .and_then(|block| block.as_text())
        .context("missing text response")?;
    Ok(serde_json::from_str(&text.text)?)
}

async fn exercise(client: &Peer<RoleClient>) -> anyhow::Result<Vec<String>> {
    let mut names: Vec<_> = client
        .list_all_tools()
        .await?
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "get_doc",
            "list_docs",
            "search_docs",
            "validate_rangular_template"
        ]
    );

    let search = call(client, "search_docs", json!({"query":"serenade-trunk"})).await?;
    assert!(
        !search["items"]
            .as_array()
            .context("search results")?
            .is_empty()
    );
    assert_eq!(
        search,
        call(client, "search_docs", json!({"query":"serenade-trunk"})).await?
    );
    let rangular = call(
        client,
        "search_docs",
        json!({"query":"rangular-build","product":"rangular"}),
    )
    .await?;
    assert!(!rangular["items"].as_array().unwrap().is_empty());
    let first_page = call(client, "list_docs", json!({"limit":1})).await?;
    let next_page = call(
        client,
        "list_docs",
        json!({"limit":1, "cursor":first_page["next_cursor"]}),
    )
    .await?;
    assert_ne!(first_page["items"][0]["id"], next_page["items"][0]["id"]);
    let doc = call(
        client,
        "get_doc",
        json!({"id":"serenade/docs-dev/RECIPES.md"}),
    )
    .await?;
    assert!(
        doc["markdown"]
            .as_str()
            .context("markdown")?
            .contains("serenade new")
    );
    assert_eq!(doc["document"]["version"]["status"], "development snapshot");
    let section = doc["sections"][0]["id"].as_str().context("section ID")?;
    let section_content = call(
        client,
        "get_doc",
        json!({"id":"serenade/docs-dev/RECIPES.md","section":section}),
    )
    .await?;
    assert!(
        section_content["markdown"]
            .as_str()
            .unwrap()
            .starts_with('#')
    );
    let good = call(
        client,
        "validate_rangular_template",
        json!({"source":"<p>Hello</p>"}),
    )
    .await?;
    assert_eq!(good["valid"], true);
    assert_eq!(good["version"]["version"], "1.1.0");
    let bad = call(
        client,
        "validate_rangular_template",
        json!({"source":"<div>"}),
    )
    .await?;
    assert_eq!(bad["valid"], false);
    assert!(
        !bad["diagnostics"]
            .as_array()
            .context("diagnostics")?
            .is_empty()
    );
    for (name, args) in [
        ("search_docs", json!({"query":" "})),
        ("list_docs", json!({"limit":51})),
        (
            "validate_rangular_template",
            json!({"source":"", "kind":"unknown"}),
        ),
    ] {
        assert!(
            client
                .call_tool(
                    CallToolRequestParams::new(name)
                        .with_arguments(args.as_object().unwrap().clone())
                )
                .await
                .is_err()
        );
    }
    assert!(bad["diagnostics"][0]["line"].as_u64().is_some());
    let many = call(
        client,
        "validate_rangular_template",
        json!({"source":"<div>".repeat(150)}),
    )
    .await?;
    assert_eq!(many["diagnostics"].as_array().unwrap().len(), 100);
    assert_eq!(many["diagnostics_truncated"], true);
    let resources = client.list_all_resources().await?;
    assert!(
        resources
            .iter()
            .any(|resource| resource.uri == "docs://rangular/docs/SPEC.md")
    );
    client
        .read_resource(ReadResourceRequestParams::new(
            "docs://serenade/docs-dev/RECIPES.md",
        ))
        .await?;
    assert!(
        client
            .call_tool(
                CallToolRequestParams::new("get_doc")
                    .with_arguments(json!({"id":"missing"}).as_object().unwrap().clone())
            )
            .await
            .is_err()
    );
    Ok(names)
}

#[tokio::test]
async fn stdio_and_http_serve_the_same_documentation_contract() -> anyhow::Result<()> {
    let binary = env!("CARGO_BIN_EXE_serenade-rangular-mcp");
    let mut command = Command::new(binary);
    command.current_dir(std::env::temp_dir());
    let stdio_client = ().serve(TokioChildProcess::new(command)?).await?;
    let stdio_names = exercise(stdio_client.peer()).await?;
    stdio_client.cancel().await?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    drop(listener);
    let mut process = Command::new(binary)
        .current_dir(std::env::temp_dir())
        .args(["--transport", "http", "--port", &address.port().to_string()])
        .kill_on_drop(true)
        .spawn()?;
    wait_ready(address, &mut process).await?;
    let transport = StreamableHttpClientTransport::from_uri(format!("http://{address}/mcp"));
    let http_client = ().serve(transport).await?;
    let http_names = exercise(http_client.peer()).await?;
    assert_eq!(stdio_names, http_names);
    http_client.cancel().await?;
    let reconnect = StreamableHttpClientTransport::from_uri(format!("http://{address}/mcp"));
    let reconnected = ().serve(reconnect).await?;
    assert_eq!(reconnected.list_all_tools().await?.len(), 4);
    reconnected.cancel().await?;
    assert_eq!(http_status(address, "attacker.example", None, "{}")?, 403);
    assert_eq!(
        http_status(address, "localhost", Some("https://attacker.example"), "{}")?,
        403
    );
    assert_eq!(
        http_status(address, "localhost", None, &"x".repeat(128_001))?,
        413
    );
    process.kill().await?;
    process.wait().await?;
    let mut restarted = Command::new(binary)
        .current_dir(std::env::temp_dir())
        .args(["--transport", "http", "--port", &address.port().to_string()])
        .kill_on_drop(true)
        .spawn()?;
    wait_ready(address, &mut restarted).await?;
    let transport = StreamableHttpClientTransport::from_uri(format!("http://{address}/mcp"));
    let client = ().serve(transport).await?;
    assert_eq!(client.list_all_tools().await?.len(), 4);
    client.cancel().await?;
    restarted.kill().await?;
    restarted.wait().await?;
    Ok(())
}

fn http_status(
    address: SocketAddr,
    host: &str,
    origin: Option<&str>,
    body: &str,
) -> anyhow::Result<u16> {
    let mut stream = StdTcpStream::connect(address)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let origin = origin.map_or_else(String::new, |value| format!("Origin: {value}\r\n"));
    write!(
        stream,
        "POST /mcp HTTP/1.1\r\nHost: {host}\r\n{origin}Content-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response
        .split_whitespace()
        .nth(1)
        .context("HTTP status")?
        .parse()?)
}

#[tokio::test]
async fn serenade_profile_hides_rangular_tools_and_content() -> anyhow::Result<()> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_serenade-rangular-mcp"));
    command
        .args(["--profile", "serenade"])
        .current_dir(std::env::temp_dir());
    let client = ().serve(TokioChildProcess::new(command)?).await?;
    assert!(
        !client
            .list_all_tools()
            .await?
            .iter()
            .any(|tool| tool.name == "validate_rangular_template")
    );
    assert!(
        client
            .list_all_resources()
            .await?
            .iter()
            .all(|resource| resource.uri.starts_with("docs://serenade/"))
    );
    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn rangular_profile_hides_serenade_content() -> anyhow::Result<()> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_serenade-rangular-mcp"));
    command.args(["--profile", "rangular"]);
    let client = ().serve(TokioChildProcess::new(command)?).await?;
    let resources = client.list_all_resources().await?;
    assert!(!resources.is_empty());
    assert!(
        resources
            .iter()
            .all(|r| r.uri.starts_with("docs://rangular/"))
    );
    assert!(
        client
            .call_tool(
                CallToolRequestParams::new("list_docs")
                    .with_arguments(json!({"product":"serenade"}).as_object().unwrap().clone())
            )
            .await
            .is_err()
    );
    client.cancel().await?;
    Ok(())
}

async fn wait_ready(
    address: SocketAddr,
    process: &mut tokio::process::Child,
) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(status) = process.try_wait()? {
                bail!("HTTP server exited during startup: {status}");
            }
            if TcpStream::connect(address).await.is_ok() {
                return Ok(());
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .context("HTTP server did not start")?
}
