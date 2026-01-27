#!/usr/bin/env rust-script
//! Flutter Device Selector
//!
//! Selects a Flutter device by index or ID.
//!
//! Usage:
//!   rust-script flutter_select_device.rs [device_spec]
//!
//! Arguments:
//!   device_spec    Device ID (e.g., "emulator-5554") or index number (0-based, e.g., "0" for first device, "1" for second)
//!                  If empty, returns empty (auto-select)
//!
//! Examples:
//!   rust-script flutter_select_device.rs 0           # Select first available device
//!   rust-script flutter_select_device.rs 1           # Select second available device
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
    let device_spec = args.get(0).map(|s| s.as_str()).unwrap_or("");

    // Empty means auto-select
    if device_spec.is_empty() {
        return;
    }

    // If it's a pure number, treat as index (0-based: 0 = first device, 1 = second device)
    if let Ok(index) = device_spec.parse::<usize>() {

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
                    Ok(devices) => {
                        if index < devices.len() {
                            let device = &devices[index];
                            print!("{}", device.id);
                        } else {
                            eprintln!("Error: Device index {} out of range (found {} devices)", index, devices.len());
                            std::process::exit(1);
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
    } else {
        // Not a number, treat as device ID
        print!("{}", device_spec);
    }
}
