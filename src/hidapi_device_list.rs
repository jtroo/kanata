use hidapi::{HidApi, HidError};
use serde::Serialize;

/// Represents a keyboard device with rich metadata
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
    pub path: String,
}

impl KeyboardDevice {
    /// Returns a human-readable display name for the device
    pub fn display_name(&self) -> String {
        match (&self.manufacturer_string, &self.product_string) {
            (Some(manufacturer), Some(product)) => format!("{manufacturer} {product}"),
            (None, Some(product)) => product.clone(),
            (Some(manufacturer), None) => format!("{manufacturer} Device"),
            (None, None) => format!(
                "HID Device ({:04X}:{:04X})",
                self.vendor_id, self.product_id
            ),
        }
    }

    /// Returns a VID:PID identifier string
    #[allow(dead_code)]
    pub fn vid_pid_string(&self) -> String {
        format!("{:04X}:{:04X}", self.vendor_id, self.product_id)
    }
}

/// Output format for device listing
#[derive(Debug, Clone)]
pub enum OutputFormat {
    Human,
    Json,
}

/// Determines if a HID device is a keyboard
fn is_keyboard(device_info: &hidapi::DeviceInfo) -> bool {
    // HID Usage Page 1 (Generic Desktop), Usage 6 (Keyboard)
    device_info.usage_page() == 0x01 && device_info.usage() == 0x06
}

/// Enumerates all connected keyboard devices using hidapi
pub fn enumerate_keyboards() -> Result<Vec<KeyboardDevice>, HidError> {
    let api = HidApi::new()?;
    let devices: Vec<KeyboardDevice> = api
        .device_list()
        .filter(|device| is_keyboard(device))
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

/// Formats device list in human-readable format
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
        println!(
            "     VID: 0x{:04X}, PID: 0x{:04X}",
            device.vendor_id, device.product_id
        );

        if let Some(manufacturer) = &device.manufacturer_string {
            println!("     Manufacturer: {manufacturer}");
        }

        if let Some(serial) = &device.serial_number {
            println!("     Serial: {serial}");
        }

        println!("     Interface: {}", device.interface_number);
        println!("     Path: {}", device.path);
        println!();
    }

    print_configuration_example(devices);
}

/// Formats device list in JSON format
pub fn format_json_output(devices: &[KeyboardDevice]) -> Result<(), serde_json::Error> {
    let output = serde_json::to_string_pretty(devices)?;
    println!("{output}");
    Ok(())
}

/// Prints platform-specific troubleshooting information
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
        println!("  4. Try running with sudo temporarily to test permissions");
    }

    #[cfg(target_os = "windows")]
    {
        println!("  1. Ensure devices are connected and working");
        println!("  2. Try running as administrator if needed");
        println!("  3. Verify interception driver is installed (if using interception features)");
    }
}

/// Prints platform-specific configuration examples
fn print_configuration_example(devices: &[KeyboardDevice]) {
    println!("Configuration example:");

    #[cfg(target_os = "macos")]
    {
        println!("  (defcfg");
        println!("    macos-dev-names-include (");
        for device in devices {
            println!("      \"{}\"", device.display_name());
        }
        println!("    )");
        println!("  )");

        println!("\n  Alternative using VID/PID (for future device-specific targeting):");
        println!("  ; VID:PID format for reference");
        for device in devices {
            println!(
                "  ; {} -> {}",
                device.display_name(),
                device.vid_pid_string()
            );
        }
    }

    #[cfg(target_os = "linux")]
    {
        println!("  (defcfg");
        println!("    linux-dev-names-include (");
        for device in devices {
            println!("      \"{}\"", device.display_name());
        }
        println!("    )");
        println!("  )");

        println!("\n  Alternative using VID/PID (for future device-specific targeting):");
        println!("  ; VID:PID format for reference");
        for device in devices {
            println!(
                "  ; {} -> {}",
                device.display_name(),
                device.vid_pid_string()
            );
        }
    }

    #[cfg(all(target_os = "windows", feature = "interception_driver"))]
    {
        println!("  (defcfg");
        println!("    windows-interception-keyboard-hwids (");
        for device in devices {
            println!(
                "      \"VID_{:04X}&PID_{:04X}\"  ; {}",
                device.vendor_id,
                device.product_id,
                device.display_name()
            );
        }
        println!("    )");
        println!("  )");
    }
}

/// Main entry point for keyboard listing with hidapi
pub fn list_keyboards_hidapi(format: OutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    match enumerate_keyboards() {
        Ok(devices) => match format {
            OutputFormat::Human => format_human_output(&devices),
            OutputFormat::Json => format_json_output(&devices)?,
        },
        Err(HidError::HidApiError { message }) => {
            eprintln!("Error: Unable to access HID devices.");
            eprintln!("Details: {message}");
            eprintln!("This may be due to insufficient permissions.");
            print_troubleshooting_info();
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error enumerating devices: {e:?}");
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_device_display_name() {
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

        assert_eq!(device.display_name(), "Logitech MX Keys");
        assert_eq!(device.vid_pid_string(), "046D:C52B");
    }

    #[test]
    fn test_keyboard_device_serialization() {
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
        assert!(json.contains("1133")); // 0x046D in decimal
        assert!(json.contains("50475")); // 0xC52B in decimal
    }

    #[test]
    fn test_device_display_name_fallbacks() {
        // Test with only manufacturer
        let device1 = KeyboardDevice {
            vendor_id: 0x1234,
            product_id: 0x5678,
            manufacturer_string: Some("Test Manufacturer".to_string()),
            product_string: None,
            serial_number: None,
            interface_number: 0,
            usage_page: 0x01,
            usage: 0x06,
            path: "/test".to_string(),
        };
        assert_eq!(device1.display_name(), "Test Manufacturer Device");

        // Test with only product
        let device2 = KeyboardDevice {
            vendor_id: 0x1234,
            product_id: 0x5678,
            manufacturer_string: None,
            product_string: Some("Test Product".to_string()),
            serial_number: None,
            interface_number: 0,
            usage_page: 0x01,
            usage: 0x06,
            path: "/test".to_string(),
        };
        assert_eq!(device2.display_name(), "Test Product");

        // Test with neither
        let device3 = KeyboardDevice {
            vendor_id: 0x1234,
            product_id: 0x5678,
            manufacturer_string: None,
            product_string: None,
            serial_number: None,
            interface_number: 0,
            usage_page: 0x01,
            usage: 0x06,
            path: "/test".to_string(),
        };
        assert_eq!(device3.display_name(), "HID Device (1234:5678)");
    }
}
