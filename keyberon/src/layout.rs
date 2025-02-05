//! Layout management.

/// A procedural macro to generate [Layers](type.Layers.html)
/// ## Syntax
/// Items inside the macro are converted to Actions as such:
/// - [`Action::KeyCode`]: Idents are automatically understood as keycodes: `A`, `RCtrl`, `Space`
///     - Punctuation, numbers and other literals that aren't special to the rust parser are converted
///       to KeyCodes as well: `,` becomes `KeyCode::Commma`, `2` becomes `KeyCode::Kb2`, `/` becomes `KeyCode::Slash`
///     - Characters which require shifted keys are converted to `Action::MultipleKeyCodes(&[LShift, <character>])`:
///       `!` becomes `Action::MultipleKeyCodes(&[LShift, Kb1])` etc
///     - Characters special to the rust parser (parentheses, brackets, braces, quotes, apostrophes, underscores, backslashes and backticks)
///       left alone cause parsing errors and as such have to be enclosed by apostrophes: `'['` becomes `KeyCode::LBracket`,
///       `'\''` becomes `KeyCode::Quote`, `'\\'` becomes `KeyCode::BSlash`
/// - [`Action::NoOp`]: Lowercase `n`
/// - [`Action::Trans`]: Lowercase `t`
/// - [`Action::Layer`]: A number in parentheses: `(1)`, `(4 - 2)`, `(0x4u8 as usize)`
/// - [`Action::MultipleActions`]: Actions in brackets: `[LCtrl S]`, `[LAlt LCtrl C]`, `[(2) B {Action::NoOp}]`
/// - Other `Action`s: anything in braces (`{}`) is copied unchanged to the final layout - `{ Action::Custom(42) }`
///   simply becomes `Action::Custom(42)`
///
/// **Important note**: comma (`,`) is a keycode on its own, and can't be used to separate keycodes as one would have
/// to do when not using a macro.
pub use kanata_keyberon_macros::*;

use crate::chord::*;
use crate::key_code::KeyCode;
use crate::{action::*, multikey_buffer::MultiKeyBuffer};
use arraydeque::ArrayDeque;
use heapless::Vec;

use State::*;

/// The coordinate type.
/// First item is either 0 or 1 denoting real key or virtual key, respectively.
/// Second item is the position in layout.
pub type KCoord = (u8, u16);

/// The Layers type.
///
/// `Layers` type is an array of layers which contain the description
/// of actions on the switch matrix. For example `layers[1][2][3]`
/// corresponds to the key on the first layer, row 2, column 3.
/// The generic parameters are in order: the number of columns, rows and layers,
/// and the type contained in custom actions.
pub type Layers<'a, const C: usize, const R: usize, T = core::convert::Infallible> =
    &'a [[[Action<'a, T>; C]; R]];

const QUEUE_SIZE: usize = 32;
pub type QueueLen = u8;

#[test]
fn check_queue_size() {
    use std::convert::TryFrom;
    let _v = QueueLen::try_from(QUEUE_SIZE).unwrap();
}

/// The current event queue.
///
/// Events can be retrieved by iterating over this struct and calling [Queued::event].
pub(crate) type Queue = ArrayDeque<Queued, QUEUE_SIZE, arraydeque::behavior::Wrapping>;

/// A list of queued press events. Used for special handling of potentially multiple press events
/// that occur during a Waiting event.
type PressedQueue = ArrayDeque<KCoord, QUEUE_SIZE>;

/// The maximum number of actions that can be activated concurrently via chord decomposition or
/// activation of multiple switch cases using fallthrough.
pub const ACTION_QUEUE_LEN: usize = 8;

/// The queue is currently only used for chord decomposition when a longer chord does not result in
/// an action, but splitting it into smaller chords would. The buffer size of 8 should be more than
/// enough for real world usage, but if one wanted to be extra safe, this should be ChordKeys::BITS
/// since that should guarantee that all potentially queueable actions can fit.
type ActionQueue<'a, T> =
    ArrayDeque<QueuedAction<'a, T>, ACTION_QUEUE_LEN, arraydeque::behavior::Wrapping>;
type Delay = u16;
pub(crate) type QueuedAction<'a, T> = Option<(KCoord, Delay, &'a Action<'a, T>)>;

const REAL_KEY_ROW: u8 = 0;

const HISTORICAL_EVENT_LEN: usize = 8;
const EXTRA_WAITING_LEN: usize = 8;
#[test]
fn extra_waiting_size_constraint() {
    assert!(EXTRA_WAITING_LEN < i8::MAX as usize);
}

/// The layout manager. It takes `Event`s and `tick`s as input, and
/// generate keyboard reports.
pub struct Layout<'a, const C: usize, const R: usize, T = core::convert::Infallible>
where
    T: 'a + std::fmt::Debug,
{
    /// Fallback for transparent keys inside actions that are on `default_layer`.
    pub src_keys: &'a [Action<'a, T>; C],
    pub layers: &'a [[[Action<'a, T>; C]; R]],
    pub default_layer: usize,
    /// Key states.
    pub states: Vec<State<'a, T>, 64>,
    pub waiting: Option<WaitingState<'a, T>>,
    pub extra_waiting:
        ArrayDeque<WaitingState<'a, T>, EXTRA_WAITING_LEN, arraydeque::behavior::Wrapping>,
    pub tap_dance_eager: Option<TapDanceEagerState<'a, T>>,
    pub queue: Queue,
    pub oneshot: OneShotState,
    pub last_press_tracker: LastPressTracker,
    pub active_sequences: ArrayDeque<SequenceState<'a, T>, 4, arraydeque::behavior::Wrapping>,
    pub action_queue: ActionQueue<'a, T>,
    pub rpt_action: Option<&'a Action<'a, T>>,
    pub historical_keys: History<KeyCode>,
    pub historical_inputs: History<KCoord>,
    pub quick_tap_hold_timeout: bool,
    pub chords_v2: Option<ChordsV2<'a, T>>,
    rpt_multikey_key_buffer: MultiKeyBuffer<'a, T>,
    trans_resolution_behavior_v2: bool,
    delegate_to_first_layer: bool,
}

pub struct History<T> {
    events: ArrayDeque<T, HISTORICAL_EVENT_LEN, arraydeque::behavior::Wrapping>,
    ticks_since_occurrences: ArrayDeque<u16, HISTORICAL_EVENT_LEN, arraydeque::behavior::Wrapping>,
}

#[derive(Copy, Clone)]
pub struct HistoricalEvent<T> {
    pub event: T,
    pub ticks_since_occurrence: u16,
}

impl<T> History<T>
where
    T: Copy,
{
    fn new() -> Self {
        Self {
            ticks_since_occurrences: ArrayDeque::new(),
            events: ArrayDeque::new(),
        }
    }

    fn tick_hist(&mut self) {
        let ticks = self.ticks_since_occurrences.as_uninit_slice_mut();
        for tick_count in ticks {
            unsafe {
                *tick_count.assume_init_mut() = tick_count.assume_init().saturating_add(1);
            }
        }
    }

    fn push_front(&mut self, event: T) {
        self.ticks_since_occurrences.push_front(0);
        self.events.push_front(event);
    }

    pub fn iter_hevents(&self) -> impl Iterator<Item = HistoricalEvent<T>> + '_ + Clone {
        self.events
            .iter()
            .copied()
            .zip(self.ticks_since_occurrences.iter().copied())
            .map(|(event, ticks_since_occurrence)| HistoricalEvent {
                event,
                ticks_since_occurrence,
            })
    }
}

/// An event on the key matrix.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Event {
    /// Press event with coordinates (i, j).
    Press(u8, u16),
    /// Release event with coordinates (i, j).
    Release(u8, u16),
}
impl Event {
    /// Returns the coordinates (i, j) of the event.
    pub fn coord(self) -> KCoord {
        match self {
            Event::Press(i, j) => (i, j),
            Event::Release(i, j) => (i, j),
        }
    }

    /// Transforms the coordinates of the event.
    ///
    /// # Example
    ///
    /// ```
    /// # use kanata_keyberon::layout::Event;
    /// assert_eq!(
    ///     Event::Press(3, 10),
    ///     Event::Press(3, 1).transform(|i, j| (i, 11 - j)),
    /// );
    /// ```
    pub fn transform(self, f: impl FnOnce(u8, u16) -> KCoord) -> Self {
        match self {
            Event::Press(i, j) => {
                let (i, j) = f(i, j);
                Event::Press(i, j)
            }
            Event::Release(i, j) => {
                let (i, j) = f(i, j);
                Event::Release(i, j)
            }
        }
    }

    /// Returns `true` if the event is a key press.
    pub fn is_press(self) -> bool {
        match self {
            Event::Press(..) => true,
            Event::Release(..) => false,
        }
    }

    /// Returns `true` if the event is a key release.
    pub fn is_release(self) -> bool {
        match self {
            Event::Release(..) => true,
            Event::Press(..) => false,
        }
    }
}

