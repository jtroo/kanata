//! The different actions that can be executed via any given key.

use crate::key_code::KeyCode;
use crate::layout::{QueuedIter, WaitingAction};
use core::fmt::Debug;

pub mod switch;
pub use switch::*;

/// The different types of actions we support for key sequences/macros
#[non_exhaustive]
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum SequenceEvent<'a, T: 'a> {
    /// No operation action: just do nothing (a placeholder).
    NoOp,
    /// A keypress/keydown
    Press(KeyCode),
    /// Key release/keyup
    Release(KeyCode),
    /// A shortcut for `Press(KeyCode), Release(KeyCode)`
    Tap(KeyCode),
    /// For sequences that need to wait a bit before continuing
    Delay {
        /// How long (in ticks) this Delay will last
        duration: u32, // NOTE: This isn't a u16 because that's only max ~65 seconds (assuming 1000 ticks/sec)
    },
    /// Custom event in sequence.
    Custom(&'a T),
    /// Cancels the running sequence and can be used to mark the end of a sequence
    /// instead of using a number of Release() events
    Complete,
}

impl<'a, T> Debug for SequenceEvent<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoOp => write!(f, "NoOp"),
            Self::Press(arg0) => f.debug_tuple("Press").field(arg0).finish(),
            Self::Release(arg0) => f.debug_tuple("Release").field(arg0).finish(),
            Self::Tap(arg0) => f.debug_tuple("Tap").field(arg0).finish(),
            Self::Delay { duration } => {
                f.debug_struct("Delay").field("duration", duration).finish()
            }
            Self::Custom(_) => write!(f, "Custom"),
            Self::Complete => write!(f, "Complete"),
        }
    }
}

/// Behavior configuration of HoldTap.
#[non_exhaustive]
#[derive(Clone, Copy)]
pub enum HoldTapConfig<'a> {
    /// Only the timeout will determine between hold and tap action.
    ///
    /// This is a sane default.
    Default,
    /// If there is a key press, the hold action is activated.
    ///
    /// This behavior is interesting for a key which the tap action is
    /// not used in the flow of typing, like escape for example. If
    /// you are annoyed by accidental tap, you can try this behavior.
    HoldOnOtherKeyPress,
    /// If there is a press and release of another key, the hold
    /// action is activated.
    ///
    /// This behavior is interesting for fast typist: the different
    /// between hold and tap would more be based on the sequence of
    /// events than on timing. Be aware that doing the good succession
    /// of key might require some training.
    PermissiveHold,
    /// A custom configuration. Allows the behavior to be controlled by a caller
    /// supplied handler function.
    ///
    /// The input to the custom handler will be an iterator that returns
    /// [Stacked] [Events](Event). The order of the events matches the order the
    /// corresponding key was pressed/released, i.e. the first event is the
    /// event first received after the HoldTap action key is pressed.
    ///
    /// The return value should be the intended action that should be used. A
    /// [Some] value will cause one of: [WaitingAction::Tap] for the configured
    /// tap action, [WaitingAction::Hold] for the hold action, and
    /// [WaitingAction::NoOp] to drop handling of the key press. A [None]
    /// value will cause a fallback to the timeout-based approach. If the
    /// timeout is not triggered, the next tick will call the custom handler
    /// again.
    /// The bool value defines if the timeout check should be skipped at the
    /// next tick. This should generally be false. This is used by `tap-hold-
    /// except-keys` to handle presses even when the timeout has been reached.
    Custom(&'a (dyn Fn(QueuedIter) -> (Option<WaitingAction>, bool) + Send + Sync)),
}

impl<'a> Debug for HoldTapConfig<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HoldTapConfig::Default => f.write_str("Default"),
            HoldTapConfig::HoldOnOtherKeyPress => f.write_str("HoldOnOtherKeyPress"),
            HoldTapConfig::PermissiveHold => f.write_str("PermissiveHold"),
            HoldTapConfig::Custom(_) => f.write_str("Custom"),
        }
    }
}

