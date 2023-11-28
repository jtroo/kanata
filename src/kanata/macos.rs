use anyhow::{anyhow, bail, Result};
use log::info;
use parking_lot::Mutex;
use std::convert::TryFrom;
use std::sync::mpsc::SyncSender as Sender;
use std::sync::Arc;

use super::*;

impl Kanata {
    /// Enter an infinite loop that listens for OS key events and sends them to the processing
    /// thread.
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        todo!();
    }

    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        Ok(())
    }

}
