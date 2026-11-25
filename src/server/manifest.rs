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

#[cfg(test)]
mod tests {
    use super::{Profile, inspect};
    use std::fmt::Write;

    #[test]
    fn rejects_invalid_manifest_shapes_and_size() {
        for (source, message) in [
            (
                "dependencies = 'invalid'",
                "dependencies must be a TOML table",
            ),
            (
                "[dependencies]\nui = 42",
                "dependencies.ui must be a version string or table",
            ),
            (
                "[dependencies]\nui = { package = 42 }",
                "dependencies.ui.package must be a string",
            ),
            ("target = 42", "target must be a TOML table"),
            ("[target]\nlinux = 42", "target.linux must be a TOML table"),
        ] {
            let error = inspect(source, Profile::All).unwrap_err();
            assert!(error.message.contains(message), "{error:?}");
        }
        let error = inspect(&" ".repeat(64_001), Profile::All).unwrap_err();
        assert!(error.message.contains("manifest exceeds 64,000 bytes"));
    }

    #[test]
    fn limits_matching_dependencies_and_filters_profiles() {
        let mut source = String::from("[dependencies]\n");
        for index in 0..100 {
            writeln!(
                source,
                "ui{index} = {{ package = 'rangular', version = '1' }}"
            )
            .unwrap();
        }
        assert_eq!(
            inspect(&source, Profile::Rangular).unwrap()["dependencies"]
                .as_array()
                .unwrap()
                .len(),
            100
        );
        source.push_str("ui100 = { package = 'rangular', version = '1' }\n");
        let error = inspect(&source, Profile::All).unwrap_err();
        assert!(
            error
                .message
                .contains("more than 100 matching dependencies")
        );
        let filtered = inspect(&source, Profile::Serenade).unwrap();
        assert_eq!(filtered["dependencies"], serde_json::json!([]));
    }

    #[test]
    fn inspects_workspace_dependencies_and_ignores_unrelated_packages() {
        let result = inspect(
            "[workspace.dependencies]\nui = { package = 'rangular', version = '1' }\nserde = '1'\n",
            Profile::All,
        )
        .unwrap();
        let dependencies = result["dependencies"].as_array().unwrap();
        assert_eq!(dependencies.len(), 1);
        assert_eq!(dependencies[0]["package"], "rangular");
        assert_eq!(dependencies[0]["scope"], "workspace.dependencies");
    }
}
