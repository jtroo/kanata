//! iced_gui code for the Kanata processing loop
//!
//! Send GUI update to TCP connections that are subscribed for GUI updates:
//! - on every N ticks
//! - before idling

use super::*;
use itertools::Itertools;
use kanata_tcp_protocol::DetailedInfo;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct IcedGuiState {
    tick_count: u16,
}

const TICKS_PER_GUI_REFRESH: u16 = 17;

impl Kanata {
    pub(crate) fn tick_iced_gui_ms(&mut self, ms_elapsed: u16, tx: &Option<Sender<ServerMessage>>) {
        let Some(tx) = tx else {
            return;
        };
        log::debug!("ticks hit {TICKS_PER_GUI_REFRESH}, sending detailed info");
        self.iced_gui_state.tick_count += ms_elapsed;
        if self.iced_gui_state.tick_count >= TICKS_PER_GUI_REFRESH {
            self.iced_gui_state.tick_count = 0;
            self.send_detailed_info(tx);
        }
    }

    pub(crate) fn iced_gui_handle_idle(&self, tx: &Option<Sender<ServerMessage>>) {
        let Some(tx) = tx else {
            return;
        };
        log::debug!("about to idle, sending detailed info");
        self.send_detailed_info(tx);
    }

    fn send_detailed_info(&self, tx: &Sender<ServerMessage>) {
        let current_layer = self.layout.b().current_layer();
        log::debug!("sending detailed info msg");
        let layer_config = self.layer_info[current_layer].name.clone();
        let vkey_names = &self.virtual_keys_by_idx;
        let active_vkey_names = self
            .layout
            .b()
            .states
            .iter()
            .filter_map(State::coord)
            .filter_map(|(row, idx)| match row {
                FAKE_KEY_ROW => vkey_names.get(&(idx as usize)).cloned(),
                _ => None,
            })
            .join(" ");

        let zippychord_state = "TODO".to_owned();
        let msg = ServerMessage::DetailedInfo(DetailedInfo {
            layer_config,
            active_vkey_names,
            zippychord_state,
        });
        if let Err(e) = tx.try_send(msg) {
            log::error!("could not send msg to gui: {e:?}");
        }
    }
}
