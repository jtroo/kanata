//! iced_gui code for the Kanata processing loop
//!
//! Send GUI update to TCP connections that are subscribed for GUI updates:
//! - on every N ticks
//! - before idling

use super::*;
use kanata_tcp_protocol::DetailedInfo;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct IcedGuiState {
    tick_count: u16,
}

const TICKS_PER_GUI_REFRESH: u16 = 50;

impl Kanata {
    pub(crate) fn tick_iced_gui_ms(&mut self, ms_elapsed: u16, tx: &Option<Sender<ServerMessage>>) {
        let Some(tx) = tx else {
            return;
        };
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
        self.send_detailed_info(tx);
    }

    fn send_detailed_info(&self, tx: &Sender<ServerMessage>) {
        let current_layer = self.layout.b().current_layer();
        let layer_config = self.layer_info[current_layer].name.clone();
        let msg = ServerMessage::DetailedInfo(DetailedInfo {
            layer_config,
            active_vkey_names: todo!(),
            chordsv2_state: todo!(),
            zippychord_state: todo!(),
        });
        todo!()
    }
}
