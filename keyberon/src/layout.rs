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

use crate::key_code::KeyCode;
use crate::{action::*, multikey_buffer::MultiKeyBuffer};
use arraydeque::ArrayDeque;
use heapless::Vec;

use State::*;

/// The coordinate type.
pub type KCoord = (u8, u16);

/// The Layers type.
///
/// `Layers` type is an array of layers which contain the description
/// of actions on the switch matrix. For example `layers[1][2][3]`
/// corresponds to the key on the first layer, row 2, column 3.
/// The generic parameters are in order: the number of columns, rows and layers,
/// and the type contained in custom actions.
pub type Layers<'a, const C: usize, const R: usize, const L: usize, T = core::convert::Infallible> =
    [[[Action<'a, T>; C]; R]; L];

const QUEUE_SIZE: usize = 32;

/// The current event queue.
///
/// Events can be retrieved by iterating over this struct and calling [Queued::event].
type Queue = ArrayDeque<[Queued; QUEUE_SIZE], arraydeque::behavior::Wrapping>;

/// A list of queued press events. Used for special handling of potentially multiple press events
/// that occur during a Waiting event.
type PressedQueue = ArrayDeque<[KCoord; QUEUE_SIZE]>;

/// The maximum number of actions that can be activated concurrently via chord decomposition or
/// activation of multiple switch cases using fallthrough.
pub const ACTION_QUEUE_LEN: usize = 8;

/// The queue is currently only used for chord decomposition when a longer chord does not result in
/// an action, but splitting it into smaller chords would. The buffer size of 8 should be more than
/// enough for real world usage, but if one wanted to be extra safe, this should be ChordKeys::BITS
/// since that should guarantee that all potentially queueable actions can fit.
type ActionQueue<'a, T> =
    ArrayDeque<[QueuedAction<'a, T>; ACTION_QUEUE_LEN], arraydeque::behavior::Wrapping>;
type QueuedAction<'a, T> = Option<(KCoord, &'a Action<'a, T>)>;

/// The layout manager. It takes `Event`s and `tick`s as input, and
/// generate keyboard reports.
pub struct Layout<'a, const C: usize, const R: usize, const L: usize, T = core::convert::Infallible>
where
    T: 'a + std::fmt::Debug,
{
    pub layers: &'a [[[Action<'a, T>; C]; R]; L],
    pub default_layer: usize,
    /// Key states.
    pub states: Vec<State<'a, T>, 64>,
    pub waiting: Option<WaitingState<'a, T>>,
    pub tap_dance_eager: Option<TapDanceEagerState<'a, T>>,
    pub queue: Queue,
    pub oneshot: OneShotState,
    pub last_press_tracker: LastPressTracker,
    pub active_sequences: ArrayDeque<[SequenceState<'a, T>; 4], arraydeque::behavior::Wrapping>,
    pub action_queue: ActionQueue<'a, T>,
    pub rpt_action: Option<&'a Action<'a, T>>,
    pub historical_keys: ArrayDeque<[KeyCode; 8], arraydeque::behavior::Wrapping>,
    rpt_multikey_key_buffer: MultiKeyBuffer<'a, T>,
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
impl<'a, T> CustomEvent<'a, T> {
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
pub struct NormalKeyFlags(u16);

const NORMAL_KEY_FLAG_CLEAR_ON_NEXT_ACTION: u16 = 0x0001;

impl NormalKeyFlags {
    pub fn clear_on_next_action(self) -> bool {
        (self.0 & NORMAL_KEY_FLAG_CLEAR_ON_NEXT_ACTION) == NORMAL_KEY_FLAG_CLEAR_ON_NEXT_ACTION
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
impl<'a, T> Copy for State<'a, T> {}
impl<'a, T> Clone for State<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T: 'a> State<'a, T> {
    fn keycode(&self) -> Option<KeyCode> {
        match self {
            NormalKey { keycode, .. } => Some(*keycode),
            FakeKey { keycode } => Some(*keycode),
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
    fn tick(&self) -> Option<Self> {
        Some(*self)
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
            (NormalKey { keycode: k1, .. }, ReleasableState::KeyCode(k2)) => {
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

impl<'a, T> TapDanceEagerState<'a, T> {
    fn tick(&mut self) {
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
    fn tick(
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
                } else {
                    skip_timeout = local_skip;
                }
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
        } else if (self.timeout == 0) && (!skip_timeout) {
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
        if self.timeout == 0 || usize::from(num_taps) >= max_taps {
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
        // need to keep track of how many Press events we handled so we can filter them out later
        let mut handled_press_events = 0;
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
        queued: &mut Queue,
        action_queue: &mut ActionQueue<'a, T>,
    ) {
        let mut chord_key_order = [0u32; ChordKeys::BITS as usize];

        // Default to the initial coordinate. But if a key is released early (before the timeout
        // occurs), use that key for action releases. That way the chord is released as early as
        // possible.
        let mut action_queue_coord = self.coord;

        let starting_mask = config.get_keys(self.coord).unwrap_or(0);
        let mut mask_bits_set = 1;
        chord_key_order[0] = starting_mask;
        let _ = queued.iter().try_fold(starting_mask, |active, s| {
            if self.delay.saturating_sub(s.since) > self.timeout {
                Ok(active)
            } else if let Some(chord_keys) = config.get_keys(s.event.coord()) {
                match s.event {
                    Event::Press(_, _) => {
                        if active | chord_keys != active {
                            chord_key_order[mask_bits_set] = chord_keys;
                            mask_bits_set += 1;
                        }
                        Ok(active | chord_keys)
                    }
                    Event::Release(i, j) => {
                        action_queue_coord = (i, j);
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
        while start < len {
            let sub_chord = &chord_keys[start..end];
            let chord_mask = sub_chord
                .iter()
                .copied()
                .reduce(|acc, e| acc | e)
                .unwrap_or(0);
            if let Some(action) = config.get_chord(chord_mask) {
                let _ = action_queue.push_back(Some((action_queue_coord, action)));
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
                        let _ = action_queue.push_back(Some((action_queue_coord, action)));
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

type OneShotCoords = ArrayDeque<[KCoord; ONE_SHOT_MAX_ACTIVE], arraydeque::behavior::Wrapping>;

#[derive(Debug, Copy, Clone)]
pub struct SequenceState<'a, T: 'a> {
    cur_event: Option<SequenceEvent<'a, T>>,
    delay: u32,              // Keeps track of SequenceEvent::Delay time remaining
    tapped: Option<KeyCode>, // Keycode of a key that should be released at the next tick
    remaining_events: &'a [SequenceEvent<'a, T>],
}

type OneShotKeys = [KCoord; ONE_SHOT_MAX_ACTIVE];
type ReleasedOneShotKeys = Vec<KCoord, ONE_SHOT_MAX_ACTIVE>;

/// Contains the state of one shot keys that are currently active.
pub struct OneShotState {
    /// KCoordinates of one shot keys that are active
    pub keys: ArrayDeque<OneShotKeys, arraydeque::behavior::Wrapping>,
    /// KCoordinates of one shot keys that have been released
    pub released_keys: ArrayDeque<OneShotKeys, arraydeque::behavior::Wrapping>,
    /// Used to keep track of already-pressed keys for the release variants.
    pub other_pressed_keys: ArrayDeque<OneShotKeys, arraydeque::behavior::Wrapping>,
    /// Timeout (ms) after which all one shot keys expire
    pub timeout: u16,
    /// Contains the end config of the most recently pressed one shot key
    pub end_config: OneShotEndConfig,
    /// Marks if release of the one shot keys should be done on the next tick
    pub release_on_next_tick: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum OneShotHandlePressKey {
    OneShotKey(KCoord),
    Other(KCoord),
}

impl OneShotState {
    fn tick(&mut self) -> Option<ReleasedOneShotKeys> {
        if self.keys.is_empty() {
            return None;
        }
        self.timeout = self.timeout.saturating_sub(1);
        if self.release_on_next_tick || self.timeout == 0 {
            self.release_on_next_tick = false;
            self.timeout = 0;
            self.keys.clear();
            self.other_pressed_keys.clear();
            Some(self.released_keys.drain(..).collect())
        } else {
            None
        }
    }

    fn handle_press(&mut self, key: OneShotHandlePressKey) -> OneShotCoords {
        let mut oneshot_coords = ArrayDeque::new();
        if self.keys.is_empty() {
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
                    self.release_on_next_tick = true;
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
    event: Event,
    since: u16,
}
impl From<Event> for Queued {
    fn from(event: Event) -> Self {
        Queued { event, since: 0 }
    }
}
impl Queued {
    fn tick(&mut self) {
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
    fn tick(&mut self) {
        self.tap_hold_timeout = self.tap_hold_timeout.saturating_sub(1);
    }
}

impl<'a, const C: usize, const R: usize, const L: usize, T: 'a + Copy + std::fmt::Debug>
    Layout<'a, C, R, L, T>
{
    /// Creates a new `Layout` object.
    pub fn new(layers: &'a [[[Action<T>; C]; R]; L]) -> Self {
        Self {
            layers,
            default_layer: 0,
            states: Vec::new(),
            waiting: None,
            tap_dance_eager: None,
            queue: ArrayDeque::new(),
            oneshot: OneShotState {
                timeout: 0,
                end_config: OneShotEndConfig::EndOnFirstPress,
                keys: ArrayDeque::new(),
                released_keys: ArrayDeque::new(),
                other_pressed_keys: ArrayDeque::new(),
                release_on_next_tick: false,
            },
            last_press_tracker: Default::default(),
            active_sequences: ArrayDeque::new(),
            action_queue: ArrayDeque::new(),
            rpt_action: None,
            historical_keys: ArrayDeque::new(),
            rpt_multikey_key_buffer: unsafe { MultiKeyBuffer::new() },
        }
    }
    /// Iterates on the key codes of the current state.
    pub fn keycodes(&self) -> impl Iterator<Item = KeyCode> + Clone + '_ {
        self.states.iter().filter_map(State::keycode)
    }
    fn waiting_into_hold(&mut self) -> CustomEvent<'a, T> {
        if let Some(w) = &self.waiting {
            let hold = w.hold;
            let coord = w.coord;
            let delay = match w.config {
                WaitingConfig::HoldTap(..) | WaitingConfig::Chord(_) => w.delay + w.ticks,
                WaitingConfig::TapDance(_) => 0,
            };
            self.waiting = None;
            if coord == self.last_press_tracker.coord {
                self.last_press_tracker.tap_hold_timeout = 0;
            }
            self.do_action(hold, coord, delay, false)
        } else {
            CustomEvent::NoEvent
        }
    }
    fn waiting_into_tap(&mut self, pq: Option<PressedQueue>) -> CustomEvent<'a, T> {
        if let Some(w) = &self.waiting {
            let tap = w.tap;
            let coord = w.coord;
            let delay = match w.config {
                WaitingConfig::HoldTap(..) | WaitingConfig::Chord(_) => w.delay + w.ticks,
                WaitingConfig::TapDance(_) => 0,
            };
            self.waiting = None;
            let ret = self.do_action(tap, coord, delay, false);
            if let Some(pq) = pq {
                if matches!(
                    tap,
                    Action::KeyCode(_)
                        | Action::MultipleKeyCodes(_)
                        | Action::OneShot(_)
                        | Action::Layer(_)
                ) {
                    // The current intent of this block is to ensure that simple actions like
                    // key presses or layer-while-held remain pressed as long as a single key from
                    // the input chord remains held. The behaviour of these actions is correct in
                    // the case of repeating do_action, so there is currently no harm in doing
                    // this. Other action types are more problematic though.
                    for other_coord in pq.iter().copied() {
                        self.do_action(tap, other_coord, delay, false);
                    }
                }
            }
            ret
        } else {
            CustomEvent::NoEvent
        }
    }
    fn waiting_into_timeout(&mut self) -> CustomEvent<'a, T> {
        if let Some(w) = &self.waiting {
            let timeout_action = w.timeout_action;
            let coord = w.coord;
            let delay = match w.config {
                WaitingConfig::HoldTap(..) | WaitingConfig::Chord(_) => w.delay + w.ticks,
                WaitingConfig::TapDance(_) => 0,
            };
            self.waiting = None;
            if coord == self.last_press_tracker.coord {
                self.last_press_tracker.tap_hold_timeout = 0;
            }
            self.do_action(timeout_action, coord, delay, false)
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
        if let Some(Some((coord, action))) = self.action_queue.pop_front() {
            // If there's anything in the action queue, don't process anything else yet - execute
            // everything. Otherwise an action may never be released.
            return self.do_action(action, coord, 0, false);
        }
        self.states = self.states.iter().filter_map(State::tick).collect();
        self.queue.iter_mut().for_each(Queued::tick);
        self.last_press_tracker.tick();
        if let Some(ref mut tde) = self.tap_dance_eager {
            tde.tick();
            if tde.is_expired() {
                self.tap_dance_eager = None;
            }
        }
        self.process_sequences();

        let mut custom = CustomEvent::NoEvent;
        if let Some(released_keys) = self.oneshot.tick() {
            for key in released_keys.iter() {
                custom.update(self.dequeue(Queued {
                    event: Event::Release(key.0, key.1),
                    since: 0,
                }));
            }
        }

        custom.update(match &mut self.waiting {
            Some(w) => match w.tick(&mut self.queue, &mut self.action_queue) {
                Some((WaitingAction::Hold, _)) => self.waiting_into_hold(),
                Some((WaitingAction::Tap, pq)) => self.waiting_into_tap(pq),
                Some((WaitingAction::Timeout, _)) => self.waiting_into_timeout(),
                Some((WaitingAction::NoOp, _)) => self.drop_waiting(),
                None => CustomEvent::NoEvent,
            },
            None => match self.queue.pop_front() {
                Some(s) => self.dequeue(s),
                None => CustomEvent::NoEvent,
            },
        });
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
                    match seq.remaining_events {
                        [e, tail @ ..] => {
                            seq.cur_event = Some(*e);
                            seq.remaining_events = tail;
                        }
                        [] => (),
                    }
                    // Process it (SequenceEvent)
                    match seq.cur_event {
                        Some(SequenceEvent::Complete) => {
                            for fake_key in self.states.clone().iter() {
                                if let FakeKey { keycode } = *fake_key {
                                    self.states.retain(|s| s.seq_release(keycode).is_some());
                                }
                            }
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
                    self.states
                        .retain(|s| s.release((i, j), &mut custom).is_some());
                }
                if let Some((i2, j2)) = overflow_key {
                    self.states
                        .retain(|s| s.release((i2, j2), &mut custom).is_some());
                }
                custom
            }

            Press(i, j) => {
                if let Some(tde) = self.tap_dance_eager {
                    if (i, j) == self.last_press_tracker.coord && !tde.is_expired() {
                        let custom = self.do_action(
                            tde.actions[usize::from(tde.num_taps)],
                            (i, j),
                            queue.since,
                            false,
                        );
                        // unwrap is here because tde cannot be ref mut
                        self.tap_dance_eager.as_mut().expect("some").incr_taps();
                        custom

                    // i == 0 means real key, i == 1 means fake key. Let fake keys do whatever, but
                    // interrupt tap-dance-eager if real key.
                    } else if i == 0 {
                        // unwrap is here because tde cannot be ref mut
                        self.tap_dance_eager.as_mut().expect("some").set_expired();
                        let action = self.press_as_action((i, j), self.current_layer());
                        self.do_action(action, (i, j), queue.since, false)
                    } else {
                        let action = self.press_as_action((i, j), self.current_layer());
                        self.do_action(action, (i, j), queue.since, false)
                    }
                } else {
                    let action = self.press_as_action((i, j), self.current_layer());
                    self.do_action(action, (i, j), queue.since, false)
                }
            }
        }
    }
    /// Register a key event.
    pub fn event(&mut self, event: Event) {
        if let Some(queued) = self.queue.push_back(event.into()) {
            self.waiting_into_hold();
            self.dequeue(queued);
        }
    }
    fn press_as_action(&self, coord: KCoord, layer: usize) -> &'a Action<'a, T> {
        use crate::action::Action::*;
        let action = self
            .layers
            .get(layer)
            .and_then(|l| l.get(coord.0 as usize))
            .and_then(|l| l.get(coord.1 as usize));
        match action {
            None => &NoOp,
            Some(Trans) => {
                if layer != self.default_layer {
                    self.press_as_action(coord, self.default_layer)
                } else {
                    &NoOp
                }
            }
            Some(action) => action,
        }
    }
    fn do_action(
        &mut self,
        action: &'a Action<'a, T>,
        coord: KCoord,
        delay: u16,
        is_oneshot: bool,
    ) -> CustomEvent<'a, T> {
        self.clear_and_handle_waiting(action);
        if self.last_press_tracker.coord != coord {
            self.last_press_tracker.tap_hold_timeout = 0;
        }
        use Action::*;
        self.states.retain(|s| match s {
            NormalKey { flags, .. } => !flags.clear_on_next_action(),
            _ => true,
        });
        match action {
            NoOp | Trans => {
                if !is_oneshot {
                    self.oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
                self.rpt_action = Some(action);
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
                    self.do_action(ac, coord, delay, is_oneshot);
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
                        timeout: *timeout,
                        delay,
                        ticks: 0,
                        hold,
                        tap,
                        timeout_action,
                        config: WaitingConfig::HoldTap(*config),
                    };
                    self.waiting = Some(waiting);
                    self.last_press_tracker.tap_hold_timeout = *tap_hold_interval;
                } else {
                    self.last_press_tracker.tap_hold_timeout = 0;
                    custom.update(self.do_action(tap, coord, delay, is_oneshot));
                }
                // Need to set tap_hold_tracker coord AFTER the checks.
                self.last_press_tracker.coord = coord;
                return custom;
            }
            &OneShot(oneshot) => {
                self.last_press_tracker.coord = coord;
                let custom = self.do_action(oneshot.action, coord, delay, true);
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
            &TapDance(td) => {
                self.last_press_tracker.coord = coord;
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
                        self.do_action(td.actions[0], coord, delay, false);
                    }
                }
            }
            &Chords(chords) => {
                self.last_press_tracker.coord = coord;
                self.waiting = Some(WaitingState {
                    coord,
                    timeout: chords.timeout,
                    delay,
                    ticks: 0,
                    hold: &Action::NoOp,
                    tap: &Action::NoOp,
                    timeout_action: &Action::NoOp,
                    config: WaitingConfig::Chord(chords),
                });
            }
            &KeyCode(keycode) => {
                self.last_press_tracker.coord = coord;
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
                self.last_press_tracker.coord = coord;
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
                self.last_press_tracker.coord = coord;
                let mut custom = CustomEvent::NoEvent;
                for action in *v {
                    custom.update(self.do_action(action, coord, delay, is_oneshot));
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
                self.last_press_tracker.coord = coord;
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
                self.last_press_tracker.coord = coord;
                self.set_default_layer(*value);
                if !is_oneshot {
                    self.oneshot
                        .handle_press(OneShotHandlePressKey::Other(coord));
                }
            }
            Custom(value) => {
                self.last_press_tracker.coord = coord;
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
                    false => self.do_action(&fcfg.left, coord, delay, false),
                    true => self.do_action(&fcfg.right, coord, delay, false),
                };
                // Repeat the fork rather than the terminal action.
                self.rpt_action = Some(action);
                return ret;
            }
            Switch(sw) => {
                let active_keys = self.states.iter().filter_map(State::keycode);
                let historical_keys = self.historical_keys.iter().copied();
                let action_queue = &mut self.action_queue;
                for ac in sw.actions(active_keys, historical_keys) {
                    action_queue.push_back(Some((coord, ac)));
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

    /// Clear the waiting state if it is about to be overwritten by a new waiting state.
    ///
    /// If something is waiting **and** another waiting action is currently being activated, that
    /// probably means that there were multiple actions in the action queue caused by a single
    /// terminal state. In this scenario, do some sensible default for the waiting state and end it
    /// early, since a new action should interrupt the waiting action anyway.
    ///
    /// Another potential concern is if there is some processing in the event queue that needs to
    /// happen as part of the cleanup, i.e. the code runs in `handle_tap_dance`, `handle_chord`
    /// where some queued events are consumed. I'm fairly sure that there is no extra processing
    /// that needs to happen. Actions in the action queue should be activated on subsequent ticks
    /// with no room for key events to be a factor when handling this case.
    fn clear_and_handle_waiting(&mut self, action: &'a Action<'a, T>) {
        if !matches!(
            action,
            Action::HoldTap(_) | Action::TapDance(_) | Action::Chords(_)
        ) {
            return;
        }
        let mut waiting_action = None;
        if let Some(waiting) = &self.waiting {
            waiting_action = match waiting.config {
                WaitingConfig::HoldTap(_) => Some((waiting.tap, waiting.coord, waiting.delay)),
                WaitingConfig::TapDance(tdc) => {
                    Some((tdc.actions[0], waiting.coord, waiting.delay))
                }
                WaitingConfig::Chord(_) => None,
            };
            self.waiting = None;
        };
        if let Some((action, coord, delay)) = waiting_action {
            self.do_action(action, coord, delay, false);
        };
    }

    /// Obtain the index of the current active layer
    pub fn current_layer(&self) -> usize {
        self.states
            .iter()
            .rev()
            .find_map(State::get_layer)
            .unwrap_or(self.default_layer)
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
        static LAYERS: Layers<2, 1, 2> = [
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
        let mut layout = Layout::new(&LAYERS);
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
        static LAYERS: Layers<2, 1, 2> = [
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
        let mut layout = Layout::new(&LAYERS);
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
        static LAYERS: Layers<2, 1, 1> = [[[
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
        let mut layout = Layout::new(&LAYERS);
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
        static LAYERS: Layers<2, 1, 1> = [[[
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
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<2, 1, 1> = [[[
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
        let mut layout = Layout::new(&LAYERS);

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
    fn multiple_actions() {
        static LAYERS: Layers<2, 1, 2> = [
            [[MultipleActions(&[l(1), k(LShift)].as_slice()), k(F)]],
            [[Trans, k(E)]],
        ];
        let mut layout = Layout::new(&LAYERS);
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
        static LAYERS: Layers<1, 1, 1, u8> = [[[Action::Custom(42)]]];
        let mut layout = Layout::new(&LAYERS);
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
        static LAYERS: Layers<2, 1, 4> = [
            [[l(1), l(2)]],
            [[k(A), l(3)]],
            [[l(0), k(B)]],
            [[k(C), k(D)]],
        ];
        let mut layout = Layout::new(&LAYERS);
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
        static LAYERS: Layers<4, 1, 1> = [[[
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
        let mut layout = Layout::new(&LAYERS);
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
        static LAYERS: Layers<2, 1, 1> = [[[
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
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<3, 1, 1> = [[[
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
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<1, 1, 1> = [[[HoldTap(&HoldTapAction {
            timeout: 50,
            hold: k(LAlt),
            timeout_action: k(LAlt),
            tap: k(Space),
            config: HoldTapConfig::Default,
            tap_hold_interval: 200,
        })]]];
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<2, 1, 1> = [[[
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
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<3, 1, 1> = [[[
            OneShot(&crate::action::OneShot {
                timeout: 100,
                action: &k(LShift),
                end_config: OneShotEndConfig::EndOnFirstPress,
            }),
            k(A),
            k(B),
        ]]];
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<3, 1, 1> = [[[
            OneShot(&crate::action::OneShot {
                timeout: 100,
                action: &k(LShift),
                end_config: OneShotEndConfig::EndOnFirstPressOrRepress,
            }),
            k(A),
            k(B),
        ]]];
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<3, 1, 1> = [[[
            OneShot(&crate::action::OneShot {
                timeout: 100,
                action: &k(LShift),
                end_config: OneShotEndConfig::EndOnFirstRelease,
            }),
            k(A),
            k(B),
        ]]];
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<4, 1, 2> = [
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
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<3, 1, 2> = [
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
        let mut layout = Layout::new(&LAYERS);

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
    fn tap_dance() {
        static LAYERS: Layers<2, 2, 1> = [[
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
        let mut layout = Layout::new(&LAYERS);

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
        assert_keys(&[], layout.keycodes());
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
        for _ in 0..101 {
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
        static LAYERS: Layers<2, 2, 1> = [[
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
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<2, 1, 2> = [
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

        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<6, 1, 1> = [[[
            NoOp,
            NoOp,
            Chords(&GROUP),
            Chords(&GROUP),
            Chords(&GROUP),
            Chords(&GROUP),
        ]]];

        let mut layout = Layout::new(&LAYERS);
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
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 3));
        layout.event(Release(0, 4));

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
        assert_keys(&[], layout.keycodes());
        layout.event(Release(0, 3));
        layout.event(Release(0, 2));
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
        assert_keys(&[], layout.keycodes());
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
        static LAYERS: Layers<6, 1, 1> = [[[
            NoOp,
            k(A),
            Chords(&GROUP),
            Chords(&GROUP),
            Chords(&GROUP),
            Chords(&GROUP),
        ]]];

        let mut layout = Layout::new(&LAYERS);
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
    fn test_fork() {
        static LAYERS: Layers<2, 1, 1> = [[[
            Fork(&ForkConfig {
                left: k(Kb1),
                right: k(Kb2),
                right_triggers: &[Space],
            }),
            k(Space),
        ]]];
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<5, 1, 2> = [
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
        let mut layout = Layout::new(&LAYERS);

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
        static LAYERS: Layers<2, 1, 1> = [[[k(A), MultipleKeyCodes(&[LCtrl, Enter].as_slice())]]];
        let mut layout = Layout::new(&LAYERS);
        layout.event(Press(0, 1));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[LCtrl, Enter], layout.keycodes());
        // Cancel chord keys on next keypress.
        layout.event(Press(0, 0));
        assert_eq!(CustomEvent::NoEvent, layout.tick());
        assert_keys(&[A], layout.keycodes());
    }
}
