#!/usr/bin/env rust-script
//! Bump Flutter Version
//!
//! Updates the version in pubspec.yaml.
//! Also ensures a lightweight git tag exists for the *current* version before bumping.
//!
//! ## What it does
//! - Reads the current `version:` from `pubspec.yaml` (YAML parser, with a regex fallback).
//! - Checks if a tag already exists for that version (`vX.Y.Z` or `X.Y.Z`, including prerelease).
//! - If neither exists, creates a **lightweight** tag pointing at `HEAD`.
//! - Then bumps the version in `pubspec.yaml` by the requested part.
//!
//! Notes:
//! - Tag creation is **local only** (no fetch/push).
//! - If not in a git repo, if `HEAD` is unborn (no commits), or if the version can't be read,
//!   the tag step is skipped.
//! - Writing the new `version:` uses a regex replace to preserve formatting/comments.
//!
//! Usage:
//!   rust-script bump_version.rs <major|minor|patch|build> [--pubspec PATH] [--tag-prefix v|none]
//!
//! Examples:
//! - Patch bump, default tag prefix `v`:
//!   `rust-script bump_version.rs patch`
//! - Build bump using a different pubspec:
//!   `rust-script bump_version.rs build --pubspec path/to/pubspec.yaml`
//! - Create tags without `v` prefix:
//!   `rust-script bump_version.rs minor --tag-prefix none`
//!
//! ```cargo
//! [dependencies]
//! clap = { version = "4.4", features = ["derive"] }
//! regex = "1.10"
//! anyhow = "1.0"
//! semver = "1.0"
//! gix = "0.78"
//! serde = { version = "1.0", features = ["derive"] }
//! serde_yaml = "0.9"
//! ```

