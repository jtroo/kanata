use kanata_tcp_protocol::PermissionState;

#[derive(Debug)]
pub struct MacosPermissionStatus {
    pub accessibility: PermissionState,
    pub input_monitoring: PermissionState,
}

pub fn check_macos_permissions() -> MacosPermissionStatus {
    MacosPermissionStatus {
        accessibility: PermissionState::NotApplicable,
        input_monitoring: PermissionState::NotApplicable,
    }
}

pub fn restart_process() -> Result<(), String> {
    Err("Process restart is only supported on macOS".to_string())
}
