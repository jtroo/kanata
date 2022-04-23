//! Platform specific code for OS key code mappings.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "windows")]
#[allow(dead_code)]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;