use clap::{Parser, ValueEnum};
use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use regex::Regex;
use semver::{Version, Prerelease, BuildMetadata};
use gix::refs::transaction::PreviousValue;
use serde::Deserialize;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The part of the version to increment
    #[arg(value_enum)]
    part: VersionPart,

    /// Path to pubspec.yaml
    #[arg(long, default_value = "pubspec.yaml")]
    pubspec: String,

    /// Tag prefix for the auto-created lightweight tag.
    ///
    /// Use `v` to create tags like `v1.2.3`, or `none` to create `1.2.3`.
    #[arg(long, value_enum, default_value = "v")]
    tag_prefix: TagPrefix,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum TagPrefix {
    V,
    None,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum VersionPart {
    Major,
    Minor,
    Patch,
    Build,
}

fn tag_exists(repo: &gix::Repository, tag: &str) -> Result<bool> {
    let full = format!("refs/tags/{tag}");
    Ok(repo.try_find_reference(full.as_str())?.is_some())
}

#[derive(Debug, Deserialize)]
struct PubspecYaml {
    version: Option<String>,
}

fn read_pubspec_version(content: &str) -> Option<String> {
    // YAML parse (preferred): robust against indentation/ordering differences.
    if let Ok(doc) = serde_yaml::from_str::<PubspecYaml>(content) {
        if let Some(v) = doc.version {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }

    // Fallback: handle partial/invalid YAML while still supporting the common case.
    let version_line_regex = Regex::new(r"(?m)^version:\s*(.+)$").ok()?;
    version_line_regex
        .captures(content)
        .map(|c| c[1].trim().to_string())
        .filter(|s| !s.is_empty())
}

fn ensure_current_version_tag(pubspec_path: &Path, tag_prefix: TagPrefix) -> Result<()> {
    let start_dir = pubspec_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let repo = match gix::discover(start_dir) {
        Ok(r) => r,
        Err(_) => {
            println!("[bump-version] Skipping tag check (not in a git repository)");
            return Ok(());
        }
    };

    let content = fs::read_to_string(pubspec_path)
        .with_context(|| format!("Failed to read {}", pubspec_path.display()))?;

    let version_str = match read_pubspec_version(&content) {
        Some(v) => v,
        None => {
            println!("[bump-version] Skipping tag check (no version found in pubspec)");
            return Ok(());
        }
    };
    let v = match Version::parse(&version_str) {
        Ok(v) => v,
        Err(e) => {
            println!(
                "[bump-version] Skipping tag check (invalid semver in pubspec '{}'): {}",
                version_str, e
            );
            return Ok(());
        }
    };

    // Tag name: use semver core (+ optional prerelease), ignore build metadata.
    let mut base = format!("{}.{}.{}", v.major, v.minor, v.patch);
    if !v.pre.is_empty() {
        base = format!("{}-{}", base, v.pre);
    }
    let tag_plain = base.clone();
    let tag_v = format!("v{}", base);

    let preferred_tag = match tag_prefix {
        TagPrefix::V => tag_v.clone(),
        TagPrefix::None => tag_plain.clone(),
    };

    // If either convention exists, do nothing.
    if tag_exists(&repo, &tag_plain)? || tag_exists(&repo, &tag_v)? {
        println!(
            "[bump-version] Tag already exists for current version: {} (checked '{}' and '{}')",
            version_str, tag_plain, tag_v
        );
        return Ok(());
    }

    let head_id = match repo.head_id() {
        Ok(id) => id.detach(),
        Err(_) => {
            println!("[bump-version] Skipping tag creation (repository has no commits yet)");
            return Ok(());
        }
    };

    repo.tag_reference(&preferred_tag, head_id, PreviousValue::MustNotExist)
        .with_context(|| format!("Failed to create lightweight tag '{preferred_tag}'"))?;
    println!(
        "[bump-version] Created lightweight tag '{}' for current version {}",
        preferred_tag, version_str
    );
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let pubspec_path = Path::new(&args.pubspec);

    // Ensure the current version is tagged before bumping.
    ensure_current_version_tag(pubspec_path, args.tag_prefix)?;

    let content = fs::read_to_string(pubspec_path)
        .with_context(|| format!("Failed to read {}", pubspec_path.display()))?;

    // Regex to locate the version line, preserving indentation and formatting
    let version_line_regex = Regex::new(r"(?m)^version:\s*(.+)$").unwrap();

    let mut new_version_string = String::new();

    let new_content = version_line_regex.replace(&content, |caps: &regex::Captures| {
        let old_version_str = caps[1].trim();
        // Use semver crate to parse the version string
        let mut v = Version::parse(old_version_str)
            .unwrap_or_else(|e| panic!("Invalid semver format in pubspec.yaml '{}': {}", old_version_str, e));
        
        // Helper to parse numeric build number (Flutter standard)
        // Returns 0 if no build number or not numeric
        let current_build_num: u64 = if v.build.is_empty() {
            0
        } else {
            v.build.as_str().parse().unwrap_or(0)
        };

        match args.part {
            VersionPart::Major => {
                v.major += 1;
                v.minor = 0;
                v.patch = 0;
                v.pre = Prerelease::EMPTY;
                // Reset build number to 1 on major bump
                v.build = BuildMetadata::new("1").unwrap();
            }
            VersionPart::Minor => {
                v.minor += 1;
                v.patch = 0;
                v.pre = Prerelease::EMPTY;
                // Reset build number to 1 on minor bump
                v.build = BuildMetadata::new("1").unwrap();
            }
            VersionPart::Patch => {
                v.patch += 1;
                v.pre = Prerelease::EMPTY;
                // Reset build number to 1 on patch bump
                v.build = BuildMetadata::new("1").unwrap();
            }
            VersionPart::Build => {
                let new_build = current_build_num + 1;
                v.build = BuildMetadata::new(&new_build.to_string()).unwrap();
            }
        }
        
        new_version_string = v.to_string();
        format!("version: {}", new_version_string)
    });

    if new_version_string.is_empty() {
        println!("No version line found in {}", pubspec_path.display());
        return Ok(());
    }

    fs::write(pubspec_path, new_content.to_string())?;
    println!("Bumped version to: {}", new_version_string);

    Ok(())
}