impl<'a> PartialEq for HoldTapConfig<'a> {
    fn eq(&self, other: &Self) -> bool {
        #[allow(clippy::match_like_matches_macro)]
        match (self, other) {
            (HoldTapConfig::Default, HoldTapConfig::Default)
            | (HoldTapConfig::HoldOnOtherKeyPress, HoldTapConfig::HoldOnOtherKeyPress)
            | (HoldTapConfig::PermissiveHold, HoldTapConfig::PermissiveHold) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// A state that that can be released from the active states via the ReleaseState action.
pub enum ReleasableState {
    /// Release an active keycode
    KeyCode(KeyCode),
    /// Release an active layer
    Layer(usize),
}

/// Perform different actions on key hold/tap.
///
/// If the key is held more than `timeout` ticks (usually
/// milliseconds), performs the `hold` action, else performs the
/// `tap` action.  Mostly used with a modifier for the hold action
/// and a normal key on the tap action. Any action can be
/// performed, but using a `HoldTap` in a `HoldTap` is not
/// specified (but guaranteed to not crash).
///
/// Different behaviors can be configured using the config field,
/// but whatever the configuration is, if the key is pressed more
/// than `timeout`, the hold action is activated (if no other
/// action was determined before).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoldTapAction<'a, T>
where
    T: 'a,
{
    /// The duration, in ticks (usually milliseconds) giving the
    /// difference between a hold and a tap.
    pub timeout: u16,
    /// The hold action.
    pub hold: Action<'a, T>,
    /// The tap action.
    pub tap: Action<'a, T>,
    /// The timeout action
    pub timeout_action: Action<'a, T>,
    /// Behavior configuration.
    pub config: HoldTapConfig<'a>,
    /// Configuration of the tap and hold holds the tap action.
    ///
    /// If you press and release the key in such a way that the tap
    /// action is performed, and then press it again in less than
    /// `tap_hold_interval` ticks, the tap action will
    /// be held. This allows the tap action to be held by
    /// pressing, releasing and holding the key, allowing the computer
    /// to auto repeat the tap behavior. The timeout starts on the
    /// first press of the key, NOT on the release.
    ///
    /// Pressing a different key in between will not result in the
    /// behaviour described above; the HoldTap key must be pressed twice
    /// in a row.
    ///
    /// To deactivate the functionality, set this to 0.
    pub tap_hold_interval: u16,
}

/// Define one shot key behaviour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OneShot<'a, T = core::convert::Infallible>
where
    T: 'a,
{
    /// Action to activate until timeout expires or exactly one non-one-shot key is activated.
    pub action: &'a Action<'a, T>,
    /// Timeout after which one shot will expire. Note: timeout will be overwritten if another
    /// one shot key is pressed.
    pub timeout: u16,
    /// Configuration of one shot end behaviour. Note: this will be overwritten if another one shot
    /// key is pressed. Consider keeping this consistent between all your one shot keys to prevent
    /// surprising behaviour.
    pub end_config: OneShotEndConfig,
}

/// Determine the ending behaviour of the one shot key.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OneShotEndConfig {
    /// End one shot activation on first non-one-shot key press.
    EndOnFirstPress,
    /// End one shot activation on first non-one-shot key press or a repress of an already-pressed
    /// one-shot key.
    EndOnFirstPressOrRepress,
    /// End one shot activation on first non-one-shot key release.
    EndOnFirstRelease,
    /// End one shot activation on first non-one-shot key release or a repress of an already-pressed
    /// one-shot key.
    EndOnFirstReleaseOrRepress,
}

/// Defines the maximum number of one shot keys that can be combined.
pub const ONE_SHOT_MAX_ACTIVE: usize = 16;

/// Define tap dance behaviour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TapDance<'a, T = core::convert::Infallible>
where
    T: 'a,
{
    /// List of actions that activate based on number of taps. Only one of the actions will
    /// activate. Tapping the tap-dance key once will activate the action in index 0, three
    /// times will activate the action in index 2.
    pub actions: &'a [&'a Action<'a, T>],
    /// Timeout after which a tap will expire and become an action. A new tap for the same
    /// tap-dance key will reset this timeout.
    pub timeout: u16,
    /// Determine behaviour of tap dance. Eager evaluation will activate every action in the
    /// sequence as keys are pressed. Lazy will activate only a single action, decided by the
    /// number of taps in the sequence.
    pub config: TapDanceConfig,
}

/// Determines the behaviour for a `TapDance`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TapDanceConfig {
    Lazy,
    Eager,
}

/// A group of chords (actions mapped to a combination of multiple physical keys pressed together).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChordsGroup<'a, T = core::convert::Infallible>
where
    T: 'a,
{
    /// List of key coordinates participating in this chord group, each with the corresponding [ChordKeys] they map to.
    pub coords: &'a [((u8, u16), ChordKeys)],
    /// Map of chords to actions they execute.
    pub chords: &'a [(ChordKeys, &'a Action<'a, T>)],
    /// Timeout after which a chord will expire and either trigger its action or be discarded if there is no corresponding action.
    /// A chord may trigger its action even before this timeout expires, if a chord key is released, a non-chord key is pressed or the pressed chord is already uniquely identifyable.
    pub timeout: u16,
}

impl<'a, T> ChordsGroup<'a, T> {
    /// Gets the chord keys corresponding to the given key coordinates.
    pub fn get_keys(&self, coord: (u8, u16)) -> Option<ChordKeys> {
        self.coords.iter().find(|c| c.0 == coord).map(|c| c.1)
    }

