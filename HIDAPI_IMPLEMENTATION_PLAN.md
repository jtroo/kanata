# hidapi-rs Cross-Platform --list Implementation Plan

## Overview

This branch implements a unified cross-platform keyboard enumeration system using hidapi-rs, providing richer device metadata and consistent API across all platforms.

## Key Advantages Over Current Approach

- **Cross-platform consistency**: Single API instead of three platform-specific implementations
- **Rich device metadata**: VID, PID, manufacturer, product, serial numbers
- **JSON output support**: Machine-readable format for scripting
- **Better device targeting foundation**: Natural path toward device-specific configurations
- **Simplified maintenance**: One implementation to maintain instead of three

## Implementation Plan

### Phase 1: Foundation Setup ✅

#### 1.1 Add hidapi-rs Dependency
```toml
[dependencies]
hidapi = { version = "2.6", features = ["macos-shared-device"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

#### 1.2 Design Unified Device Structure
```rust
#[derive(Debug, Clone, Serialize)]
pub struct KeyboardDevice {
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer_string: Option<String>,
    pub product_string: Option<String>,
    pub serial_number: Option<String>,
    pub interface_number: i32,
    pub usage_page: u16,
    pub usage: u16,
    pub path: String,  // Platform-specific device path
}
```

#### 1.3 Create Output Format Enum
```rust
#[derive(Debug, Clone)]
pub enum OutputFormat {
    Human,
    Json,
}
```

### Phase 2: Core Implementation

#### 2.1 Keyboard Detection Logic
```rust
fn is_keyboard(device_info: &hidapi::DeviceInfo) -> bool {
    // HID Usage Page 1 (Generic Desktop), Usage 6 (Keyboard)
    device_info.usage_page() == 0x01 && device_info.usage() == 0x06
}
```

#### 2.2 Device Enumeration Function
```rust
pub fn enumerate_keyboards() -> Result<Vec<KeyboardDevice>, HidError> {
    let api = HidApi::new()?;
    let devices: Vec<KeyboardDevice> = api
        .device_list()
        .filter(is_keyboard)
        .map(|device| KeyboardDevice {
            vendor_id: device.vendor_id(),
            product_id: device.product_id(),
            manufacturer_string: device.manufacturer_string().map(|s| s.to_string()),
            product_string: device.product_string().map(|s| s.to_string()),
            serial_number: device.serial_number().map(|s| s.to_string()),
            interface_number: device.interface_number(),
            usage_page: device.usage_page(),
            usage: device.usage(),
            path: device.path().to_string_lossy().to_string(),
        })
        .collect();
    
    Ok(devices)
}
```

### Phase 3: CLI Integration

#### 3.1 Update CLI Arguments
```rust
/// List the keyboards available for grabbing and exit.
#[cfg(any(
    target_os = "macos",
    target_os = "linux",
    all(target_os = "windows", feature = "interception_driver")
))]
#[arg(short, long)]
list: bool,

