use std::{env, fmt::Write as _, fs, path::Path};

use sha2::{Digest, Sha256};

fn main() {
    println!("cargo:rerun-if-changed=corpus/manifest.json");
    println!("cargo:rerun-if-changed=corpus/documents");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read("corpus/manifest.json").expect("missing corpus; run make refresh-docs"),
    )
    .expect("invalid corpus manifest");
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
        let actual =
            Sha256::digest(contents)
                .iter()
                .fold(String::with_capacity(64), |mut output, byte| {
                    write!(output, "{byte:02x}").expect("writing to a string cannot fail");
                    output
                });
        assert_eq!(
            Some(actual.as_str()),
            document["sha256"].as_str(),
            "corpus hash mismatch for {id}"
        );
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
