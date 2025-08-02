#[cfg(all(target_os = "windows", feature = "gui"))]
pub(crate) mod win_gui;

#[cfg(target_os = "macos")]
pub(crate) fn list_devices_macos() {
    use crate::oskbd::capture_stdout;
    use karabiner_driverkit::list_keyboards;

    println!("Available keyboard devices:");
    println!("===========================");

    let kb_list = capture_stdout(list_keyboards);
    let device_names: Vec<&str> = kb_list.lines().collect();

    if device_names.is_empty() {
        println!("No devices found. Ensure Karabiner-VirtualHIDDevice driver is activated.");
        return;
    }

    let mut valid_count = 0;
    let mut empty_count = 0;

    for (i, device) in device_names.iter().enumerate() {
        let trimmed = device.trim();
        if trimmed.is_empty() {
            println!("  {}. (empty name) - ⚠️  Will be skipped", i + 1);
            empty_count += 1;
        } else {
            println!("  {}. \"{}\"", i + 1, trimmed);
            valid_count += 1;
        }
    }

    if empty_count > 0 {
        println!(
            "\n⚠️  Note: {empty_count} device(s) with empty names will be skipped to prevent crashes."
        );
    }

    if valid_count > 0 {
        println!("\nConfiguration example:");
        println!("  (defcfg");
        println!("    macos-dev-names-include (");
        for device in device_names.iter().filter(|d| !d.trim().is_empty()) {
            println!("      \"{}\"", device.trim());
        }
        println!("    )");
        println!("  )");
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn list_devices_linux() {
    use crate::oskbd::discover_devices;
    use kanata_parser::cfg::DeviceDetectMode;

    println!("Available keyboard devices:");
    println!("===========================");

    let devices = discover_devices(None, None, DeviceDetectMode::KeyboardOnly);

    if devices.is_empty() {
        println!("No keyboard devices found.");
        println!("\nTroubleshooting:");
        println!("  1. Check permissions: sudo usermod -a -G input $USER");
        println!("  2. Log out and back in for group changes to take effect");
        println!("  3. Ensure devices are connected and working");
        return;
    }

    println!("Found {} keyboard device(s):\n", devices.len());

    for (i, (device, path)) in devices.iter().enumerate() {
        println!("  {}. \"{}\"", i + 1, device.name().unwrap_or("Unknown"));
        println!("     Path: {path}");
        println!();
    }

    println!("Configuration example:");
    println!("  (defcfg");
    println!("    linux-dev-names-include (");
    for (device, _path) in devices.iter() {
        println!("      \"{}\"", device.name().unwrap_or("Unknown"));
    }
    println!("    )");
    println!("  )");
}

#[cfg(all(target_os = "windows", feature = "interception_driver"))]
#[allow(dead_code)]
struct WindowsDeviceInfo {
    display_name: String,        // For user display
    raw_wide_bytes: Vec<u8>,     // For kanata configuration (original wide string bytes)
    hardware_id: Option<String>, // Parsed hardware ID (e.g., "HID#VID_046D&PID_C52B")
}

#[cfg(all(target_os = "windows", feature = "interception_driver"))]
#[allow(dead_code)]
fn get_device_info(device_handle: winapi::um::winnt::HANDLE) -> Option<WindowsDeviceInfo> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::ptr::null_mut;
    use winapi::shared::minwindef::{PUINT, UINT};
    use winapi::um::winuser::{GetRawInputDeviceInfoW, RIDI_DEVICENAME};

    unsafe {
        let mut name_size: UINT = 0;

        // First call to get the required buffer size (in characters, not bytes)
        GetRawInputDeviceInfoW(
            device_handle,
            RIDI_DEVICENAME,
            null_mut(),
            &mut name_size as PUINT,
        );

        if name_size > 0 {
            // Allocate buffer for wide characters
            let mut name_buffer: Vec<u16> = vec![0; name_size as usize];
            let result = GetRawInputDeviceInfoW(
                device_handle,
                RIDI_DEVICENAME,
                name_buffer.as_mut_ptr() as *mut _,
                &mut name_size as PUINT,
            );

            if result != u32::MAX {
                // Truncate buffer to actual data length (result is in bytes, divide by 2 for chars)
                let actual_char_count = (result / 2) as usize;
                name_buffer.truncate(actual_char_count);

                // Remove null terminator if present
                if let Some(&0) = name_buffer.last() {
                    name_buffer.pop();
                }

                // Convert to raw bytes (preserve original wide string format)
                let raw_wide_bytes: Vec<u8> =
                    name_buffer.iter().flat_map(|&c| c.to_le_bytes()).collect();

                // Create display name using OsString (preserves invalid UTF-16)
                let os_string = OsString::from_wide(&name_buffer);
                let display_name = os_string.to_string_lossy().into_owned();

                // Extract hardware ID from display name
                let hardware_id = extract_hardware_id(&display_name);

                return Some(WindowsDeviceInfo {
                    display_name,
                    raw_wide_bytes,
                    hardware_id,
                });
            }
        }
    }
    None
}

#[cfg(all(target_os = "windows", feature = "interception_driver"))]
#[allow(dead_code)]
pub(crate) fn list_devices_windows() {
    use std::ptr::null_mut;
    use winapi::shared::minwindef::{PUINT, UINT};
    use winapi::um::winuser::{GetRawInputDeviceList, RAWINPUTDEVICELIST, RIM_TYPEKEYBOARD};

    println!("Available keyboard devices:");
    println!("===========================");

    unsafe {
        // First, get the number of devices
        let mut num_devices: UINT = 0;
        let result = GetRawInputDeviceList(
            null_mut(),
            &mut num_devices as PUINT,
            std::mem::size_of::<RAWINPUTDEVICELIST>() as UINT,
        );

        if result == u32::MAX {
            println!("Error: Failed to get device count");
            return;
        }

        if num_devices == 0 {
            println!("No input devices found.");
            return;
        }

        // Allocate buffer for device list
        let mut devices: Vec<RAWINPUTDEVICELIST> = vec![std::mem::zeroed(); num_devices as usize];

        let result = GetRawInputDeviceList(
            devices.as_mut_ptr(),
            &mut num_devices as PUINT,
            std::mem::size_of::<RAWINPUTDEVICELIST>() as UINT,
        );

        if result == u32::MAX {
            println!("Error: Failed to get device list");
            return;
        }

        // Filter for keyboards and get device info
        let keyboards: Vec<&RAWINPUTDEVICELIST> = devices
            .iter()
            .filter(|device| device.dwType == RIM_TYPEKEYBOARD)
            .collect();

        if keyboards.is_empty() {
            println!("No keyboard devices found.");
            println!("\nTroubleshooting:");
            println!("  1. Ensure keyboards are connected and working");
            println!("  2. Try running as administrator if needed");
            return;
        }

        println!("Found {} keyboard device(s):\n", keyboards.len());

        for (i, device) in keyboards.iter().enumerate() {
            if let Some(device_info) = get_device_info(device.hDevice) {
                println!("  {}. Device: {}", i + 1, device_info.display_name);

                // Show hardware ID if available
                if let Some(hwid) = &device_info.hardware_id {
                    println!("     Hardware ID: {hwid}");
                }

                // Show raw wide string bytes for kanata configuration
                println!(
                    "     Raw wide string bytes: {:?}",
                    device_info.raw_wide_bytes
                );
                println!();
            }
        }

        if !keyboards.is_empty() {
            println!("Configuration example:");
            println!("  (defcfg");
            println!("    windows-interception-keyboard-hwids (");

            for device in keyboards.iter() {
                if let Some(device_info) = get_device_info(device.hDevice) {
                    // Use the preserved raw wide string bytes for configuration
                    print!("      {:?}", device_info.raw_wide_bytes);

                    // Add comment with hardware ID and display name for clarity
                    if let Some(hwid) = &device_info.hardware_id {
                        println!("  ; {} ({})", hwid, device_info.display_name);
                    } else {
                        println!("  ; {}", device_info.display_name);
                    }
                }
            }

            println!("    )");
            println!("  )");
        }
    }
}

#[cfg(all(target_os = "windows", feature = "interception_driver"))]
#[allow(dead_code)]
fn extract_hardware_id(device_name: &str) -> Option<String> {
    // Windows device names typically look like:
    // \\?\HID#VID_046D&PID_C52B&MI_01#7&1234abcd&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}
    // We want to extract the HID#VID_046D&PID_C52B&MI_01 part

    if let Some(start) = device_name.find("HID#") {
        if let Some(end) = device_name[start..].find('#') {
            let hwid_part = &device_name[start..start + end];
            return Some(hwid_part.to_string());
        }
    }

    None
}
