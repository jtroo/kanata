#[cfg(feature = "gui")]
pub mod m_gui;
#[cfg(feature = "gui")]
use m_gui::main_gui;
#[cfg(not(feature = "gui"))]
pub mod m_cli;
#[cfg(not(feature = "gui"))]
use crate::m_cli::main_cli;

use anyhow::{Result};
#[cfg(not(feature = "gui"))]
fn main() -> Result<()> {
    let ret = main_cli();
    ret
}

#[cfg(feature = "gui")]
fn main() {
    main_gui()
}