    /// Gets the chord action assigned to the given chord keys.
    pub fn get_chord(&self, keys: ChordKeys) -> Option<&'a Action<'a, T>> {
        self.chords
            .iter()
            .find(|(chord_keys, _)| *chord_keys == keys)
            .map(|(_, action)| *action)
    }

    /// Gets the chord action assigned to the given chord keys if they are already unambigous (i.e. there is no key that could still be pressed that would result in a different chord).
    pub fn get_chord_if_unambiguous(&self, keys: ChordKeys) -> Option<&'a Action<'a, T>> {
        self.chords
            .iter()
            .try_fold(None, |res, &(chord_keys, action)| {
                if chord_keys == keys {
                    Ok(Some(action))
                } else if chord_keys | keys == chord_keys {
                    // The given keys are a subset of this chord but not an exact match
                    // -> ambiguity
                    Err(())
                } else {
                    Ok(res)
                }
            })
            .unwrap_or_default()
    }
}

/// A set of virtual keys (represented as a bit mask) pressed together.
/// The keys do not directly correspond to physical keys. They are unique to a given [ChordGroup] and their mapping from physical keys is definied in [ChordGroup.coords].
/// As such, each chord group can effectively have at most 32 different keys (though multiple physical keys may be mapped to the same virtual key).
pub type ChordKeys = u32;

/// Defines the maximum number of (virtual) keys that can be used in a single chords group.
pub const MAX_CHORD_KEYS: usize = ChordKeys::BITS as usize;

/// An action that can do one of two actions. The `left` action is the default. The `right` action
/// will trigger if any of the key codes in `right_triggers` are active in the current layout
/// state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForkConfig<'a, T> {
    pub left: Action<'a, T>,
    pub right: Action<'a, T>,
    pub right_triggers: &'a [KeyCode],
}

