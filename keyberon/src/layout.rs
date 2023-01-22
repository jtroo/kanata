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

use crate::action::*;
use crate::key_code::KeyCode;
use arraydeque::ArrayDeque;
use heapless::Vec;

use State::*;

/// The Layers type.
///
/// `Layers` type is an array of layers which contain the description
/// of actions on the switch matrix. For example `layers[1][2][3]`
/// corresponds to the key on the first layer, row 2, column 3.
/// The generic parameters are in order: the number of columns, rows and layers,
/// and the type contained in custom actions.
pub type Layers<'a, const C: usize, const R: usize, const L: usize, T = core::convert::Infallible> =
    [[[Action<'a, T>; C]; R]; L];

/// The current event stack.
///
/// Events can be retrieved by iterating over this struct and calling [Stacked::event].
type Stack = ArrayDeque<[Stacked; 16], arraydeque::behavior::Wrapping>;

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
    pub stacked: Stack,
    pub oneshot: OneShotState,
    pub last_press_tracker: LastPressTracker,
    pub active_sequences: ArrayDeque<[SequenceState<'a, T>; 4], arraydeque::behavior::Wrapping>,
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
    pub fn coord(self) -> (u8, u16) {
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
    pub fn transform(self, f: impl FnOnce(u8, u16) -> (u8, u16)) -> Self {
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
#[derive(Debug, PartialEq, Eq)]
pub enum CustomEvent<'a, T: 'a> {
    /// No custom action.
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
impl<'a, T> Default for CustomEvent<'a, T> {
    fn default() -> Self {
        CustomEvent::NoEvent
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum State<'a, T: 'a> {
    NormalKey { keycode: KeyCode, coord: (u8, u16) },
    LayerModifier { value: usize, coord: (u8, u16) },
    Custom { value: &'a T, coord: (u8, u16) },
    FakeKey { keycode: KeyCode }, // Fake key event for sequences
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
    fn tick(&self) -> Option<Self> {
        Some(*self)
    }
    /// Returns None if the key has been released and Some otherwise.
    pub fn release(&self, c: (u8, u16), custom: &mut CustomEvent<'a, T>) -> Option<Self> {
        match *self {
            NormalKey { coord, .. } | LayerModifier { coord, .. } if coord == c => None,
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
    coord: (u8, u16),
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
    HoldTap(HoldTapConfig),
    TapDance(TapDanceState<'a, T>),
    Chord(&'a ChordsGroup<'a, T>),
}

#[derive(Debug)]
pub struct WaitingState<'a, T: 'a + std::fmt::Debug> {
    coord: (u8, u16),
    timeout: u16,
    delay: u16,
    ticks: u16,
    hold: &'a Action<'a, T>,
    tap: &'a Action<'a, T>,
    config: WaitingConfig<'a, T>,
}

/// Actions that can be triggered for a key configured for HoldTap.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WaitingAction {
    /// Trigger the holding event.
    Hold,
    /// Trigger the tapping event.
    Tap,
    /// Drop this event. It will act as if no key was pressed.
    NoOp,
}

impl<'a, T: std::fmt::Debug> WaitingState<'a, T> {
    fn tick(&mut self, stacked: &mut Stack) -> Option<WaitingAction> {
        self.timeout = self.timeout.saturating_sub(1);
        self.ticks = self.ticks.saturating_add(1);
        let (ret, cfg_change) = match self.config {
            WaitingConfig::HoldTap(htc) => (self.handle_hold_tap(htc, stacked), None),
            WaitingConfig::TapDance(ref tds) => {
                let (ret, num_taps) =
                    self.handle_tap_dance(tds.num_taps, tds.actions.len(), stacked);
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
                if let Some((ret, action)) = self.handle_chord(config, stacked) {
                    self.tap = action;
                    (Some(ret), None)
                } else {
                    (None, None)
                }
            }
        };
        if let Some(cfg) = cfg_change {
            self.config = cfg;
        }
        ret
    }

    fn handle_hold_tap(&mut self, cfg: HoldTapConfig, stacked: &Stack) -> Option<WaitingAction> {
        match cfg {
            HoldTapConfig::Default => (),
            HoldTapConfig::HoldOnOtherKeyPress => {
                if stacked.iter().any(|s| s.event.is_press()) {
                    return Some(WaitingAction::Hold);
                }
            }
            HoldTapConfig::PermissiveHold => {
                for (x, s) in stacked.iter().enumerate() {
                    if s.event.is_press() {
                        let (i, j) = s.event.coord();
                        let target = Event::Release(i, j);
                        if stacked.iter().skip(x + 1).any(|s| s.event == target) {
                            return Some(WaitingAction::Hold);
                        }
                    }
                }
            }
            HoldTapConfig::Custom(func) => {
                if let waiting_action @ Some(_) = (func)(StackedIter(stacked.iter())) {
                    return waiting_action;
                }
            }
        }
        if let Some(&Stacked { since, .. }) = stacked
            .iter()
            .find(|s| self.is_corresponding_release(&s.event))
        {
            if self.timeout >= self.delay.saturating_sub(since) {
                Some(WaitingAction::Tap)
            } else {
                Some(WaitingAction::Hold)
            }
        } else if self.timeout == 0 {
            Some(WaitingAction::Hold)
        } else {
            None
        }
    }

    fn handle_tap_dance(
        &self,
        num_taps: u16,
        max_taps: usize,
        stacked: &mut Stack,
    ) -> (Option<WaitingAction>, u16) {
        // Evict events with the same coordinates except for the final release. E.g. if 3 taps have
        // occurred, this will remove all `Press` events and 2 `Release` events. This is done so
        // that the state machine processes the entire tap dance sequence as a single press and
        // single release regardless of how many taps were actually done.
        let evict_same_coord_events = |num_taps: u16, stacked: &mut Stack| {
            let mut releases_to_remove = num_taps.saturating_sub(1);
            stacked.retain(|s| {
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
            evict_same_coord_events(num_taps, stacked);
            return (Some(WaitingAction::Tap), num_taps);
        }
        // Get the number of sequential taps for this tap-dance key. If a different key was
        // pressed, activate a tap-dance action.
        match stacked.iter().try_fold(1, |same_tap_count, s| {
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
                evict_same_coord_events(num_taps, stacked);
                (Some(WaitingAction::Tap), num_taps)
            }
        }
    }

    fn handle_chord(
        &self,
        config: &'a ChordsGroup<'a, T>,
        stacked: &mut Stack,
    ) -> Option<(WaitingAction, &'a Action<'a, T>)> {
        // need to keep track of how many Press events we handled so we can filter them out later
        let mut handled_press_events = 0;

        // Compute the set of chord keys that are currently pressed
        // `Ok` when chording mode may continue
        // `Err` when it should end for various reasons
        let active = stacked
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
                        Event::Release(_, _) => Err(active), // released a chord key, abort
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
                    (WaitingAction::Tap, action)
                } else {
                    return None; // nothing to do yet, we'll check back later
                }
            }
            Err(active) => {
                // Abort chording mode. Trigger a chord action if there is one.
                if let Some(action) = config.get_chord(active) {
                    (WaitingAction::Tap, action)
                } else {
                    (WaitingAction::NoOp, &Action::NoOp)
                }
            }
        };

        // Consume all press events that were logically handled by this chording event
        stacked.retain(|s| {
            if self.delay.saturating_sub(s.since) > self.timeout {
                true
            } else if matches!(s.event, Event::Press(i, j) if config.get_keys((i, j)).is_some())
                && handled_press_events > 0
            {
                handled_press_events -= 1;
                false
            } else {
                true
            }
        });

        Some(res)
    }

    fn is_corresponding_release(&self, event: &Event) -> bool {
        matches!(event, Event::Release(i, j) if (*i, *j) == self.coord)
    }

    fn is_corresponding_press(&self, event: &Event) -> bool {
        matches!(event, Event::Press(i, j) if (*i, *j) == self.coord)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SequenceState<'a, T: 'a> {
    cur_event: Option<SequenceEvent<'a, T>>,
    delay: u32,              // Keeps track of SequenceEvent::Delay time remaining
    tapped: Option<KeyCode>, // Keycode of a key that should be released at the next tick
    remaining_events: &'a [SequenceEvent<'a, T>],
}

type OneShotKeys = [(u8, u16); ONE_SHOT_MAX_ACTIVE];
type ReleasedOneShotKeys = Vec<(u8, u16), ONE_SHOT_MAX_ACTIVE>;

/// Contains the state of one shot keys that are currently active.
pub struct OneShotState {
    /// Coordinates of one shot keys that are active
    pub keys: ArrayDeque<OneShotKeys, arraydeque::behavior::Wrapping>,
    /// Coordinates of one shot keys that have been released
    pub released_keys: ArrayDeque<OneShotKeys, arraydeque::behavior::Wrapping>,
    /// Timeout (ms) after which all one shot keys expire
    pub timeout: u16,
    /// Contains the end config of the most recently pressed one shot key
    pub end_config: OneShotEndConfig,
    /// Marks if release of the one shot keys should be done on the next tick
    pub release_on_next_tick: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum OneShotHandlePressKey {
    OneShotKey((u8, u16)),
    Other,
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
            Some(self.released_keys.drain(..).collect())
        } else {
            None
        }
    }

    fn handle_press(&mut self, key: OneShotHandlePressKey) {
        if self.keys.is_empty() {
            return;
        }
        match key {
            OneShotHandlePressKey::OneShotKey(pressed_coord) => {
                // Release the one-shot key if it's re-pressed
                self.released_keys.retain(|coord| *coord != pressed_coord);
            }
            OneShotHandlePressKey::Other => {
                if self.end_config == OneShotEndConfig::EndOnFirstPress {
                    self.release_on_next_tick = true;
                }
            }
        }
    }

    /// Returns true if the caller should handle the release normally and false otherwise.
    /// The second value in the tuple represents an overflow of released one shot keys and should
    /// be released is it is `Some`.
    fn handle_release(&mut self, (i, j): (u8, u16)) -> (bool, Option<(u8, u16)>) {
        if self.keys.is_empty() {
            return (true, None);
        }
        if !self.keys.contains(&(i, j)) {
            if self.end_config == OneShotEndConfig::EndOnFirstRelease {
                self.release_on_next_tick = true;
            }
            (true, None)
        } else {
            // delay release for one shot keys
            (false, self.released_keys.push_back((i, j)))
        }
    }
}

/// An iterator over the currently stacked events.
///
/// Events can be retrieved by iterating over this struct and calling [Stacked::event].
pub struct StackedIter<'a>(arraydeque::Iter<'a, Stacked>);

impl<'a> Iterator for StackedIter<'a> {
    type Item = &'a Stacked;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// An event, waiting in a stack to be processed.
#[derive(Debug)]
pub struct Stacked {
    event: Event,
    since: u16,
}
impl From<Event> for Stacked {
    fn from(event: Event) -> Self {
        Stacked { event, since: 0 }
    }
}
impl Stacked {
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
    pub coord: (u8, u16),
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
            stacked: ArrayDeque::new(),
            oneshot: OneShotState {
                timeout: 0,
                end_config: OneShotEndConfig::EndOnFirstPress,
                keys: ArrayDeque::new(),
                released_keys: ArrayDeque::new(),
                release_on_next_tick: false,
            },
            last_press_tracker: Default::default(),
            active_sequences: ArrayDeque::new(),
        }
    }
    /// Iterates on the key codes of the current state.
    pub fn keycodes(&self) -> impl Iterator<Item = KeyCode> + '_ {
        self.states.iter().filter_map(State::keycode)
    }
    fn waiting_into_hold(&mut self) -> CustomEvent<'a, T> {
        if let Some(w) = &self.waiting {
            let hold = w.hold;
            let coord = w.coord;
            let delay = match w.config {
                WaitingConfig::HoldTap(..) => w.delay + w.ticks,
                WaitingConfig::TapDance(_) | WaitingConfig::Chord(_) => 0,
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
    fn waiting_into_tap(&mut self) -> CustomEvent<'a, T> {
        if let Some(w) = &self.waiting {
            let tap = w.tap;
            let coord = w.coord;
            let delay = match w.config {
                WaitingConfig::HoldTap(..) => w.delay + w.ticks,
                WaitingConfig::TapDance(_) | WaitingConfig::Chord(_) => 0,
            };
            self.waiting = None;
            self.do_action(tap, coord, delay, false)
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
        self.states = self.states.iter().filter_map(State::tick).collect();
        self.stacked.iter_mut().for_each(Stacked::tick);
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
                custom.update(self.unstack(Stacked {
                    event: Event::Release(key.0, key.1),
                    since: 0,
                }));
            }
        }

        custom.update(match &mut self.waiting {
            Some(w) => match w.tick(&mut self.stacked) {
                Some(WaitingAction::Hold) => self.waiting_into_hold(),
                Some(WaitingAction::Tap) => self.waiting_into_tap(),
                Some(WaitingAction::NoOp) => self.drop_waiting(),
                None => CustomEvent::NoEvent,
            },
            None => match self.stacked.pop_front() {
                Some(s) => self.unstack(s),
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

                            // Coord here doesn't matter; only matters if 2nd param is true. Need
                            // to fake the coord anyway; sequence events don't have associated
                            // coordinates.
                            self.oneshot.handle_press(OneShotHandlePressKey::Other);
                        }
                        Some(SequenceEvent::Tap(keycode)) => {
                            // Same as Press() except we track it for one tick via seq.tapped:
                            let _ = self.states.push(FakeKey { keycode });
                            self.oneshot.handle_press(OneShotHandlePressKey::Other);

                            seq.tapped = Some(keycode);
                        }
                        Some(SequenceEvent::Release(keycode)) => {
                            // Clear out the Press() matching this Release's keycode
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
    fn unstack(&mut self, stacked: Stacked) -> CustomEvent<'a, T> {
        use Event::*;
        match stacked.event {
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
                            stacked.since,
                            false,
                        );
                        // unwrap is here because tde cannot be ref mut
                        self.tap_dance_eager.as_mut().unwrap().incr_taps();
                        custom

                    // i == 0 means real key, i == 1 means fake key. Let fake keys do whatever, but
                    // interrupt tap-dance-eager if real key.
                    } else if i == 0 {
                        // unwrap is here because tde cannot be ref mut
                        self.tap_dance_eager.as_mut().unwrap().set_expired();
                        let action = self.press_as_action((i, j), self.current_layer());
                        self.do_action(action, (i, j), stacked.since, false)
                    } else {
                        let action = self.press_as_action((i, j), self.current_layer());
                        self.do_action(action, (i, j), stacked.since, false)
                    }
                } else {
                    let action = self.press_as_action((i, j), self.current_layer());
                    self.do_action(action, (i, j), stacked.since, false)
                }
            }
        }
    }
    /// Register a key event.
    pub fn event(&mut self, event: Event) {
        if let Some(stacked) = self.stacked.push_back(event.into()) {
            self.waiting_into_hold();
            self.unstack(stacked);
        }
    }
    fn press_as_action(&self, coord: (u8, u16), layer: usize) -> &'a Action<'a, T> {
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
        coord: (u8, u16),
        delay: u16,
        is_oneshot: bool,
    ) -> CustomEvent<'a, T> {
        assert!(self.waiting.is_none() || matches!(action, Action::Custom(..)));
        if self.last_press_tracker.coord != coord {
            self.last_press_tracker.tap_hold_timeout = 0;
        }
        use Action::*;
        match action {
            NoOp | Trans => (),
            HoldTap(HoldTapAction {
                timeout,
                hold,
                tap,
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
                    config: WaitingConfig::Chord(chords),
                });
            }
            &KeyCode(keycode) => {
                self.last_press_tracker.coord = coord;
                let _ = self.states.push(NormalKey { coord, keycode });
                if !is_oneshot {
                    self.oneshot.handle_press(OneShotHandlePressKey::Other);
                }
            }
            &MultipleKeyCodes(v) => {
                self.last_press_tracker.coord = coord;
                for &keycode in v {
                    let _ = self.states.push(NormalKey { coord, keycode });
                }
                if !is_oneshot {
                    self.oneshot.handle_press(OneShotHandlePressKey::Other);
                }
            }
            &MultipleActions(v) => {
                self.last_press_tracker.coord = coord;
                let mut custom = CustomEvent::NoEvent;
                for action in v {
                    custom.update(self.do_action(action, coord, delay, is_oneshot));
                }
                return custom;
            }
            Sequence { events } => {
                self.active_sequences.push_back(SequenceState {
                    cur_event: None,
                    delay: 0,
                    tapped: None,
                    remaining_events: events,
                });
            }
            CancelSequences => {
                // Clear any and all running sequences then clean up any leftover FakeKey events
                self.active_sequences.clear();
                for fake_key in self.states.clone().iter() {
                    if let FakeKey { keycode } = *fake_key {
                        self.states.retain(|s| s.seq_release(keycode).is_some());
                    }
                }
            }
            &Layer(value) => {
                self.last_press_tracker.coord = coord;
                let _ = self.states.push(LayerModifier { value, coord });
            }
            DefaultLayer(value) => {
                self.last_press_tracker.coord = coord;
                self.set_default_layer(*value);
            }
            Custom(value) => {
                self.last_press_tracker.coord = coord;
                if self.states.push(State::Custom { value, coord }).is_ok() {
                    return CustomEvent::Press(value);
                }
            }
            ReleaseState(rs) => {
                self.states.retain(|s| s.release_state(*rs).is_some());
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
    use crate::action::{k, l, m};
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
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 0,
                }),
                HoldTap(&HoldTapAction {
                    timeout: 200,
                    hold: k(LCtrl),
                    tap: k(Enter),
                    config: HoldTapConfig::Default,
                    tap_hold_interval: 0,
                }),
            ]],
            [[Trans, m([LCtrl, Enter].as_slice())]],
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
                tap: k(Space),
                config: HoldTapConfig::Default,
                tap_hold_interval: 0,
            }),
            HoldTap(&HoldTapAction {
                timeout: 20,
                hold: k(LCtrl),
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
        fn always_tap(_: StackedIter) -> Option<WaitingAction> {
            Some(WaitingAction::Tap)
        }
        fn always_hold(_: StackedIter) -> Option<WaitingAction> {
            Some(WaitingAction::Hold)
        }
        fn always_nop(_: StackedIter) -> Option<WaitingAction> {
            Some(WaitingAction::NoOp)
        }
        fn always_none(_: StackedIter) -> Option<WaitingAction> {
            None
        }
        static LAYERS: Layers<4, 1, 1> = [[[
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(Kb1),
                tap: k(Kb0),
                config: HoldTapConfig::Custom(always_tap),
                tap_hold_interval: 0,
            }),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(Kb3),
                tap: k(Kb2),
                config: HoldTapConfig::Custom(always_hold),
                tap_hold_interval: 0,
            }),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(Kb5),
                tap: k(Kb4),
                config: HoldTapConfig::Custom(always_nop),
                tap_hold_interval: 0,
            }),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(Kb7),
                tap: k(Kb6),
                config: HoldTapConfig::Custom(always_none),
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
                tap: k(Space),
                config: HoldTapConfig::Default,
                tap_hold_interval: 200,
            }),
            k(Enter),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(LAlt),
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
                tap: k(Space),
                config: HoldTapConfig::Default,
                tap_hold_interval: 200,
            }),
            HoldTap(&HoldTapAction {
                timeout: 200,
                hold: k(RAlt),
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

        // Test:
        // 1. press one-shot
        // 2. release one-shot
        // 3. press one-shot within timeout
        // 4. release one-shot
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
        // 1. press one-shot
        // 2. release one-shot
        // 3. press one-shot within timeout
        // 4. release one-shot
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
                MultipleActions(&[KeyCode(LCtrl), Layer(1)]),
                MultipleActions(&[KeyCode(LAlt), Layer(1)]),
            ]],
            [[
                MultipleActions(&[ReleaseState(ReleasableState::KeyCode(LAlt)), KeyCode(Space)]),
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
}
