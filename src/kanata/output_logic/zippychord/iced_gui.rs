use super::*;

use itertools::Itertools;

impl ZchState {
    /// Return the current input keys
    pub(crate) fn zch_active_keys(&self) -> &[u16] {
        self.zchd.zchd_input_keys.zchik_keys()
    }

    /// Return the available prioritized chords
    pub(crate) fn zch_prioritized_possible_chords(&self, input_keys: impl AsRef<[u16]>) -> String {
        // TODO:
        // - save cfg line with the chord output
        // - iterate over the sets, skipping identical lines
        self.zchd
            .zchd_prioritized_chords
            .as_ref()
            .map(|pc| {
                pc.lock()
                    .0
                    .iter_supersets(input_keys.as_ref())
                    .map(|(k, v)| format!("{k:?} {v:?}"))
                    .join("\n")
            })
            .unwrap_or_default()
    }

    /// Return the available chords
    pub(crate) fn zch_possible_chords(&self, _input_keys: impl AsRef<[u16]>) -> String {
        todo!()
    }
}
