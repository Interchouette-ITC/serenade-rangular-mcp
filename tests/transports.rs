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

async fn exercise(client: &Peer<RoleClient>, local: bool) -> anyhow::Result<Vec<String>> {
    let mut names: Vec<_> = client
        .list_all_tools()
        .await?
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    names.sort();
    let mut expected = vec![
        "get_coding_guide",
        "get_doc",
        "inspect_cargo_manifest",
        "list_docs",
        "search_docs",
        "validate_rangular_template",
    ];
    if local {
        expected.extend(["run_cargo_task", "scaffold_serenade_app"]);
        expected.sort_unstable();
    }
    assert_eq!(names, expected);
    exercise_search(client).await?;
    exercise_guides(client).await?;
    exercise_documents(client).await?;
    exercise_examples(client).await?;
    exercise_validation(client).await?;
    exercise_resources(client).await?;
    Ok(names)
}

async fn exercise_search(client: &Peer<RoleClient>) -> anyhow::Result<()> {
    let search = call(client, "search_docs", json!({"query":"serenade-trunk"})).await?;
    assert_ne!(
        search["items"].as_array().context("search results")?.len(),
        0
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
    assert_ne!(rangular["items"].as_array().unwrap().len(), 0);
    let natural = call(
        client,
        "search_docs",
        json!({"query":"how to build a website with Serenade","product":"serenade","limit":10}),
    )
    .await?;
    assert!(
        natural["items"]
            .as_array()
            .context("natural language results")?
            .iter()
            .any(|hit| {
                let id = hit["document"]["id"].as_str().unwrap_or("");
                id.contains("FRONT-MODES") || id.contains("choose-your-mode")
            })
    );
    for (query, product, expected) in [
        (
            "how to apply a recipe",
            "serenade",
            "serenade/docs-dev/RECIPES.md",
        ),
        (
            "ssg prerender",
            "serenade",
            "serenade/docs-dev/FRONT-MODES.md",
        ),
        (
            "rangular-build component",
            "rangular",
            "rangular/crates/rangular-build/README.md",
        ),
    ] {
        let hits = call(
            client,
            "search_docs",
            json!({"query":query,"product":product,"limit":10}),
        )
        .await?;
        let items = hits["items"].as_array().context("ranked search results")?;
        assert!(
            items.iter().any(|hit| hit["document"]["id"] == expected),
            "{query}: {items:?}"
        );
    }
    Ok(())
}

async fn exercise_guides(client: &Peer<RoleClient>) -> anyhow::Result<()> {
    for (task, action, source) in [
        ("create_app", "serenade new", "serenade/docs-dev/RECIPES.md"),
        ("html", "make html", "serenade/examples/demo-html/README.md"),
        (
            "spa",
            "make spa",
            "serenade/examples/demo-monorepo/apps/demo-spa/src/main.rs",
        ),
        (
            "ssg",
            "make ssg",
            "serenade/examples/demo-monorepo/apps/demo-ssg/src/bin/prerender.rs",
        ),
        (
            "universal",
            "make universal",
            "serenade/examples/demo-universal/README.md",
        ),
        (
            "rangular_component",
            "rangular_build::build",
            "rangular/crates/rangular/README.md",
        ),
        (
            "rangular_serenade",
            "ManagedHead",
            "serenade/docs-dev/VIEW.md",
        ),
    ] {
        let guide = call(client, "get_coding_guide", json!({"task": task})).await?;
        assert!(
            guide["steps"]
                .as_array()
                .context("guide steps")?
                .iter()
                .any(|step| { step.as_str().is_some_and(|text| text.contains(action)) })
        );
        assert!(
            guide["sources"]
                .as_array()
                .context("guide sources")?
                .iter()
                .any(|doc| { doc["id"] == source && doc["source_url"].as_str().is_some() })
        );
        for source in guide["sources"].as_array().context("guide sources")? {
            let id = source["id"].as_str().context("source ID")?;
            let doc = call(client, "get_doc", json!({"id": id})).await?;
            assert_eq!(doc["document"]["id"], id);
            assert!(
                doc["markdown"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty())
                    || doc["sections"]
                        .as_array()
                        .is_some_and(|sections| !sections.is_empty()),
                "source {id} has no readable content"
            );
        }
    }
    let inspection = call(
        client,
        "inspect_cargo_manifest",
        json!({"manifest": "[package]\nname = 'app'\nversion = '0.1.0'\n[dependencies]\nui = { package = 'rangular', version = '1', features = ['x'] }\nserenade-view = { workspace = true }\n[target.'cfg(target_arch = \"wasm32\")'.build-dependencies]\nrangular-build = '1'\n"}),
    ).await?;
    assert_eq!(inspection["package"], "app");
    let dependencies = inspection["dependencies"]
        .as_array()
        .context("dependencies")?;
    assert!(dependencies.iter().any(|dep| dep["alias"] == "ui"
        && dep["package"] == "rangular"
        && dep["declaration"]["features"][0] == "x"));
    assert!(dependencies.iter().any(|dep| dep["package"] == "serenade-view" && dep["declaration"]["workspace"] == true));
    assert!(dependencies.iter().any(|dep| {
        dep["package"] == "rangular-build"
            && dep["scope"]
                .as_str()
                .unwrap()
                .contains("build-dependencies")
    }));
    Ok(())
}

async fn exercise_documents(client: &Peer<RoleClient>) -> anyhow::Result<()> {
    for id in [
        "serenade/guides/how-to/agent-mcp.md",
        "serenade/guides/concepts/index.md",
        "serenade/guides/index.mdx",
        "serenade/crates/serenade-form/README.md",
        "rangular/docs/SPEC.md",
        "rangular/crates/rangular-build/README.md",
    ] {
        let doc = call(client, "get_doc", json!({"id": id})).await?;
        assert_eq!(doc["document"]["id"], id);
    }
    let agent_guide = call(
        client,
        "get_doc",
        json!({"id":"serenade/guides/how-to/agent-mcp.md"}),
    )
    .await?;
    let agent_markdown = agent_guide["markdown"].as_str().context("agent guide")?;
    assert!(agent_markdown.contains("cargo install --locked serenade-rangular-mcp"));
    assert!(!agent_markdown.contains("cargo install --locked --path"));
    if let Some(note) = agent_guide["document"]["source_note"].as_str() {
        assert!(note.contains("differs from the recorded Serenade revision"));
    }
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
    let installation = call(
        client,
        "get_doc",
        json!({"id":"serenade/guides/first-steps/installation.md"}),
    )
    .await?;
    assert!(
        installation["markdown"]
            .as_str()
            .context("installation guide")?
            .contains("cargo install serenade-cli")
    );
    let rangular_guide = call(client, "get_doc", json!({"id":"rangular/docs/README.md"})).await?;
    let guide = rangular_guide["markdown"]
        .as_str()
        .context("Rangular guide")?;
    assert!(guide.contains("rangular = \"1.1.0\""));
    assert!(guide.contains("rangular-build = \"1.1.0\""));
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
    Ok(())
}

async fn exercise_examples(client: &Peer<RoleClient>) -> anyhow::Result<()> {
    for (id, expected) in [
        ("serenade/examples/demo-app/src/main.rs", "FrameworkBundle"),
        ("serenade/examples/demo-html/src/main.rs", "app::run"),
        (
            "serenade/examples/demo-monorepo/apps/demo-spa/src/main.rs",
            "mount_to_body",
        ),
        (
            "serenade/examples/demo-monorepo/apps/demo-ssg/src/bin/prerender.rs",
            "StaticDocumentWriter",
        ),
        (
            "serenade/examples/demo-universal/libs/demo-contract/src/lib.rs",
            "HelloResponse",
        ),
        (
            "rangular/demo-leptos/src/components/item_list/item_list.html",
            "@for (item of items",
        ),
        (
            "rangular/demo-leptos/src/components/two_way/two_way.html",
            "[(value)]",
        ),
        ("rangular/demo-tauri/src/src/lib.rs", "tauri::Builder"),
    ] {
        let example = call(client, "get_doc", json!({"id": id})).await?;
        assert!(
            example["markdown"]
                .as_str()
                .context("example source")?
                .contains(expected)
        );
        assert_eq!(
            example["document"]["version"]["status"],
            "development snapshot"
        );
    }
    let search = call(
        client,
        "search_docs",
        json!({"query":"demo-spa mount_to_body","product":"serenade"}),
    )
    .await?;
    assert!(
        search["items"]
            .as_array()
            .context("example search")?
            .iter()
            .any(|hit| {
                hit["document"]["id"] == "serenade/examples/demo-monorepo/apps/demo-spa/src/main.rs"
            })
    );
    let front_modes = call(
        client,
        "get_doc",
        json!({"id":"serenade/docs-dev/FRONT-MODES.md"}),
    )
    .await?;
    let modes = front_modes["markdown"].as_str().context("front modes")?;
    assert!(!modes.contains("serenade run-many -t"));
    assert!(modes.contains("serenade-html --all"));
    assert!(
        client
            .call_tool(
                CallToolRequestParams::new("get_doc").with_arguments(
                    json!({"id":"serenade/examples/demo-universal/apps/demo-front/Cargo.toml"})
                        .as_object()
                        .unwrap()
                        .clone()
                )
            )
            .await
            .is_err()
    );
    Ok(())
}

async fn exercise_validation(client: &Peer<RoleClient>) -> anyhow::Result<()> {
    let good = call(
        client,
        "validate_rangular_template",
        json!({"source":"<p>Hello</p>"}),
    )
    .await?;
    assert_eq!(good["valid"], true);
    assert_eq!(good["version"]["version"], "1.1.0");
    assert_eq!(good["version"]["status"], "crates.io release");
    assert_eq!(
        good["documentation_version"]["status"],
        "development snapshot"
    );
    assert!(good["scope"].as_str().unwrap().contains("may differ"));
    let bad = call(
        client,
        "validate_rangular_template",
        json!({"source":"<div>"}),
    )
    .await?;
    assert_eq!(bad["valid"], false);
    assert_ne!(
        bad["diagnostics"].as_array().context("diagnostics")?.len(),
        0
    );
    for (name, args) in [
        ("search_docs", json!({"query":" "})),
        ("list_docs", json!({"limit":51})),
        ("inspect_cargo_manifest", json!({"manifest":"[broken"})),
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
    Ok(())
}

async fn exercise_resources(client: &Peer<RoleClient>) -> anyhow::Result<()> {
    let resources = client.list_all_resources().await?;
    assert!(
        resources
            .iter()
            .any(|resource| resource.uri == "docs://rangular/docs/SPEC.md")
    );
    let repository_names = [
        "pull_request_template.md",
        "CODE_OF_CONDUCT.md",
        "CONTRIBUTING.md",
        "RELEASING.md",
        "SUPPLY-CHAIN.md",
    ];
    assert!(resources.iter().all(|resource| {
        let name = resource.uri.rsplit('/').next().unwrap_or("");
        !repository_names.contains(&name)
    }));
    assert!(
        client
            .call_tool(
                CallToolRequestParams::new("get_doc").with_arguments(
                    json!({"id": format!("serenade/docs/{}", repository_names[0])})
                        .as_object()
                        .unwrap()
                        .clone()
                )
            )
            .await
            .is_err()
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
    Ok(())
}

#[tokio::test]
async fn stdio_and_http_serve_the_same_documentation_contract() -> anyhow::Result<()> {
    let binary = env!("CARGO_BIN_EXE_serenade-rangular-mcp");
    let mut command = Command::new(binary);
    command.current_dir(std::env::temp_dir());
    let stdio_client = ().serve(TokioChildProcess::new(command)?).await?;
    let stdio_names = exercise(stdio_client.peer(), true).await?;
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
    let http_names = exercise(http_client.peer(), false).await?;
    assert_eq!(
        stdio_names
            .iter()
            .filter(|name| *name != "run_cargo_task" && *name != "scaffold_serenade_app")
            .collect::<Vec<_>>(),
        http_names.iter().collect::<Vec<_>>()
    );
    http_client.cancel().await?;
    let reconnect = StreamableHttpClientTransport::from_uri(format!("http://{address}/mcp"));
    let reconnected = ().serve(reconnect).await?;
    assert_eq!(reconnected.list_all_tools().await?.len(), 6);
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
    assert_eq!(client.list_all_tools().await?.len(), 6);
    client.cancel().await?;
    restarted.kill().await?;
    restarted.wait().await?;
    Ok(())
}

#[tokio::test]
async fn local_cargo_actions_run_and_report_failures() -> anyhow::Result<()> {
    let project = tempfile::tempdir()?;
    std::fs::create_dir(project.path().join("src"))?;
    std::fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = 'mcp-fixture'\nversion = '0.1.0'\nedition = '2024'\n[features]\nextra = []\n",
    )?;
    std::fs::write(
        project.path().join("src/lib.rs"),
        "pub fn answer() -> u8 { 42 }\n",
    )?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_serenade-rangular-mcp"));
    command.env("CARGO_NET_OFFLINE", "true");
    let client = ().serve(TokioChildProcess::new(command)?).await?;
    let args = json!({
        "directory": project.path(),
        "task": "test",
        "package": "mcp-fixture",
        "features": ["extra"]
    });
    let output = call(client.peer(), "run_cargo_task", args.clone()).await?;
    assert_eq!(output["success"], true);
    assert_eq!(output["exit_code"], 0);
    assert!(
        output["stdout"]
            .as_str()
            .unwrap()
            .contains("test result: ok")
    );

    std::fs::write(
        project.path().join("src/lib.rs"),
        "compile_error!(\"fixture error\");\n",
    )?;
    let failed = client
        .call_tool(
            CallToolRequestParams::new("run_cargo_task")
                .with_arguments(args.as_object().unwrap().clone()),
        )
        .await?;
    assert_eq!(failed.is_error, Some(true));
    let text = &failed.content[0].as_text().context("failure text")?.text;
    let failure: Value = serde_json::from_str(text)?;
    assert_eq!(failure["success"], false);
    assert!(
        failure["stderr"]
            .as_str()
            .unwrap()
            .contains("fixture error")
    );
    for args in [
        json!({"directory":"relative", "task":"test"}),
        json!({"directory":project.path(), "task":"test", "features":["--help"]}),
    ] {
        assert!(
            client
                .call_tool(
                    CallToolRequestParams::new("run_cargo_task")
                        .with_arguments(args.as_object().unwrap().clone())
                )
                .await
                .is_err()
        );
    }
    let existing = client
        .call_tool(
            CallToolRequestParams::new("scaffold_serenade_app").with_arguments(
                json!({"directory":project.path(), "name":"src"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;
    assert!(existing.is_err());
    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn http_removes_local_actions_even_when_enabled_by_caller() -> anyhow::Result<()> {
    use serenade_rangular_mcp::{
        server::{DocsServer, Profile},
        transport,
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    drop(listener);
    let server = tokio::spawn(transport::http(
        DocsServer::with_local_actions(Profile::All, true),
        address,
        vec!["127.0.0.1".to_owned()],
    ));
    tokio::time::timeout(Duration::from_secs(15), async {
        while TcpStream::connect(address).await.is_err() {
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .context("HTTP server did not start")?;
    let client = ()
        .serve(StreamableHttpClientTransport::from_uri(format!(
            "http://{address}/mcp"
        )))
        .await?;
    let names = client.list_all_tools().await?;
    for name in ["run_cargo_task", "scaffold_serenade_app"] {
        assert!(!names.iter().any(|tool| tool.name == name));
        let response = client
            .call_tool(
                CallToolRequestParams::new(name).with_arguments(
                    json!({"directory":"/", "task":"check", "name":"test"})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
            )
            .await;
        assert!(response.is_err() || response.unwrap().is_error == Some(true));
    }
    client.cancel().await?;
    server.abort();
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

async fn exercise_product_profile(client: &Peer<RoleClient>, product: &str) -> anyhow::Result<()> {
    let (tasks, expected_source, other) = match product {
        "serenade" => (
            &["create_app", "html", "spa", "ssg", "universal"][..],
            "serenade/docs-dev/FRONT-MODES.md",
            "rangular",
        ),
        "rangular" => (
            &["rangular_component"][..],
            "rangular/crates/rangular-build/README.md",
            "serenade",
        ),
        _ => bail!("unknown test profile: {product}"),
    };
    for task in tasks {
        let guide = call(client, "get_coding_guide", json!({"task": task})).await?;
        assert!(
            !guide["steps"].as_array().context("guide steps")?.is_empty(),
            "missing steps for {task}"
        );
        assert!(
            guide["sources"]
                .as_array()
                .context("guide sources")?
                .iter()
                .all(|source| source["product"] == product)
        );
    }
    let doc = call(client, "get_doc", json!({"id": expected_source})).await?;
    assert_eq!(doc["document"]["product"], product);
    assert!(
        client
            .call_tool(
                CallToolRequestParams::new("list_docs")
                    .with_arguments(json!({"product": other}).as_object().unwrap().clone())
            )
            .await
            .is_err()
    );
    if product == "rangular" {
        let good = call(
            client,
            "validate_rangular_template",
            json!({"source": "<p>Hello</p>"}),
        )
        .await?;
        let bad = call(
            client,
            "validate_rangular_template",
            json!({"source": "<div>"}),
        )
        .await?;
        assert_eq!(good["valid"], true);
        assert_eq!(bad["valid"], false);
        assert!(bad["diagnostics"][0]["line"].as_u64().is_some());
    }
    Ok(())
}

#[tokio::test]
async fn serenade_profile_hides_rangular_tools_and_content() -> anyhow::Result<()> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_serenade-rangular-mcp"));
    command
        .args(["--profile", "serenade"])
        .current_dir(std::env::temp_dir());
    let client = ().serve(TokioChildProcess::new(command)?).await?;
    exercise_product_profile(client.peer(), "serenade").await?;
    let tools = client.list_all_tools().await?;
    assert!(
        !tools
            .iter()
            .any(|tool| tool.name == "validate_rangular_template")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool.name == "scaffold_serenade_app")
    );
    assert!(tools.iter().any(|tool| tool.name == "run_cargo_task"));
    assert!(
        client
            .list_all_resources()
            .await?
            .iter()
            .all(|resource| resource.uri.starts_with("docs://serenade/"))
    );
    assert!(
        client
            .call_tool(
                CallToolRequestParams::new("get_coding_guide").with_arguments(
                    json!({"task":"rangular_component"})
                        .as_object()
                        .unwrap()
                        .clone()
                )
            )
            .await
            .is_err()
    );
    let inspected = call(
        client.peer(),
        "inspect_cargo_manifest",
        json!({"manifest":"[dependencies]\nrangular = '1'\nserenade-view = '1'"}),
    )
    .await?;
    assert_eq!(inspected["dependencies"].as_array().unwrap().len(), 1);
    assert_eq!(inspected["dependencies"][0]["package"], "serenade-view");
    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn rangular_profile_hides_serenade_content() -> anyhow::Result<()> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_serenade-rangular-mcp"));
    command.args(["--profile", "rangular"]);
    let client = ().serve(TokioChildProcess::new(command)?).await?;
    exercise_product_profile(client.peer(), "rangular").await?;
    let tools = client.list_all_tools().await?;
    assert!(
        !tools
            .iter()
            .any(|tool| tool.name == "scaffold_serenade_app")
    );
    assert!(tools.iter().any(|tool| tool.name == "run_cargo_task"));
    let resources = client.list_all_resources().await?;
    assert_ne!(resources.len(), 0);
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
    assert!(
        client
            .call_tool(
                CallToolRequestParams::new("get_coding_guide").with_arguments(
                    json!({"task":"rangular_serenade"})
                        .as_object()
                        .unwrap()
                        .clone()
                )
            )
            .await
            .is_err()
    );
    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn http_product_profiles_serve_relevant_guidance() -> anyhow::Result<()> {
    for profile in ["serenade", "rangular"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        drop(listener);
        let mut process = Command::new(env!("CARGO_BIN_EXE_serenade-rangular-mcp"))
            .current_dir(std::env::temp_dir())
            .args([
                "--transport",
                "http",
                "--port",
                &address.port().to_string(),
                "--profile",
                profile,
            ])
            .kill_on_drop(true)
            .spawn()?;
        wait_ready(address, &mut process).await?;
        let client = ()
            .serve(StreamableHttpClientTransport::from_uri(format!(
                "http://{address}/mcp"
            )))
            .await?;
        exercise_product_profile(client.peer(), profile).await?;
        let tools = client.list_all_tools().await?;
        assert!(
            !tools.iter().any(|tool| {
                tool.name == "run_cargo_task" || tool.name == "scaffold_serenade_app"
            })
        );
        client.cancel().await?;
        process.kill().await?;
        process.wait().await?;
    }
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
