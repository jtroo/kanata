#![cfg_attr(feature = "gui", windows_subsystem = "windows")] //disable console on Windows

#[cfg(not(feature = "gui"))]
use kanata_state_machine::lib_main::lib_main_cli;
#[cfg(feature = "gui")]
use kanata_state_machine::lib_main::lib_main_gui;

#[cfg(not(feature = "gui"))]
use anyhow::Result;
#[cfg(not(feature = "gui"))]
fn main() -> Result<()> {
    lib_main_cli()
}

#[cfg(feature = "gui")]
fn main() {
    lib_main_gui();
}
