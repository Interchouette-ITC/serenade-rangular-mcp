use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use rmcp::ErrorData;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

const OUTPUT_LIMIT: usize = 32_000;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScaffoldArgs {
    /// Existing absolute parent directory for the new application.
    pub directory: PathBuf,
    /// New package and directory name; an existing destination is never overwritten.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CargoTask {
    Check,
    Build,
    Test,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CargoArgs {
    /// Absolute directory containing the Cargo.toml to use.
    pub directory: PathBuf,
    /// Cargo operation to run.
    pub task: CargoTask,
    /// Optional workspace package name.
    pub package: Option<String>,
    /// Optional Cargo features.
    pub features: Option<Vec<String>>,
    /// Disable default features.
    pub no_default_features: Option<bool>,
    /// Use the release profile.
    pub release: Option<bool>,
    /// Require the lockfile to remain unchanged.
    pub locked: Option<bool>,
}

fn directory(path: &Path) -> Result<PathBuf, ErrorData> {
    if !path.is_absolute() {
        return Err(ErrorData::invalid_params(
            "directory must be absolute",
            None,
        ));
    }
    let path = path.canonicalize().map_err(|error| {
        ErrorData::invalid_params(format!("cannot access directory: {error}"), None)
    })?;
    if !path.is_dir() {
        return Err(ErrorData::invalid_params(
            "directory must be a directory",
            None,
        ));
    }
    Ok(path)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

async fn capture(mut reader: impl AsyncRead + Unpin) -> std::io::Result<(String, bool)> {
    let mut buffer = [0; 8192];
    let mut output = Vec::new();
    let mut truncated = false;
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        let keep = count.min(OUTPUT_LIMIT.saturating_sub(output.len()));
        output.extend_from_slice(&buffer[..keep]);
        truncated |= keep < count;
    }
    Ok((String::from_utf8_lossy(&output).into_owned(), truncated))
}

async fn run(mut command: Command) -> Result<(bool, Value), ErrorData> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| {
            ErrorData::internal_error(format!("cannot start command: {error}"), None)
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ErrorData::internal_error("command stdout unavailable", None))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ErrorData::internal_error("command stderr unavailable", None))?;
    let (stdout, stderr, status) = tokio::try_join!(
        Box::pin(capture(stdout)),
        Box::pin(capture(stderr)),
        child.wait()
    )
    .map_err(|error| ErrorData::internal_error(format!("command failed: {error}"), None))?;
    Ok((
        status.success(),
        json!({
            "success": status.success(),
            "exit_code": status.code(),
            "status": status.to_string(),
            "stdout": stdout.0,
            "stdout_truncated": stdout.1,
            "stderr": stderr.0,
            "stderr_truncated": stderr.1
        }),
    ))
}

pub async fn scaffold(args: ScaffoldArgs) -> Result<(bool, Value), ErrorData> {
    let parent = directory(&args.directory)?;
    if !valid_name(&args.name) || args.name.ends_with('-') {
        return Err(ErrorData::invalid_params("invalid application name", None));
    }
    match std::fs::symlink_metadata(parent.join(&args.name)) {
        Ok(_) => {
            return Err(ErrorData::invalid_params(
                "destination already exists",
                None,
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(ErrorData::invalid_params(
                format!("cannot inspect destination: {error}"),
                None,
            ));
        }
    }
    let mut command = Command::new("serenade");
    command
        .current_dir(&parent)
        .args(["new", &args.name, "--path"]);
    command.arg(&parent);
    run(command).await
}

pub async fn cargo(args: CargoArgs) -> Result<(bool, Value), ErrorData> {
    let root = directory(&args.directory)?;
    if !root.join("Cargo.toml").is_file() {
        return Err(ErrorData::invalid_params(
            "directory has no Cargo.toml",
            None,
        ));
    }
    let mut command = Command::new("cargo");
    command.current_dir(root).arg(match args.task {
        CargoTask::Check => "check",
        CargoTask::Build => "build",
        CargoTask::Test => "test",
    });
    if let Some(package) = args.package {
        if !valid_name(&package) {
            return Err(ErrorData::invalid_params("invalid package name", None));
        }
        command.args(["--package", &package]);
    }
    if let Some(features) = args.features {
        if features.is_empty()
            || features.len() > 100
            || features.iter().any(|feature| {
                feature.is_empty()
                    || feature.len() > 128
                    || feature.starts_with('-')
                    || !feature
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_./?".contains(&b))
            })
        {
            return Err(ErrorData::invalid_params("invalid feature list", None));
        }
        command.args(["--features", &features.join(",")]);
    }
    if args.no_default_features.unwrap_or(false) {
        command.arg("--no-default-features");
    }
    if args.release.unwrap_or(false) {
        command.arg("--release");
    }
    if args.locked.unwrap_or(false) {
        command.arg("--locked");
    }
    run(command).await
}

#[cfg(test)]
mod tests {
    use super::{OUTPUT_LIMIT, capture};

    #[tokio::test]
    async fn output_capture_is_bounded_and_reports_truncation() {
        let input = vec![b'x'; OUTPUT_LIMIT + 1];
        let (output, truncated) = capture(input.as_slice()).await.unwrap();
        assert_eq!(output.len(), OUTPUT_LIMIT);
        assert!(truncated);
        let (output, truncated) = capture(&input[..OUTPUT_LIMIT]).await.unwrap();
        assert_eq!(output.len(), OUTPUT_LIMIT);
        assert!(!truncated);
    }
}