/// Event from custom action.
#[derive(Debug, Default, PartialEq, Eq)]
pub enum CustomEvent<'a, T: 'a> {
    /// No custom action.
    #[default]
    NoEvent,
    /// The given custom action key is pressed.
    Press(&'a T),
    /// The given custom action key is released.
    Release(&'a T),
}
impl<T> CustomEvent<'_, T> {
    /// Update an event according to a new event.
    ///
    ///The event can only be modified in the order `NoEvent < Press <
    /// Release`
    fn update(&mut self, e: Self) {
        use CustomEvent::*;
        match (&e, &self) {
            (Release(_), NoEvent) | (Release(_), Press(_)) => *self = e,
            (Press(_), NoEvent) => *self = e,
            _ => (),
        }
    }
}

/// Metadata about normal key flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NormalKeyFlags(pub u8);

pub const NORMAL_KEY_FLAG_CLEAR_ON_NEXT_ACTION: u8 = 0x01;
pub const NORMAL_KEY_FLAG_CLEAR_ON_NEXT_RELEASE: u8 = 0x02;

impl NormalKeyFlags {
    pub fn nkf_clear_on_next_action(self) -> bool {
        (self.0 & NORMAL_KEY_FLAG_CLEAR_ON_NEXT_ACTION) == NORMAL_KEY_FLAG_CLEAR_ON_NEXT_ACTION
    }
    pub fn nkf_clear_on_next_release(self) -> bool {
        (self.0 & NORMAL_KEY_FLAG_CLEAR_ON_NEXT_RELEASE) == NORMAL_KEY_FLAG_CLEAR_ON_NEXT_RELEASE
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum State<'a, T: 'a> {
    NormalKey {
        keycode: KeyCode,
        coord: KCoord,
        flags: NormalKeyFlags,
    },
    LayerModifier {
        value: usize,
        coord: KCoord,
    },
    Custom {
        value: &'a T,
        coord: KCoord,
    },
    FakeKey {
        keycode: KeyCode,
    }, // Fake key event for sequences
    RepeatingSequence {
        sequence: &'a &'a [SequenceEvent<'a, T>],
        coord: KCoord,
    },
    SeqCustomPending(&'a T),
    SeqCustomActive(&'a T),
    Tombstone,
}
impl<T> Copy for State<'_, T> {}
impl<T> Clone for State<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T: 'a> State<'a, T> {
    pub fn keycode(&self) -> Option<KeyCode> {
        match self {
            NormalKey { keycode, .. } => Some(*keycode),
            FakeKey { keycode } => Some(*keycode),
            _ => None,
        }
    }
    pub fn coord(&self) -> Option<KCoord> {
        match self {
            NormalKey { coord, .. }
            | LayerModifier { coord, .. }
            | Custom { coord, .. }
            | RepeatingSequence { coord, .. } => Some(*coord),
            _ => None,
        }
    }
    fn keycode_in_coords(&self, coords: &OneShotCoords) -> Option<KeyCode> {
        match self {
            NormalKey { keycode, coord, .. } => {
                if coords.contains(coord) {
                    Some(*keycode)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    /// Returns None if the key has been released and Some otherwise.
    pub fn release(&self, c: KCoord, custom: &mut CustomEvent<'a, T>) -> Option<Self> {
        match *self {
            NormalKey { coord, .. }
            | LayerModifier { coord, .. }
            | RepeatingSequence { coord, .. }
                if coord == c =>
            {
                None
            }
            Custom { value, coord } if coord == c => {
                custom.update(CustomEvent::Release(value));
                None
            }
            _ => Some(*self),
        }
    }
    pub fn release_state(&self, s: ReleasableState) -> Option<Self> {
        match (*self, s) {
            (
                NormalKey { keycode: k1, .. } | FakeKey { keycode: k1 },
                ReleasableState::KeyCode(k2),
            ) => {
                if k1 == k2 {
                    None
                } else {
                    Some(*self)
                }
            }
            (LayerModifier { value: l1, .. }, ReleasableState::Layer(l2)) => {
                if l1 == l2 {
                    None
                } else {
                    Some(*self)
                }
            }
            _ => Some(*self),
        }
    }
    fn seq_release(&self, kc: KeyCode) -> Option<Self> {
        match *self {
            FakeKey { keycode, .. } if keycode == kc => None,
            _ => Some(*self),
        }
    }
    fn get_layer(&self) -> Option<usize> {
        match self {
            LayerModifier { value, .. } => Some(*value),
            _ => None,
        }
    }
    pub fn clear_on_next_release(&self) -> bool {
        match self {
            NormalKey { flags, .. } => {
                (flags.0 & NORMAL_KEY_FLAG_CLEAR_ON_NEXT_RELEASE)
                    == NORMAL_KEY_FLAG_CLEAR_ON_NEXT_RELEASE
            }
            _ => false,
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct TapDanceState<'a, T: 'a> {
    actions: &'a [&'a Action<'a, T>],
    timeout: u16,
    num_taps: u16,
}

#[derive(Copy, Clone, Debug)]
pub struct TapDanceEagerState<'a, T: 'a> {
    coord: KCoord,
    actions: &'a [&'a Action<'a, T>],
    timeout: u16,
    orig_timeout: u16,
    num_taps: u16,
}

impl<T> TapDanceEagerState<'_, T> {
    fn tick_tde(&mut self) {
        self.timeout = self.timeout.saturating_sub(1);
    }

    fn is_expired(&self) -> bool {
        self.timeout == 0 || usize::from(self.num_taps) >= self.actions.len()
    }

    fn set_expired(&mut self) {
        self.timeout = 0;
    }

    fn incr_taps(&mut self) {
        self.num_taps += 1;
        self.timeout = self.orig_timeout;
    }
}

#[derive(Debug)]
enum WaitingConfig<'a, T: 'a + std::fmt::Debug> {
    HoldTap(HoldTapConfig<'a>),
    TapDance(TapDanceState<'a, T>),
    Chord(&'a ChordsGroup<'a, T>),
}

#[derive(Debug)]
pub struct WaitingState<'a, T: 'a + std::fmt::Debug> {
    coord: KCoord,
    timeout: u16,
    delay: u16,
    ticks: u16,
    hold: &'a Action<'a, T>,
    tap: &'a Action<'a, T>,
    timeout_action: &'a Action<'a, T>,
    config: WaitingConfig<'a, T>,
    layer_stack: LayerStack,
    prev_queue_len: QueueLen,
}

/// Actions that can be triggered for a key configured for HoldTap.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WaitingAction {
    /// Trigger the holding event.
    Hold,
    /// Trigger the tapping event.
    Tap,
    /// Trigger the timeout event.
    Timeout,
    /// Drop this event. It will act as if no key was pressed.
    NoOp,
}

impl<'a, T: std::fmt::Debug> WaitingState<'a, T> {
    fn tick_wt(
        &mut self,
        queued: &mut Queue,
        action_queue: &mut ActionQueue<'a, T>,
    ) -> Option<(WaitingAction, Option<PressedQueue>)> {
        self.timeout = self.timeout.saturating_sub(1);
        self.ticks = self.ticks.saturating_add(1);
        let mut pq = None;
        let (ret, cfg_change) = match self.config {
            WaitingConfig::HoldTap(htc) => (self.handle_hold_tap(htc, queued), None),
            WaitingConfig::TapDance(ref tds) => {
                let (ret, num_taps) =
                    self.handle_tap_dance(tds.num_taps, tds.actions.len(), queued);
                self.prev_queue_len = queued.len() as u8;
                // Due to ownership issues, handle_tap_dance can't contain all of the necessary
                // logic.
                if ret.is_some() {
                    let idx = core::cmp::min(num_taps.into(), tds.actions.len()).saturating_sub(1);
                    self.tap = tds.actions[idx];
                }
                if num_taps > tds.num_taps {
                    self.timeout = tds.timeout;
                }
                (
                    ret,
                    Some(WaitingConfig::TapDance(TapDanceState { num_taps, ..*tds })),
                )
            }
            WaitingConfig::Chord(config) => {
                if let Some((ret, action, cpq)) = self.handle_chord(config, queued, action_queue) {
                    self.tap = action;
                    pq = Some(cpq);
                    (Some(ret), None)
                } else {
                    (None, None)
                }
            }
        };
        if let Some(cfg) = cfg_change {
            self.config = cfg;
        }
        ret.map(|v| (v, pq))
    }

    fn handle_hold_tap(&mut self, cfg: HoldTapConfig, queued: &Queue) -> Option<WaitingAction> {
        if queued.len() as u8 == self.prev_queue_len && self.timeout > 0 {
            // Fast path: nothing has changed since last tick and we haven't timed out yet.
            return None;
        }
        self.prev_queue_len = queued.len() as u8;
        let mut skip_timeout = false;
        match cfg {
            HoldTapConfig::Default => (),
            HoldTapConfig::HoldOnOtherKeyPress => {
                if queued.iter().any(|s| s.event.is_press()) {
                    return Some(WaitingAction::Hold);
                }
            }
            HoldTapConfig::PermissiveHold => {
                let mut queued = queued.iter();
                while let Some(q) = queued.next() {
                    if q.event.is_press() {
                        let (i, j) = q.event.coord();
                        let target = Event::Release(i, j);
                        if queued.clone().any(|q| q.event == target) {
                            return Some(WaitingAction::Hold);
                        }
                    }
                }
            }
            HoldTapConfig::Custom(func) => {
                let (waiting_action, local_skip) = (func)(QueuedIter(queued.iter()));
                if waiting_action.is_some() {
                    return waiting_action;
                }
                skip_timeout = local_skip;
            }
        }
        if let Some(&Queued { since, .. }) = queued
            .iter()
            .find(|s| self.is_corresponding_release(&s.event))
        {
            if self.timeout > self.delay.saturating_sub(since) {
                Some(WaitingAction::Tap)
            } else {
                Some(WaitingAction::Timeout)
            }
        } else if self.timeout == 0 && (!skip_timeout) {
            Some(WaitingAction::Timeout)
        } else {
            None
        }
    }

    fn handle_tap_dance(
        &self,
        num_taps: u16,
        max_taps: usize,
        queued: &mut Queue,
    ) -> (Option<WaitingAction>, u16) {
        if queued.len() as u8 == self.prev_queue_len && self.timeout > 0 {
            // Fast path: nothing has changed since last tick and we haven't timed out yet.
            return (None, num_taps);
        }
        // Evict events with the same coordinates except for the final release. E.g. if 3 taps have
        // occurred, this will remove all `Press` events and 2 `Release` events. This is done so
        // that the state machine processes the entire tap dance sequence as a single press and
        // single release regardless of how many taps were actually done.
        let evict_same_coord_events = |num_taps: u16, queued: &mut Queue| {
            let mut releases_to_remove = num_taps.saturating_sub(1);
            queued.retain(|s| {
                let mut do_retain = true;
                if self.is_corresponding_release(&s.event) {
                    if releases_to_remove > 0 {
                        do_retain = false;
                        releases_to_remove = releases_to_remove.saturating_sub(1)
                    }
                } else if self.is_corresponding_press(&s.event) {
                    do_retain = false;
                }
                do_retain
            });
        };
        if self.timeout == 0 {
            evict_same_coord_events(num_taps, queued);
            return (Some(WaitingAction::Tap), num_taps);
        }
        // Get the number of sequential taps for this tap-dance key. If a different key was
        // pressed, activate a tap-dance action.
        match queued.iter().try_fold(1, |same_tap_count, s| {
            if self.is_corresponding_press(&s.event) {
                Ok(same_tap_count + 1)
            } else if matches!(s.event, Event::Press(..)) {
                Err((same_tap_count, ()))
            } else {
                Ok(same_tap_count)
            }
        }) {
            Ok(num_taps) if usize::from(num_taps) >= max_taps => {
                evict_same_coord_events(num_taps, queued);
                (Some(WaitingAction::Tap), num_taps)
            }
            Ok(num_taps) => (None, num_taps),
            Err((num_taps, _)) => {
                evict_same_coord_events(num_taps, queued);
                (Some(WaitingAction::Tap), num_taps)
            }
        }
    }

    fn handle_chord(
        &mut self,
        config: &'a ChordsGroup<'a, T>,
        queued: &mut Queue,
        action_queue: &mut ActionQueue<'a, T>,
    ) -> Option<(WaitingAction, &'a Action<'a, T>, PressedQueue)> {
        if queued.len() as u8 == self.prev_queue_len && self.timeout.saturating_sub(self.delay) > 0
        {
            // Fast path: nothing has changed since last tick and we haven't timed out yet.
            return None;
        }
        self.prev_queue_len = queued.len() as u8;

        // need to keep track of how many Press events we handled so we can filter them out later
        let mut handled_press_events = 0;
        let start_chord_coord = self.coord;
        let mut released_coord = None;

        // Compute the set of chord keys that are currently pressed
        // `Ok` when chording mode may continue
        // `Err` when it should end for various reasons
        let active = queued
            .iter()
            .try_fold(config.get_keys(self.coord).unwrap_or(0), |active, s| {
                if self.delay.saturating_sub(s.since) > self.timeout {
                    Ok(active)
                } else if let Some(chord_keys) = config.get_keys(s.event.coord()) {
                    match s.event {
                        Event::Press(_, _) => {
                            handled_press_events += 1;
                            Ok(active | chord_keys)
                        }
                        Event::Release(i, j) => {
                            // release chord quickly by changing the coordinate to the released
                            // key, to be consistent with chord decomposition behaviour.
                            released_coord = Some((i, j));
                            Err(active)
                        }
                    }
                } else if matches!(s.event, Event::Press(..)) {
                    Err(active) // pressed a non-chord key, abort
                } else {
                    Ok(active)
                }
            })
            .and_then(|active| {
                if self.timeout.saturating_sub(self.delay) == 0 {
                    Err(active) // timeout expired, abort
                } else {
                    Ok(active)
                }
            });

        let res = match active {
            Ok(active) => {
                // Chording mode still active, only trigger action if it's unambiguous
                if let Some(action) = config.get_chord_if_unambiguous(active) {
                    if let Some(coord) = released_coord {
                        self.coord = coord;
                    }
                    (WaitingAction::Tap, action)
                } else {
                    return None; // nothing to do yet, we'll check back later
                }
            }
            Err(active) => {
                // Abort chording mode. Trigger a chord action if there is one.
                if let Some(action) = config.get_chord(active) {
                    if let Some(coord) = released_coord {
                        self.coord = coord;
                    }
                    (WaitingAction::Tap, action)
                } else {
                    self.decompose_chord_into_action_queue(config, queued, action_queue);
                    (WaitingAction::NoOp, &Action::NoOp)
                }
            }
        };

        let mut pq = PressedQueue::new();
        let _ = pq.push_back(start_chord_coord);

        // Return all press events that were logically handled by this chording event
        queued.retain(|s| {
            if self.delay.saturating_sub(s.since) > self.timeout {
                true
            } else if matches!(s.event, Event::Press(i, j) if config.get_keys((i, j)).is_some())
                && handled_press_events > 0
            {
                handled_press_events -= 1;
                let _ = pq.push_back(s.event().coord());
                false
            } else {
                true
            }
        });

        Some((res.0, res.1, pq))
    }

    fn decompose_chord_into_action_queue(
        &mut self,
        config: &'a ChordsGroup<'a, T>,
        queued: &Queue,
        action_queue: &mut ActionQueue<'a, T>,
    ) {
        let mut chord_key_order = [0u128; ChordKeys::BITS as usize];

        // Default to the initial coordinate. But if a key is released early (before the timeout
        // occurs), use that key for action releases. That way the chord is released as early as
        // possible.
        let mut default_associated_coord = self.coord;

        let starting_mask = config.get_keys(self.coord).unwrap_or(0);
        let mut mask_bits_set = 1;
        chord_key_order[0] = starting_mask;
        let _ = queued.iter().try_fold(starting_mask, |active, s| {
            if self.delay.saturating_sub(s.since) > self.timeout {
                Ok(active)
            } else if let Some(chord_keys) = config.get_keys(s.event.coord()) {
                match s.event {
                    Event::Press(..) => {
                        if active | chord_keys != active {
                            chord_key_order[mask_bits_set] = chord_keys;
                            mask_bits_set += 1;
                        }
                        Ok(active | chord_keys)
                    }
                    Event::Release(i, j) => {
                        default_associated_coord = (i, j);
                        Err(active) // released a chord key, abort
                    }
                }
            } else if matches!(s.event, Event::Press(..)) {
                Err(active) // pressed a non-chord key, abort
            } else {
                Ok(active)
            }
        });
        let len = mask_bits_set;
        let chord_keys = &chord_key_order[0..len];

        let get_coord_for_chord = |mask: ChordKeys| -> (u8, u16) {
            if config.get_keys(default_associated_coord).unwrap_or(0) & mask > 0 {
                // This might be a release.
                // If it belongs to the associated action, prefer to use it.
                return default_associated_coord;
            }
            if self.coord != default_associated_coord
                && config.get_keys(self.coord).unwrap_or(0) & mask > 0
            {
                // The first coordinate not in queued
                // so must be explicitly checked if it is not the default coord.
                return self.coord;
            }
            queued
                .iter()
                .find_map(|q| {
                    let coord = q.event.coord();
                    let qmask = config.get_keys(coord).unwrap_or(0);
                    match qmask & mask {
                        0 => None,
                        _ => Some(coord),
                    }
                })
                .unwrap_or(default_associated_coord)
        };

        // Compute actions using the following description:
        //
        // Let's say we have a chord group with keys (h j k l). The full set (h j k l) is not
        // defined with an action, but the user has pressed all of h, j, k, l in the listed order,
        // so now kanata needs to break down the combo. How should it work?
        //
        // Figuratively "release" keys in reverse-temporal order until a valid chord is found. So
        // first, l is figuratively released, and if (h j k) is a valid chord, that action will
        // activate. If (l) by itself is valid that then activates after (h j k) is finished.
        //
        // In the case that (h j k) is not a chord, instead activate (h j). If that is a valid
        // chord, then try to activate (k l) together, and if not, evaluate (k), then (l).
        //
        // If (h j) is not a valid chord, try to activate (h). Then try to activate (j k l). If
        // that is invalid, try (j k), then (j). If (j k) is valid, try (l). If (j) is valid, try
        // (k l). If (k l) is invalid, try (k) then (l).
        //
        // The possible executions, listed in descending order of priority (first listed has
        // highest execution priority) are:
        // (h   j   k   l)
        // (h   j   k) (l)
        // (h   j) (k   l)
        // (h   j) (k) (l)
        // (h) (j   k   l)
        // (h) (j   k) (l)
        // (h) (j) (k   l)
        // (h) (j) (k) (l)

        let mut start = 0;
        let mut end = len;
        let delay = self.delay + self.ticks;
        while start < len {
            let sub_chord = &chord_keys[start..end];
            let chord_mask = sub_chord
                .iter()
                .copied()
                .reduce(|acc, e| acc | e)
                .unwrap_or(0);
            if let Some(action) = config.get_chord(chord_mask) {
                let coord = get_coord_for_chord(chord_mask);
                let _ = action_queue.push_back(Some((coord, delay, action)));
            } else {
                end -= 1;
                // shrink from end until something is found, or have checked up to and including
                // the individual start key.
                while end > start {
                    let sub_chord = &chord_keys[start..end];
                    let chord_mask = sub_chord
                        .iter()
                        .copied()
                        .reduce(|acc, e| acc | e)
                        .unwrap_or(0);
                    if let Some(action) = config.get_chord(chord_mask) {
                        let coord = get_coord_for_chord(chord_mask);
                        let _ = action_queue.push_back(Some((coord, delay, action)));
                        break;
                    }
                    end -= 1;
                }
            }
            start = if end <= start { start + 1 } else { end };
            end = len;
        }
    }

    fn is_corresponding_release(&self, event: &Event) -> bool {
        matches!(event, Event::Release(i, j) if (*i, *j) == self.coord)
    }

    fn is_corresponding_press(&self, event: &Event) -> bool {
        matches!(event, Event::Press(i, j) if (*i, *j) == self.coord)
    }
}

type OneShotCoords = ArrayDeque<KCoord, ONE_SHOT_MAX_ACTIVE, arraydeque::behavior::Wrapping>;

#[derive(Debug, Copy, Clone)]
pub struct SequenceState<'a, T: 'a> {
    cur_event: Option<SequenceEvent<'a, T>>,
    delay: u32,              // Keeps track of SequenceEvent::Delay time remaining
    tapped: Option<KeyCode>, // Keycode of a key that should be released at the next tick
    remaining_events: &'a [SequenceEvent<'a, T>],
}

type ReleasedOneShotKeys = Vec<KCoord, ONE_SHOT_MAX_ACTIVE>;

// Using a u16 for indices instead of usize.
// Need to check against this value in code that creates layers.
pub const MAX_LAYERS: usize = 60000;

// Use heapless Vec for perf - avoid pointer indirections.
// Use u16 for more efficient cache. 12*u16 = 3*u64 = 24 bytes.
// Then there is a usize for the length, totaling 32 bytes.
// Cache line is typically 64 bytes, so this takes half a cache line.
// Above all assumes x86-64.
pub const MAX_ACTIVE_LAYERS: usize = 12;

/// Because we only need a read-only stack and efficient iteration over contained
/// items, LayerStack items are in reverse order over usual back-to-front order
/// of items in array-based stack implementations.
type LayerStack = Vec<u16, MAX_ACTIVE_LAYERS>;

/// Contains the state of one shot keys that are currently active.
pub struct OneShotState {
    /// KCoordinates of one shot keys that are active
    pub keys: ArrayDeque<KCoord, ONE_SHOT_MAX_ACTIVE, arraydeque::behavior::Wrapping>,
    /// KCoordinates of one shot keys that have been released
    pub released_keys: ArrayDeque<KCoord, ONE_SHOT_MAX_ACTIVE, arraydeque::behavior::Wrapping>,
    /// Used to keep track of already-pressed keys for the release variants.
    pub other_pressed_keys: ArrayDeque<KCoord, ONE_SHOT_MAX_ACTIVE, arraydeque::behavior::Wrapping>,
    /// Timeout (ms) after which all one shot keys expire
    pub timeout: u16,
    /// Contains the end config of the most recently pressed one shot key
    pub end_config: OneShotEndConfig,
    /// Marks if release of the one shot keys should be done on the next tick
    pub release_on_next_tick: bool,
    /// The number of ticks to delay the release of the one-shot activation
    /// for EndOnFirstPress(OrRepress).
    /// This used to not exist and effectively be 1 (1ms),
    /// but that is too short for some environments.
    /// When too short, applications or desktop environments process
    /// the key release before the next press,
    /// even if temporally the release was sent after.
    pub pause_input_processing_delay: u16,
    /// If pause_input_processing_delay is used, this will be >0,
    /// meaning input processing should be paused to prevent extra presses
    /// from coming in while OneShot has not yet been released.
    ///
    /// May also be reused for other purposes...
    pub pause_input_processing_ticks: u16,

    /// Number of ticks to ignore press events for.
    pub ticks_to_ignore_events: u16,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum OneShotHandlePressKey {
    OneShotKey(KCoord),
    Other(KCoord),
}

impl OneShotState {
    fn tick_osh(&mut self) -> Option<ReleasedOneShotKeys> {
        if self.keys.is_empty() {
            return None;
        }
        self.ticks_to_ignore_events = self.ticks_to_ignore_events.saturating_sub(1);
        self.timeout = self.timeout.saturating_sub(1);
        if self.release_on_next_tick || self.timeout == 0 {
            self.release_on_next_tick = false;
            self.timeout = 0;
            self.pause_input_processing_ticks = 0;
            self.ticks_to_ignore_events = 0;
            self.keys.clear();
            self.other_pressed_keys.clear();
            Some(self.released_keys.drain(..).collect())
        } else {
            None
        }
    }

    fn handle_press(&mut self, key: OneShotHandlePressKey) -> OneShotCoords {
        let mut oneshot_coords = ArrayDeque::new();
        if self.keys.is_empty() || self.ticks_to_ignore_events > 0 {
            return oneshot_coords;
        }
        match key {
            OneShotHandlePressKey::OneShotKey(pressed_coord) => {
                if matches!(
                    self.end_config,
                    OneShotEndConfig::EndOnFirstReleaseOrRepress
                        | OneShotEndConfig::EndOnFirstPressOrRepress
                ) && self.keys.contains(&pressed_coord)
                {
                    self.release_on_next_tick = true;
                    oneshot_coords.extend(self.keys.iter().copied());
                }
                self.released_keys.retain(|coord| *coord != pressed_coord);
            }
            OneShotHandlePressKey::Other(pressed_coord) => {
                if matches!(
                    self.end_config,
                    OneShotEndConfig::EndOnFirstPress | OneShotEndConfig::EndOnFirstPressOrRepress
                ) {
                    self.timeout = core::cmp::min(self.pause_input_processing_delay, self.timeout);
                    self.pause_input_processing_ticks = self.pause_input_processing_delay;
                } else {
                    let _ = self.other_pressed_keys.push_back(pressed_coord);
                }
                oneshot_coords.extend(self.keys.iter().copied());
            }
        };
        oneshot_coords
    }

    /// Returns true if the caller should handle the release normally and false otherwise.
    /// The second value in the tuple represents an overflow of released one shot keys and should
    /// be released is it is `Some`.
    fn handle_release(&mut self, (i, j): KCoord) -> (bool, Option<KCoord>) {
        if self.keys.is_empty() {
            return (true, None);
        }
        if !self.keys.contains(&(i, j)) {
            if matches!(
                self.end_config,
                OneShotEndConfig::EndOnFirstRelease | OneShotEndConfig::EndOnFirstReleaseOrRepress
            ) && self.other_pressed_keys.contains(&(i, j))
            {
                self.release_on_next_tick = true;
            }
            (true, None)
        } else {
            // delay release for one shot keys
            (false, self.released_keys.push_back((i, j)))
        }
    }
}

/// An iterator over the currently queued events.
///
/// Events can be retrieved by iterating over this struct and calling [Queued::event].
#[derive(Clone)]
pub struct QueuedIter<'a>(arraydeque::Iter<'a, Queued>);

impl<'a> Iterator for QueuedIter<'a> {
    type Item = &'a Queued;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// An event, waiting in a queue to be processed.
#[derive(Debug, Copy, Clone)]
pub struct Queued {
    pub(crate) event: Event,
    pub(crate) since: u16,
}
impl From<Event> for Queued {
    fn from(event: Event) -> Self {
        Queued { event, since: 0 }
    }
}
impl Queued {
    pub(crate) fn new_press(i: u8, j: u16) -> Self {
        Self {
            since: 0,
            event: Event::Press(i, j),
        }
    }

    pub(crate) fn new_release(i: u8, j: u16) -> Self {
        Self {
            since: 0,
            event: Event::Release(i, j),
        }
    }

    pub(crate) fn tick_qd(&mut self) {
        self.since = self.since.saturating_add(1);
    }

    /// Get the [Event] from this object.
    pub fn event(&self) -> Event {
        self.event
    }
}

#[derive(Default)]
pub struct LastPressTracker {
    pub coord: KCoord,
    pub tap_hold_timeout: u16,
}

impl LastPressTracker {
    fn tick_lpt(&mut self) {
        self.tap_hold_timeout = self.tap_hold_timeout.saturating_sub(1);
    }
    fn update_coord(&mut self, coord: KCoord) {
        if coord.0 == REAL_KEY_ROW {
            // Only update if it's a real key press.
            self.coord = coord;
        }
    }
}

impl<'a, const C: usize, const R: usize, T: 'a + Copy + std::fmt::Debug> Layout<'a, C, R, T> {
    /// Creates a new `Layout` object.
    fn new(layers: &'a [[[Action<T>; C]; R]]) -> Self {
        assert!(layers.len() < MAX_LAYERS);
        Self {
            src_keys: &[Action::NoOp; C],
            layers,
            default_layer: 0,
            states: Vec::new(),
            waiting: None,
            extra_waiting: ArrayDeque::new(),
            tap_dance_eager: None,
            queue: ArrayDeque::new(),
            oneshot: OneShotState {
                timeout: 0,
                end_config: OneShotEndConfig::EndOnFirstPress,
                keys: ArrayDeque::new(),
                released_keys: ArrayDeque::new(),
                other_pressed_keys: ArrayDeque::new(),
                release_on_next_tick: false,
                pause_input_processing_delay: 0,
                pause_input_processing_ticks: 0,
                ticks_to_ignore_events: 0,
            },
            last_press_tracker: Default::default(),
            active_sequences: ArrayDeque::new(),
            action_queue: ArrayDeque::new(),
            rpt_action: None,
            historical_keys: History::new(),
            historical_inputs: History::new(),
            rpt_multikey_key_buffer: unsafe { MultiKeyBuffer::new() },
            quick_tap_hold_timeout: false,
            trans_resolution_behavior_v2: true,
            delegate_to_first_layer: false,
            chords_v2: None,
        }
    }
    pub fn new_with_trans_action_settings(
        src_keys: &'a [Action<T>; C],
        layers: &'a [[[Action<T>; C]; R]],
        trans_resolution_behavior_v2: bool,
        delegate_to_first_layer: bool,
    ) -> Self {
        let mut new = Self::new(layers);
        new.src_keys = src_keys;
        new.trans_resolution_behavior_v2 = trans_resolution_behavior_v2;
        new.delegate_to_first_layer = delegate_to_first_layer;
        new
    }

    /// Iterates on the key codes of the current state.
    pub fn keycodes(&self) -> impl Iterator<Item = KeyCode> + Clone + '_ {
        self.states.iter().filter_map(State::keycode)
    }
    fn waiting_into_hold(&mut self, idx: i8) -> CustomEvent<'a, T> {
        let waiting = if idx < 0 {
            self.waiting.as_ref()
        } else {
            self.extra_waiting.get(idx as usize)
        };
        if let Some(w) = waiting {
            let hold = w.hold;
            let coord = w.coord;
            let delay = match w.config {
                WaitingConfig::HoldTap(..) | WaitingConfig::Chord(_) => w.delay + w.ticks,
                WaitingConfig::TapDance(_) => 0,
            };
            let layer_stack = w.layer_stack.clone();
            if idx < 0 {
                self.waiting = None;
            } else {
                self.extra_waiting.remove(idx as usize);
            }
            if coord == self.last_press_tracker.coord {
                self.last_press_tracker.tap_hold_timeout = 0;
            }
            // Similar issue happens for the quick tap-hold tap as with on-press release;
            // the rapidity of the release can cause issues. See pause_input_processing_delay
            // comments for more detail.
            self.oneshot.pause_input_processing_ticks = self.oneshot.pause_input_processing_delay;
            self.do_action(hold, coord, delay, false, &mut layer_stack.into_iter())
        } else {
            CustomEvent::NoEvent
        }
    }
    fn waiting_into_tap(&mut self, pq: Option<PressedQueue>, idx: i8) -> CustomEvent<'a, T> {
        let waiting = if idx < 0 {
            self.waiting.as_ref()
        } else {
            self.extra_waiting.get(idx as usize)
        };
        if let Some(w) = waiting {
            let tap = w.tap;
            let coord = w.coord;
            let delay = match w.config {
                WaitingConfig::HoldTap(..) | WaitingConfig::Chord(_) => w.delay + w.ticks,
                WaitingConfig::TapDance(_) => 0,
            };
            let layer_stack = w.layer_stack.clone();
            if idx < 0 {
                self.waiting = None;
            } else {
                self.extra_waiting.remove(idx as usize);
            }
            let ret = self.do_action(
                tap,
                coord,
                delay,
                false,
                &mut layer_stack.clone().into_iter(),
            );
            if let Some(pq) = pq {
                match tap {
                    Action::KeyCode(_)
                    | Action::MultipleKeyCodes(_)
                    | Action::OneShot(_)
                    | Action::Layer(_) => {
                        // The current intent of this block is to ensure that simple actions like
                        // key presses or layer-while-held remain pressed as long as a single key from
                        // the input chord remains held. The behaviour of these actions is correct in
                        // the case of repeating do_action, so there is currently no harm in doing
                        // this. Other action types are more problematic though.
                        for other_coord in pq.iter().copied() {
                            self.do_action(
                                tap,
                                other_coord,
                                delay,
                                false,
                                &mut layer_stack.clone().into_iter(),
                            );
                        }
                    }
                    Action::MultipleActions(acs) => {
                        // Like above block, but for the same simple actions within MultipleActions
                        for ac in acs.iter() {
                            if matches!(
                                ac,
                                Action::KeyCode(_)
                                    | Action::MultipleKeyCodes(_)
                                    | Action::OneShot(_)
                                    | Action::Layer(_)
                            ) {
                                for other_coord in pq.iter().copied() {
                                    self.do_action(
                                        ac,
                                        other_coord,
                                        delay,
                                        false,
                                        &mut layer_stack.clone().into_iter(),
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            // Similar issue happens for the quick tap-hold tap as with on-press release;
            // the rapidity of the release can cause issues. See pause_input_processing_delay
            // comments for more detail.
            self.oneshot.pause_input_processing_ticks = self.oneshot.pause_input_processing_delay;
            ret
        } else {
            CustomEvent::NoEvent
        }
    }
    fn waiting_into_timeout(&mut self, idx: i8) -> CustomEvent<'a, T> {
        let waiting = if idx < 0 {
            self.waiting.as_ref()
        } else {
            self.extra_waiting.get(idx as usize)
        };
        if let Some(w) = waiting {
            let timeout_action = w.timeout_action;
            let coord = w.coord;
            let delay = match w.config {
                WaitingConfig::HoldTap(..) | WaitingConfig::Chord(_) => w.delay + w.ticks,
                WaitingConfig::TapDance(_) => 0,
            };
            let layer_stack = w.layer_stack.clone();
            if idx < 0 {
                self.waiting = None;
            } else {
                self.extra_waiting.remove(idx as usize);
            }
            if coord == self.last_press_tracker.coord {
                self.last_press_tracker.tap_hold_timeout = 0;
            }
            self.do_action(
                timeout_action,
                coord,
                delay,
                false,
                &mut layer_stack.into_iter(),
            )
        } else {
            CustomEvent::NoEvent
        }
    }
    fn drop_waiting(&mut self) -> CustomEvent<'a, T> {
        self.waiting = None;
        CustomEvent::NoEvent
    }
    /// A time event.
    ///
    /// This method must be called regularly, typically every millisecond.
    ///
    /// Returns the corresponding `CustomEvent`, allowing to manage
    /// custom actions thanks to the `Action::Custom` variant.
    pub fn tick(&mut self) -> CustomEvent<'a, T> {
        let active_layer = self.current_layer() as u16;
        if let Some(chv2) = self.chords_v2.as_mut() {
            self.queue.extend(chv2.tick_chv2(active_layer).drain(0..));
            if let chord_action @ Some(_) = chv2.get_action_chv2() {
                self.action_queue.push_back(chord_action);
                self.oneshot.pause_input_processing_ticks =
                    self.oneshot.pause_input_processing_delay;
            }
        }
        if let Some(Some((coord, delay, action))) = self.action_queue.pop_front() {
            // If there's anything in the action queue, don't process anything else yet - execute
            // everything. Otherwise an action may never be released.
            return self.do_action(
                action,
                coord,
                delay,
                false,
                &mut self.trans_resolution_layer_order().into_iter().skip(1),
            );
        }
        self.queue.iter_mut().for_each(Queued::tick_qd);
        self.last_press_tracker.tick_lpt();
        if let Some(ref mut tde) = self.tap_dance_eager {
            tde.tick_tde();
            if tde.is_expired() {
                self.tap_dance_eager = None;
            }
        }
        self.process_sequences();

        self.historical_keys.tick_hist();
        self.historical_inputs.tick_hist();

        let mut custom = CustomEvent::NoEvent;
        if let Some(released_keys) = self.oneshot.tick_osh() {
            for key in released_keys.iter() {
                custom.update(self.dequeue(Queued {
                    event: Event::Release(key.0, key.1),
                    since: 0,
                }));
            }
        }

        custom.update(match &mut self.waiting {
            Some(w) => match w.tick_wt(&mut self.queue, &mut self.action_queue) {
                Some((WaitingAction::Hold, _)) => self.waiting_into_hold(-1),
                Some((WaitingAction::Tap, pq)) => self.waiting_into_tap(pq, -1),
                Some((WaitingAction::Timeout, _)) => self.waiting_into_timeout(-1),
                Some((WaitingAction::NoOp, _)) => self.drop_waiting(),
                None => CustomEvent::NoEvent,
            },
            None => {
                if self.extra_waiting.is_empty() {
                    // Due to the possible delay in the key release for EndOnFirstPress
                    // because some apps/DEs do not handle it properly if done too quickly,
                    // undesirable behaviour of extra presses making it in before
                    // the release happens might occur.
                    //
                    // A mitigation against that is to pause input processing.
                    if self.oneshot.pause_input_processing_ticks > 0 {
                        self.oneshot.pause_input_processing_ticks =
                            self.oneshot.pause_input_processing_ticks.saturating_sub(1);
                        CustomEvent::NoEvent
                    } else {
                        match self.queue.pop_front() {
                            Some(s) => self.dequeue(s),
                            None => CustomEvent::NoEvent,
                        }
                    }
                } else {
                    CustomEvent::NoEvent
                }
            }
        });
        let custom = self.process_extra_waitings(custom);
        self.process_sequence_custom(custom)
    }
    /// Takes care of draining and populating the `active_sequences` ArrayDeque,
    /// giving us sequences (aka macros) of nearly limitless length!
    fn process_sequences(&mut self) {
        // Iterate over all active sequence events
        for _ in 0..self.active_sequences.len() {
            if let Some(mut seq) = self.active_sequences.pop_front() {
                // If we've encountered a SequenceEvent::Delay we must count
                // that down completely before doing anything else...
                if seq.delay > 0 {
                    seq.delay = seq.delay.saturating_sub(1);
                } else if let Some(keycode) = seq.tapped {
                    // Clear out the Press() matching this Tap()'s keycode
                    self.states.retain(|s| s.seq_release(keycode).is_some());
                    seq.tapped = None;
                } else {
                    // Pull the next SequenceEvent
                    if let [e, tail @ ..] = seq.remaining_events {
                        seq.cur_event = Some(*e);
                        seq.remaining_events = tail;
                    }
                    // Process it (SequenceEvent)
                    match seq.cur_event {
                        Some(SequenceEvent::Complete) => {
                            seq.remaining_events = &[];
                        }
                        Some(SequenceEvent::Press(keycode)) => {
                            // Start tracking this fake key Press() event
                            let _ = self.states.push(FakeKey { keycode });
                            self.historical_keys.push_front(keycode);
                            // Fine to fake (0, 0). This is sequences anyway. In Kanata, nothing
                            // valid should be at (0, 0) that this would interfere with.
                            self.oneshot
                                .handle_press(OneShotHandlePressKey::Other((0, 0)));
                        }
                        Some(SequenceEvent::Tap(keycode)) => {
                            // Same as Press() except we track it for one tick via seq.tapped:
                            let _ = self.states.push(FakeKey { keycode });
                            self.historical_keys.push_front(keycode);
                            self.oneshot
                                .handle_press(OneShotHandlePressKey::Other((0, 0)));
                            seq.tapped = Some(keycode);
                        }
                        Some(SequenceEvent::Release(keycode)) => {
                            // Nothing valid should be at (0, 0). It's fine to fake this.
                            self.oneshot.handle_release((0, 0));
                            self.states.retain(|s| s.seq_release(keycode).is_some());
                        }
                        Some(SequenceEvent::Delay { duration }) => {
                            // Setup a delay that will be decremented once per tick until 0
                            if duration > 0 {
                                // -1 to start since this tick counts
                                seq.delay = duration - 1;
                            }
                        }
                        Some(SequenceEvent::Custom(custom)) => {
                            let _ = self.states.push(State::SeqCustomPending(custom));
                        }
                        _ => {} // We'll never get here
                    }
                }
                if !seq.remaining_events.is_empty() {
                    // Put it back
                    self.active_sequences.push_back(seq);
                }
            }
        }
        if self.active_sequences.is_empty() {
            // Push only the latest pressed repeating macro.
            if let Some(State::RepeatingSequence { sequence, .. }) = self
                .states
                .iter()
                .rev()
                .find(|s| matches!(s, State::RepeatingSequence { .. }))
            {
                self.active_sequences.push_back(SequenceState {
                    cur_event: None,
                    delay: 0,
                    tapped: None,
                    remaining_events: sequence,
                });
            }
        }
    }

    fn process_extra_waitings(&mut self, current_custom: CustomEvent<'a, T>) -> CustomEvent<'a, T> {
        if !matches!(current_custom, CustomEvent::NoEvent) {
            return current_custom;
        }
        let mut waiting_action = (0, None);
        for (i, w) in self.extra_waiting.iter_mut().enumerate() {
            match w.tick_wt(&mut self.queue, &mut self.action_queue) {
                None => {}
                wa => {
                    waiting_action = (i as isize, wa);
                    // break - only complete one at a time even if potentially multiple have
                    // completed, so that only one custom event is returned.
                    //
                    // Theoretically if we could call the waiting_into_* functions, we could do that
                    // here and break only if custom is None, but that runs into mutability
                    // problems. I don't expect any perceptible degradation between from not doing
                    // the above.
                    break;
                }
            }
        }
        let i = waiting_action.0;
        match waiting_action.1 {
            Some((WaitingAction::Hold, _)) => self.waiting_into_hold(i as i8),
            Some((WaitingAction::Tap, pq)) => self.waiting_into_tap(pq, i as i8),
            Some((WaitingAction::Timeout, _)) => self.waiting_into_timeout(i as i8),
            Some((WaitingAction::NoOp, _)) => self.drop_waiting(),
            None => current_custom,
        }
    }

    fn process_sequence_custom(
        &mut self,
        mut current_custom: CustomEvent<'a, T>,
    ) -> CustomEvent<'a, T> {
        if self.states.is_empty() || !matches!(current_custom, CustomEvent::NoEvent) {
            return current_custom;
        }
        self.states.retain(|s| !matches!(s, State::Tombstone));
        for state in self.states.iter_mut() {
            match state {
                State::SeqCustomPending(custom) => {
                    current_custom.update(CustomEvent::Press(custom));
                    *state = State::SeqCustomActive(custom);
                    break;
                }
                State::SeqCustomActive(custom) => {
                    current_custom.update(CustomEvent::Release(custom));
                    *state = State::Tombstone;
                    break;
                }
                _ => continue,
            };
        }
        current_custom
    }
    fn dequeue(&mut self, queue: Queued) -> CustomEvent<'a, T> {
        use Event::*;
        match queue.event {
            Release(i, j) => {
                let mut custom = CustomEvent::NoEvent;
                let (do_release, overflow_key) = self.oneshot.handle_release((i, j));
                if do_release {
                    self.states.retain(|s| {
                        !s.clear_on_next_release() && s.release((i, j), &mut custom).is_some()
                    });
                }
                if let Some((i2, j2)) = overflow_key {
                    self.states
                        .retain(|s| s.release((i2, j2), &mut custom).is_some());
                }
                custom
            }

            Press(i, j) => {
                let mut layer_stack = self.trans_resolution_layer_order().into_iter();
                if let Some(tde) = self.tap_dance_eager {
                    if (i, j) == self.last_press_tracker.coord && !tde.is_expired() {
                        let custom = self.do_action(
                            tde.actions[usize::from(tde.num_taps)],
                            (i, j),
                            queue.since,
                            false,
                            &mut layer_stack.skip(1),
                        );
                        // unwrap is here because tde cannot be ref mut
                        self.tap_dance_eager.as_mut().expect("some").incr_taps();
                        custom
                    } else {
                        // i == 0 means real key, i == 1 means fake key. Let fake keys do whatever, but
                        // interrupt tap-dance-eager if real key.
                        if i == REAL_KEY_ROW {
                            // unwrap is here because tde cannot be ref mut
                            self.tap_dance_eager.as_mut().expect("some").set_expired();
                        }
                        self.do_action(&Action::Trans, (i, j), queue.since, false, &mut layer_stack)
                    }
                } else {
                    self.do_action(&Action::Trans, (i, j), queue.since, false, &mut layer_stack)
                }
            }
        }
    }
    /// Register a key event.
    pub fn event(&mut self, event: Event) {
        if let Event::Press(x, y) = event {
            self.historical_inputs.push_front((x, y));
        }
        if let Some(overflow) = if let Some(ch) = self.chords_v2.as_mut() {
            ch.push_back_chv2(event.into())
        } else {
            self.queue.push_back(event.into())
        } {
            for i in -1..(EXTRA_WAITING_LEN as i8) {
                self.waiting_into_hold(i);
            }
            self.dequeue(overflow);
        }
    }
    /// Resolve coordinate to first non-Trans actions.
    /// Trans on base layer, resolves to key from defsrc.
    fn resolve_coord(
        &self,
        coord: KCoord,
        layer_stack: &mut (impl Iterator<Item = u16> + Clone),
    ) -> &'a Action<'a, T> {
        use crate::action::Action::*;
        let x = coord.0 as usize;
        let y = coord.1 as usize;
        assert!(x <= self.layers[0].len());
        assert!(y <= self.layers[0][0].len());
        for layer in layer_stack {
            assert!(usize::from(layer) <= self.layers.len());
            let action = &self.layers[usize::from(layer)][x][y];
            match action {
                Trans => continue,
                action => return action,
            }
        }
        if x == 0 {
            &self.src_keys[y]
        } else {
            &NoOp
        }
    }
    fn do_action(
        &mut self,
        action: &'a Action<'a, T>,
        coord: KCoord,
        delay: u16,
        is_oneshot: bool,
        layer_stack: &mut (impl Iterator<Item = u16> + Clone), // used to resolve Trans action
    ) -> CustomEvent<'a, T> {
        let mut action = action;
        if let Trans = action {
            action = self.resolve_coord(coord, layer_stack);
        }
        let action = action;

        if self.last_press_tracker.coord != coord {
            self.last_press_tracker.tap_hold_timeout = 0;
        }
        use Action::*;
        self.states.retain(|s| match s {
            NormalKey { flags, .. } => !flags.nkf_clear_on_next_action(),
            _ => true,
        });
        match action {
            NoOp => {
                // There is an interaction between oneshot and chordsv2 here.
                // chordsv2 sends fake queued press/release events at the coordinate level in order
                // to trigger other "waiting" style actions, namely tap-hold. However, these can
                // potentially interfere with oneshot by triggering early oneshot activation. This
                // is resolved by ignoring actions at the coordinate at which the fake events are
                // sent.
                if !is_oneshot && coord != TRIGGER_TAPHOLD_COORD {
                    self.oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
                self.rpt_action = Some(action);
            }
            Src => {
                let action = &self.src_keys[usize::from(coord.1)];
                // Risk: infinite recursive resulting in stack overflow.
                // In practice this is not expected to happen.
                // The `src_keys` actions are all expected to be `KeyCode` or `NoOp` actions.
                self.do_action(action, coord, delay, is_oneshot, &mut std::iter::empty());
            }
            Trans => {
                // Transparent action should be resolved to non-transparent one near the top
                // of `do_action`.
                unreachable!("Trans action should have been resolved earlier")
            }
            Repeat => {
                // Notes around repeat:
                //
                // Though this action seems conceptually simple, in reality there are a lot of
                // decisions to be made around how exactly actions repeat. For example: in a
                // tap-dance action, would one expect the tap-dance to be repeated or the inner
                // action that was most activated within the tap-dance?
                //
                // Currently the answer to these questions is: what is easy/possible to do? E.g.
                // fork and switch are inconsistent with each other even though the actions are
                // conceptually very similar. This is because switch can potentially activate
                // multiple actions (but not always), so uses the action queue, while fork does
                // not. As another example, tap-dance and tap-hold will repeat the inner action and
                // not the outer (tap-dance|hold) but multi will repeat the entire outer multi
                // action.
                if let Some(ac) = self.rpt_action {
                    self.do_action(ac, coord, delay, is_oneshot, &mut std::iter::empty());
                }
            }
            HoldTap(HoldTapAction {
                timeout,
                hold,
                tap,
                timeout_action,
                config,
                tap_hold_interval,
            }) => {
                let mut custom = CustomEvent::NoEvent;
                if *tap_hold_interval == 0
                    || coord != self.last_press_tracker.coord
                    || self.last_press_tracker.tap_hold_timeout == 0
                {
                    let waiting: WaitingState<T> = WaitingState {
                        coord,
                        timeout: if self.quick_tap_hold_timeout {
                            timeout.saturating_sub(delay)
                        } else {
                            *timeout
                        },
                        delay: if self.quick_tap_hold_timeout {
                            // Note: don't want to double-count this.
                            0
                        } else {
                            delay
                        },
                        ticks: 0,
                        hold,
                        tap,
                        timeout_action,
                        config: WaitingConfig::HoldTap(*config),
                        layer_stack: layer_stack.collect(),
                        prev_queue_len: QueueLen::MAX,
                    };
                    if self.waiting.is_some() {
                        self.extra_waiting.push_back(waiting);
                    } else {
                        self.waiting = Some(waiting);
                    }
                    self.last_press_tracker.tap_hold_timeout = *tap_hold_interval;
                } else {
                    self.last_press_tracker.tap_hold_timeout = 0;
                    custom.update(self.do_action(tap, coord, delay, is_oneshot, layer_stack));
                }
                // Need to set tap_hold_tracker coord AFTER the checks.
                self.last_press_tracker.update_coord(coord);
                return custom;
            }
            &OneShot(oneshot) => {
                self.last_press_tracker.update_coord(coord);
                let custom =
                    self.do_action(oneshot.action, coord, delay, true, &mut std::iter::empty());
                // Note - set rpt_action after doing the inner oneshot action. This means that the
                // whole oneshot will be repeated by rpt-any rather than only the inner action.
                self.rpt_action = Some(action);
                self.oneshot
                    .handle_press(OneShotHandlePressKey::OneShotKey(coord));
                self.oneshot.timeout = oneshot.timeout;
                self.oneshot.end_config = oneshot.end_config;
                if let Some(overflow) = self.oneshot.keys.push_back((coord.0, coord.1)) {
                    self.event(Event::Release(overflow.0, overflow.1));
                }
                return custom;
            }
            &OneShotIgnoreEventsTicks(ticks) => {
                self.last_press_tracker.update_coord(coord);
                self.rpt_action = Some(action);
                self.oneshot.ticks_to_ignore_events = ticks;
            }
            &TapDance(td) => {
                self.last_press_tracker.update_coord(coord);
                match td.config {
                    TapDanceConfig::Lazy => {
                        self.waiting = Some(WaitingState {
                            coord,
                            timeout: td.timeout,
                            delay,
                            ticks: 0,
                            hold: &Action::NoOp,
                            tap: &Action::NoOp,
                            timeout_action: &Action::NoOp,
                            config: WaitingConfig::TapDance(TapDanceState {
                                actions: td.actions,
                                timeout: td.timeout,
                                num_taps: 1,
                            }),
                            layer_stack: layer_stack.collect(),
                            prev_queue_len: QueueLen::MAX,
                        });
                    }
                    TapDanceConfig::Eager => {
                        match self.tap_dance_eager {
                            None => {
                                self.tap_dance_eager = Some(TapDanceEagerState {
                                    coord,
                                    actions: td.actions,
                                    timeout: td.timeout,
                                    orig_timeout: td.timeout,
                                    num_taps: 1,
                                })
                            }
                            Some(tde) => {
                                if tde.coord != coord {
                                    self.tap_dance_eager = Some(TapDanceEagerState {
                                        coord,
                                        actions: td.actions,
                                        timeout: td.timeout,
                                        orig_timeout: td.timeout,
                                        num_taps: 1,
                                    });
                                }
                            }
                        };
                        self.do_action(td.actions[0], coord, delay, false, layer_stack);
                    }
                }
            }
            &Chords(chords) => {
                self.last_press_tracker.update_coord(coord);
                self.waiting = Some(WaitingState {
                    coord,
                    timeout: chords.timeout,
                    delay,
                    ticks: 0,
                    hold: &Action::NoOp,
                    tap: &Action::NoOp,
                    timeout_action: &Action::NoOp,
                    config: WaitingConfig::Chord(chords),
                    layer_stack: layer_stack.collect(),
                    prev_queue_len: QueueLen::MAX,
                });
            }
            &KeyCode(keycode) => {
                self.last_press_tracker.update_coord(coord);
                // Most-recent-first!
                self.historical_keys.push_front(keycode);
                let _ = self.states.push(NormalKey {
                    coord,
                    keycode,
                    flags: NormalKeyFlags(0),
                });
                let mut oneshot_coords = ArrayDeque::new();
                if !is_oneshot {
                    oneshot_coords = self
                        .oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
                if oneshot_coords.is_empty() {
                    self.rpt_action = Some(action);
                } else {
                    self.rpt_action = None;
                    unsafe {
                        self.rpt_multikey_key_buffer.clear();
                        for kc in self
                            .states
                            .iter()
                            .filter_map(|kc| State::keycode_in_coords(kc, &oneshot_coords))
                        {
                            self.rpt_multikey_key_buffer.push(kc);
                        }
                        self.rpt_multikey_key_buffer.push(keycode);
                        self.rpt_action = Some(self.rpt_multikey_key_buffer.get_ref());
                    }
                }
            }
            &MultipleKeyCodes(v) => {
                self.last_press_tracker.update_coord(coord);
                for &keycode in *v {
                    self.historical_keys.push_front(keycode);
                    let _ = self.states.push(NormalKey {
                        coord,
                        keycode,
                        // In Kanata, this action is only ever used with output chords. Output
                        // chords within a one-shot are ignored because someone might do something
                        // like (one-shot C-S-lalt to get 3 modifiers. These are probably intended
                        // to remain held. However, other output chords are usually used to type
                        // symbols or accented characters, e.g. S-1 or RA-a. Clearing chord keys on
                        // the next action allows a subsequent typed key to not have modifiers
                        // alongside it. But if the symbol or accented character is held down, key
                        // repeat works just fine.
                        flags: NormalKeyFlags(if is_oneshot {
                            0
                        } else {
                            NORMAL_KEY_FLAG_CLEAR_ON_NEXT_ACTION
                        }),
                    });
                }

                let mut oneshot_coords = ArrayDeque::new();
                if !is_oneshot {
                    oneshot_coords = self
                        .oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
                if oneshot_coords.is_empty() {
                    self.rpt_action = Some(action);
                } else {
                    self.rpt_action = None;
                    unsafe {
                        self.rpt_multikey_key_buffer.clear();
                        for kc in self
                            .states
                            .iter()
                            .filter_map(|s| s.keycode_in_coords(&oneshot_coords))
                        {
                            self.rpt_multikey_key_buffer.push(kc);
                        }
                        for &keycode in *v {
                            self.rpt_multikey_key_buffer.push(keycode);
                        }
                        self.rpt_action = Some(self.rpt_multikey_key_buffer.get_ref());
                    }
                }
            }
            &MultipleActions(v) => {
                self.last_press_tracker.update_coord(coord);
                let mut custom = CustomEvent::NoEvent;
                for action in *v {
                    custom.update(self.do_action(
                        action,
                        coord,
                        delay,
                        is_oneshot,
                        &mut layer_stack.clone(),
                    ));
                }
                // Save the whole multi action instead of the final action in multi so that Repeat
                // repeats all of the actions in this multi.
                self.rpt_action = Some(action);
                return custom;
            }
            Sequence { events } => {
                self.active_sequences.push_back(SequenceState {
                    cur_event: None,
                    delay: 0,
                    tapped: None,
                    remaining_events: events,
                });
                if !is_oneshot {
                    self.oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
                self.rpt_action = Some(action);
            }
            RepeatableSequence { events } => {
                self.active_sequences.push_back(SequenceState {
                    cur_event: None,
                    delay: 0,
                    tapped: None,
                    remaining_events: events,
                });
                let _ = self.states.push(RepeatingSequence {
                    sequence: events,
                    coord,
                });
                if !is_oneshot {
                    self.oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
                self.rpt_action = Some(action);
            }
            CancelSequences => {
                // Clear any and all running sequences then clean up any leftover FakeKey events
                self.active_sequences.clear();
                for fake_key in self.states.clone().iter() {
                    if let FakeKey { keycode } = *fake_key {
                        self.states.retain(|s| s.seq_release(keycode).is_some());
                    }
                }
                if !is_oneshot {
                    self.oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
                self.rpt_action = Some(action);
            }
            &Layer(value) => {
                self.last_press_tracker.update_coord(coord);
                let _ = self.states.push(LayerModifier { value, coord });
                if !is_oneshot {
                    self.oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
                // Notably missing in Layer and below in DefaultLayer is setting rpt_action. This
                // is so that if the Repeat key is on a different layer than the base, it can still
                // be used to repeat the previous non-layer-changing action.
            }
            DefaultLayer(value) => {
                self.last_press_tracker.update_coord(coord);
                self.set_default_layer(*value);
                if !is_oneshot {
                    self.oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
            }
            Custom(value) => {
                self.last_press_tracker.update_coord(coord);
                if !is_oneshot {
                    self.oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
                self.rpt_action = Some(action);
                if self.states.push(State::Custom { value, coord }).is_ok() {
                    return CustomEvent::Press(value);
                }
            }
            ReleaseState(rs) => {
                self.states.retain(|s| s.release_state(*rs).is_some());
                if !is_oneshot {
                    self.oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
                self.rpt_action = Some(action);
            }
            Fork(fcfg) => {
                let ret = match self.states.iter().any(|s| match s {
                    NormalKey { keycode, .. } | FakeKey { keycode } => {
                        fcfg.right_triggers.contains(keycode)
                    }
                    _ => false,
                }) {
                    false => {
                        self.do_action(&fcfg.left, coord, delay, false, &mut layer_stack.clone())
                    }
                    true => {
                        self.do_action(&fcfg.right, coord, delay, false, &mut layer_stack.clone())
                    }
                };
                // Repeat the fork rather than the terminal action.
                self.rpt_action = Some(action);
                return ret;
            }
            Switch(sw) => {
                let active_keys = self.states.iter().filter_map(State::keycode);
                let active_coords = self.states.iter().filter_map(State::coord);
                let historical_keys = self.historical_keys.iter_hevents();
                let historical_coords = self.historical_inputs.iter_hevents();
                let layers = self.trans_resolution_layer_order().into_iter();
                let action_queue = &mut self.action_queue;
                for ac in sw.actions(
                    active_keys,
                    active_coords,
                    historical_keys,
                    historical_coords,
                    layers,
                    // Note on truncating cast: I expect default layer to be in range by other
                    // assertions.
                    self.default_layer as u16,
                ) {
                    action_queue.push_back(Some((coord, 0, ac)));
                }
                // Switch is not properly repeatable. This has to use the action queue for the
                // purpose of proper Custom action handling, because a single switch action can
                // activate multiple inner actions. But because of the use of the action queue,
                // switch has no way to set `rpt_action` after the queue is depleted. I suppose
                // that can be fixable, but for now will keep it as-is.
            }
        }
        CustomEvent::NoEvent
    }

    /// Obtain the index of the current active layer
    pub fn current_layer(&self) -> usize {
        self.states
            .iter()
            .rev()
            .find_map(State::get_layer)
            .unwrap_or(self.default_layer)
    }

    pub fn active_held_layers(&self) -> impl Iterator<Item = u16> + Clone + '_ {
        self.states
            .iter()
            .filter_map(|s| State::get_layer(s).map(|l| l as u16))
            .rev()
    }

    /// Returns a list indices of layers that should be used for [`Action::Trans`] resolution.
    pub fn trans_resolution_layer_order(&self) -> LayerStack {
        let current_layer = self.current_layer();
        if self.trans_resolution_behavior_v2 {
            let mut v = self.active_held_layers().collect::<LayerStack>();
            let _ = v.push(self.default_layer as u16);
            if self.delegate_to_first_layer && current_layer != 0 && self.default_layer != 0 {
                let _ = v.push(0);
            }
            v
        } else {
            let mut v = Vec::new();
            let _ = v.push(current_layer as u16);
            if self.delegate_to_first_layer && current_layer != 0 {
                let _ = v.push(0);
            }
            v
        }
    }

    /// Sets the default layer for the layout
    pub fn set_default_layer(&mut self, value: usize) {
        if value < self.layers.len() {
            self.default_layer = value
        }
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{Event::*, Layout, *};
    use crate::action::Action::*;
    use crate::action::HoldTapConfig;
    use crate::action::{k, l};
    use crate::key_code::KeyCode;
    use crate::key_code::KeyCode::*;
    use std::collections::BTreeSet;

    #[track_caller]
    fn assert_keys(expected: &[KeyCode], iter: impl Iterator<Item = KeyCode>) {
        let expected: BTreeSet<_> = expected.iter().copied().collect();
        let tested = iter.collect();
        assert_eq!(expected, tested);
    }

    #[test]
    fn basic_hold_tap() {
        static LAYERS: Layers<2, 1> = &[
            [[
                HoldTap(&HoldTapAction {
                    timeout: 200,
                    hold: l(1),
                    tap: k(Space),
                    timeout_action: k(RShift),
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 0,
                }),
                HoldTap(&HoldTapAction {
                    timeout: 200,
                    hold: k(LCtrl),
                    timeout_action: k(LShift),
                    tap: k(Enter),
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 0,
                }),
            ]],
            [[Trans, MultipleKeyCodes(&[LCtrl, Enter].as_slice())]],
        ];
        let mut layout = Layout::new(LAYERS);
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..197 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn basic_hold_tap_timeout() {
        static LAYERS: Layers<2, 1> = &[
            [[
                HoldTap(&HoldTapAction {
                    timeout: 200,
                    hold: l(1),
                    tap: k(Space),
                    timeout_action: l(1),
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 0,
                }),
                HoldTap(&HoldTapAction {
                    timeout: 200,
                    hold: k(LCtrl),
                    timeout_action: k(LCtrl),
                    tap: k(Enter),
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 0,
                }),
            ]],
            [[Trans, MultipleKeyCodes(&[LCtrl, Enter].as_slice())]],
        ];
        let mut layout = Layout::new(LAYERS);
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..197 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LCtrl], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LCtrl], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LCtrl, Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LCtrl], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn hold_tap_interleaved_timeout() {
        static LAYERS: Layers<2, 1> = &[[[
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(LAlt),
                timeout_action: k(LAlt),
                tap: k(Space),
                config: HoldTapConfig::Default,
                tap_hold_interval: 0,
            }),
            HoldTap(&HoldTapAction {
                timeout: 20,
                hold: k(LCtrl),
                timeout_action: k(LCtrl),
                tap: k(Enter),
                config: HoldTapConfig::Default,
                tap_hold_interval: 0,
            }),
        ]]];
        let mut layout = Layout::new(LAYERS);
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        for _ in 0..15 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());
        for _ in 0..10 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[Space], layout.keycodes());
        }
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space, LCtrl], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LCtrl], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn hold_on_press() {
        static LAYERS: Layers<2, 1> = &[[[
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(LAlt),
                timeout_action: k(LAlt),
                tap: k(Space),
                config: HoldTapConfig::HoldOnOtherKeyPress,
                tap_hold_interval: 0,
            }),
            k(Enter),
        ]]];
        let mut layout = Layout::new(LAYERS);

        // Press another key before timeout
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt, Enter], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Enter], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Press another key after timeout
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt, Enter], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Enter], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn permissive_hold() {
        static LAYERS: Layers<2, 1> = &[[[
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(LAlt),
                timeout_action: k(LAlt),
                tap: k(Space),
                config: HoldTapConfig::PermissiveHold,
                tap_hold_interval: 0,
            }),
            k(Enter),
        ]]];
        let mut layout = Layout::new(LAYERS);

        // Press and release another key before timeout
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt, Enter], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn simultaneous_hold() {
        static LAYERS: Layers<3, 1> = &[[[
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(LAlt),
                timeout_action: k(LAlt),
                tap: k(Space),
                config: HoldTapConfig::Default,
                tap_hold_interval: 0,
            }),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(RAlt),
                timeout_action: k(RAlt),
                tap: k(A),
                config: HoldTapConfig::Default,
                tap_hold_interval: 0,
            }),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(LCtrl),
                timeout_action: k(LCtrl),
                tap: k(A),
                config: HoldTapConfig::Default,
                tap_hold_interval: 0,
            }),
        ]]];
        let mut layout = Layout::new(LAYERS);
        layout.quick_tap_hold_timeout = true;

        // Press and release another key before timeout
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        for _ in 0..196 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt, RAlt], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt, RAlt], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt, RAlt, LCtrl], layout.keycodes());
    }

    #[test]
    fn multiple_actions() {
        static LAYERS: Layers<2, 1> = &[
            [[MultipleActions(&[l(1), k(LShift)].as_slice()), k(F)]],
            [[Trans, k(E)]],
        ];
        let mut layout = Layout::new(LAYERS);
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, E], layout.keycodes());
        layout.event(Release(0, 1));
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn custom() {
        static LAYERS: Layers<1, 1, i32> = &[[[Action::Custom(42)]]];
        let mut layout = Layout::new(LAYERS);
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Custom event
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::Press(&42), layout.tick());
        assert_keys(&[], layout.keycodes());

        // nothing more
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // release custom
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::Release(&42), layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn multiple_layers() {
        static LAYERS: Layers<2, 1> = &[
            [[l(1), l(2)]],
            [[k(A), l(3)]],
            [[l(0), k(B)]],
            [[k(C), k(D)]],
        ];
        let mut layout = Layout::new(LAYERS);
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_eq!(0, layout.current_layer());
        assert_keys(&[], layout.keycodes());

        // press L1
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_eq!(1, layout.current_layer());
        assert_keys(&[], layout.keycodes());
        // press L3 on L1
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_eq!(3, layout.current_layer());
        assert_keys(&[], layout.keycodes());
        // release L1, still on l3
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_eq!(3, layout.current_layer());
        assert_keys(&[], layout.keycodes());
        // press and release C on L3
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[C], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        // release L3, back to L0
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_eq!(0, layout.current_layer());
        assert_keys(&[], layout.keycodes());

        // back to empty, going to L2
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_eq!(2, layout.current_layer());
        assert_keys(&[], layout.keycodes());
        // and press the L0 key on L2
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_eq!(0, layout.current_layer());
        assert_keys(&[], layout.keycodes());
        // release the L0, back to L2
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_eq!(2, layout.current_layer());
        assert_keys(&[], layout.keycodes());
        // release the L2, back to L0
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_eq!(0, layout.current_layer());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn custom_handler() {
        fn always_tap(_: QueuedIter) -> (Option<WaitingAction>, bool) {
            (Some(WaitingAction::Tap), false)
        }
        fn always_hold(_: QueuedIter) -> (Option<WaitingAction>, bool) {
            (Some(WaitingAction::Hold), false)
        }
        fn always_nop(_: QueuedIter) -> (Option<WaitingAction>, bool) {
            (Some(WaitingAction::NoOp), false)
        }
        fn always_none(_: QueuedIter) -> (Option<WaitingAction>, bool) {
            (None, false)
        }
        static LAYERS: Layers<4, 1> = &[[[
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(Kb1),
                timeout_action: k(Kb1),
                tap: k(Kb0),
                config: HoldTapConfig::Custom(&always_tap),
                tap_hold_interval: 0,
            }),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(Kb3),
                timeout_action: k(Kb3),
                tap: k(Kb2),
                config: HoldTapConfig::Custom(&always_hold),
                tap_hold_interval: 0,
            }),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(Kb5),
                timeout_action: k(Kb5),
                tap: k(Kb4),
                config: HoldTapConfig::Custom(&always_nop),
                tap_hold_interval: 0,
            }),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(Kb7),
                timeout_action: k(Kb7),
                tap: k(Kb6),
                config: HoldTapConfig::Custom(&always_none),
                tap_hold_interval: 0,
            }),
        ]]];
        let mut layout = Layout::new(LAYERS);
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Custom handler always taps
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb0], layout.keycodes());

        // nothing more
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Custom handler always holds
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb3], layout.keycodes());

        // nothing more
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Custom handler always prevents any event
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // even timeout does not trigger
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }

        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // nothing more
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Custom handler timeout fallback
        layout.event(Press(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        for _ in 0..199 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }

        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb7], layout.keycodes());
    }

    #[test]
    fn tap_hold_interval() {
        static LAYERS: Layers<2, 1> = &[[[
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(LAlt),
                timeout_action: k(LAlt),
                tap: k(Space),
                config: HoldTapConfig::Default,
                tap_hold_interval: 200,
            }),
            k(Enter),
        ]]];
        let mut layout = Layout::new(LAYERS);

        // press and release the HT key, expect tap action
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // press again within tap_hold_interval, tap action should be in keycode immediately
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());

        // tap action should continue to be in keycodes even after timeout
        for _ in 0..300 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[Space], layout.keycodes());
        }
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Press again. This is outside the tap_hold_interval window, so should result in hold
        // action.
        layout.event(Press(0, 0));
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn tap_hold_interval_interleave() {
        static LAYERS: Layers<3, 1> = &[[[
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(LAlt),
                timeout_action: k(LAlt),
                tap: k(Space),
                config: HoldTapConfig::Default,
                tap_hold_interval: 200,
            }),
            k(Enter),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(LAlt),
                timeout_action: k(LAlt),
                tap: k(Enter),
                config: HoldTapConfig::Default,
                tap_hold_interval: 200,
            }),
        ]]];
        let mut layout = Layout::new(LAYERS);

        // press and release the HT key, expect tap action
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // press a different key in between
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Enter], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // press HT key again, should result in hold action
        layout.event(Press(0, 0));
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // press HT key, press+release diff key, release HT key
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Enter, Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // press HT key again, should result in hold action
        layout.event(Press(0, 0));
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // press HT key, press+release diff (HT) key, release HT key
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Enter, Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // press HT key again, should result in hold action
        layout.event(Press(0, 0));
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
    }

    #[test]
    fn tap_hold_interval_short_hold() {
        static LAYERS: Layers<1, 1> = &[[[HoldTap(&HoldTapAction {
            timeout: 50,
            hold: k(LAlt),
            timeout_action: k(LAlt),
            tap: k(Space),
            config: HoldTapConfig::Default,
            tap_hold_interval: 200,
        })]]];
        let mut layout = Layout::new(LAYERS);

        // press and hold the HT key, expect hold action
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // press and hold the HT key, expect hold action, even though it's within the
        // tap_hold_interval
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn tap_hold_interval_different_hold() {
        static LAYERS: Layers<2, 1> = &[[[
            HoldTap(&HoldTapAction {
                timeout: 50,
                hold: k(LAlt),
                timeout_action: k(LAlt),
                tap: k(Space),
                config: HoldTapConfig::Default,
                tap_hold_interval: 200,
            }),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(RAlt),
                timeout_action: k(RAlt),
                tap: k(Enter),
                config: HoldTapConfig::Default,
                tap_hold_interval: 200,
            }),
        ]]];
        let mut layout = Layout::new(LAYERS);

        // press HT1, press HT2, release HT1 after hold timeout, release HT2, press HT2
        layout.event(Press(0, 0));
        layout.event(Press(0, 1));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt, Enter], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Enter], layout.keycodes());
        // press HT2 again, should result in tap action
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        for _ in 0..300 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[Enter], layout.keycodes());
        }
    }

    #[test]
    fn one_shot() {
        static LAYERS: Layers<3, 1> = &[[[
            OneShot(&crate::action::OneShot {
                timeout: 100,
                action: &k(LShift),
                end_config: OneShotEndConfig::EndOnFirstPress,
            }),
            k(A),
            k(B),
        ]]];
        let mut layout = Layout::new(LAYERS);
        layout.oneshot.pause_input_processing_delay = 1;

        // Test:
        // 1. press one-shot
        // 2. release one-shot
        // 3. press A within timeout
        // 4. press B within timeout
        // 5. release A, B
        layout.event(Press(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Release(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Press(0, 1));
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A, LShift], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A, B], layout.keycodes());
        layout.event(Release(0, 1));
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[B], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 1. press one-shot
        // 2. release one-shot
        // 3. press A after timeout
        // 4. release A
        layout.event(Press(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Release(0, 0));
        for _ in 0..75 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 1. press one-shot
        // 2. press A
        // 3. release A
        // 4. release one-shot
        layout.event(Press(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, A], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 1. press one-shot
        // 2. press A after timeout
        // 3. release A
        // 4. release one-shot
        layout.event(Press(0, 0));
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, A], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn one_shot_end_press_or_repress() {
        static LAYERS: Layers<3, 1> = &[[[
            OneShot(&crate::action::OneShot {
                timeout: 100,
                action: &k(LShift),
                end_config: OneShotEndConfig::EndOnFirstPressOrRepress,
            }),
            k(A),
            k(B),
        ]]];
        let mut layout = Layout::new(LAYERS);
        layout.oneshot.pause_input_processing_delay = 1;

        // Test:
        // 1. press one-shot
        // 2. release one-shot
        // 3. press A within timeout
        // 4. press B within timeout
        // 5. release A, B
        layout.event(Press(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Release(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Press(0, 1));
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A, LShift], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A, B], layout.keycodes());
        layout.event(Release(0, 1));
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[B], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 1. press one-shot
        // 2. release one-shot
        // 3. press A after timeout
        // 4. release A
        layout.event(Press(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Release(0, 0));
        for _ in 0..75 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 1. press one-shot
        // 2. press A
        // 3. release A
        // 4. release one-shot
        layout.event(Press(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, A], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 1. press one-shot
        // 2. press A after timeout
        // 3. release A
        // 4. release one-shot
        layout.event(Press(0, 0));
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, A], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 1. press one-shot
        // 2. release one-shot
        // 3. press one-shot within timeout
        // 4. release one-shot quickly - should end
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 1. press one-shot
        // 2. release one-shot
        // 3. press one-shot within timeout
        // 4. release one-shot after timeout
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn one_shot_end_on_release() {
        static LAYERS: Layers<3, 1> = &[[[
            OneShot(&crate::action::OneShot {
                timeout: 100,
                action: &k(LShift),
                end_config: OneShotEndConfig::EndOnFirstRelease,
            }),
            k(A),
            k(B),
        ]]];
        let mut layout = Layout::new(LAYERS);

        // Test:
        // 1. press one-shot
        // 2. release one-shot
        // 3. press A within timeout
        // 4. press B within timeout
        // 5. release A, B
        layout.event(Press(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Release(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Press(0, 1));
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A, LShift], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A, B, LShift], layout.keycodes());
        layout.event(Release(0, 1));
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[B, LShift], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 1. press one-shot
        // 2. release one-shot
        // 3. press A after timeout
        // 4. release A
        layout.event(Press(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Release(0, 0));
        for _ in 0..75 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 1. press one-shot
        // 2. press A
        // 3. release A
        // 4. release one-shot
        layout.event(Press(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, A], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 1. press one-shot
        // 2. press A after timeout
        // 3. release A
        // 4. release one-shot
        layout.event(Press(0, 0));
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, A], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test:
        // 3. press A
        // 1. press one-shot
        // 2. release one-shot
        // 3. release A
        // 4. press B within timeout
        // 5. release B
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A], layout.keycodes());
        layout.event(Press(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[A, LShift], layout.keycodes());
        }
        layout.event(Release(0, 0));
        for _ in 0..25 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[A, LShift], layout.keycodes());
        }
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[B, LShift], layout.keycodes());
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn one_shot_multi() {
        static LAYERS: Layers<4, 1> = &[
            [[
                OneShot(&crate::action::OneShot {
                    timeout: 100,
                    action: &k(LShift),
                    end_config: OneShotEndConfig::EndOnFirstPress,
                }),
                OneShot(&crate::action::OneShot {
                    timeout: 100,
                    action: &k(LCtrl),
                    end_config: OneShotEndConfig::EndOnFirstPress,
                }),
                OneShot(&crate::action::OneShot {
                    timeout: 100,
                    action: &Layer(1),
                    end_config: OneShotEndConfig::EndOnFirstPress,
                }),
                NoOp,
            ]],
            [[k(A), k(B), k(C), k(D)]],
        ];
        let mut layout = Layout::new(LAYERS);
        layout.oneshot.pause_input_processing_delay = 1;

        layout.event(Press(0, 0));
        layout.event(Release(0, 0));
        for _ in 0..90 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Press(0, 1));
        for _ in 0..90 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift, LCtrl], layout.keycodes());
        }
        assert_eq!(layout.current_layer(), 0);
        layout.event(Press(0, 2));
        layout.event(Release(0, 2));
        for _ in 0..90 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift, LCtrl], layout.keycodes());
            assert_eq!(layout.current_layer(), 1);
        }
        layout.event(Press(0, 3));
        layout.event(Release(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, LCtrl, D], layout.keycodes());
        assert_eq!(layout.current_layer(), 1);
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LCtrl], layout.keycodes());
        assert_eq!(layout.current_layer(), 0);
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn one_shot_tap_hold() {
        static LAYERS: Layers<3, 1> = &[
            [[
                OneShot(&crate::action::OneShot {
                    timeout: 200,
                    action: &k(LShift),
                    end_config: OneShotEndConfig::EndOnFirstPress,
                }),
                HoldTap(&HoldTapAction {
                    timeout: 100,
                    hold: k(LAlt),
                    timeout_action: k(LAlt),
                    tap: k(Space),
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 0,
                }),
                NoOp,
            ]],
            [[k(A), k(B), k(C)]],
        ];
        let mut layout = Layout::new(LAYERS);
        layout.oneshot.pause_input_processing_delay = 1;

        layout.event(Press(0, 0));
        layout.event(Release(0, 0));
        for _ in 0..90 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Press(0, 1));
        for _ in 0..90 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        layout.event(Press(0, 0));
        layout.event(Release(0, 0));
        for _ in 0..90 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        layout.event(Press(0, 1));
        for _ in 0..100 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LShift], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, LAlt], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn tap_dance_uneager() {
        static LAYERS: Layers<2, 2> = &[[
            [
                TapDance(&crate::action::TapDance {
                    timeout: 100,
                    actions: &[
                        &k(LShift),
                        &OneShot(&crate::action::OneShot {
                            timeout: 100,
                            action: &k(LCtrl),
                            end_config: OneShotEndConfig::EndOnFirstPress,
                        }),
                        &HoldTap(&HoldTapAction {
                            timeout: 100,
                            hold: k(LAlt),
                            timeout_action: k(LAlt),
                            tap: k(Space),
                            config: HoldTapConfig::Default,
                            tap_hold_interval: 0,
                        }),
                    ],
                    config: TapDanceConfig::Lazy,
                }),
                k(A),
            ],
            [k(B), k(C)],
        ]];
        let mut layout = Layout::new(LAYERS);

        // Test: tap-dance first key, timeout
        layout.event(Press(0, 0));
        for _ in 0..100 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test: tap-dance first key, press another key
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 0));
        assert_keys(&[LShift], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, A], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test: tap-dance second key, timeout
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..99 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        for _ in 0..100 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LCtrl], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test: tap-dance third key, timeout, tap
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test: tap-dance third key, timeout, hold
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 0));
        for _ in 0..100 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[LAlt], layout.keycodes());
        }
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn tap_dance_eager() {
        static LAYERS: Layers<2, 2> = &[[
            [
                TapDance(&crate::action::TapDance {
                    timeout: 100,
                    actions: &[&k(Kb1), &k(Kb2), &k(Kb3)],
                    config: TapDanceConfig::Eager,
                }),
                k(A),
            ],
            [k(B), k(C)],
        ]];
        let mut layout = Layout::new(LAYERS);

        // Test: tap-dance-eager first key
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb1], layout.keycodes());
        for _ in 0..200 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[Kb1], layout.keycodes());
        }
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test: tap-dance-eager first key, press another key
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb1], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb1, A], layout.keycodes());
        layout.event(Release(0, 0));
        assert_keys(&[Kb1, A], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test: tap-dance second key
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb1], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb2], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..99 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Test: tap-dance third key
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb1], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb2], layout.keycodes());
        layout.event(Release(0, 0));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb3], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn release_state() {
        static LAYERS: Layers<2, 1> = &[
            [[
                MultipleActions(&(&[KeyCode(LCtrl), Layer(1)] as _)),
                MultipleActions(&(&[KeyCode(LAlt), Layer(1)] as _)),
            ]],
            [[
                MultipleActions(
                    &(&[ReleaseState(ReleasableState::KeyCode(LAlt)), KeyCode(Space)] as _),
                ),
                ReleaseState(ReleasableState::Layer(1)),
            ]],
        ];

        let mut layout = Layout::new(LAYERS);

        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LAlt], layout.keycodes());
        assert_eq!(1, layout.current_layer());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LCtrl], layout.keycodes());
        assert_eq!(1, layout.current_layer());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_eq!(0, layout.current_layer());
        assert_keys(&[LCtrl], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LCtrl], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_chord() {
        const GROUP: ChordsGroup<core::convert::Infallible> = ChordsGroup {
            coords: &[((0, 2), 1), ((0, 3), 2), ((0, 4), 4), ((0, 5), 8)],
            chords: &[
                (1, &KeyCode(Kb1)),
                (2, &KeyCode(Kb2)),
                (4, &KeyCode(Kb3)),
                (8, &KeyCode(Kb4)),
                (3, &KeyCode(Kb5)),
                (11, &KeyCode(Kb6)),
            ],
            timeout: 100,
        };
        static LAYERS: Layers<6, 1> = &[[[
            NoOp,
            NoOp,
            Chords(&GROUP),
            Chords(&GROUP),
            Chords(&GROUP),
            Chords(&GROUP),
        ]]];

        let mut layout = Layout::new(LAYERS);
        layout.event(Press(0, 2));
        // timeout on non-terminal chord
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 3));
        for _ in 0..49 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb5], layout.keycodes());
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb5], layout.keycodes());
        layout.event(Release(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // timeout on terminal chord with no action associated
        // combo like (h j k) -> (h j) (k)
        layout.event(Press(0, 2));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 3));
        for _ in 0..30 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 4));
        for _ in 0..20 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb5], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb5, Kb3], layout.keycodes());
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb3], layout.keycodes());
        layout.event(Release(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb3], layout.keycodes());
        layout.event(Release(0, 4));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // release terminal chord with no action associated
        // combo like (h j k) -> (h j) (k)
        layout.event(Press(0, 2));
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 3));
        for _ in 0..30 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 4));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 4));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb5], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb5, Kb3], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb5], layout.keycodes());
        layout.event(Release(0, 2));
        layout.event(Release(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // release terminal chord with no action associated
        // Test combo like (h j k l) -> (h) (j k l)
        layout.event(Press(0, 4));
        layout.event(Press(0, 2));
        layout.event(Press(0, 3));
        layout.event(Press(0, 5));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        for _ in 0..30 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb3], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb3, Kb6], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb3], layout.keycodes());
    }

    #[test]
    fn test_chord_normalkey_order() {
        const GROUP: ChordsGroup<core::convert::Infallible> = ChordsGroup {
            coords: &[((0, 2), 1), ((0, 3), 2), ((0, 4), 4), ((0, 5), 8)],
            chords: &[
                (1, &KeyCode(Kb1)),
                (2, &KeyCode(Kb2)),
                (4, &KeyCode(Kb3)),
                (8, &KeyCode(Kb4)),
                (3, &KeyCode(Kb5)),
                (11, &KeyCode(Kb6)),
            ],
            timeout: 100,
        };
        static LAYERS: Layers<6, 1> = &[[[
            NoOp,
            k(A),
            Chords(&GROUP),
            Chords(&GROUP),
            Chords(&GROUP),
            Chords(&GROUP),
        ]]];

        let mut layout = Layout::new(LAYERS);
        layout.event(Press(0, 2));
        // timeout on non-terminal chord
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb1], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb1, A], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb1], layout.keycodes());
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_chord_multi_waiting_decomposition() {
        const GROUP: ChordsGroup<core::convert::Infallible> = ChordsGroup {
            coords: &[((0, 0), 1), ((0, 1), 2)],
            chords: &[
                (
                    1,
                    &HoldTap(&HoldTapAction {
                        timeout: 100,
                        hold: k(A),
                        timeout_action: k(A),
                        tap: k(Kb1),
                        config: HoldTapConfig::Default,
                        tap_hold_interval: 0,
                    }),
                ),
                (
                    2,
                    &HoldTap(&HoldTapAction {
                        timeout: 100,
                        hold: k(B),
                        timeout_action: k(B),
                        tap: k(Kb2),
                        config: HoldTapConfig::Default,
                        tap_hold_interval: 0,
                    }),
                ),
            ],
            timeout: 100,
        };
        static LAYERS: Layers<2, 1> = &[[[Chords(&GROUP), Chords(&GROUP)]]];

        let mut layout = Layout::new(LAYERS);
        layout.quick_tap_hold_timeout = true;
        layout.event(Press(0, 0));
        layout.event(Press(0, 1));
        // Why does this take 103 ticks?
        // 0: chord begin
        // 1: chord decompose
        // 2: action queue dequeue
        // 3: action queue dequeue
        // 4-103: timeout ticks
        for _ in 0..102 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A, B], layout.keycodes());
        layout.event(Release(0, 0));
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[B], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_fork() {
        static LAYERS: Layers<2, 1> = &[[[
            Fork(&ForkConfig {
                left: k(Kb1),
                right: k(Kb2),
                right_triggers: &[Space],
            }),
            k(Space),
        ]]];
        let mut layout = Layout::new(LAYERS);

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb1], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        layout.event(Press(0, 1));
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Space, Kb2], layout.keycodes());
        layout.event(Release(0, 1));
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[Kb2], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_repeat() {
        static LAYERS: Layers<5, 1> = &[
            [[
                k(A),
                MultipleKeyCodes(&[LShift, B].as_slice()),
                Repeat,
                MultipleActions(&[k(C), k(D)].as_slice()),
                Layer(1),
            ]],
            [[
                k(E),
                MultipleKeyCodes(&[LShift, F].as_slice()),
                Repeat,
                MultipleActions(&[k(G), k(H)].as_slice()),
                Layer(1),
            ]],
        ];
        let mut layout = Layout::new(LAYERS);

        // Press a key
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Repeat it, should be the same
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A], layout.keycodes());
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Press a chord
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, B], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Repeat it, should be the same
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LShift, B], layout.keycodes());
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Press a multiple action
        layout.event(Press(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[C, D], layout.keycodes());
        layout.event(Release(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Repeat it, should be the same
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[C, D], layout.keycodes());
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Go to a different layer and press a key
        layout.event(Press(0, 4));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[E], layout.keycodes());
        layout.event(Release(0, 4));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[E], layout.keycodes());
        layout.event(Release(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Repeat, should be the same as the other layer
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[E], layout.keycodes());
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        // Activate the layer action and press repeat there, should still be the same action
        layout.event(Press(0, 4));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[E], layout.keycodes());
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 4));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_clear_multiple_keycodes() {
        static LAYERS: Layers<2, 1> = &[[[k(A), MultipleKeyCodes(&[LCtrl, Enter].as_slice())]]];
        let mut layout = Layout::new(LAYERS);
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LCtrl, Enter], layout.keycodes());
        // Cancel chord keys on next keypress.
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A], layout.keycodes());
    }

    // Tests the new Trans behavior.
    // https://github.com/jtroo/kanata/issues/738
    #[test]
    fn test_trans_in_stacked_held_layers() {
        static LAYERS: Layers<4, 1> = &[
            [[Layer(1), NoOp, NoOp, k(A)]],
            [[NoOp, Layer(2), NoOp, k(B)]],
            [[NoOp, NoOp, Layer(3), Trans]],
            [[NoOp, NoOp, NoOp, Trans]],
        ];
        let mut layout = Layout::new(LAYERS);

        // change to layer 2
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        // change to layer 3
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        // change to layer 4
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        // pressing Trans should press a key in layer 2, compared to previous behavior,
        // where a key in layer 1 would be pressed
        layout.event(Press(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[B], layout.keycodes());
        layout.event(Release(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_trans_in_action_on_first_layer() {
        static DEFSRC_LAYER: [Action; 2] = [NoOp, k(X)];
        static LAYERS: Layers<2, 1> = &[
            [[Layer(1), Trans]],
            [[NoOp, MultipleActions(&[Trans].as_slice())]],
        ];
        let mut layout = Layout::new_with_trans_action_settings(&DEFSRC_LAYER, LAYERS, true, true);

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[X], layout.keycodes());
        layout.event(Release(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_trans_in_taphold_tap() {
        static LAYERS: Layers<3, 1> = &[
            [[Layer(1), NoOp, k(A)]],
            [[NoOp, Layer(2), k(B)]],
            [[
                NoOp,
                NoOp,
                HoldTap(&HoldTapAction {
                    timeout: 50,
                    hold: k(Space),
                    timeout_action: k(Space),
                    tap: Trans,
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 200,
                }),
            ]],
        ];
        let mut layout = Layout::new(LAYERS);

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 2)); // press th
        for _ in 0..10 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Release(0, 2)); // release th
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[B], layout.keycodes()); // B is resolved from Trans
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        // test tap action repeat
        layout.event(Press(0, 2));
        for _ in 0..30 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[B], layout.keycodes());
        }
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_trans_in_taphold_hold() {
        static LAYERS: Layers<3, 1> = &[
            [[Layer(1), NoOp, k(A)]],
            [[NoOp, Layer(2), k(B)]],
            [[
                NoOp,
                NoOp,
                HoldTap(&HoldTapAction {
                    timeout: 50,
                    hold: Trans,
                    timeout_action: Trans,
                    tap: k(Space),
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 200,
                }),
            ]],
        ];
        let mut layout = Layout::new(LAYERS);

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 2)); // press th
        for _ in 0..50 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        for _ in 0..70 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[B], layout.keycodes()); // B is resolved from Trans
        }
        layout.event(Release(0, 2)); // release th
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_trans_in_tapdance_lazy() {
        static LAYERS: Layers<3, 1> = &[
            [[Layer(1), NoOp, k(A)]],
            [[NoOp, Layer(2), k(B)]],
            [[
                NoOp,
                NoOp,
                TapDance(&crate::action::TapDance {
                    timeout: 100,
                    actions: &[&Trans, &k(X)],
                    config: TapDanceConfig::Lazy,
                }),
            ]],
        ];
        let mut layout = Layout::new(LAYERS);

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 2));
        for _ in 0..10 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Release(0, 2));
        for _ in 0..90 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[B], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_trans_in_tapdance_eager() {
        static LAYERS: Layers<3, 1> = &[
            [[Layer(1), NoOp, k(A)]],
            [[NoOp, Layer(2), k(B)]],
            [[
                NoOp,
                NoOp,
                TapDance(&crate::action::TapDance {
                    timeout: 100,
                    actions: &[&Trans, &k(X)],
                    config: TapDanceConfig::Eager,
                }),
            ]],
        ];
        let mut layout = Layout::new(LAYERS);

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 2));
        for _ in 0..10 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[B], layout.keycodes());
        }
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_trans_in_multi() {
        static LAYERS: Layers<3, 1> = &[
            [[Layer(1), NoOp, k(A)]],
            [[NoOp, Layer(2), k(B)]],
            [[NoOp, NoOp, MultipleActions(&[Trans, k(X)].as_slice())]],
        ];
        let mut layout = Layout::new(LAYERS);

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 2));
        for _ in 0..10 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[B, X], layout.keycodes());
        }
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_trans_in_chords() {
        const GROUP: ChordsGroup<core::convert::Infallible> = ChordsGroup {
            coords: &[((0, 2), 1), ((0, 3), 2)],
            chords: &[(1, &Trans), (2, &Trans), (3, &KeyCode(X))],
            timeout: 100,
        };
        static LAYERS: Layers<4, 1> = &[
            [[Layer(1), NoOp, k(A), k(B)]],
            [[NoOp, Layer(2), k(C), k(D)]],
            [[NoOp, NoOp, Chords(&GROUP), Chords(&GROUP)]],
        ];
        let mut layout = Layout::new(LAYERS);

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 2));
        for _ in 0..10 {
            assert_eq!(CustomEvent::NoEvent, layout.tick());
            assert_keys(&[], layout.keycodes());
        }
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[C], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_trans_in_fork() {
        static LAYERS: Layers<3, 1> = &[
            [[Layer(1), NoOp, k(A)]],
            [[NoOp, Layer(2), k(B)]],
            [[
                NoOp,
                NoOp,
                Fork(&ForkConfig {
                    left: Trans,
                    right: Trans,
                    right_triggers: &[Space],
                }),
            ]],
        ];
        let mut layout = Layout::new(LAYERS);

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[B], layout.keycodes());
        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_trans_in_switch() {
        static LAYERS: Layers<3, 1> = &[
            [[Layer(1), NoOp, k(A)]],
            [[NoOp, Layer(2), k(B)]],
            [[
                NoOp,
                NoOp,
                Switch(&switch::Switch {
                    cases: &[(&[], &Trans, BreakOrFallthrough::Break)],
                }),
            ]],
        ];
        let mut layout = Layout::new(LAYERS);

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());

        layout.event(Press(0, 2));
        // No idea why we have to wait 2 ticks here. Is this a bug in switch?
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[B], layout.keycodes());

        layout.event(Release(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
    }

    #[test]
    fn test_multiple_taphold_trans() {
        static LAYERS: Layers<4, 1> = &[
            [[Layer(1), NoOp, NoOp, k(A)]],
            [[
                NoOp,
                Layer(2),
                NoOp,
                HoldTap(&HoldTapAction {
                    timeout: 50,
                    hold: k(B),
                    timeout_action: k(B),
                    tap: Trans,
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 200,
                }),
            ]],
            [[
                NoOp,
                NoOp,
                Layer(3),
                HoldTap(&HoldTapAction {
                    timeout: 50,
                    hold: k(C),
                    timeout_action: k(C),
                    tap: Trans,
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 200,
                }),
            ]],
            [[
                NoOp,
                NoOp,
                NoOp,
                HoldTap(&HoldTapAction {
                    timeout: 50,
                    hold: k(D),
                    timeout_action: k(D),
                    tap: Trans,
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 200,
                }),
            ]],
        ];
        let mut layout = Layout::new(LAYERS);

        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 2));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Press(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 3));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A], layout.keycodes());
    }

    #[test]
    fn trans_in_multi_works_with_all_trans_settings() {
        let permutations: &[(bool, bool)] =
            &[(false, false), (false, true), (true, false), (true, true)];

        for &(trans_v2, delegate_to_1st) in permutations {
            static DEFSRC_LAYER: [Action; 3] = [NoOp, NoOp, k(X)];
            static LAYERS: Layers<3, 1> = &[
                [[
                    Layer(1),
                    DefaultLayer(1),
                    MultipleActions(&[Trans, k(Y)].as_slice()),
                ]],
                [[NoOp, Layer(2), k(B)]],
                [[NoOp, NoOp, Trans]],
            ];
            for &do_layer_switch in &[false, true] {
                let mut layout = Layout::new_with_trans_action_settings(
                    &DEFSRC_LAYER,
                    LAYERS,
                    trans_v2,
                    delegate_to_1st,
                );

                layout.event(Press(0, 2));
                assert_eq!(CustomEvent::NoEvent, layout.tick());
                assert_keys(&[X, Y], layout.keycodes());
                layout.event(Release(0, 2));
                assert_eq!(CustomEvent::NoEvent, layout.tick());
                assert_keys(&[], layout.keycodes());

                if do_layer_switch {
                    layout.event(Press(0, 1));
                    assert_eq!(CustomEvent::NoEvent, layout.tick());
                    assert_keys(&[], layout.keycodes());
                    layout.event(Release(0, 1));
                    assert_eq!(CustomEvent::NoEvent, layout.tick());
                    assert_keys(&[], layout.keycodes());
                    assert_eq!(layout.default_layer, 1);
                } else {
                    layout.event(Press(0, 0));
                    assert_eq!(CustomEvent::NoEvent, layout.tick());
                    assert_keys(&[], layout.keycodes());
                }

                layout.event(Press(0, 2));
                assert_eq!(CustomEvent::NoEvent, layout.tick());
                assert_keys(&[B], layout.keycodes());
                layout.event(Release(0, 2));
                assert_eq!(CustomEvent::NoEvent, layout.tick());
                assert_keys(&[], layout.keycodes());

                layout.event(Press(0, 1));
                assert_eq!(CustomEvent::NoEvent, layout.tick());
                assert_keys(&[], layout.keycodes());
                layout.event(Press(0, 2));
                assert_eq!(CustomEvent::NoEvent, layout.tick());
                assert_keys(
                    match (trans_v2, delegate_to_1st) {
                        (false, false) => &[X],
                        (false, true) => &[X, Y],
                        (true, _) => &[B],
                    },
                    layout.keycodes(),
                );
                layout.event(Release(0, 2));
                assert_eq!(CustomEvent::NoEvent, layout.tick());
                assert_keys(&[], layout.keycodes());
            }
        }
    }
}
