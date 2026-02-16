#!/usr/bin/env rust-script
//! Generate Changelog with AI
//!
//! Generates a changelog from git log using the Claude API.
//! Reads commits between the previous tag and HEAD (or a specified tag),
//! sends them to Claude, and outputs a markdown changelog.
//!
//! ## Environment variables (used as fallbacks when CLI args are not provided)
//! - `ANTHROPIC_API_KEY` — API key (fallback for `--api-key`)
//! - `ANTHROPIC_AUTH_TOKEN` — API key (fallback when `ANTHROPIC_API_KEY` is also unset)
//! - `ANTHROPIC_BASE_URL` — base URL (fallback for `--base-url`)
//!
//! ## Usage
//!   rust-script gen_changelog.rs [--tag vX.Y.Z] [--output changelog.md]
//!   rust-script gen_changelog.rs --api-key sk-xxx --base-url https://my-proxy.com
//!   rust-script gen_changelog.rs --prompt "Summarize changes for {tag} since {prev_tag}:\n{git_log}"
//!   rust-script gen_changelog.rs --lang Chinese
//!
//! Without --tag, uses the latest tag on HEAD.
//! Without --output, prints to stdout.
//! Without --api-key, falls back to ANTHROPIC_API_KEY env, then ANTHROPIC_AUTH_TOKEN env.
//! Without --base-url, falls back to ANTHROPIC_BASE_URL env var, then https://api.anthropic.com.
//! With --prompt, uses a custom prompt. Placeholders: {tag}, {prev_tag}, {git_log}, {lang}.
//! With --lang, sets the output language (default: English).
//!
//! ```cargo
//! [dependencies]
//! clap = { version = "4.4", features = ["derive"] }
//! anyhow = "1.0"
//! gix = { version = "0.78", default-features = false, features = ["revision"] }
//! semver = "1.0"
//! serde = { version = "1.0", features = ["derive"] }
//! serde_json = "1.0"
//! ureq = { version = "3.0", features = ["json"] }
//! isolang = { version = "2.4", features = ["english_names"] }
//! ```

use anyhow::{bail, Context, Result};
use clap::Parser;
use gix::bstr::ByteSlice;
use isolang::Language;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Resolve a language string to its English name via `isolang`.
/// Accepts an English name ("Chinese"), ISO 639-1 code ("zh"), or ISO 639-3 code ("zho").
fn resolve_lang(input: &str) -> Result<String> {
    // Try by English name (case-insensitive via title-case attempt)
    if let Some(l) = Language::from_name(input) {
        return Ok(l.to_name().to_string());
    }
    // Try by ISO 639-1 code (e.g. "en", "zh")
    let lower = input.to_lowercase();
    if let Some(l) = Language::from_639_1(&lower) {
        return Ok(l.to_name().to_string());
    }
    // Try by ISO 639-3 code (e.g. "eng", "zho")
    if let Some(l) = Language::from_639_3(&lower) {
        return Ok(l.to_name().to_string());
    }
    bail!(
        "Unknown language '{}'. Use an English name (e.g. Chinese), \
         ISO 639-1 code (e.g. zh), or ISO 639-3 code (e.g. zho).",
        input
    );
}

#[derive(Parser, Debug)]
#[command(name = "gen-changelog", about = "Generate changelog with Claude AI")]
struct Args {
    /// Current release tag (e.g. v0.8.2). Auto-detected if omitted.
    #[arg(long)]
    tag: Option<String>,

    /// Output file path. Prints to stdout if omitted.
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Max number of commits to include when no previous tag exists.
    #[arg(long, default_value = "50")]
    max_commits: usize,

    /// Claude model to use.
    #[arg(long, default_value = "claude-opus-4-6")]
    model: String,

    /// Anthropic API key. Falls back to ANTHROPIC_API_KEY, then ANTHROPIC_AUTH_TOKEN env var.
    #[arg(long)]
    api_key: Option<String>,

    /// Anthropic API base URL. Falls back to ANTHROPIC_BASE_URL env var,
    /// then defaults to https://api.anthropic.com.
    #[arg(long)]
    base_url: Option<String>,

    /// Custom prompt to use instead of the built-in one.
    /// The placeholders {tag}, {prev_tag}, {git_log}, and {lang} will be replaced.
    #[arg(long)]
    prompt: Option<String>,

    /// Language for the generated changelog.
    #[arg(long, default_value = "English")]
    lang: String,
}

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    content: Option<Vec<ContentBlock>>,
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
}

/// Collect all version tags sorted descending by semver.
fn sorted_version_tags(repo: &gix::Repository) -> Result<Vec<(semver::Version, String)>> {
    let mut tags = Vec::new();
    for r in repo.references()?.tags()? {
        let r = r.map_err(|e| anyhow::anyhow!("{}", e))?;
        let name = r.name().shorten().to_string();
        let ver_str = name.strip_prefix('v').unwrap_or(&name);
        if let Ok(v) = semver::Version::parse(ver_str) {
            tags.push((v, name));
        }
    }
    tags.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(tags)
}

/// Detect the tag pointing at HEAD.
fn detect_current_tag(repo: &gix::Repository) -> Result<String> {
    let head_id = repo.head_id()?;
    for r in repo.references()?.tags()? {
        let r = r.map_err(|e| anyhow::anyhow!("{}", e))?;
        let target = r
            .try_id()
            .map(|id| id.detach())
            .or_else(|| r.try_id().map(|id| id.detach()));
        if let Some(id) = target {
            // Peel annotated tags to the commit
            let peeled = repo
                .find_object(id)
                .ok()
                .and_then(|o| o.peel_to_kind(gix::object::Kind::Commit).ok())
                .map(|o| o.id);
            let tag_commit = peeled.unwrap_or(id);
            if *tag_commit == *head_id {
                return Ok(r.name().shorten().to_string());
            }
        }
    }
    bail!("No tag found on HEAD. Use --tag to specify one.");
}

