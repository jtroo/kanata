use super::*;

impl Kanata {
    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        // Silence warning
        check_for_exit(&KeyEvent::new(OsCode::KEY_UNKNOWN, KeyValue::Release));
        Ok(())
    }
}
