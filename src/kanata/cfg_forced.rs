//! Options in the configuration file that are overidden/forced to some value other than what's in
//! the configuration file, with the primary example being CLI arguments.

use std::sync::OnceLock;

static LOG_LAYER_CHANGES: OnceLock<bool> = OnceLock::new();

/// Force the log_layer_changes configuration to some value.
/// This can only be called up to once. Panics if called a second time.
pub fn force_log_layer_changes(v: bool) {
    LOG_LAYER_CHANGES
        .set(v)
        .expect("force cfg fns can only be called once");
}

/// Get the forced log_layer_changes configuration if it was set.
pub fn get_forced_log_layer_changes() -> Option<bool> {
    LOG_LAYER_CHANGES.get().copied()
}
