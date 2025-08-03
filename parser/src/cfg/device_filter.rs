/// Device filtering functionality for kanata configuration
///
/// This module provides data structures and logic for filtering devices
/// based on VID/PID identifiers and device names across platforms.
use super::error::{ParseError, Result};

/// Represents different ways to identify a device
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DeviceIdentifier {
    /// Identify by Vendor ID and Product ID (decimal format)
    VidPid(u16, u16),
    /// Identify by device name (case-sensitive substring matching)
    Name(String),
}

impl DeviceIdentifier {
    /// Parse a device identifier string
    ///
    /// If the string contains ':' and matches VID:PID pattern (numbers:numbers),
    /// it's parsed as VID/PID. Otherwise, it's treated as a device name.
    pub fn parse(s: &str) -> Result<Self> {
        if s.contains(':') && Self::is_vid_pid_format(s) {
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() != 2 {
                return Err(ParseError::new_without_span(
                    format!("Invalid VID:PID format '{s}'. Expected 'VENDOR_ID:PRODUCT_ID' (e.g., '1452:641')")
                ));
            }

            let vendor_id = parts[0].parse::<u16>().map_err(|_| {
                ParseError::new_without_span(format!(
                    "Invalid vendor ID '{}', must be a number 0-65535",
                    parts[0]
                ))
            })?;

            let product_id = parts[1].parse::<u16>().map_err(|_| {
                ParseError::new_without_span(format!(
                    "Invalid product ID '{}', must be a number 0-65535",
                    parts[1]
                ))
            })?;

            Ok(DeviceIdentifier::VidPid(vendor_id, product_id))
        } else {
            Ok(DeviceIdentifier::Name(s.to_string()))
        }
    }

    /// Check if a string matches VID:PID format (numbers:numbers)
    fn is_vid_pid_format(s: &str) -> bool {
        let parts: Vec<&str> = s.split(':').collect();
        parts.len() == 2
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    }
}

/// Enhanced device information containing both identification data and platform details
#[derive(Clone, Debug)]
pub struct EnhancedDeviceInfo {
    /// Human-readable device name
    pub name: String,
    /// Vendor ID (VID)
    pub vid: u16,
    /// Product ID (PID)  
    pub pid: u16,
    /// Platform-specific device data
    pub platform_data: PlatformDeviceData,
}

/// Platform-specific device data
#[derive(Clone, Debug)]
pub enum PlatformDeviceData {
    #[cfg(any(target_os = "linux", target_os = "unknown", test))]
    Linux { path: String },
    #[cfg(any(target_os = "macos", target_os = "unknown", test))]
    MacOS { karabiner_name: String },
    #[cfg(any(target_os = "windows", target_os = "unknown", test))]
    Windows {
        hardware_id: Option<String>,
        raw_wide_bytes: Vec<u8>,
    },
}

/// Device filtering configuration with include/exclude logic
#[derive(Clone, Debug, Default)]
pub struct DeviceFilter {
    /// Devices to include (if empty, include all devices)
    pub include_identifiers: Vec<DeviceIdentifier>,
    /// Devices to exclude (takes precedence over include)
    pub exclude_identifiers: Vec<DeviceIdentifier>,
}

impl DeviceFilter {
    /// Create a new empty device filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a device filter with include identifiers
    pub fn with_include(identifiers: Vec<DeviceIdentifier>) -> Self {
        Self {
            include_identifiers: identifiers,
            exclude_identifiers: vec![],
        }
    }

    /// Create a device filter with exclude identifiers
    pub fn with_exclude(identifiers: Vec<DeviceIdentifier>) -> Self {
        Self {
            include_identifiers: vec![],
            exclude_identifiers: identifiers,
        }
    }

    /// Determine if a device should be included based on filter rules
    ///
    /// Logic:
    /// 1. If include list exists and is not empty, device must match at least one include identifier
    /// 2. If exclude list is not empty, device must not match any exclude identifier  
    /// 3. Include takes precedence - if no include list, all devices are included by default
    /// 4. Exclude takes precedence over include - if device matches exclude, it's rejected
    pub fn should_include_device(&self, device: &EnhancedDeviceInfo) -> bool {
        // First check include filter
        let include_match = if self.include_identifiers.is_empty() {
            true // No include filter means include all devices
        } else {
            self.include_identifiers
                .iter()
                .any(|id| self.matches_device(id, device))
        };

        // Then check exclude filter
        let exclude_match = self
            .exclude_identifiers
            .iter()
            .any(|id| self.matches_device(id, device));

        // Device is included if it matches include criteria AND doesn't match exclude criteria
        include_match && !exclude_match
    }

