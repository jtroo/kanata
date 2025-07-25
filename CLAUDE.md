# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Kanata - Advanced Keyboard Remapper

## Project Overview

Kanata is a cross-platform software keyboard remapper for Linux, macOS, and Windows that provides QMK-like features for any keyboard. It enables multi-layer key functionality, advanced key behaviors (tap-hold, macros, unicode), and complex keyboard customizations through a human-readable configuration file.

**Key Features:**
- Multi-layer keyboard functionality
- Advanced actions: tap-hold, unicode output, dynamic/static macros
- Vim-like leader sequences
- Live configuration reloading
- TCP server for external program integration
- Cross-platform support (Linux, macOS, Windows)

## Architecture

### High-Level Structure

The project is organized as a Rust workspace with multiple crates:

```
kanata/                    # Main binary crate
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # State machine library
│   ├── kanata/           # Core keyboard processing logic
│   ├── oskbd/            # OS-specific keyboard interfaces
│   ├── gui/              # Windows GUI components
│   └── tests/            # Integration tests with simulation
├── parser/               # Configuration parsing crate
├── keyberon/             # Keyboard firmware library (fork)
├── tcp_protocol/         # TCP server protocol definitions
├── example_tcp_client/   # Example TCP client
├── windows_key_tester/   # Windows key testing utility
├── simulated_input/      # Input simulation for testing
├── simulated_passthru/   # Passthrough simulation
└── cfg_samples/          # Example configuration files
```

### Core Components

1. **Event Loop**: Reads key events from OS and sends to processing loop
2. **Processing Loop**: Handles events, manages state machine, outputs key events
3. **Layout Engine**: Uses keyberon library for key mapping and layer management
4. **TCP Server**: Optional server for external program communication
5. **OS Abstraction**: Platform-specific keyboard input/output handling

### Configuration Language

Kanata uses a Lisp-like S-expression syntax for configuration:
- `defsrc`: Defines intercepted keys
- `deflayer`: Defines key mappings for each layer
- `defalias`: Defines reusable key combinations
- `defcfg`: Global configuration options

## Development Commands

### Building

```bash
# Standard debug build
cargo build

# Release build
cargo build --release

# Build with specific features
cargo build --features cmd                    # Enable cmd actions
cargo build --features interception_driver   # Windows Interception driver
cargo build --features gui                    # Windows GUI
cargo build --features simulated_output       # Testing with simulation
```

### Testing

```bash
# Run all tests using justfile
just test

# Manual test commands
cargo test -p kanata -p kanata-parser -p kanata-keyberon -- --nocapture
cargo test --features=simulated_output sim_tests
cargo clippy --all
```

### Formatting and Linting

```bash
# Format code
just fmt
# OR
cargo fmt --all

# Run clippy
cargo clippy --all
cargo clippy --all -- -D warnings  # Treat warnings as errors
```

### Platform-Specific Builds

```bash
# Linux release builds
just build_release_linux output_dir

# Windows release builds (multiple variants)
just build_release_windows output_dir

# Generate SHA256 checksums
just sha256sums output_dir
```

### Development Helpers

```bash
# GUI development
just guic    # Check GUI build
just guif    # Format and fix GUI clippy issues

# AHK passthrough development  
just ahkc    # Check AHK build
just ahkf    # Format and fix AHK clippy issues

# Coverage reporting
just cov     # Generate code coverage report

# Documentation
just cfg_to_html output_dir    # Generate HTML config docs
just wasm_pack output_dir      # Build WASM version

# Run single test file
cargo test --features=simulated_output sim_tests::<test_name>

# Watch for changes during development
cargo watch -x 'test --features=simulated_output'
```

## Running Kanata

### Basic Usage

```bash
# Run with configuration file
kanata --cfg path/to/config.kbd

# With TCP server
kanata --cfg config.kbd --port 1337

# Multiple config files
kanata --cfg config1.kbd config2.kbd
```

### Platform-Specific Notes

**Linux:**
```bash
sudo ./target/release/kanata --cfg config.kbd
# See docs for avoiding sudo: https://github.com/jtroo/kanata/wiki/Avoid-using-sudo-on-Linux
```

