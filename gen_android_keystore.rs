#!/usr/bin/env rust-script
//! Generate Android Keystore
//!
//! Generates a new Android release keystore (.jks) by reading passwords
//! from key.properties and key alias from app.pkl (via `pkl eval`).
//!
//! ## What it does
//! - Reads `storePassword`, `keyPassword` from key.properties
//! - Reads `key_alias` from app.pkl via `pkl eval`
//! - Runs `keytool` to generate a new RSA 2048-bit keystore valid for 100 years
//! - Skips if keystore already exists (use --force to override)
//!
//! Usage:
//!   rust-script gen_android_keystore.rs [--props PATH] [--output PATH] [--force]
//!   rust-script gen_android_keystore.rs --alias mykey
//!
//! ```cargo
//! [dependencies]
//! clap = { version = "4.4", features = ["derive"] }
//! anyhow = "1.0"
//! java-properties = "2.0"
//! which = "8.0"
//! ```

use anyhow::{bail, Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "gen-android-keystore",
    about = "Generate Android release keystore"
)]
struct Args {
    /// Path to key.properties file
    #[arg(long, default_value = "platforms/android/key.properties")]
    props: PathBuf,

    /// Output path for the keystore file
    #[arg(long, default_value = "platforms/android/keystore.jks")]
    output: PathBuf,

    /// Key alias. If omitted, reads from app.pkl.
    #[arg(long)]
    alias: Option<String>,

    /// Path to app.pkl config
    #[arg(long, default_value = "app.pkl")]
    config: PathBuf,

    /// Distinguished name for the certificate
    #[arg(
        long,
        default_value = "CN=Android, OU=Dev, O=Dev, L=Unknown, ST=Unknown, C=CN"
    )]
    dname: String,

    /// Overwrite existing keystore
    #[arg(long)]
    force: bool,
}

fn read_alias_from_pkl(config_path: &std::path::Path) -> Result<String> {
    let pkl_cmd = which::which("pkl").context("pkl not found in PATH")?;
    let output = Command::new(&pkl_cmd)
        .args(["eval", "--expression", "android.template_vars.key_alias"])
        .arg(config_path)
        .output()
        .context("Failed to run pkl eval")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("pkl eval failed: {}", stderr);
    }

    let alias = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('"')
        .to_string();
    if alias.is_empty() {
        bail!("key_alias is empty in {}", config_path.display());
    }
    Ok(alias)
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Check key.properties exists
    if !args.props.exists() {
        bail!(
            "{} not found.\nCopy key.properties.example to key.properties and fill in passwords.",
            args.props.display()
        );
    }

    // Check keytool is available
    let keytool = which::which("keytool").context("keytool not found in PATH. Install a JDK.")?;

    // Read passwords from key.properties
    let file = fs::File::open(&args.props)
        .with_context(|| format!("Failed to read {}", args.props.display()))?;
    let props = java_properties::read(std::io::BufReader::new(file))
        .with_context(|| format!("Failed to parse {}", args.props.display()))?;

    let store_password = props
        .get("storePassword")
        .filter(|s| !s.is_empty())
        .context("storePassword is missing or empty in key.properties")?;
    let key_password = props
        .get("keyPassword")
        .filter(|s| !s.is_empty())
        .context("keyPassword is missing or empty in key.properties")?;

    // Read alias: CLI flag > app.pkl
    let key_alias = match &args.alias {
        Some(a) => a.clone(),
        None => read_alias_from_pkl(&args.config)?,
    };

    // Check existing keystore
    if args.output.exists() && !args.force {
        println!(
            "Keystore already exists at {}. Skipping. Use --force to overwrite.",
            args.output.display()
        );
        return Ok(());
    }

    // Ensure output directory exists
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Remove existing file if --force
    if args.output.exists() {
        fs::remove_file(&args.output)
            .with_context(|| format!("Failed to remove {}", args.output.display()))?;
    }

    // Run keytool
    let status = Command::new(&keytool)
        .args([
            "-genkey",
            "-v",
            "-keystore",
            &args.output.to_string_lossy(),
            "-keyalg",
            "RSA",
            "-keysize",
            "2048",
            "-validity",
            "36500",
            "-alias",
            &key_alias,
            "-storepass",
            store_password,
            "-keypass",
            key_password,
            "-dname",
            &args.dname,
        ])
        .status()
        .context("Failed to run keytool")?;

    if !status.success() {
        bail!("keytool failed with status: {}", status);
    }

    println!("Keystore generated at: {}", args.output.display());
    Ok(())
}
