#!/usr/bin/env rust-script
//! GitHub API Rate Limit Checker
//!
//! Checks the current GitHub API rate limit status for the caller's IP
//! (unauthenticated) or token (authenticated via `GITHUB_TOKEN` / `gh` CLI).
//!
//! ## What it does
//! - Queries `https://api.github.com/rate_limit`
//! - Shows remaining / limit / reset time for core, search, and graphql
//! - Exits with code 1 if core remaining == 0 (rate-limited)
//!
//! Usage:
//!   rust-script gh_rate_limit.rs
//!   rust-script gh_rate_limit.rs --no-auth   # check unauthenticated limit
//!   rust-script gh_rate_limit.rs --json
//!
//! ```cargo
//! [dependencies]
//! anyhow = "1.0"
//! clap = { version = "4.4", features = ["derive"] }
//! ureq = { version = "2.9", features = ["json"] }
//! serde = { version = "1.0", features = ["derive"] }
//! chrono = "0.4"
//! ```

use anyhow::{Context, Result};
use chrono::{Local, TimeZone};
use clap::Parser;
use serde::Deserialize;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "gh-rate-limit", about = "Check GitHub API rate limit status")]
struct Args {
    /// Output raw JSON instead of formatted table
    #[arg(long)]
    json: bool,

    /// Force unauthenticated request (ignore GITHUB_TOKEN / gh CLI)
    #[arg(long, alias = "unauth")]
    no_auth: bool,
}

#[derive(Deserialize)]
struct RateLimitResponse {
    resources: Resources,
}

#[derive(Deserialize)]
struct Resources {
    core: ResourceInfo,
    search: ResourceInfo,
    graphql: ResourceInfo,
}

#[derive(Deserialize)]
struct ResourceInfo {
    limit: u32,
    remaining: u32,
    reset: i64,
    used: u32,
}

/// Try to get a GitHub token from environment or `gh` CLI.
fn resolve_token() -> Option<String> {
    // 1. Explicit env var
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }

    // 2. gh CLI
    let output = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()?;
    if output.status.success() {
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !token.is_empty() {
            return Some(token);
        }
    }

    None
}

fn main() -> Result<()> {
    let args = Args::parse();

    let token = if args.no_auth { None } else { resolve_token() };
    let authenticated = token.is_some();

    let mut req = ureq::get("https://api.github.com/rate_limit")
        .set("Accept", "application/vnd.github.v3+json")
        .set("User-Agent", "flutter-tools-gh-rate-limit");

    if let Some(ref t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }

    let resp = req.call().context("Failed to query GitHub API")?;

    if args.json {
        let body: String = resp.into_string()?;
        println!("{body}");
        return Ok(());
    }

    let body: RateLimitResponse = resp.into_json().context("Failed to parse response")?;

    let mode = if authenticated {
        "authenticated"
    } else {
        "unauthenticated"
    };

    println!("GitHub API Rate Limit ({mode})\n");
    println!(
        "{:<12} {:>6} / {:<6}  {:>6}  {}",
        "Resource", "Left", "Limit", "Used", "Resets at"
    );
    println!("{}", "-".repeat(60));

    let resources = [
        ("core", &body.resources.core),
        ("search", &body.resources.search),
        ("graphql", &body.resources.graphql),
    ];

    let mut blocked = false;

    for (name, info) in &resources {
        let reset_time = Local
            .timestamp_opt(info.reset, 0)
            .single()
            .map(|t| t.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "?".into());

        let status = if info.remaining == 0 && info.limit > 0 {
            blocked = blocked || *name == "core";
            " << EXHAUSTED"
        } else {
            ""
        };

        println!(
            "{:<12} {:>6} / {:<6}  {:>6}  {}{status}",
            name, info.remaining, info.limit, info.used, reset_time,
        );
    }

    println!();
    if blocked {
        println!("Core API is rate-limited. Wait until reset or use a GITHUB_TOKEN.");
        std::process::exit(1);
    } else {
        println!("OK");
    }

    Ok(())
}
