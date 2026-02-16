use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config::AndroidConfig;

/// Files to skip when copying from platforms/android/ to android/.
const SKIP_FILES: &[&str] = &["keystore.jks", "key.properties.example"];

/// File extensions that support `{{var}}` template substitution.
const TEMPLATE_EXTENSIONS: &[&str] = &["kts", "xml", "properties"];

/// Recursively copy files from `src` to `dst`, applying `{{var}}` template
/// substitution on supported file types. Files listed in `SKIP_FILES` are
/// not copied.
fn copy_with_templates(src: &Path, dst: &Path, vars: &HashMap<String, String>) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("Failed to create dir: {}", dst.display()))?;

    for entry in
        fs::read_dir(src).with_context(|| format!("Failed to read dir: {}", src.display()))?
    {
        let entry = entry?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if src_path.is_dir() {
            copy_with_templates(&src_path, &dst_path, vars)?;
            continue;
        }

        // Skip non-copyable files
        let name = file_name.to_string_lossy();
        if SKIP_FILES.iter().any(|s| *s == name.as_ref()) {
            continue;
        }

        // Check if this file type supports template substitution
        let is_template = src_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| TEMPLATE_EXTENSIONS.iter().any(|t| *t == ext))
            .unwrap_or(false);

        if is_template && !vars.is_empty() {
            let content = fs::read_to_string(&src_path)
                .with_context(|| format!("Failed to read: {}", src_path.display()))?;
            let rendered = apply_template(&content, vars);
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&dst_path, rendered)
                .with_context(|| format!("Failed to write: {}", dst_path.display()))?;
        } else {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "Failed to copy {} -> {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }

    Ok(())
}

/// Replace all `{{key}}` occurrences in `content` with values from `vars`.
fn apply_template(content: &str, vars: &HashMap<String, String>) -> String {
    let mut result = content.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key); // {{key}}
        result = result.replace(&placeholder, value);
    }
    result
}

pub fn apply_gradle_wrapper_properties(path: &Path, distribution_url: &str) -> Result<()> {
    let mut props = read_properties(path)?;
    props.insert("distributionUrl".to_string(), distribution_url.to_string());
    write_properties(path, &props)?;
    Ok(())
}

fn read_properties(path: &Path) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let file =
        fs::File::open(path).with_context(|| format!("Failed to read file: {}", path.display()))?;
    let props = java_properties::read(std::io::BufReader::new(file))
        .with_context(|| format!("Failed to parse properties: {}", path.display()))?;
    Ok(props)
}

fn write_properties(path: &Path, props: &HashMap<String, String>) -> Result<()> {
    let file = fs::File::create(path)
        .with_context(|| format!("Failed to write file: {}", path.display()))?;
    java_properties::write(std::io::BufWriter::new(file), props)
        .with_context(|| format!("Failed to write properties: {}", path.display()))?;
    Ok(())
}

pub fn process_android_platform(
    project_dir: &Path,
    config: &AndroidConfig,
    platforms_dir: Option<&str>,
    template_vars: &HashMap<String, String>,
) -> Result<()> {
    let android_dir = project_dir.join("android");

    let platforms_root = platforms_dir
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .unwrap_or("platforms");
    let src_dir = project_dir.join(platforms_root).join("android");

    if !src_dir.exists() {
        anyhow::bail!(
            "Android platform templates directory not found: {}",
            src_dir.display()
        );
    }

    // Recursively copy platforms/android/ → android/, applying {{var}} substitution
    copy_with_templates(&src_dir, &android_dir, template_vars)?;

    // Apply gradle wrapper distribution URL if configured
    if let Some(distribution_url) = &config.gradle_wrapper.distribution_url {
        apply_gradle_wrapper_properties(
            &android_dir.join("gradle/wrapper/gradle-wrapper.properties"),
            distribution_url,
        )?;
    }

    println!("Android directory generated at: {}", android_dir.display());
    Ok(())
}
