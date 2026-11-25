use rmcp::ErrorData;
use serde_json::{Value, json};

use super::Profile;

fn dependencies(
    value: &toml::Value,
    scope: &str,
    profile: Profile,
    found: &mut Vec<Value>,
) -> Result<(), ErrorData> {
    let table = value
        .as_table()
        .ok_or_else(|| ErrorData::invalid_params(format!("{scope} must be a TOML table"), None))?;
    for (alias, declaration) in table {
        let package = match declaration {
            toml::Value::String(_) => alias.as_str(),
            toml::Value::Table(fields) => match fields.get("package") {
                Some(value) => value.as_str().ok_or_else(|| {
                    ErrorData::invalid_params(
                        format!("{scope}.{alias}.package must be a string"),
                        None,
                    )
                })?,
                None => alias.as_str(),
            },
            _ => {
                return Err(ErrorData::invalid_params(
                    format!("{scope}.{alias} must be a version string or table"),
                    None,
                ));
            }
        };
        let product = if package == "rangular" || package.starts_with("rangular-") {
            "rangular"
        } else if package.starts_with("serenade-") {
            "serenade"
        } else {
            continue;
        };
        if profile.includes(product) {
            found.push(json!({
                "product": product,
                "package": package,
                "alias": alias,
                "scope": scope,
                "declaration": declaration,
            }));
        }
    }
    Ok(())
}

pub fn inspect(source: &str, profile: Profile) -> Result<Value, ErrorData> {
    if source.len() > 64_000 {
        return Err(ErrorData::invalid_params(
            "manifest exceeds 64,000 bytes",
            None,
        ));
    }
    let manifest: toml::Value = toml::from_str(source)
        .map_err(|error| ErrorData::invalid_params(format!("invalid TOML: {error}"), None))?;
    let mut found = Vec::new();
    for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(value) = manifest.get(key) {
            dependencies(value, key, profile, &mut found)?;
        }
    }
    if let Some(value) = manifest
        .get("workspace")
        .and_then(|v| v.get("dependencies"))
    {
        dependencies(value, "workspace.dependencies", profile, &mut found)?;
    }
    if let Some(targets) = manifest.get("target") {
        let targets = targets
            .as_table()
            .ok_or_else(|| ErrorData::invalid_params("target must be a TOML table", None))?;
        for (target, sections) in targets {
            if !sections.is_table() {
                return Err(ErrorData::invalid_params(
                    format!("target.{target} must be a TOML table"),
                    None,
                ));
            }
            for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(value) = sections.get(key) {
                    dependencies(
                        value,
                        &format!("target.{target}.{key}"),
                        profile,
                        &mut found,
                    )?;
                }
            }
        }
    }
    if found.len() > 100 {
        return Err(ErrorData::invalid_params(
            "manifest contains more than 100 matching dependencies",
            None,
        ));
    }
    Ok(json!({
        "package": manifest.get("package").and_then(|v| v.get("name")).and_then(toml::Value::as_str),
        "dependencies": found,
        "scope": "Declarations in the supplied manifest only; workspace-inherited dependencies and Cargo's resolved graph require their own manifests or cargo metadata."
    }))
}
