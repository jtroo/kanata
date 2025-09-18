//! iced GUI code for the tcp server
//!
//! Manage subscription list to GUI info.
//! Provides functions to get a subscription list,
//! which a caller should likely check if it is empty before doing any extra processing,
//! and functions to send to the subscription list.

use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct SubscribedToDetailedInfo {
    subscribed_conn_keys: Arc<Mutex<rustc_hash::FxHashSet<String>>>,
}

impl SubscribedToDetailedInfo {
    pub(crate) fn add_subscriber(&self, sub: String) {
        self.subscribed_conn_keys.lock().insert(sub);
    }
    pub(crate) fn unsubscribe(&self, sub: &str) {
        self.subscribed_conn_keys.lock().remove(sub);
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = String> {
        let v = self
            .subscribed_conn_keys
            .lock()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        v.into_iter()
    }
}
