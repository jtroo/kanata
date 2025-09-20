use super::*;

impl ZchState {
    /// Return the current input keys
    pub(crate) fn zch_active_keys(&self) -> impl Iterator<Item = u16> + Clone {
        self.zchd.zchd_input_keys.zchik_keys().iter().copied()
    }

    /// Return the available prioritized chords
    pub(crate) fn zch_prioritized_possible_chords(&self, _input_keys: impl Iterator<Item = u16>) -> String {
        todo!()
    }

    /// Return the available chords
    pub(crate) fn zch_possible_chords(&self, _input_keys: impl Iterator<Item = u16>) -> String {
        todo!()
    }
}
