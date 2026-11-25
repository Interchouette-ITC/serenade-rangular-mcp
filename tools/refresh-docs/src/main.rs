use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

use anyhow::{Context, bail, ensure};
use clap::Parser;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Parser)]
#[command(about = "Refresh embedded documentation from selected source commits")]
struct Args {
    #[arg(long)]
    serenade_root: PathBuf,
    #[arg(long)]
    serenade_revision: String,
    #[arg(long)]
    rangular_root: PathBuf,
    #[arg(long)]
    rangular_revision: String,
}

const SERENADE_PATHS: &[&str] = &[
    "docs-dev",
    "docs/README.md",
    "docs/CHANGELOG.md",
    "docs/astro/src/content/docs/first-steps",
    "docs/astro/src/content/docs/tutorials",
    "docs/astro/src/content/docs/concepts/architecture.md",
    "docs/astro/src/content/docs/concepts/bundles.md",
    "docs/astro/src/content/docs/reference/api-docs.md",
    "examples/demo-app/src",
    "examples/demo-universal/apps",
];
const RANGULAR_PATHS: &[&str] = &[
    "docs/README.md",
    "docs/SPEC.md",
    "docs/OVERVIEW.md",
    "docs/CHANGELOG.md",
    "demo-leptos/src/components/item_list",
];

fn git(root: &Path, args: &[&str]) -> anyhow::Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("run git in {}", root.display()))?;
    if !output.status.success() {
        bail!(
            "git failed in {}: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

fn source_file(root: &Path, revision: &str, path: &str) -> anyhow::Result<Vec<u8>> {
    git(root, &["show", &format!("{revision}:{path}")])
}

fn safe_path(path: &str) -> bool {
    Path::new(path)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn check_published_sources(root: &Path, revision: &str, version: &str) -> anyhow::Result<()> {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["metadata", "--locked", "--offline", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .output()
        .context("run offline cargo metadata")?;
    ensure!(
        output.status.success(),
        "cargo metadata failed; fetch locked dependencies first: {}",
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

#[derive(Default)]
struct Inventory {
    products: BTreeMap<String, Value>,
    documents: Vec<Value>,
    contents: BTreeMap<String, Vec<u8>>,
}

fn refresh_product(
    product: &str,
    root: &Path,
    revision: &str,
    paths: &[&str],
    repository: &str,
    inventory: &mut Inventory,
) -> anyhow::Result<()> {
    ensure!(
        revision.len() == 40
            && revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "{product}: revision must be a full lowercase commit SHA"
    );
    git(root, &["cat-file", "-e", &format!("{revision}^{{commit}}")])?;
    let manifest = String::from_utf8(source_file(root, revision, "Cargo.toml")?)?;
    let cargo: toml::Value = toml::from_str(&manifest)?;
    let version = cargo["workspace"]["package"]["version"]
        .as_str()
        .context("missing workspace version")?;
    let tag = format!("v{version}");
    let tag_ref = format!("refs/tags/{tag}");
    let tag_check = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["show-ref", "--verify", "--quiet", &tag_ref])
        .output()
        .with_context(|| format!("check tag in {}", root.display()))?;
    let matching_tag = match tag_check.status.code() {
        Some(0) => {
            let tag_revision = git(
                root,
                &["rev-parse", "--verify", &format!("{tag_ref}^{{commit}}")],
            )?;
            String::from_utf8(tag_revision)?.trim() == revision
        }
        Some(1) => false,
        _ => bail!(
            "check tag failed: {}",
            String::from_utf8_lossy(&tag_check.stderr)
        ),
    };
    if product == "rangular" {
        ensure!(
            matching_tag,
            "Rangular documentation must match release tag {tag}"
        );
    }
    inventory.products.insert(
        product.to_owned(),
        json!({
            "version": version,
            "revision": revision,
            "status": if matching_tag { "tagged source" } else { "development snapshot" },
            "tag": if matching_tag { Some(tag) } else { None },
            "repository": repository
        }),
    );
    let mut args = vec!["ls-tree", "-r", "--name-only", revision, "--"];
    args.extend_from_slice(paths);
    let files = String::from_utf8(git(root, &args)?)?;
    let before = inventory.documents.len();
    for path in files.lines() {
        let extension = Path::new(path).extension().and_then(|value| value.to_str());
        if !matches!(extension, Some("md" | "rs" | "html" | "scss" | "toml")) {
            continue;
        }
        ensure!(safe_path(path), "unsafe source path: {path}");
        let data = source_file(root, revision, path)?;
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
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .context("refresh-docs must be under tools/")?;
    let cargo: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?;
    let rangular_manifest = String::from_utf8(source_file(
        &args.rangular_root,
        &args.rangular_revision,
        "Cargo.toml",
    )?)?;
    let rangular: toml::Value = toml::from_str(&rangular_manifest)?;
    let version = rangular["workspace"]["package"]["version"]
        .as_str()
        .context("missing Rangular workspace version")?;
    ensure!(
        cargo["dependencies"]["rangular-parser"].as_str() == Some(format!("={version}").as_str()),
        "update the exact rangular-parser version before refreshing"
    );
    check_published_sources(root, &args.rangular_revision, version)?;
    let mut inventory = Inventory::default();
    refresh_product(
        "serenade",
        &args.serenade_root,
        &args.serenade_revision,
        SERENADE_PATHS,
        "https://github.com/Interchouette-ITC/Serenade",
        &mut inventory,
    )?;
    refresh_product(
        "rangular",
        &args.rangular_root,
        &args.rangular_revision,
        RANGULAR_PATHS,
        "https://github.com/Interchouette-ITC/rangular",
        &mut inventory,
    )?;
    let Inventory {
        products,
        documents,
        contents,
    } = inventory;
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
        "Refreshed {} documents from the selected commits.",
        contents.len()
    );
    Ok(())
}
