#[cfg(target_os = "macos")]
mod mac;
#[cfg(not(target_os = "macos"))]
mod stub;

#[cfg(target_os = "macos")]
pub use mac::*;
#[cfg(not(target_os = "macos"))]
pub use stub::*;
