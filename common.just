# Flutter Common Recipes
# Shared recipes for Flutter projects using the flutter_tools submodule.
# Import this file from your project's justfile:
#   import 'flutter_tools/common.just'
#
# Override any recipe by redefining it in your project's justfile.

# Configurable workflow name for CI helpers (override in project justfile)
ci_workflow := "release.yaml"

# Tool prefix for external tool managers (e.g., "pixi run")
_tool_prefix := ""
# Python script runner (e.g., "uv run --script" or "pixi run python")
_py := "uv run --script"

# =============================================
# Setup & Dependencies
# =============================================

# Get dependencies
get:
    flutter pub get

# Upgrade packages
upgrade *ARGS:
    flutter pub upgrade {{ARGS}}

# Clean build artifacts
clean:
    flutter clean

# Check Flutter development environment
doctor *ARGS:
    flutter doctor {{ARGS}}

# Fix code issues automatically
fix *ARGS:
    dart fix --apply {{ARGS}}

# =============================================
# Development
# =============================================

# Run on Android device (auto-detects first Android device)
# Usage: just android [debug|release] [extra-flags...]
android MODE='debug' *FLAGS:
    DEVICE_ID=$({{_tool_prefix}} rust-script flutter_tools/flutter_select_device.rs --platform android); if [ -z "$DEVICE_ID" ]; then echo "No Android device found. Run: just devices" 1>&2; exit 1; fi; VERBOSE=""; if [ "{{MODE}}" = "debug" ]; then VERBOSE="--verbose"; fi; {{_tool_prefix}} rust-script flutter_tools/cmd_run.rs --log=logs/flutter-android-{{MODE}}.log flutter run -d "$DEVICE_ID" --{{MODE}} $VERBOSE {{FLAGS}}

# Run on Windows
# Usage: just windows [debug|release] [extra-flags...]
windows MODE='debug' *FLAGS:
    VERBOSE=""; if [ "{{MODE}}" = "debug" ]; then VERBOSE="--verbose"; fi; {{_tool_prefix}} rust-script flutter_tools/cmd_run.rs --log=logs/flutter-windows-{{MODE}}.log flutter run -d windows --{{MODE}} $VERBOSE {{FLAGS}}

# Run on iOS device
# Usage: just ios [debug|release] [extra-flags...]
ios MODE='debug' *FLAGS:
    DEVICE_ID=$({{_tool_prefix}} rust-script flutter_tools/flutter_select_device.rs --platform ios); if [ -z "$DEVICE_ID" ]; then echo "No iOS device found. Run: just devices" 1>&2; exit 1; fi; VERBOSE=""; if [ "{{MODE}}" = "debug" ]; then VERBOSE="--verbose"; fi; {{_tool_prefix}} rust-script flutter_tools/cmd_run.rs --log=logs/flutter-ios-{{MODE}}.log flutter run -d "$DEVICE_ID" --{{MODE}} $VERBOSE {{FLAGS}}

# List all connected devices
devices *ARGS:
    flutter devices {{ARGS}}

# Kill processes locking Android directory
kill-android:
    {{_tool_prefix}} rust-script flutter_tools/kill_file_handles.rs android

# Install and run the built release APK on Android (skips build)
install-apk:
    DEVICE_ID=$({{_tool_prefix}} rust-script flutter_tools/flutter_select_device.rs --platform android); if [ -z "$DEVICE_ID" ]; then echo "No Android device found." 1>&2; exit 1; fi; flutter run -d "$DEVICE_ID" --release --use-application-binary=build/app/outputs/flutter-apk/app-release.apk

# =============================================
# Code Generation & Assets
# =============================================

# Generate platform configuration from app.pkl
gen-platforms *ARGS:
    {{_tool_prefix}} cargo run --manifest-path flutter_tools/flutter_gen_platforms/Cargo.toml -- {{ARGS}}

# Generate all image assets (branding first, then logos with launcher icons/splash)
gen-assets:
    {{_py}} flutter_tools/flutter_gen_branding.py
    {{_py}} flutter_tools/flutter_gen_logo.py

# Generate logo variants + apply to launcher icons/splash
gen-logo *ARGS:
    {{_py}} flutter_tools/flutter_gen_logo.py {{ARGS}}

# Generate logo variants only (without applying to launcher icons)
logo:
    {{_py}} flutter_tools/flutter_gen_logo.py --no-apply

# Generate branding images (light and dark theme)
gen-branding *ARGS:
    {{_py}} flutter_tools/flutter_gen_branding.py {{ARGS}}

# Generate Android release keystore (reads KEYSTORE_PASSWORD and KEY_PASSWORD env vars)
gen-android-keystore *FLAGS:
    {{_tool_prefix}} rust-script flutter_tools/gen_android_keystore.rs {{FLAGS}}

