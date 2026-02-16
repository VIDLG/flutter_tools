use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::config::WindowsConfig;

/// Process Windows platform directory
pub fn process_windows_platform(project_dir: &Path, config: &WindowsConfig) -> Result<()> {
    let windows_dir = project_dir.join("windows");

    if !windows_dir.exists() {
        anyhow::bail!(
            "Windows directory not found. Run 'flutter create --platforms=windows .' first."
        );
    }

    // Update main.cpp with window size if configured
    if let (Some(width), Some(height)) = (config.window_width, config.window_height) {
        let main_cpp_path = windows_dir.join("runner").join("main.cpp");
        if main_cpp_path.exists() {
            let content = fs::read_to_string(&main_cpp_path).context("Failed to read main.cpp")?;

            // Replace the window size line
            let updated_content = content
                .lines()
                .map(|line| {
                    if line.contains("Win32Window::Size size(") {
                        format!(
                            "  Win32Window::Size size({}, {});  // Configured window size",
                            width, height
                        )
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");

            fs::write(&main_cpp_path, updated_content).context("Failed to write main.cpp")?;

            println!(
                "✓ Windows main.cpp updated with window size {}x{}",
                width, height
            );
        }
    }

    println!("✓ Windows platform directory configured");

    Ok(())
}
