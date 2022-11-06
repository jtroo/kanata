//! The different actions that can be executed via any given key.

use crate::key_code::KeyCode;
use crate::layout::{StackedIter, WaitingAction};
use core::fmt::Debug;

/// The different types of actions we support for key sequences/macros
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SequenceEvent {
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
    /// A marker that indicates there's more of the macro than would fit
    /// in the 'sequenced' ArrayDeque
    Continue {
        /// The current chunk
        index: usize,
        /// The full list of Sequence Events (that aren't Continue())
        events: &'static [SequenceEvent],
    },
    /// Cancels the running sequence and can be used to mark the end of a sequence
    /// instead of using a number of Release() events
    Complete,
}

/// Behavior configuration of HoldTap.
#[non_exhaustive]
#[derive(Clone, Copy)]
pub enum HoldTapConfig {
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
    Custom(fn(StackedIter) -> Option<WaitingAction>),
}

impl Debug for HoldTapConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HoldTapConfig::Default => f.write_str("Default"),
            HoldTapConfig::HoldOnOtherKeyPress => f.write_str("HoldOnOtherKeyPress"),
            HoldTapConfig::PermissiveHold => f.write_str("PermissiveHold"),
            HoldTapConfig::Custom(func) => f
                .debug_tuple("Custom")
                .field(&(*func as fn(StackedIter<'static>) -> Option<WaitingAction>) as &dyn Debug)
                .finish(),
        }
    }
}

impl PartialEq for HoldTapConfig {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (HoldTapConfig::Default, HoldTapConfig::Default)
            | (HoldTapConfig::HoldOnOtherKeyPress, HoldTapConfig::HoldOnOtherKeyPress)
            | (HoldTapConfig::PermissiveHold, HoldTapConfig::PermissiveHold) => true,
            (HoldTapConfig::Custom(self_func), HoldTapConfig::Custom(other_func)) => {
                *self_func as fn(StackedIter<'static>) -> Option<WaitingAction> == *other_func
            }
            _ => false,
        }
    }
}

impl Eq for HoldTapConfig {}

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
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct HoldTapAction<T>
where
    T: 'static,
{
    /// The duration, in ticks (usually milliseconds) giving the
    /// difference between a hold and a tap.
    pub timeout: u16,
    /// The hold action.
    pub hold: Action<T>,
    /// The tap action.
    pub tap: Action<T>,
    /// Behavior configuration.
    pub config: HoldTapConfig,
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
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct OneShot<T = core::convert::Infallible>
where
    T: 'static,
{
    /// Action to activate until timeout expires or exactly one non-one-shot key is activated.
    pub action: &'static Action<T>,
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
    /// End one shot activation on first non-one-shot key release.
    EndOnFirstRelease,
}

/// Defines the maximum number of one shot keys that can be combined.
pub const ONE_SHOT_MAX_ACTIVE: usize = 8;

/// Define tap dance behaviour.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TapDance<T = core::convert::Infallible>
where
    T: 'static,
{
    /// List of actions that activate based on number of taps. Only one of the actions will
    /// activate. Tapping the tap-dance key once will activate the action in index 0, three
    /// times will activate the action in index 2.
    pub actions: &'static [&'static Action<T>],
    /// Timeout after which a tap will expire and become an action. A new tap for the same
    /// tap-dance key will reset this timeout.
    pub timeout: u16,
}

/// The different actions that can be done.
#[non_exhaustive]
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Action<T = core::convert::Infallible>
where
    T: 'static,
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
    MultipleKeyCodes(&'static [KeyCode]),
    /// Multiple actions sent at the same time.
    MultipleActions(&'static [Action<T>]),
    /// While pressed, change the current layer. That's the classic
    /// Fn key. If several layer actions are hold at the same time,
    /// the last pressed defines the current layer.
    Layer(usize),
    /// Change the default layer.
    DefaultLayer(usize),

    /// A sequence of SequenceEvents
    Sequence {
        /// An array of SequenceEvents that will be triggered (in order)
        events: &'static [SequenceEvent],
    },
    /// Cancels any running sequences
    CancelSequences,
    /// Action to release either a keycode state or a layer state.
    ReleaseState(ReleasableState),

    /// Perform different actions on key hold/tap (see [`HoldTapAction`]).
    HoldTap(&'static HoldTapAction<T>),
    /// Custom action.
    ///
    /// Define a user defined action. This enum can be anything you
    /// want, as long as it has the `'static` lifetime. It can be used
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
    OneShot(&'static OneShot<T>),
    /// Tap-dance key. When tapping the key N times in quck succession, activates the N'th action
    /// in `actions`. The action will activate in the following conditions:
    ///
    /// - a different key is pressed
    /// - `timeout` ticks elapse since the last tap of the same tap-dance key
    /// - the number of taps is equal to the length of `actions`.
    TapDance(&'static TapDance<T>),
}

impl<T> Debug for Action<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoOp => write!(f, "NoOp"),
            Self::Trans => write!(f, "Trans"),
            Self::KeyCode(arg0) => f.debug_tuple("KeyCode").field(arg0).finish(),
            Self::MultipleKeyCodes(arg0) => f.debug_tuple("MultipleKeyCodes").field(arg0).finish(),
            Self::MultipleActions(arg0) => f.debug_tuple("MultipleActions").field(arg0).finish(),
            Self::Layer(arg0) => f.debug_tuple("Layer").field(arg0).finish(),
            Self::DefaultLayer(arg0) => f.debug_tuple("DefaultLayer").field(arg0).finish(),
            Self::HoldTap(HoldTapAction {
                timeout,
                hold,
                tap,
                config,
                tap_hold_interval,
            }) => f
                .debug_struct("HoldTap")
                .field("timeout", timeout)
                .field("hold", hold)
                .field("tap", tap)
                .field("config", config)
                .field("tap_hold_interval", tap_hold_interval)
                .finish(),
            Self::Sequence { events } => {
                f.debug_struct("Sequence").field("events", events).finish()
            }
            Self::CancelSequences => write!(f, "CancelSequences"),
            Self::OneShot(OneShot {
                action,
                timeout,
                end_config,
            }) => f
                .debug_struct("OneShot")
                .field("action", action)
                .field("timeout", timeout)
                .field("end_config", end_config)
                .finish(),
            Self::TapDance(TapDance { actions, timeout }) => f
                .debug_struct("TapDance")
                .field("actions", actions)
                .field("timeout", timeout)
                .finish(),
            Self::Custom(_) => f.debug_tuple("Custom").finish(),
            Self::ReleaseState(arg0) => f.debug_tuple("ReleaseState").field(arg0).finish(),
        }
    }
}

impl<T> Action<T> {
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
pub const fn k<T>(kc: KeyCode) -> Action<T> {
    Action::KeyCode(kc)
}

/// A shortcut to create a `Action::Layer`, useful to create compact
/// layout.
pub const fn l<T>(layer: usize) -> Action<T> {
    Action::Layer(layer)
}

/// A shortcut to create a `Action::DefaultLayer`, useful to create compact
/// layout.
pub const fn d<T>(layer: usize) -> Action<T> {
    Action::DefaultLayer(layer)
}

/// A shortcut to create a `Action::MultipleKeyCodes`, useful to
/// create compact layout.
pub const fn m<T>(kcs: &'static &'static [KeyCode]) -> Action<T> {
    Action::MultipleKeyCodes(kcs)
}
