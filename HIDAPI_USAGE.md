# hidapi-rs Implementation Usage Guide

## Overview

This branch implements a unified cross-platform keyboard enumeration system using hidapi-rs, providing richer device metadata and consistent API across all platforms.

## Features

### ✅ **Implemented**

- **Cross-platform keyboard enumeration** using hidapi-rs
- **Rich device metadata** including:
  - Vendor ID (VID) and Product ID (PID)
  - Manufacturer and product strings
  - Serial numbers
  - Interface numbers
  - Device paths
- **JSON output support** for machine-readable data
- **Platform-specific feature gating** (Windows requires `interception_driver`)
- **Comprehensive error handling** with troubleshooting guidance
- **Unit tests** for core functionality

### 🔄 **Advantages Over Current Approach**

| Feature | Current Implementation | hidapi-rs Implementation |
|---------|----------------------|--------------------------|
| **API Consistency** | 3 different platform-specific APIs | Single unified hidapi API |
| **Device Metadata** | Basic device names/paths | VID, PID, manufacturer, product, serial |
| **Output Formats** | Human-readable only | Human-readable + JSON |
| **Maintenance** | 3 separate codepaths | 1 unified codebase |
| **Future Extensions** | Platform-specific changes needed | Easy to add features across all platforms |

## Usage Examples

### Basic Device Enumeration

```bash
# Human-readable output (default)
kanata --list

# JSON output for scripting 
kanata --list --json

# JSON with filtering (using jq)
kanata --list --json | jq '.[] | select(.manufacturer_string == "Apple Inc.")'
```

### Sample Output

#### Human-Readable Format
```
Available keyboard devices:
===========================
Found 1 keyboard device(s):

  1. Apple Inc. Apple Internal Keyboard / Trackpad
     VID: 0x05AC, PID: 0x0281
     Manufacturer: Apple Inc.
     Serial: F0T2197Z4A3N4J6BS+TEG
     Interface: -1
     Path: DevSrvsID:4294969549

Configuration example:
  (defcfg
    macos-dev-names-include (
      "Apple Inc. Apple Internal Keyboard / Trackpad"
    )
  )

  Alternative using VID/PID (for future device-specific targeting):
  ; VID:PID format for reference
  ; Apple Inc. Apple Internal Keyboard / Trackpad -> 05AC:0281
```

#### JSON Format
```json
[
  {
    "vendor_id": 1452,
    "product_id": 641,
    "manufacturer_string": "Apple Inc.",
    "product_string": "Apple Internal Keyboard / Trackpad",
    "serial_number": "F0T2197Z4A3N4J6BS+TEG",
    "interface_number": -1,
    "usage_page": 1,
    "usage": 6,
    "path": "DevSrvsID:4294969549"
  }
]
```

## Technical Implementation

### Core Components

1. **KeyboardDevice Struct**: Unified device representation with rich metadata
2. **hidapi Integration**: Uses hidapi-rs v2.6.3 with macOS shared device support
3. **Keyboard Filtering**: Identifies keyboards using HID Usage Page 1, Usage 6
4. **Error Handling**: Comprehensive error handling with platform-specific troubleshooting
5. **Output Formatting**: Both human-readable and JSON formats

### Dependencies Added

```toml
serde = { version = "1.0", features = ["derive"] }
hidapi = { version = "2.6", features = ["macos-shared-device"] }
```

### Platform Support

- **macOS**: ✅ Tested and working (requires Input Monitoring permission)
- **Linux**: ✅ Implemented with permission troubleshooting
- **Windows**: ✅ Feature-gated to `interception_driver` builds only

## Troubleshooting

### macOS
- Grant 'Input Monitoring' permission in System Preferences
- Restart application after granting permission

### Linux  
- Add user to input group: `sudo usermod -a -G input $USER`
- Log out and back in for group changes to take effect
- Try `sudo kanata --list` to test permissions

### Windows
- Ensure `interception_driver` feature flag is enabled during build
- Run as administrator if needed
- Verify interception driver is installed

## Testing

```bash
# Run unit tests
cargo test hidapi_device_list

# Test CLI functionality
cargo build --release
./target/release/kanata --list
./target/release/kanata --list --json
```

## Future Enhancements

This implementation provides the foundation for:

1. **Device-Specific Configuration**: Target keyboards by VID/PID
2. **Real-time Device Monitoring**: Watch for connect/disconnect events  
3. **Advanced Device Filtering**: Filter by manufacturer, serial number, etc.
4. **Device Capabilities Detection**: Identify device features

## Comparison with Dave's Approach

Dave's feedback highlighted several advantages that this implementation provides:

| Dave's Request | Implementation Status |
|----------------|----------------------|
| Manufacturer and product strings | ✅ Implemented |
| Serial numbers | ✅ Implemented |
| VID/PID extraction | ✅ Implemented |
| JSON output | ✅ Implemented |
| Simpler API | ✅ Unified hidapi interface |
| Cross-platform consistency | ✅ Same code for all platforms |

This implementation directly addresses all of Dave's technical feedback while maintaining the benefits of our original cross-platform approach.