#!/usr/bin/env rust-script
//! Flutter Device Selector
//!
//! Selects a Flutter device by index, ID, or platform.
//!
//! Usage:
//!   rust-script flutter_select_device.rs [device_spec] [--platform PLATFORM]
//!
//! Arguments:
//!   device_spec    Device ID (e.g., "emulator-5554") or index number (0-based, e.g., "0" for first device, "1" for second)
//!                  If empty, returns empty (auto-select)
//!
//! Options:
//!   --platform     Filter by platform (e.g., "android", "windows", "linux", "macos", "ios", "web")
//!                  Can use partial match (e.g., "android" matches "android-x64", "android-arm")
//!
//! Examples:
//!   rust-script flutter_select_device.rs 0           # Select first available device
//!   rust-script flutter_select_device.rs 1           # Select second available device
//!   rust-script flutter_select_device.rs 0 --platform android  # Select first Android device
//!   rust-script flutter_select_device.rs --platform windows    # Select first Windows device (auto index 0)
//!   rust-script flutter_select_device.rs emulator-5554  # Select by device ID
//!
//! ```cargo
//! [dependencies]
//! serde = { version = "1.0", features = ["derive"] }
//! serde_json = "1.0"
//! ```

use std::env;
use std::process::Command;

#[derive(serde::Deserialize, Debug)]
#[allow(non_snake_case)]
struct Device {
    id: String,
    name: String,
    targetPlatform: String,
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    
    // Parse arguments
    let mut device_spec = "";
    let mut platform_filter: Option<String> = None;
    
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--platform" && i + 1 < args.len() {
            platform_filter = Some(args[i + 1].to_lowercase());
            i += 2;
        } else if device_spec.is_empty() {
            device_spec = &args[i];
            i += 1;
        } else {
            i += 1;
        }
    }

    // Empty device_spec means auto-select (index 0 when filtering)
    let auto_select = device_spec.is_empty();
    
    // Get device list
    let output = Command::new("flutter")
        .args(&["devices", "--machine"])
        .output();

    let output = if output.is_err() && cfg!(windows) {
        Command::new("flutter.bat")
            .args(&["devices", "--machine"])
            .output()
    } else {
        output
    };

    match output {
        Ok(output) if output.status.success() => {
            let json_str = String::from_utf8_lossy(&output.stdout);
            let start = json_str.find('[').unwrap_or(0);
            let end = json_str.rfind(']').map(|i| i + 1).unwrap_or(json_str.len());
            let clean_json = if start < end { &json_str[start..end] } else { "[]" };

            match serde_json::from_str::<Vec<Device>>(clean_json) {
                Ok(mut devices) => {
                    // Apply platform filter if specified
                    if let Some(platform) = &platform_filter {
                        devices.retain(|d| d.targetPlatform.to_lowercase().starts_with(platform));
                        
                        if devices.is_empty() {
                            eprintln!("Error: No devices found matching platform '{}'", platform);
                            std::process::exit(1);
                        }
                    }
                    
                    // If auto-select with platform filter, select first device
                    if auto_select && platform_filter.is_some() {
                        print!("{}", devices[0].id);
                        return;
                    }
                    
                    // If auto-select without platform filter, return empty (flutter auto-select)
                    if auto_select {
                        return;
                    }
                    
                    // If it's a pure number, treat as index
                    if let Ok(index) = device_spec.parse::<usize>() {
                        if index < devices.len() {
                            let device = &devices[index];
                            print!("{}", device.id);
                        } else {
                            eprintln!("Error: Device index {} out of range (found {} devices)", index, devices.len());
                            std::process::exit(1);
                        }
                    } else {
                        // Not a number, treat as device ID
                        print!("{}", device_spec);
                    }
                }
                Err(e) => {
                    eprintln!("Error parsing device list: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Ok(_) => {
            eprintln!("Error: flutter devices command failed");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error running flutter devices: {}", e);
            std::process::exit(1);
        }
    }
}
