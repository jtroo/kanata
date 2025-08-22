use kanata_tcp_protocol::PermissionState;

type Boolean = u8;
type CFDictionaryRef = *const core::ffi::c_void;

#[repr(C)]
pub enum IOHIDRequestType {
    IOHIDRequestTypeListenEvent = 0,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDCheckAccess(request_type: IOHIDRequestType) -> Boolean;
}

#[derive(Debug)]
pub struct MacosPermissionStatus {
    pub accessibility: PermissionState,
    pub input_monitoring: PermissionState,
}

pub fn check_macos_permissions() -> MacosPermissionStatus {
    let accessibility = unsafe {
        let trusted = AXIsProcessTrustedWithOptions(core::ptr::null());
        if trusted == 1 {
            PermissionState::Granted
        } else {
            PermissionState::Denied
        }
    };

    let input_monitoring = unsafe {
        let granted = IOHIDCheckAccess(IOHIDRequestType::IOHIDRequestTypeListenEvent);
        if granted == 1 {
            PermissionState::Granted
        } else {
            PermissionState::Denied
        }
    };

    MacosPermissionStatus {
        accessibility,
        input_monitoring,
    }
}

pub fn restart_process() -> Result<(), String> {
    use std::ffi::CString;

    let args: Vec<_> = std::env::args().collect();
    if args.is_empty() {
        return Err("No arguments found".to_string());
    }

    let c_args: Result<Vec<CString>, _> = args.iter().map(|s| CString::new(s.as_str())).collect();
    let c_args = match c_args {
        Ok(v) => v,
        Err(e) => return Err(format!("Invalid argument: {}", e)),
    };

    let mut c_ptrs: Vec<*const libc::c_char> = c_args.iter().map(|s| s.as_ptr()).collect();
    c_ptrs.push(core::ptr::null());

    unsafe {
        libc::execv(c_ptrs[0], c_ptrs.as_ptr());
        Err(format!("execv failed: {}", std::io::Error::last_os_error()))
    }
}
