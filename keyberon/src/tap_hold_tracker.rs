//! Tracks tap-hold activation events for external consumers (e.g. TCP broadcast).
//!
//! When the `tap_hold_tracker` feature is enabled, this module stores the
//! coordinate of the most recent hold/tap activation so that higher-level code
//! can relay it over the network.  When the feature is disabled the tracker is
//! a zero-sized no-op — all setters are empty and all getters return `None`.
//!
//! The `config` parameter on the setters accepts a `&WaitingConfig` reference;
//! the `matches!` guard lives inside the method body so that the no-op stub's
//! empty body causes the compiler to eliminate the call entirely.

#[cfg(feature = "tap_hold_tracker")]
mod inner {
    use crate::layout::{KCoord, WaitingConfig};

    /// Information about a tap-hold key that just transitioned to hold state.
    #[derive(Debug, Clone, Copy)]
    pub struct HoldActivatedInfo {
        /// The key coordinate (row, column).
        pub coord: KCoord,
    }

    /// Information about a tap-hold key that just triggered its tap action.
    #[derive(Debug, Clone, Copy)]
    pub struct TapActivatedInfo {
        /// The key coordinate (row, column).
        pub coord: KCoord,
    }

    /// Records the most recent tap-hold activation event.
    #[derive(Debug, Default)]
    pub struct TapHoldTracker {
        hold_activated: Option<HoldActivatedInfo>,
        tap_activated: Option<TapActivatedInfo>,
    }

    impl TapHoldTracker {
        pub(crate) fn set_hold_activated<'a, T: std::fmt::Debug>(
            &mut self,
            coord: KCoord,
            config: &WaitingConfig<'a, T>,
        ) {
            if matches!(config, WaitingConfig::HoldTap(..)) {
                self.hold_activated = Some(HoldActivatedInfo { coord });
            }
        }

        pub(crate) fn set_tap_activated<'a, T: std::fmt::Debug>(
            &mut self,
            coord: KCoord,
            config: &WaitingConfig<'a, T>,
        ) {
            if matches!(config, WaitingConfig::HoldTap(..)) {
                self.tap_activated = Some(TapActivatedInfo { coord });
            }
        }

        pub fn take_hold_activated(&mut self) -> Option<HoldActivatedInfo> {
            self.hold_activated.take()
        }

        pub fn take_tap_activated(&mut self) -> Option<TapActivatedInfo> {
            self.tap_activated.take()
        }
    }
}

#[cfg(not(feature = "tap_hold_tracker"))]
mod inner {
    use crate::layout::{KCoord, WaitingConfig};

    /// Stub: no coordinate data stored when the feature is disabled.
    #[derive(Debug, Clone, Copy)]
    pub struct HoldActivatedInfo {
        /// The key coordinate (row, column).
        pub coord: KCoord,
    }

    /// Stub: no coordinate data stored when the feature is disabled.
    #[derive(Debug, Clone, Copy)]
    pub struct TapActivatedInfo {
        /// The key coordinate (row, column).
        pub coord: KCoord,
    }

    /// Zero-sized no-op tracker when the feature is disabled.
    #[derive(Debug, Default)]
    pub struct TapHoldTracker;

    impl TapHoldTracker {
        #[inline(always)]
        pub(crate) fn set_hold_activated<'a, T: std::fmt::Debug>(
            &mut self,
            _coord: KCoord,
            _config: &WaitingConfig<'a, T>,
        ) {
        }

        #[inline(always)]
        pub(crate) fn set_tap_activated<'a, T: std::fmt::Debug>(
            &mut self,
            _coord: KCoord,
            _config: &WaitingConfig<'a, T>,
        ) {
        }

        #[inline(always)]
        pub fn take_hold_activated(&mut self) -> Option<HoldActivatedInfo> {
            None
        }

        #[inline(always)]
        pub fn take_tap_activated(&mut self) -> Option<TapActivatedInfo> {
            None
        }
    }
}

pub use inner::*;
