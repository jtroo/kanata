//! Tracks chord and tap-dance resolution events for external consumers (e.g. TCP broadcast).
//!
//! Gated on the `tap_hold_tracker` feature (same gate as tap-hold tracking since
//! both serve the TCP server use case). When disabled, all methods are no-ops.

use crate::layout::KCoord;
use arraydeque::ArrayDeque;

pub const MAX_CHORD_KEYS: usize = 8;
pub type ChordKeyArray = ArrayDeque<KCoord, MAX_CHORD_KEYS, arraydeque::behavior::Saturating>;

#[cfg(feature = "tap_hold_tracker")]
mod inner {
    use super::*;

    #[derive(Debug, Clone)]
    pub struct ChordResolvedInfo {
        pub keys: ChordKeyArray,
        pub action: heapless::String<64>,
    }

    #[derive(Debug, Clone)]
    pub struct TapDanceResolvedInfo {
        pub coord: KCoord,
        pub num_taps: u16,
        pub action: heapless::String<64>,
    }

    #[derive(Debug, Default)]
    pub struct ChordTapDanceTracker {
        chord_resolved: Option<ChordResolvedInfo>,
        tap_dance_resolved: Option<TapDanceResolvedInfo>,
    }

    impl ChordTapDanceTracker {
        pub(crate) fn set_chord_resolved(
            &mut self,
            keys: ChordKeyArray,
            action_desc: &dyn core::fmt::Display,
        ) {
            let mut action = heapless::String::new();
            let _ = core::fmt::Write::write_fmt(&mut action, format_args!("{}", action_desc));
            self.chord_resolved = Some(ChordResolvedInfo { keys, action });
        }

        pub(crate) fn set_tap_dance_resolved(
            &mut self,
            coord: KCoord,
            num_taps: u16,
            action_desc: &dyn core::fmt::Display,
        ) {
            let mut action = heapless::String::new();
            let _ = core::fmt::Write::write_fmt(&mut action, format_args!("{}", action_desc));
            self.tap_dance_resolved = Some(TapDanceResolvedInfo {
                coord,
                num_taps,
                action,
            });
        }

        pub fn take_chord_resolved(&mut self) -> Option<ChordResolvedInfo> {
            self.chord_resolved.take()
        }

        pub fn take_tap_dance_resolved(&mut self) -> Option<TapDanceResolvedInfo> {
            self.tap_dance_resolved.take()
        }
    }
}

#[cfg(not(feature = "tap_hold_tracker"))]
mod inner {
    use super::*;

    #[derive(Debug, Clone)]
    pub struct ChordResolvedInfo {
        pub keys: ChordKeyArray,
        pub action: heapless::String<64>,
    }

    #[derive(Debug, Clone)]
    pub struct TapDanceResolvedInfo {
        pub coord: KCoord,
        pub num_taps: u16,
        pub action: heapless::String<64>,
    }

    #[derive(Debug, Default)]
    pub struct ChordTapDanceTracker;

    impl ChordTapDanceTracker {
        #[inline(always)]
        pub(crate) fn set_chord_resolved(
            &mut self,
            _keys: ChordKeyArray,
            _action_desc: &dyn core::fmt::Display,
        ) {
        }

        #[inline(always)]
        pub(crate) fn set_tap_dance_resolved(
            &mut self,
            _coord: KCoord,
            _num_taps: u16,
            _action_desc: &dyn core::fmt::Display,
        ) {
        }

        #[inline(always)]
        pub fn take_chord_resolved(&mut self) -> Option<ChordResolvedInfo> {
            None
        }

        #[inline(always)]
        pub fn take_tap_dance_resolved(&mut self) -> Option<TapDanceResolvedInfo> {
            None
        }
    }
}

pub use inner::*;