**Windows:**
```bash
.\target\release\kanata.exe --cfg config.kbd
```

**macOS:**
```bash
# Requires Karabiner driver installation first
sudo ./target/release/kanata --cfg config.kbd
```

## Feature Flags

Important feature flags that change functionality:

- `cmd`: Enable command execution actions
- `tcp_server`: Enable TCP server (default)
- `win_sendinput_send_scancodes`: Windows SendInput API (default)
- `win_llhook_read_scancodes`: Windows low-level hook reading
- `interception_driver`: Windows Interception driver support
- `gui`: Windows GUI interface
- `simulated_output`: Testing simulation framework
- `simulated_input`: Input simulation for testing
- `passthru_ahk`: AutoHotkey passthrough mode
- `zippychord`: Zippy chord processing (default)

## Testing Conventions

### Simulation Tests

The project uses a comprehensive simulation testing framework:

1. **Location**: `src/tests/sim_tests/`
2. **Pattern**: Write config → simulate input → verify output
3. **Test Types**:
   - `tap_hold_tests.rs`: Tap-hold behavior
   - `layer_sim_tests.rs`: Layer switching
   - `macro_sim_tests.rs`: Macro execution
   - `chord_sim_tests.rs`: Chord processing
   - `timing_tests.rs`: Timing-sensitive behaviors

### Test Development Process

1. Write configuration string
2. Define simulated input sequence  
3. Run test to see actual output
4. Compare with expected behavior
5. Adjust test or fix implementation

## Configuration Examples

### Minimal Example
```lisp
(defsrc caps grv i j k l)
(deflayer default @cap @grv _ _ _ _)
(deflayer arrows _ _ up left down rght)
(defalias
  cap (tap-hold-press 200 200 caps lctl)
  grv (tap-hold-press 200 200 grv (layer-toggle arrows)))
```

### Common Patterns
- Tap-hold modifiers: Home row mods, space/shift combinations
- Layer switching: Number pad, arrow keys, function layers
- Macros: Text expansion, complex key sequences
- Chords: Multiple key press combinations

## Development Patterns

### Code Organization
- **OS-specific code**: Contained in `oskbd/` and `kanata/windows|linux|macos`
- **Core logic**: Platform-agnostic in main `kanata/` module
- **Parser**: Separate crate for configuration parsing
- **State machine**: Clean separation using keyberon library

### Error Handling
- Uses `anyhow` for error handling
- `miette` for user-friendly configuration error messages
- Comprehensive error reporting for configuration issues

### Logging
- Uses `log` crate with `simplelog`
- Configurable log levels
- Platform-specific logging considerations

## Contributing Guidelines

### Code Quality
- All code must pass `cargo clippy` with no warnings
- Format with `cargo fmt`
- Comprehensive test coverage required
- Platform-specific testing on CI

### Pull Request Process
1. Run `just test` locally
2. Ensure all feature combinations build
3. Test on target platforms when possible
4. Update documentation if needed

### License
- Main project: LGPL-3.0-only
- Keyberon directory: MIT License  
- Interception directory: MIT or Apache-2.0

## Resources

- **Main Documentation**: `/Volumes/FlashGordon/Dropbox/code/hot-reload/docs/config.adoc`
- **Example Configs**: `/Volumes/FlashGordon/Dropbox/code/hot-reload/cfg_samples/`
- **Design Document**: `/Volumes/FlashGordon/Dropbox/code/hot-reload/docs/design.md`
- **Online Simulator**: https://jtroo.github.io
- **Repository**: https://github.com/jtroo/kanata

## Important Development Notes

- **Windows GUI**: Use `rgui` command instead of `lgui` (as `lgui` has been removed)
- **Live Reload**: Configuration files are monitored and reloaded automatically  
- **Multi-platform**: Same configuration works across Linux, macOS, and Windows
- **Performance**: Optimized for low latency keyboard processing
- **Extensibility**: TCP server allows integration with external tools
- **Debugging**: Use `RUST_LOG=debug` environment variable for detailed logging
- **Testing**: Always run simulation tests when modifying key processing logic
- **Cross-compilation**: Use GitHub Actions for testing on all platforms