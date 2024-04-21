use super::*;

pub static PRESSED_KEYS: Lazy<Mutex<HashSet<OsCode>>> =
    Lazy::new(|| Mutex::new(HashSet::default()));

impl Kanata {
    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        // Silence warning
        check_for_exit(&KeyEvent::new(OsCode::KEY_UNKNOWN, KeyValue::Release));
        Ok(())
    }
}