    /// Check if a device identifier matches a device
    fn matches_device(&self, identifier: &DeviceIdentifier, device: &EnhancedDeviceInfo) -> bool {
        match identifier {
            DeviceIdentifier::VidPid(vid, pid) => device.vid == *vid && device.pid == *pid,
            DeviceIdentifier::Name(name) => {
                // Case-sensitive substring matching in either direction
                device.name.contains(name) || name.contains(&device.name)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_identifier_parse_vid_pid() {
        let id = DeviceIdentifier::parse("1452:641").unwrap();
        assert_eq!(id, DeviceIdentifier::VidPid(1452, 641));
    }

    #[test]
    fn test_device_identifier_parse_name() {
        let id = DeviceIdentifier::parse("Logitech MX Keys").unwrap();
        assert_eq!(id, DeviceIdentifier::Name("Logitech MX Keys".to_string()));
    }

    #[test]
    fn test_device_identifier_parse_invalid_vid_pid() {
        // These should be parsed as device names, not fail as VID/PID
        // because they don't match the VID:PID format (numbers:numbers)
        let id1 = DeviceIdentifier::parse("abc:123").unwrap();
        assert_eq!(id1, DeviceIdentifier::Name("abc:123".to_string()));

        let id2 = DeviceIdentifier::parse("123:abc").unwrap();
        assert_eq!(id2, DeviceIdentifier::Name("123:abc".to_string()));

        let id3 = DeviceIdentifier::parse("123:").unwrap();
        assert_eq!(id3, DeviceIdentifier::Name("123:".to_string()));

        let id4 = DeviceIdentifier::parse(":123").unwrap();
        assert_eq!(id4, DeviceIdentifier::Name(":123".to_string()));
    }

    #[test]
    fn test_is_vid_pid_format() {
        assert!(DeviceIdentifier::is_vid_pid_format("1452:641"));
        assert!(DeviceIdentifier::is_vid_pid_format("0:0"));
        assert!(DeviceIdentifier::is_vid_pid_format("65535:65535"));

        assert!(!DeviceIdentifier::is_vid_pid_format("abc:123"));
        assert!(!DeviceIdentifier::is_vid_pid_format("123:abc"));
        assert!(!DeviceIdentifier::is_vid_pid_format("Apple Keyboard"));
        assert!(!DeviceIdentifier::is_vid_pid_format("123:"));
        assert!(!DeviceIdentifier::is_vid_pid_format(":123"));
        assert!(!DeviceIdentifier::is_vid_pid_format(""));
    }

    #[test]
    fn test_device_filter_include_only() {
        let filter = DeviceFilter::with_include(vec![
            DeviceIdentifier::VidPid(1452, 641),
            DeviceIdentifier::Name("Logitech".to_string()),
        ]);

        let apple_device = EnhancedDeviceInfo {
            name: "Apple Internal Keyboard".to_string(),
            vid: 1452,
            pid: 641,
            platform_data: PlatformDeviceData::Linux {
                path: "/dev/input/event0".to_string(),
            },
        };

        let logitech_device = EnhancedDeviceInfo {
            name: "Logitech MX Keys".to_string(),
            vid: 1133,
            pid: 49970,
            platform_data: PlatformDeviceData::Linux {
                path: "/dev/input/event1".to_string(),
            },
        };

        let other_device = EnhancedDeviceInfo {
            name: "Microsoft Natural".to_string(),
            vid: 1234,
            pid: 5678,
            platform_data: PlatformDeviceData::Linux {
                path: "/dev/input/event2".to_string(),
            },
        };

        assert!(filter.should_include_device(&apple_device));
        assert!(filter.should_include_device(&logitech_device));
        assert!(!filter.should_include_device(&other_device));
    }

    #[test]
    fn test_device_filter_exclude_only() {
        let filter = DeviceFilter::with_exclude(vec![DeviceIdentifier::VidPid(1452, 641)]);

        let apple_device = EnhancedDeviceInfo {
            name: "Apple Internal Keyboard".to_string(),
            vid: 1452,
            pid: 641,
            platform_data: PlatformDeviceData::Linux {
                path: "/dev/input/event0".to_string(),
            },
        };

        let other_device = EnhancedDeviceInfo {
            name: "Logitech MX Keys".to_string(),
            vid: 1133,
            pid: 49970,
            platform_data: PlatformDeviceData::Linux {
                path: "/dev/input/event1".to_string(),
            },
        };

        assert!(!filter.should_include_device(&apple_device));
        assert!(filter.should_include_device(&other_device));
    }

    #[test]
    fn test_device_filter_include_and_exclude() {
        let filter = DeviceFilter {
            include_identifiers: vec![DeviceIdentifier::Name("Apple".to_string())],
            exclude_identifiers: vec![DeviceIdentifier::VidPid(1452, 641)],
        };

        let apple_device_excluded = EnhancedDeviceInfo {
            name: "Apple Internal Keyboard".to_string(),
            vid: 1452,
            pid: 641,
            platform_data: PlatformDeviceData::Linux {
                path: "/dev/input/event0".to_string(),
            },
        };

        let apple_device_included = EnhancedDeviceInfo {
            name: "Apple Magic Keyboard".to_string(),
            vid: 1452,
            pid: 999,
            platform_data: PlatformDeviceData::Linux {
                path: "/dev/input/event1".to_string(),
            },
        };

        let non_apple_device = EnhancedDeviceInfo {
            name: "Logitech MX Keys".to_string(),
            vid: 1133,
            pid: 49970,
            platform_data: PlatformDeviceData::Linux {
                path: "/dev/input/event2".to_string(),
            },
        };

        // Excluded because it matches exclude filter (VID/PID)
        assert!(!filter.should_include_device(&apple_device_excluded));

        // Included because it matches include filter (name) and doesn't match exclude filter
        assert!(filter.should_include_device(&apple_device_included));

        // Excluded because it doesn't match include filter
        assert!(!filter.should_include_device(&non_apple_device));
    }
}
