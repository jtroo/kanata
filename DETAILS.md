# macOS Permissions Feature Implementation

## Overview

This document details the implementation of macOS permission checking and remote restart functionality for Kanata. This feature allows external applications to check the current macOS system permissions (Accessibility and Input Monitoring) and remotely restart the Kanata process via TCP commands.

## New Feature Summary

The macOS permissions feature adds:

1. **System Permission Checking**: Check Accessibility and Input Monitoring permissions on macOS
2. **Remote Permission Status**: Query permission status via TCP server
3. **Remote Process Restart**: Restart Kanata process remotely via TCP commands
4. **Cross-platform Compatibility**: Graceful fallback for non-macOS platforms

## Files Modified/Created

### 1. New Module: `src/macos_permissions.rs`

**Purpose**: Core macOS permission checking and process restart functionality

**Key Components**:
- **Foreign Function Interface (FFI)**: Direct bindings to macOS system frameworks
  - `ApplicationServices` framework for Accessibility permissions (`AXIsProcessTrustedWithOptions`)
  - `IOKit` framework for Input Monitoring permissions (`IOHIDCheckAccess`)
- **Permission Status Types**: 
  - `MacosPermissionStatus` struct containing both permission states
  - `PermissionState` enum with states: `Granted`, `Denied`, `Error`
- **Cross-platform Support**: Conditional compilation for macOS vs other platforms
- **Process Restart**: `restart_process()` function using `execv` system call

**Key Functions**:
- `check_macos_permissions()` - Main entry point for permission checking
- `check_accessibility_permission()` - Checks Accessibility permission via AX framework
- `check_input_monitoring_permission()` - Checks Input Monitoring via IOHIDCheckAccess
- `restart_process()` - Replaces current process with new instance using execv

### 2. Library Exports: `src/lib.rs`

**Changes**: 
- Exposes the `macos_permissions` module for use by other parts of the application

### 3. TCP Server Integration: `src/tcp_server.rs`

**Changes**:
- **Import**: Adds `check_macos_permissions` and `restart_process`
- **New TCP Commands**:
  - `CheckMacosPermissions` - Returns current permission status as JSON
  - `Restart` - Attempts to restart the Kanata process

**TCP Message Handling**:
- `ClientMessage::CheckMacosPermissions {}` handler:
  - Calls `check_macos_permissions()` 
  - Responds with `ServerMessage::MacosPermissions` containing typed enum values
    (`PermissionState`) serialized as snake_case strings
- `ClientMessage::Restart {}` handler:
  - Sends an ACK (`{"status":"Ok"}`), flushes the stream, and schedules a restart
    after a short delay (≈200ms) to ensure the client receives the response
  - If the restart fails, the error is logged server-side

### 4. TCP Protocol: `tcp_protocol/src/lib.rs`

**Protocol Changes**:
- **Enum**: `PermissionState` with variants `Granted`, `Denied`, `NotApplicable`, `Error`.
  - Serialized as snake_case strings (e.g., "granted", "denied", "not_applicable", "error").
- **Client Messages**:
  - `CheckMacosPermissions {}` - Request permission status
  - `Restart {}` - Request process restart
- **Server Messages**:
  - `MacosPermissions { accessibility: PermissionState, input_monitoring: PermissionState }` - Permission status response

### 5. Documentation: `README.md`

**Updates**:
- Line added to TCP server feature description: "On macOS: Check system permissions and restart kanata remotely via TCP commands"
- Integrated into existing TCP server documentation section

## Technical Implementation Details

### Permission Checking Architecture

The implementation uses direct FFI bindings to macOS system frameworks rather than higher-level APIs for several reasons:
1. **Minimal Dependencies**: Avoids additional crate dependencies
2. **Direct Control**: Direct access to system APIs without wrapper overhead
3. **Compatibility**: Works across different macOS versions

### Safety Considerations

1. **FFI Safety**: All unsafe FFI calls are properly wrapped and error-handled
2. **Null Pointer Handling**: Proper null pointer usage for system calls
3. **Cross-platform Safety**: Non-macOS platforms return `PermissionState::NotApplicable` rather than attempting system calls
4. **Process Restart Safety**: Argument validation and proper CString handling for execv

### Error Handling

- **Permission Errors**: Graceful degradation to `PermissionState::Error` on system call failures
- **Restart Errors**: Detailed error messages including errno information
- **TCP Integration**: Proper error propagation through TCP response messages

### Cross-platform Design

The module is designed with cross-platform compatibility in mind:
- macOS-specific code is conditionally compiled with `#[cfg(target_os = "macos")]`
- Non-macOS platforms return `PermissionState::NotApplicable` for both fields
- No compilation failures on non-macOS platforms

## Usage Examples

### Via TCP Client

```bash
# Check permissions
echo '{"CheckMacosPermissions": {}}' | nc localhost 13331

# Restart process  
echo '{"Restart": {}}' | nc localhost 13331
```

### Expected Responses

```json
// Permission check response
{
  "MacosPermissions": {
    "accessibility": "granted",
    "input_monitoring": "denied"
  }
}

// Restart response ACK
{"status": "Ok"}

// Restart response (failure)
{
  "status": "Error",
  "msg": "restart failed: execv failed: Permission denied"
}
```

## Integration Points

This feature integrates with existing Kanata infrastructure:
1. **TCP Server**: Leverages existing TCP command infrastructure
2. **Logging**: Uses existing log macros for status reporting  
3. **Error Handling**: Follows existing error propagation patterns
4. **Configuration**: No additional configuration required - works with existing TCP server setup

## Future Enhancements

Potential future improvements:
1. **Permission Prompting**: Option to show system permission dialogs
2. **Automatic Restart**: Auto-restart when permissions are granted
3. **Status Monitoring**: Continuous permission status monitoring
4. **GUI Integration**: Permission status in system tray/GUI interfaces

## Security Considerations

1. **TCP Access Control**: Permission checking and restart commands are available to any client that can connect to the TCP server
2. **Process Replacement**: The restart functionality completely replaces the current process - any unsaved state is lost
3. **System Integration**: Requires appropriate macOS permissions to function correctly

This implementation provides a solid foundation for macOS permission management in Kanata while maintaining cross-platform compatibility and following the existing codebase patterns.