/// The different actions that can be done.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action<'a, T = core::convert::Infallible>
where
    T: 'a,
{
    /// No operation action: just do nothing.
    NoOp,
    /// Transparent, i.e. get the action from the default layer. On
    /// the default layer, it is equivalent to `NoOp`.
    Trans,
    /// A key code, i.e. a classic key.
    KeyCode(KeyCode),
    /// Multiple key codes sent at the same time, as if these keys
    /// were pressed at the same time. Useful to send a shifted key,
    /// or complex shortcuts like Ctrl+Alt+Del in a single key press.
    MultipleKeyCodes(&'a &'a [KeyCode]),
    /// Multiple actions sent at the same time.
    MultipleActions(&'a &'a [Action<'a, T>]),
    /// While pressed, change the current layer. That's the classic
    /// Fn key. If several layer actions are hold at the same time,
    /// the last pressed defines the current layer.
    Layer(usize),
    /// Change the default layer.
    DefaultLayer(usize),

    /// A sequence of SequenceEvents
    Sequence {
        /// An array of SequenceEvents that will be triggered (in order)
        events: &'a &'a [SequenceEvent<'a, T>],
    },
    /// A sequence of SequenceEvents, which will be repeated so long as the key is held.
    RepeatableSequence {
        /// An array of SequenceEvents that will be triggered (in order)
        events: &'a &'a [SequenceEvent<'a, T>],
    },
    /// Cancels any running sequences
    CancelSequences,
    /// Action to release either a keycode state or a layer state.
    ReleaseState(ReleasableState),

    /// Perform different actions on key hold/tap (see [`HoldTapAction`]).
    HoldTap(&'a HoldTapAction<'a, T>),
    /// Custom action.
    ///
    /// Define a user defined action. This enum can be anything you
    /// want, as long as it has the `'a` lifetime. It can be used
    /// to drive any non keyboard related actions that you might
    /// manage with key events.
    Custom(T),
    /// One shot key. Also known as "sticky key". See `struct OneShot` for configuration info.
    /// Activates `action` until a single other key that is not also a one shot key is used. For
    /// example, a one shot key can be used to activate shift for exactly one keypress or switch to
    /// another layer for exactly one keypress. Holding a one shot key will be treated as a normal
    /// held keypress.
    ///
    /// If you use one shot outside of its intended use cases (modifier key action or layer
    /// action) then you will likely have undesired behaviour. E.g. one shot with the space
    /// key will hold space until either another key is pressed or the timeout occurs, which will
    /// probably send many undesired space characters to your active application.
    OneShot(&'a OneShot<'a, T>),
    /// Tap-dance key. When tapping the key N times in quck succession, activates the N'th action
    /// in `actions`. The action will activate in the following conditions:
    ///
    /// - a different key is pressed
    /// - `timeout` ticks elapse since the last tap of the same tap-dance key
    /// - the number of taps is equal to the length of `actions`.
    TapDance(&'a TapDance<'a, T>),
    /// Chord key. Enters chording mode where multiple keys may be pressed together to active
    /// different actions depending on the specific combination ("chord") pressed.
    /// See `struct ChordGroup` for configuration info.
    ///
    /// Keys participating in chording mode are listed in `coords`.
    /// Chording mode ends when a non-participating key is pressed, a participating key is released,
    /// the timeout expires, or when the pressed chord uniquely identifies an action (i.e. there are
    /// no more keys you could press to change the result).
    Chords(&'a ChordsGroup<'a, T>),
    /// Repeat the previous action.
    Repeat,
    /// Fork action that can activate one of two potential actions depending on what keys are
    /// currently active.
    Fork(&'a ForkConfig<'a, T>),
    /// Action that can activate 0 to N actions based on what keys are currently
    /// active and the boolean logic of each case.
    ///
    /// The maximum number of actions that can activate the same time is governed by
    /// `ACTION_QUEUE_LEN`.
    Switch(&'a Switch<'a, T>),
}

impl<'a, T> Action<'a, T> {
    /// Gets the layer number if the action is the `Layer` action.
    pub fn layer(self) -> Option<usize> {
        match self {
            Action::Layer(l) => Some(l),
            _ => None,
        }
    }
    /// Returns an iterator on the `KeyCode` corresponding to the action.
    pub fn key_codes(&self) -> impl Iterator<Item = KeyCode> + '_ {
        match self {
            Action::KeyCode(kc) => core::slice::from_ref(kc).iter().cloned(),
            Action::MultipleKeyCodes(kcs) => kcs.iter().cloned(),
            _ => [].iter().cloned(),
        }
    }
}

/// A shortcut to create a `Action::KeyCode`, useful to create compact
/// layout.
pub const fn k<T>(kc: KeyCode) -> Action<'static, T> {
    Action::KeyCode(kc)
}

/// A shortcut to create a `Action::Layer`, useful to create compact
/// layout.
pub const fn l<T>(layer: usize) -> Action<'static, T> {
    Action::Layer(layer)
}

/// A shortcut to create a `Action::DefaultLayer`, useful to create compact
/// layout.
pub const fn d<T>(layer: usize) -> Action<'static, T> {
    Action::DefaultLayer(layer)
}
