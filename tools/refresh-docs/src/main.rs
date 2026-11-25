use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path},
    process::Command,
};

use anyhow::{Context, bail, ensure};
use flate2::read::GzDecoder;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const SERENADE_PATHS: &[&str] = &[
    "docs-dev",
    "docs/README.md",
    "docs/CHANGELOG.md",
    "docs/astro/src/content/docs/first-steps",
    "docs/astro/src/content/docs/tutorials",
    "docs/astro/src/content/docs/concepts/architecture.md",
    "docs/astro/src/content/docs/concepts/bundles.md",
    "docs/astro/src/content/docs/reference/api-docs.md",
    "examples",
];
const RANGULAR_PATHS: &[&str] = &[
    "docs/README.md",
    "docs/SPEC.md",
    "docs/OVERVIEW.md",
    "docs/CHANGELOG.md",
    "demo-leptos",
    "demo-tauri",
];

fn fetch(url: &str) -> anyhow::Result<Vec<u8>> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--connect-timeout",
            "15",
            "--max-time",
            "120",
            "--user-agent",
            "serenade-rangular-mcp-refresh",
            url,
        ])
        .output()
        .context("run curl")?;
    if !output.status.success() {
        bail!("fetch {url}: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(output.stdout)
}

struct Source {
    tag: Option<String>,
    revision: String,
}

fn resolve_commit(repository: &str, reference: &str) -> anyhow::Result<String> {
    let api = format!("https://api.github.com/repos/Interchouette-ITC/{repository}");
    let commit: Value = serde_json::from_slice(&fetch(&format!("{api}/commits/{reference}"))?)?;
    let revision = commit["sha"].as_str().context("missing source commit")?;
    ensure!(
        revision.len() == 40
            && revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid {repository} source commit"
    );
    Ok(revision.to_owned())
}

fn latest_release(repository: &str) -> anyhow::Result<Source> {
    let api = format!("https://api.github.com/repos/Interchouette-ITC/{repository}");
    let release: Value = serde_json::from_slice(&fetch(&format!("{api}/releases/latest"))?)?;
    let tag = release["tag_name"]
        .as_str()
        .context("missing release tag")?;
    ensure!(
        tag.starts_with('v')
            && tag.len() > 1
            && tag[1..].bytes().all(|b| b.is_ascii_digit() || b == b'.'),
        "unsupported {repository} release tag {tag}"
    );
    Ok(Source {
        tag: Some(tag.to_owned()),
        revision: resolve_commit(repository, tag)?,
    })
}

fn safe_path(path: &str) -> bool {
    Path::new(path)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn is_example_manifest(path: &str) -> bool {
    (path.starts_with("examples/") || path.starts_with("demo-"))
        && Path::new(path)
            .file_name()
            .is_some_and(|name| name == "Cargo.toml")
}

fn check_published_sources(root: &Path, revision: &str, version: &str) -> anyhow::Result<()> {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["metadata", "--locked", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .output()
        .context("run cargo metadata")?;
    ensure!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let packages = metadata["packages"]
        .as_array()
        .context("missing packages")?;
    for name in ["rangular-parser", "rangular-expr", "rangular-host"] {
        let package = packages
            .iter()
            .find(|package| {
                package["name"] == name
                    && package["version"] == version
                    && package["source"]
                        .as_str()
                        .is_some_and(|source| source.starts_with("registry+"))
            })
            .with_context(|| format!("missing locked registry package {name} {version}"))?;
        let manifest = package["manifest_path"]
            .as_str()
            .context("missing manifest path")?;
        let vcs_path = Path::new(manifest)
            .parent()
            .context("package directory")?
            .join(".cargo_vcs_info.json");
        let vcs: Value = serde_json::from_slice(
            &fs::read(&vcs_path).with_context(|| format!("read {}", vcs_path.display()))?,
        )?;
        ensure!(
            vcs["git"]["sha1"].as_str() == Some(revision),
            "{name} {version} was published from a different revision than the selected docs"
        );
    }
    Ok(())
}

fn update_parser(root: &Path, version: &str) -> anyhow::Result<()> {
    let cargo: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?;
    if cargo["dependencies"]["rangular-parser"].as_str() == Some(format!("={version}").as_str()) {
        return Ok(());
    }
    let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(root)
        .args(["add", "--package", "serenade-rangular-mcp"])
        .arg(format!("rangular-parser@={version}"))
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .status()
        .context("update rangular-parser dependency")?;
    ensure!(
        status.success(),
        "failed to update rangular-parser to {version}"
    );
    Ok(())
}

#[derive(Default)]
struct Inventory {
    products: BTreeMap<String, Value>,
    documents: Vec<Value>,
    contents: BTreeMap<String, Vec<u8>>,
}

fn refresh_product(
    product: &str,
    repository: &str,
    source: &Source,
    paths: &[&str],
    inventory: &mut Inventory,
) -> anyhow::Result<()> {
    let url = format!(
        "https://api.github.com/repos/Interchouette-ITC/{repository}/tarball/{}",
        source.revision
    );
    let archive = fetch(&url)?;
    let mut archive = tar::Archive::new(GzDecoder::new(archive.as_slice()));
    let before = inventory.documents.len();
    let mut source_version = None;
    for item in archive.entries().context("read release archive")? {
        let mut item = item.context("read archive entry")?;
        if !item.header().entry_type().is_file() {
            continue;
        }
        let archive_path = item.path()?.into_owned();
        let mut components = archive_path.components();
        components.next().context("missing archive root")?;
        let path = components.as_path();
        let path = path.to_str().context("non-UTF-8 archive path")?;
        ensure!(safe_path(path), "unsafe source path: {path}");
        if path == "Cargo.toml" {
            let mut manifest = String::new();
            item.read_to_string(&mut manifest)?;
            let cargo: toml::Value = toml::from_str(&manifest)?;
            source_version = Some(
                cargo["workspace"]["package"]["version"]
                    .as_str()
                    .context("missing workspace version")?
                    .to_owned(),
            );
            continue;
        }
        if !paths.iter().any(|prefix| {
            path == *prefix
                || path
                    .strip_prefix(*prefix)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        }) {
            continue;
        }
        if is_example_manifest(path) {
            continue;
        }
        let extension = Path::new(path).extension().and_then(|value| value.to_str());
        if !matches!(
            extension,
            Some("md" | "rs" | "html" | "scss" | "css" | "js" | "json" | "toml")
        ) {
            continue;
        }
        let mut data = Vec::new();
        item.read_to_end(&mut data)?;
        let text = std::str::from_utf8(&data).with_context(|| format!("{path} is not UTF-8"))?;
        let title = text
            .lines()
            .find_map(|line| line.strip_prefix("# "))
            .unwrap_or_else(|| {
                Path::new(path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(path)
            })
            .trim();
        let corpus_path = path
            .strip_prefix("docs/astro/src/content/docs/")
            .map_or_else(|| path.to_owned(), |relative| format!("guides/{relative}"));
        let id = format!("{product}/{corpus_path}");
        inventory.documents.push(json!({
            "id": id,
            "product": product,
            "path": path,
            "title": title,
            "sha256": Sha256::digest(&data).iter().map(|byte| format!("{byte:02x}")).collect::<String>()
        }));
        inventory.contents.insert(id, data);
    }
    ensure!(
        inventory.documents.len() > before,
        "{product}: no documents found"
    );
    let version = source_version.context("missing release workspace manifest")?;
    if let Some(tag) = &source.tag {
        ensure!(
            *tag == format!("v{version}"),
            "{product}: release tag {tag} does not match workspace version {version}"
        );
    }
    inventory.products.insert(
        product.to_owned(),
        json!({
            "version": version,
            "revision": source.revision,
            "status": if source.tag.is_some() { "tagged source" } else { "development snapshot" },
            "tag": source.tag,
            "repository": format!("https://github.com/Interchouette-ITC/{repository}")
        }),
    );
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .context("refresh-docs must be under tools/")?;
    let serenade = Source {
        tag: None,
        revision: resolve_commit("Serenade", "dev")?,
    };
    let rangular = Source {
        tag: None,
        revision: resolve_commit("rangular", "dev")?,
    };
    let parser_release = latest_release("rangular")?;
    let version = parser_release
        .tag
        .as_deref()
        .context("missing Rangular release tag")?
        .strip_prefix('v')
        .context("invalid Rangular tag")?;
    update_parser(root, version)?;
    check_published_sources(root, &parser_release.revision, version)?;
    let mut inventory = Inventory::default();
    refresh_product(
        "serenade",
        "Serenade",
        &serenade,
        SERENADE_PATHS,
        &mut inventory,
    )?;
    refresh_product(
        "rangular",
        "rangular",
        &rangular,
        RANGULAR_PATHS,
        &mut inventory,
    )?;
    let Inventory {
        products,
        mut documents,
        contents,
    } = inventory;
    documents.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let output = root.join("corpus/documents");
    let manifest_path = root.join("corpus/manifest.json");
    if manifest_path.exists() {
        let previous: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        for item in previous["documents"]
            .as_array()
            .context("missing previous documents")?
        {
            let id = item["id"]
                .as_str()
                .context("missing previous document ID")?;
            ensure!(safe_path(id), "unsafe old document path: {id}");
            if !contents.contains_key(id) {
                fs::remove_file(output.join(id))
                    .with_context(|| format!("remove obsolete {id}"))?;
            }
        }
    }
    for (id, data) in &contents {
        let path = output.join(id);
        fs::create_dir_all(path.parent().context("document parent")?)?;
        fs::write(&path, data).with_context(|| format!("write {}", path.display()))?;
    }
    let manifest = json!({"products": products, "documents": documents});
    let mut serialized = serde_json::to_string_pretty(&manifest)?;
    serialized.push('\n');
    fs::write(manifest_path, serialized)?;
    println!(
        "Refreshed {} documents from Serenade dev and Rangular dev; parser uses Rangular {}.",
        contents.len(),
        parser_release
            .tag
            .as_deref()
            .context("missing Rangular release tag")?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{is_example_manifest, update_parser};
    use std::{fs, path::Path};

    #[test]
    fn excludes_example_manifests_from_refresh() {
        assert!(is_example_manifest(
            "examples/demo-universal/apps/demo-front/Cargo.toml"
        ));
        assert!(is_example_manifest("demo-leptos/Cargo.toml"));
        assert!(!is_example_manifest("examples/demo-app/src/main.rs"));
    }

    #[test]
    fn updates_outdated_parser_and_lockfile() -> anyhow::Result<()> {
        let project = tempfile::tempdir()?;
        fs::create_dir(project.path().join("src"))?;
        fs::create_dir(project.path().join(".cargo"))?;
        fs::write(project.path().join("src/main.rs"), "fn main() {}\n")?;
        fs::write(
            project.path().join(".cargo/config.toml"),
            "[net]\noffline = true\n",
        )?;
        fs::write(
            project.path().join("Cargo.toml"),
            "[package]\nname = \"serenade-rangular-mcp\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
             [dependencies]\nrangular-parser = \"=0.0.0\"\n",
        )?;
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let manifest: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?;
        let version = manifest["dependencies"]["rangular-parser"]
            .as_str()
            .unwrap()
            .strip_prefix('=')
            .unwrap();
        update_parser(project.path(), version)?;
        let updated: toml::Value =
            toml::from_str(&fs::read_to_string(project.path().join("Cargo.toml"))?)?;
        assert_eq!(
            updated["dependencies"]["rangular-parser"].as_str(),
            Some(format!("={version}").as_str())
        );
        assert!(project.path().join("Cargo.lock").exists());
        Ok(())
    }
}
