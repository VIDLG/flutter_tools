use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;
use which::which;

use crate::config::FlutterCreateConfig;

pub fn resolve_cmd(command: &str) -> Result<std::path::PathBuf> {
    if command.contains(['/', '\\']) {
        let path = std::path::PathBuf::from(command);
        if path.exists() {
            return Ok(path);
        }
        bail!("command not found at: {}", path.display());
    }
    which(command).with_context(|| format!("command not found in PATH: {command}"))
}

pub fn run_flutter_create(
    path: &Path,
    flutter_cmd: &Path,
    project_name: &str,
    org: Option<&str>,
    description: Option<&str>,
    create: &FlutterCreateConfig,
) -> Result<()> {
    let mut command = Command::new(flutter_cmd);
    command
        .arg("create")
        .arg("--project-name")
        .arg(project_name);
    if let Some(platforms) = create.platforms.as_ref() {
        if !platforms.is_empty() {
            command.arg("--platforms").arg(platforms.join(","));
        }
    }
    if let Some(value) = create.android_language.as_deref() {
        command.arg("--android-language").arg(value);
    }
    if let Some(value) = org {
        command.arg("--org").arg(value);
    }
    if let Some(value) = description {
        command.arg("--description").arg(value);
    }
    let status = command
        .arg(path)
        .status()
        .context("Failed to run flutter create")?;
    if !status.success() {
        bail!("flutter create failed with status: {status}");
    }
    Ok(())
}

pub fn remove_dir_all_with_retry(path: &Path) -> Result<()> {
    fs::remove_dir_all(path).with_context(|| {
        format!(
            "Failed to remove directory: {}. Use kill-file-handles tool if locked.",
            path.display()
        )
    })
}
