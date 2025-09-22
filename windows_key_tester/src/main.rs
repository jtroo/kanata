//! This program is intended to be similar to `evtest` but for Windows. It will read keyboard
//! events, print out the event info, then forward it the keyboard event as-is to the rest of the
//! operating system handling.

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::*;

#[cfg(target_os = "windows")]
fn main() {
    let ret = main_impl();
    if let Err(ref e) = ret {
        log::error!("main got error {}", e);
    }
    eprintln!("\nPress any key to exit");
    let _ = std::io::stdin().read_line(&mut String::new());
}

#[cfg(not(target_os = "windows"))]
fn main() {
    print!("Hello world! Wrong OS. Doing nothing.");
}