# Upload signing secrets (keystore + passwords) to GitHub Actions
upload-secrets:
    @KS="platforms/android/keystore.jks"; \
    if [ ! -f "$KS" ]; then echo "Error: $KS not found. Run: just gen-android-keystore" >&2; exit 1; fi; \
    if [ -z "$KEYSTORE_PASSWORD" ]; then echo "Error: KEYSTORE_PASSWORD env var is not set" >&2; exit 1; fi; \
    if [ -z "$KEY_PASSWORD" ]; then echo "Error: KEY_PASSWORD env var is not set" >&2; exit 1; fi; \
    echo "Uploading KEYSTORE_BASE64..."; \
    base64 -w 0 "$KS" | {{_tool_prefix}} gh secret set KEYSTORE_BASE64; \
    echo "Uploading KEYSTORE_PASSWORD..."; \
    echo "$KEYSTORE_PASSWORD" | {{_tool_prefix}} gh secret set KEYSTORE_PASSWORD; \
    echo "Uploading KEY_PASSWORD..."; \
    echo "$KEY_PASSWORD" | {{_tool_prefix}} gh secret set KEY_PASSWORD; \
    echo "Done."

# Run Dart build_runner code generation
generate *ARGS:
    dart run build_runner build --delete-conflicting-outputs {{ARGS}}

# Watch files and auto-generate code with build_runner
watch:
    dart run build_runner watch --delete-conflicting-outputs

# =============================================
# Code Quality
# =============================================

# Run Dart static analysis
analyze *ARGS:
    flutter analyze {{ARGS}}

# Run all static checks (override in project justfile to add frontend, etc.)
lint:
    just format-check
    just analyze

# Format Dart code
format *ARGS:
    dart format {{ARGS}} lib test

# Check that lib/ and test/ are formatted (CI gate)
format-check:
    dart format --set-exit-if-changed lib test

# =============================================
# Testing
# =============================================

# Run Flutter tests
test *ARGS:
    flutter test {{ARGS}}

# Run tests with coverage report
test-coverage *ARGS:
    flutter test --coverage {{ARGS}}
    @echo "Coverage report generated at: coverage/lcov.info"

# =============================================
# Build
# =============================================

# Build APK (release with obfuscation)
# Usage: just build-apk [--verbose]
build-apk *FLAGS:
    {{_tool_prefix}} rust-script flutter_tools/cmd_run.rs --log=logs/flutter-build.log flutter build apk --release --obfuscate --split-debug-info=build/app/outputs/symbols {{FLAGS}}

# Build iOS app (release)
build-ios *FLAGS:
    flutter build ios --release {{FLAGS}}

# =============================================
# Version & Release
# =============================================

# Bump version in pubspec.yaml (major|minor|patch|build|revert)
bump-version PART *FLAGS:
    {{_tool_prefix}} rust-script flutter_tools/bump_version.rs {{PART}} {{FLAGS}}

# Tag the current version in git (e.g. v1.2.3)
tag-version *FLAGS:
    {{_tool_prefix}} rust-script flutter_tools/git_tag_version.rs {{FLAGS}}

# Generate changelog from git log using Claude AI
gen-changelog *FLAGS:
    {{_tool_prefix}} rust-script flutter_tools/gen_changelog.rs {{FLAGS}}

# Bump version, commit, tag, and push to trigger CI release
release PART='patch':
    just format
    just bump-version {{PART}}
    git add pubspec.yaml && git commit -m "bump version to $(grep '^version:' pubspec.yaml | awk '{print $2}')"
    just tag-version
    git push && git push --tags

# =============================================
# GitHub Utilities
# =============================================

# Check GitHub API rate limit status
gh-rate-limit *FLAGS:
    {{_tool_prefix}} rust-script flutter_tools/gh_rate_limit.rs {{FLAGS}}

# =============================================
# CI Helpers
# =============================================

# Trigger CI workflow on a branch
trigger-ci REF='main':
    {{_tool_prefix}} gh workflow run {{ci_workflow}} --ref {{REF}}

# Re-run the last failed CI workflow on GitHub
rerun-ci:
    {{_tool_prefix}} gh run list --workflow={{ci_workflow}} --limit 1 --json databaseId --jq '.[0].databaseId' | xargs {{_tool_prefix}} gh run rerun --failed

# Watch the latest CI run (streaming logs)
watch-ci:
    {{_tool_prefix}} gh run watch "$({{_tool_prefix}} gh run list --workflow={{ci_workflow}} --limit 1 --json databaseId --jq '.[0].databaseId')" --exit-status

# View logs of the latest CI run (--log-failed for failures only)
logs-ci *ARGS:
    {{_tool_prefix}} gh run view "$({{_tool_prefix}} gh run list --workflow={{ci_workflow}} --limit 1 --json databaseId --jq '.[0].databaseId')" --log {{ARGS}}
