# Flutter Tools

A collection of utility scripts and tools for Flutter development.

## Tools

### flutter_select_device.rs

Selects a Flutter device by index or ID for automated builds.

**Usage:**
```bash
rust-script flutter_select_device.rs [device_spec]
```

**Examples:**
```bash
# Auto-select (empty)
rust-script flutter_select_device.rs

# Select first device
rust-script flutter_select_device.rs 0

# Select by device ID
rust-script flutter_select_device.rs emulator-5554
```

### kill_file_handles.rs

Forcefully closes file handles on Windows to unlock files that are in use.

**Usage:**
```bash
rust-script kill_file_handles.rs [OPTIONS] <path>
```

**Examples:**
```bash
# Kill handles for build directory
rust-script kill_file_handles.rs build/

# Force kill handles
rust-script kill_file_handles.rs --force windows/flutter/ephemeral
```

### cmd_run.rs

Runs commands with optional logging and working directory control.

**Usage:**
```bash
rust-script cmd_run.rs [OPTIONS] <command> [args...]
```

**Examples:**
```bash
# Run with logging
rust-script cmd_run.rs --log=build.log flutter build apk

# Run in specific directory
rust-script cmd_run.rs --cwd=project cargo test
```

### flutter_gen_platforms

A Rust CLI tool for generating Flutter platform configurations.

**Build:**
```bash
cd flutter_gen_platforms
cargo build --release
```

**Usage:**
```bash
flutter_gen_platforms [OPTIONS]
```

### flutter_gen_logo.py

Generates Flutter app logos and icons.

**Usage:**
```bash
python flutter_gen_logo.py [OPTIONS]
```

## Requirements

- **Rust**: Install [Rust](https://rustup.rs/) and [rust-script](https://rust-script.org/) for `.rs` files
- **Python**: Python 3.x for `.py` files
- **Flutter**: Flutter SDK for device management features

## Development

The project uses rust-analyzer for IDE support. The `.vscode/rust-project.json` configures analysis for both standalone scripts and the Cargo project.

## License

MIT