/// Output device list in JSON format
#[cfg(any(
    target_os = "macos", 
    target_os = "linux",
    all(target_os = "windows", feature = "interception_driver")
))]
#[arg(long)]
json: bool,
```

#### 3.2 CLI Processing Logic
```rust
#[cfg(any(
    target_os = "macos",
    target_os = "linux", 
    all(target_os = "windows", feature = "interception_driver")
))]
if args.list {
    let format = if args.json { OutputFormat::Json } else { OutputFormat::Human };
    main_lib::list_keyboards_hidapi(format)?;
    std::process::exit(0);
}
```

### Phase 4: Output Formatting

#### 4.1 Human-Readable Output
```rust
pub fn format_human_output(devices: &[KeyboardDevice]) {
    println!("Available keyboard devices:");
    println!("===========================");
    
    if devices.is_empty() {
        println!("No keyboard devices found.");
        print_troubleshooting_info();
        return;
    }
    
    println!("Found {} keyboard device(s):\n", devices.len());
    
    for (i, device) in devices.iter().enumerate() {
        println!("  {}. {}", i + 1, device.display_name());
        println!("     VID: 0x{:04X}, PID: 0x{:04X}", device.vendor_id, device.product_id);
        
        if let Some(manufacturer) = &device.manufacturer_string {
            println!("     Manufacturer: {}", manufacturer);
        }
        
        if let Some(serial) = &device.serial_number {
            println!("     Serial: {}", serial);
        }
        
        println!("     Path: {}", device.path);
        println!();
    }
    
    print_configuration_example(devices);
}
```

#### 4.2 JSON Output
```rust
pub fn format_json_output(devices: &[KeyboardDevice]) -> Result<(), serde_json::Error> {
    let output = serde_json::to_string_pretty(devices)?;
    println!("{}", output);
    Ok(())
}
```

### Phase 5: Configuration Examples

#### 5.1 Platform-Specific Config Generation
```rust
fn print_configuration_example(devices: &[KeyboardDevice]) {
    println!("Configuration example:");
    
    #[cfg(target_os = "macos")]
    {
        println!("  (defcfg");
        println!("    macos-dev-names-include (");
        for device in devices {
            if let Some(product) = &device.product_string {
                println!("      \"{}\"", product);
            }
        }
        println!("    )");
        println!("  )");
    }
    
    #[cfg(target_os = "linux")]
    {
        println!("  (defcfg");
        println!("    linux-dev-names-include (");
        for device in devices {
            if let Some(product) = &device.product_string {
                println!("      \"{}\"", product);
            }
        }
        println!("    )");
        println!("  )");
    }
    
    #[cfg(all(target_os = "windows", feature = "interception_driver"))]
    {
        println!("  (defcfg");
        println!("    windows-interception-keyboard-hwids (");
        for device in devices {
            println!("      \"VID_{:04X}&PID_{:04X}\"  ; {}", 
                device.vendor_id, 
                device.product_id,
                device.display_name()
            );
        }
        println!("    )");
        println!("  )");
    }
}
```

### Phase 6: Error Handling & Edge Cases

#### 6.1 Permission Handling
```rust
fn print_troubleshooting_info() {
    println!("\nTroubleshooting:");
    
    #[cfg(target_os = "macos")]
    {
        println!("  1. Grant 'Input Monitoring' permission in System Preferences");
        println!("     → System Preferences → Security & Privacy → Privacy → Input Monitoring");
        println!("  2. Ensure devices are connected and working");
        println!("  3. Try restarting the application");
    }
    
    #[cfg(target_os = "linux")]
    {
        println!("  1. Check permissions: sudo usermod -a -G input $USER");
        println!("  2. Log out and back in for group changes to take effect");
        println!("  3. Ensure devices are connected and working");
    }
    
    #[cfg(target_os = "windows")]
    {
        println!("  1. Ensure devices are connected and working");
        println!("  2. Try running as administrator if needed");
        println!("  3. Verify interception driver is installed (if using interception features)");
    }
}
```

#### 6.2 Device Disconnection Handling
```rust
pub fn list_keyboards_hidapi(format: OutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    match enumerate_keyboards() {
        Ok(devices) => {
            match format {
                OutputFormat::Human => format_human_output(&devices),
                OutputFormat::Json => format_json_output(&devices)?,
            }
        }
        Err(HidError::HidApiError) => {
            eprintln!("Error: Unable to access HID devices.");
            eprintln!("This may be due to insufficient permissions.");
            print_troubleshooting_info();
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error enumerating devices: {}", e);
            std::process::exit(1);
        }
    }
    
    Ok(())
}
```

### Phase 7: Testing Strategy

#### 7.1 Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keyboard_detection() {
        // Mock device info for testing
        // Test is_keyboard function with various device types
    }
    
    #[test]
    fn test_device_serialization() {
        let device = KeyboardDevice {
            vendor_id: 0x046D,
            product_id: 0xC52B,
            manufacturer_string: Some("Logitech".to_string()),
            product_string: Some("MX Keys".to_string()),
            serial_number: Some("123456".to_string()),
            interface_number: 0,
            usage_page: 0x01,
            usage: 0x06,
            path: "/dev/hidraw0".to_string(),
        };
        
        let json = serde_json::to_string(&device).unwrap();
        assert!(json.contains("Logitech"));
        assert!(json.contains("MX Keys"));
    }
}
```

#### 7.2 Integration Tests
- Test on macOS with various keyboard types
- Test on Linux with permission scenarios
- Test on Windows with/without interception driver
- Test JSON output parsing
- Test configuration example generation

### Phase 8: Documentation

#### 8.1 Update Help Text
```rust
/// List the keyboards available for grabbing and exit.
/// Shows device information including VID, PID, manufacturer, and product names.
/// Use --json for machine-readable output.
```

#### 8.2 Add Usage Examples
```markdown
# Device Enumeration Examples

## Basic Usage
```bash
kanata --list
```

## JSON Output
```bash
kanata --list --json | jq '.[] | select(.manufacturer_string == "Logitech")'
```

## Device-Specific Configuration
After running `--list`, use the VID/PID or device names in your configuration.
```

### Implementation Checklist

- [ ] Phase 1: Add dependencies and design structures
- [ ] Phase 2: Implement core enumeration logic
- [ ] Phase 3: Integrate with CLI interface
- [ ] Phase 4: Add output formatting (human + JSON)
- [ ] Phase 5: Generate platform-specific config examples
- [ ] Phase 6: Implement robust error handling
- [ ] Phase 7: Create comprehensive tests
- [ ] Phase 8: Update documentation

### Future Enhancements

1. **Device-Specific Targeting**: Extend configuration parser to support VID/PID-based device selection
2. **Real-time Device Monitoring**: Watch for device connect/disconnect events
3. **Device Capabilities Detection**: Identify device features (number of keys, etc.)
4. **Performance Optimization**: Cache device information where appropriate

### Migration Path

This implementation maintains backward compatibility with existing configurations while providing enhanced capabilities. Users can gradually adopt the new features without breaking existing setups.