/// Find the previous version tag before `current_tag`.
fn find_previous_tag(repo: &gix::Repository, current_tag: &str) -> Option<String> {
    sorted_version_tags(repo)
        .ok()?
        .into_iter()
        .map(|(_, name)| name)
        .find(|name| name != current_tag)
}

/// Resolve a tag name to a commit id.
fn resolve_tag(repo: &gix::Repository, tag: &str) -> Result<gix::ObjectId> {
    let r = repo
        .find_reference(&format!("refs/tags/{}", tag))
        .with_context(|| format!("Tag '{}' not found", tag))?;
    let id = r.id().detach();
    let peeled = repo
        .find_object(id)?
        .peel_to_kind(gix::object::Kind::Commit)?
        .id;
    Ok(peeled)
}

/// Collect one-line commit summaries between two points.
fn get_git_log(
    repo: &gix::Repository,
    prev_tag: Option<&str>,
    max_commits: usize,
) -> Result<String> {
    let head_id = repo.head_id()?;

    let stop_at = prev_tag.map(|t| resolve_tag(repo, t)).transpose()?;

    let mut lines = Vec::new();
    let walk = head_id.ancestors().first_parent_only().all()?;
    for info in walk {
        let info = info?;
        if let Some(stop) = stop_at {
            if info.id == stop {
                break;
            }
        }
        if lines.len() >= max_commits {
            break;
        }

        let object = repo.find_object(info.id)?;
        let commit = object.into_commit();
        let message = commit.message_raw_sloppy();
        let first_line = message
            .lines()
            .next()
            .unwrap_or(b"")
            .to_str_lossy()
            .to_string();

        // Skip merge commits (simple heuristic)
        if first_line.starts_with("Merge ") {
            continue;
        }

        let short_id = &info.id.to_string()[..7];
        lines.push(format!("{} {}", short_id, first_line));
    }

    if lines.is_empty() {
        bail!("No commits found for changelog.");
    }
    Ok(lines.join("\n"))
}

fn call_claude(api_key: &str, base_url: &str, model: &str, prompt: &str) -> Result<String> {
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

    let request = ApiRequest {
        model: model.to_string(),
        max_tokens: 1024,
        messages: vec![Message {
            role: "user".to_string(),
            content: prompt.to_string(),
        }],
    };

    let resp: ApiResponse = ureq::post(&url)
        .header("Content-Type", "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send_json(&request)
        .context("Failed to call Claude API")?
        .body_mut()
        .read_json()
        .context("Failed to parse Claude API response")?;

    if let Some(err) = resp.error {
        bail!("Claude API error: {}", err.message);
    }

    let text = resp
        .content
        .and_then(|blocks| blocks.into_iter().find_map(|b| b.text))
        .unwrap_or_default();

    if text.is_empty() {
        bail!("Claude API returned empty response");
    }

    Ok(text)
}

fn build_prompt(tag: &str, prev_tag: &str, git_log: &str, lang: &str) -> String {
    format!(
        "Write a concise changelog for release {} (since {}) of WebFly, \
         an Android app like Expo Go for WebF engine.\n\n\
         Git log:\n{}\n\n\
         Rules:\n\
         - Group by: Features, Fixes, Improvements, Other\n\
         - Skip empty groups\n\
         - Use markdown with bullet points\n\
         - Keep it short and user-facing\n\
         - Write in {}\n\
         - Do NOT wrap in code blocks",
        tag, prev_tag, git_log, lang
    )
}

fn main() -> Result<()> {
    let args = Args::parse();

    let api_key = args
        .api_key
        .clone()
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
        .context(
            "API key not provided. Use --api-key or set ANTHROPIC_API_KEY / ANTHROPIC_AUTH_TOKEN",
        )?;
    let base_url = args
        .base_url
        .clone()
        .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok())
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());

    let lang = resolve_lang(&args.lang)?;

    let repo = gix::discover(".").context("Not a git repository")?;

    let current_tag = match &args.tag {
        Some(t) => t.clone(),
        None => detect_current_tag(&repo)?,
    };
    let prev_tag = find_previous_tag(&repo, &current_tag);

    eprintln!(
        "Generating changelog for {} (since {})...",
        current_tag,
        prev_tag.as_deref().unwrap_or("initial")
    );

    let git_log = get_git_log(&repo, prev_tag.as_deref(), args.max_commits)?;
    let prev_tag_str = prev_tag.as_deref().unwrap_or("initial");
    let prompt = match &args.prompt {
        Some(custom) => custom
            .replace("{tag}", &current_tag)
            .replace("{prev_tag}", prev_tag_str)
            .replace("{git_log}", &git_log)
            .replace("{lang}", &lang),
        None => build_prompt(&current_tag, prev_tag_str, &git_log, &lang),
    };

    let changelog = match call_claude(&api_key, &base_url, &args.model, &prompt) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("AI changelog failed: {}. Falling back to git log.", e);
            format!(
                "## Changes since {}\n\n{}",
                prev_tag_str,
                git_log
                    .lines()
                    .map(|l| format!("- {}", l))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    };

    match &args.output {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, &changelog)
                .with_context(|| format!("Failed to write {}", path.display()))?;
            eprintln!("Changelog written to: {}", path.display());
        }
        None => {
            println!("{}", changelog);
        }
    }

    Ok(())
}
