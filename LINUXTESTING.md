# Linux Testing Instructions

## Problem
The Linux CI for PR #1722 is failing on the "Run clippy no features" step with exit code 101. We need to reproduce and debug this issue on an actual Linux system.

## Current Status
- **macOS/Windows CI**: ✅ Passing
- **Linux CI**: ❌ Failing on `cargo clippy --all --no-default-features -- -D warnings`
- **Local macOS testing**: ✅ All commands pass locally

## Commands to Test

Please run these commands on Linux and report the output:

### 1. Basic Setup
```bash
git clone https://github.com/malpern/hot-reload.git
cd hot-reload
git checkout feat/hidapi-cross-platform-list
```

### 2. Environment Info
```bash
# Get system info
uname -a
rustc --version
cargo --version

# Check if required system deps are available
pkg-config --exists libudev || echo "libudev-dev missing"
pkg-config --exists libusb-1.0 || echo "libusb-1.0-dev missing"
```

### 3. The Failing Command
```bash
# This is the exact command that's failing in CI
RUSTFLAGS="-Dwarnings" cargo clippy --all --no-default-features -- -D warnings
```

### 4. Alternative Tests
If the above fails, try these for more details:

```bash
# Test individual steps
cargo check --all --no-default-features
cargo clippy --no-default-features -- -D warnings  # without --all flag
cargo clippy --all --no-default-features  # without -D warnings

# Test feature combinations
cargo clippy --features hidapi_list -- -D warnings
cargo clippy --all --features hidapi_list -- -D warnings
```

### 5. Dependency Check
```bash
# Check if hidapi tries to compile when it shouldn't
cargo tree --no-default-features | grep hidapi || echo "hidapi not in tree (good)"
cargo tree --features hidapi_list | grep hidapi || echo "hidapi missing with feature (bad)"
```

## Expected Results

- **With `--no-default-features`**: hidapi should NOT compile, no --list flag available
- **With `--features hidapi_list`**: hidapi should compile, --list flag available

## What to Report

Please provide:
1. **System info** from step 2
2. **Full error output** from the failing clippy command
3. **Results** from alternative tests
4. **Any warnings or errors** you see

## Fix Strategy

Based on the Linux test results, we can:
- Add missing system dependencies to CI
- Fix conditional compilation issues
- Add Linux-specific feature gating
- Resolve workspace dependency conflicts

## Background

This PR adds cross-platform `--list` support using hidapi-rs. The feature is behind a `hidapi_list` flag that was removed from default features to prevent Linux CI failures due to missing system dependencies (libudev-dev, libusb-dev).

The issue appears to be Linux-specific since macOS and Windows CI pass successfully.