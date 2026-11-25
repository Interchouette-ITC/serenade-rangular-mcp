use std::{env, fs, path::Path};

use sha2::{Digest, Sha256};

fn main() {
    println!("cargo:rerun-if-changed=corpus/manifest.json");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=corpus/documents");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read("corpus/manifest.json").expect("missing corpus; run make refresh-docs"),
    )
    .expect("invalid corpus manifest");
    let cargo: toml::Value =
        toml::from_str(&fs::read_to_string("Cargo.toml").expect("read Cargo.toml"))
            .expect("parse Cargo.toml");
    let parser_version = cargo["dependencies"]["rangular-parser"]
        .as_str()
        .and_then(|version| version.strip_prefix('='))
        .expect("rangular-parser must use an exact released version");
    let docs_version = manifest["products"]["rangular"]["version"]
        .as_str()
        .expect("missing Rangular version");
    assert_eq!(
        parser_version, docs_version,
        "Rangular parser and documentation versions differ"
    );
    assert_eq!(
        manifest["products"]["rangular"]["tag"].as_str(),
        Some(format!("v{docs_version}").as_str()),
        "Rangular documentation must come from the matching release tag"
    );
    assert!(
        Path::new(&env::var("CARGO_MANIFEST_DIR").expect("manifest directory"))
            .join("corpus/documents")
            .is_dir(),
        "missing embedded documents"
    );
    for document in manifest["documents"].as_array().expect("documents array") {
        let id = document["id"].as_str().expect("document id");
        assert!(
            !id.starts_with('/') && !id.split('/').any(|part| part == ".."),
            "invalid document path"
        );
        let path = Path::new("corpus/documents").join(id);
        let contents = fs::read(&path).expect("missing corpus document");
        let actual: String = Sha256::digest(contents)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            Some(actual.as_str()),
            document["sha256"].as_str(),
            "corpus hash mismatch for {id}"
        );
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
