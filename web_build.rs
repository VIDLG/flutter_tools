#!/usr/bin/env -S rust-script
//! ```cargo
//! [dependencies]
//! clap = { version = "4.5", features = ["derive"] }
//! anyhow = "1.0"
//! which = "7.0"
//! ```
//!
//! Generic tool for building web projects and copying build output to Flutter assets directory.
//! Supports pnpm, npm, yarn, and bun package managers.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "web-build")]
#[command(about = "Build web projects and copy output to Flutter assets directory")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build web project
    Build {
        /// Path to web project directory
        #[arg(short, long)]
        src: PathBuf,
        
        /// Package manager to use (pnpm, npm, yarn, bun)
        #[arg(short = 'm', long, default_value = "pnpm")]
        package_manager: String,
        
        /// Build command to run (default: "build")
        #[arg(short = 'c', long, default_value = "build")]
        build_command: String,
    },
    /// Copy built assets to Flutter assets directory
    Copy {
        /// Path to web project directory (looks for build/ or dist/ inside)
        #[arg(short, long)]
        src: PathBuf,
        
        /// Destination path for copied assets
        #[arg(short, long)]
        dst: PathBuf,
        
        /// Build output directory name (default: "build", alternative: "dist")
        #[arg(short = 'o', long, default_value = "build")]
        output_dir: String,
    },
    /// Build web project and copy assets (one-shot)
    Refresh {
        /// Path to web project directory
        #[arg(short, long)]
        src: PathBuf,
        
        /// Destination path for copied assets
        #[arg(short, long)]
        dst: PathBuf,
        
        /// Package manager to use (pnpm, npm, yarn, bun)
        #[arg(short = 'm', long, default_value = "pnpm")]
        package_manager: String,
        
        /// Build command to run (default: "build")
        #[arg(short = 'c', long, default_value = "build")]
        build_command: String,
        
        /// Build output directory name (default: "build", alternative: "dist")
        #[arg(short = 'o', long, default_value = "build")]
        output_dir: String,
    },
}

fn build_web_project(src_dir: &Path, package_manager: &str, build_command: &str) -> Result<()> {
    println!("Building web project at: {}", src_dir.display());
    
    if !src_dir.exists() {
        anyhow::bail!("Source directory does not exist: {}", src_dir.display());
    }

    // Find package manager in PATH using which crate
    let pm_path = which::which(package_manager)
        .with_context(|| format!("{} not found in PATH. Please install {} or ensure it's in your PATH.", package_manager, package_manager))?;

    println!("Using package manager: {} ({})", package_manager, pm_path.display());

    // Run install
    let status = Command::new(&pm_path)
        .arg("install")
        .current_dir(src_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to run {} install", package_manager))?;

    if !status.success() {
        anyhow::bail!("{} install failed", package_manager);
    }

    // Run build command
    let status = Command::new(&pm_path)
        .arg(build_command)
        .current_dir(src_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to run {} {}", package_manager, build_command))?;

    if !status.success() {
        anyhow::bail!("{} {} failed", package_manager, build_command);
    }

    println!("✓ Build completed successfully");
    Ok(())
}

fn copy_assets(src_dir: &Path, dst_dir: &Path, output_dir: &str) -> Result<()> {
    let build_dir = src_dir.join(output_dir);
    
    if !build_dir.exists() {
        anyhow::bail!("Build output not found: {}. Expected directory: {}", build_dir.display(), output_dir);
    }

    println!("Copying assets from {} to {}", build_dir.display(), dst_dir.display());

    // Remove destination if it exists
    if dst_dir.exists() {
        std::fs::remove_dir_all(dst_dir)
            .with_context(|| format!("Failed to remove destination: {}", dst_dir.display()))?;
    }

    // Create destination directory
    std::fs::create_dir_all(dst_dir)
        .with_context(|| format!("Failed to create destination: {}", dst_dir.display()))?;

    // Copy recursively
    copy_dir_all(&build_dir, dst_dir)
        .context("Failed to copy assets")?;

    println!("✓ Copied {} -> {}", build_dir.display(), dst_dir.display());
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { src, package_manager, build_command } => {
            build_web_project(&src, &package_manager, &build_command)?;
        }
        Commands::Copy { src, dst, output_dir } => {
            copy_assets(&src, &dst, &output_dir)?;
        }
        Commands::Refresh { src, dst, package_manager, build_command, output_dir } => {
            build_web_project(&src, &package_manager, &build_command)?;
            copy_assets(&src, &dst, &output_dir)?;
        }
    }

    Ok(())
}
