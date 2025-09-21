use super::*;

use itertools::Itertools;

impl ZchState {
    /// Return the current input keys
    pub(crate) fn zch_active_keys(&self) -> &[u16] {
        self.zchd.zchd_input_keys.zchik_keys()
    }

    /// Return the available prioritized chords
    pub(crate) fn zch_prioritized_possible_chords(&self, input_keys: impl AsRef<[u16]>) -> String {
        let pchords = self.zchd.zchd_prioritized_chords.as_ref();
        match input_keys.as_ref().len() {
            0 => pchords.map(|pc| {
                pc.lock()
                    .0
                    .iter()
                    .map(|(_, v)| &v.zch_config_line)
                    .join("\n")
            }),
            _ => pchords.map(|pc| {
                pc.lock()
                    .0
                    .iter_supersets(input_keys.as_ref())
                    .map(|(_, v)| &v.zch_config_line)
                    .join("\n")
            }),
        }
        .unwrap_or_default()
    }

    /// Return the available chords
    pub(crate) fn zch_possible_chords(&self, input_keys: impl AsRef<[u16]>) -> String {
        match input_keys.as_ref().len() {
            0 => self
                .zch_chords
                .0
                .iter_unique_set_elements()
                .copied()
                .map(kanata_parser::cfg::iced_gui::names)
                .join(" "),
            _ => self
                .zch_chords
                .0
                .iter_supersets(input_keys.as_ref())
                .map(|(_, v)| &v.zch_config_line)
                .take(10)
                .join("\n"),
        }
    }
}
