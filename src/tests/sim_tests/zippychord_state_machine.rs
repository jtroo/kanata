//! Stateful property test for zippychord, using `proptest-state-machine`.
//!
//! Zippychord is a state machine, so we test it as one. The reference state
//! machine (`ZchRef`) generates a config + chord dictionary (`init_state`) and a
//! stream of transitions (`transitions`); the SUT (`ZchSut`) is a real `Kanata`
//! built from that config. After every transition the SUT's reconstructed net
//! visible text is asserted equal to the reference's predicted `visible`.
//!
//! The oracle is split between the generator and the reference state: a
//! `ChordExpansion` transition *carries which chord it activates* (so the
//! expected expansion is known by construction), while the reference state
//! supplies the *placement* (fresh append vs followup replace vs disabled
//! passthrough) based on coarse engine state. The reference deliberately does
//! NOT reimplement the keystroke-level eager/overlap/backspace accounting — that
//! is exactly the code under test — so accounting bugs surface as a mismatch
//! rather than being mirrored. (This is how the common-prefix backspace
//! under-count bug was found and is now regression-guarded.)
//!
//! Coverage / deferred dimensions are tracked in ZIPPY_PBT_NOTES.md.

use crate::oskbd::{KeyEvent, KeyValue};
use crate::tests::CFG_PARSE_LOCK;
use crate::{Kanata, str_to_oscode};
use proptest::prelude::*;
use proptest::test_runner::Config;
use proptest_state_machine::{
    ReferenceStateMachine, StateMachineTest, prop_state_machine_persisted,
};
use rustc_hash::FxHashMap;
use std::collections::BTreeSet;
use std::sync::MutexGuard;

// Letters that participate in chords (small alphabet => frequent overlaps).
const INPUT_ALPHA: &[char] = &['a', 'b', 'c', 'd'];
// Letters used for literal (non-chord) typing — disjoint from INPUT_ALPHA so a
// literal press is always "Neither" (disables zippy), never a chord subset.
const NONCHORD_ALPHA: &[char] = &['u', 'v', 'w', 'x', 'y', 'z'];
// Alphabet for free typing. Intentionally INCLUDES chord-participating keys
// (a-d) and space so that free typing can incidentally trigger chord
// activations — which the naive "literal append" oracle mispredicts. The PBT is
// meant to discover that; see ZIPPY_PBT_NOTES.md.
const FREE_ALPHA: &[char] = &['a', 'b', 'c', 'd', ' ', 'u', 'v', 'w'];

// Fixed timers. idle-reactivate-time (wait) is large so the WaitEnable countdown
// only ever crosses on an explicit "full" Idle transition, never mid-hold (holds
// reset it to `wait` on release anyway). Deadline is irrelevant to these flows.
const WAIT: u16 = 500;
const DEADLINE: u16 = 50;
// Max per-event timing gaps for a ChordExpansion gesture. The largest chord is
// INPUT_ALPHA (4) plus a leading space (5 keys), so the worst-case cumulative
// span of a press or release phase is 5 * (GAP_MAX + 1 processing tick), which
// must stay below DEADLINE so the chord is guaranteed to form. 6 => 35 < 50.
const PRESS_GAP_MAX: u16 = 6;
const RELEASE_GAP_MAX: u16 = 6;

// ---------------------------------------------------------------------------
// Dictionary model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)] // Backspace deferred (see ZIPPY_PBT_NOTES.md)
enum OutItem {
    Char(char), // already-cased net visible char (e.g. 'a', 'A', ' ')
    Backspace,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Child {
    key: char,
    out: Vec<OutItem>,
    followups: Vec<Child>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Root {
    lead_space: bool,
    keys: BTreeSet<char>,
    out: Vec<OutItem>,
    followups: Vec<Child>,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum SmartSpace {
    None,
    AddOnly,
    Full,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ModelCfg {
    smart_space: SmartSpace,
}

// ---------------------------------------------------------------------------
// Reference state
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Enabled {
    Enabled,
    WaitEnable,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZchModel {
    cfg: ModelCfg,
    roots: Vec<Root>,
    // dynamic coarse engine state:
    enabled: Enabled,
    until_enabled: u16,
    visible: Vec<char>,
    prioritized: Option<Vec<Child>>,
    last_act_len: usize, // visible chars the last activation owns at the tail
    smart_space_sent: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Target {
    Root(usize),
    Followup(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KeyAction {
    Press(char),
    Release(char),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ZchTransition {
    /// A gesture that activates `target`. The keystrokes are a granular timed
    /// stream: each `(delay_ms, action)` ticks `delay_ms` *before* applying the
    /// action. The generator is "smart" — it emits every target key as a press
    /// (in arbitrary order, with arbitrary inter-press timing that stays inside
    /// the chord deadline) *before* any release, so all keys are simultaneously
    /// held when the last press lands and the target chord is guaranteed to fire.
    /// That keeps the oracle exact (the activation is known by construction) while
    /// still exercising the timing/ordering-dependent eager-activation paths where
    /// the backspace accounting bugs live.
    ChordExpansion {
        target: Target,
        events: Vec<(u16, KeyAction)>,
    },
    Literal {
        key: char,
    },
    Idle {
        ms: u16,
    },
    /// Free typing: hold an arbitrary set of keys (press order / release order
    /// shuffled), NOT targeted to any chord. The reference predicts naive literal
    /// append (treats them as ordinary keystrokes).
    FreeType {
        press: Vec<char>,
        release: Vec<char>,
    },
}

impl ZchTransition {
    /// Keys pressed by a `ChordExpansion`, in press order.
    fn press_order(events: &[(u16, KeyAction)]) -> Vec<char> {
        events
            .iter()
            .filter_map(|(_, a)| match a {
                KeyAction::Press(c) => Some(*c),
                KeyAction::Release(_) => None,
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Serialization to a zippy config + chord file
// ---------------------------------------------------------------------------

fn out_to_tsv(out: &[OutItem]) -> String {
    out.iter()
        .map(|i| match i {
            OutItem::Char(c) => *c,
            OutItem::Backspace => '⌫',
        })
        .collect()
}

impl ZchModel {
    fn cfg_string(&self) -> String {
        let ss = match self.cfg.smart_space {
            SmartSpace::None => "none",
            SmartSpace::AddOnly => "add-space-only",
            SmartSpace::Full => "full",
        };
        format!(
            "(defsrc lalt)(deflayer base lalt)(defzippy file \
             on-first-press-chord-deadline {DEADLINE} idle-reactivate-time {WAIT} smart-space {ss})"
        )
    }

    fn tsv(&self) -> String {
        let mut lines = Vec::new();
        for r in &self.roots {
            let input: String = {
                let mut s = String::new();
                if r.lead_space {
                    s.push(' ');
                }
                s.extend(r.keys.iter());
                s
            };
            lines.push(format!("{input}\t{}", out_to_tsv(&r.out)));
            emit_children(&r.followups, &input, &mut lines);
        }
        format!("\n{}\n", lines.join("\n"))
    }
}

/// Union of every key used by any chord (root keys + leading space + all
/// followup keys, recursively). Free typing must avoid all of these.
fn chord_keys(roots: &[Root]) -> BTreeSet<char> {
    fn collect(children: &[Child], s: &mut BTreeSet<char>) {
        for c in children {
            s.insert(c.key);
            collect(&c.followups, s);
        }
    }
    let mut s = BTreeSet::new();
    for r in roots {
        s.extend(r.keys.iter().copied());
        if r.lead_space {
            s.insert(' ');
        }
        collect(&r.followups, &mut s);
    }
    s
}

fn emit_children(children: &[Child], prefix: &str, lines: &mut Vec<String>) {
    for c in children {
        let input = format!("{prefix} {}", c.key);
        lines.push(format!("{input}\t{}", out_to_tsv(&c.out)));
        emit_children(&c.followups, &input, lines);
    }
}

// ---------------------------------------------------------------------------
// Reference engine (the placement oracle)
// ---------------------------------------------------------------------------

fn display_len(out: &[OutItem]) -> i32 {
    out.iter()
        .map(|i| match i {
            OutItem::Char(_) => 1,
            OutItem::Backspace => -1,
        })
        .sum()
}

impl ZchModel {
    fn resolve(&self, target: &Target) -> (Vec<OutItem>, Vec<Child>, bool) {
        match target {
            Target::Root(i) => {
                let r = &self.roots[*i];
                (r.out.clone(), r.followups.clone(), false)
            }
            Target::Followup(i) => {
                let c = &self.prioritized.as_ref().unwrap()[*i];
                (c.out.clone(), c.followups.clone(), true)
            }
        }
    }

    fn apply_out(&mut self, out: &[OutItem]) {
        for item in out {
            match item {
                OutItem::Char(c) => self.visible.push(*c),
                OutItem::Backspace => {
                    self.visible.pop();
                }
            }
        }
    }

    fn activate(&mut self, out: &[OutItem], followups: Vec<Child>, is_followup: bool) {
        if is_followup {
            // Followup replaces the prior activation's output (sitting at the tail).
            let n = self.last_act_len.min(self.visible.len());
            self.visible.truncate(self.visible.len() - n);
        }
        self.apply_out(out);
        let mut lal = display_len(out).max(0) as usize;
        // Smart space: add a trailing space unless output is empty or ends in
        // space/backspace.
        if self.cfg.smart_space != SmartSpace::None {
            let suppress = out.is_empty()
                || matches!(out.last(), Some(OutItem::Backspace))
                || matches!(out.last(), Some(OutItem::Char(' ')));
            if !suppress {
                self.visible.push(' ');
                lal += 1;
                self.smart_space_sent = self.cfg.smart_space == SmartSpace::Full;
            }
        }
        self.last_act_len = lal;
        self.prioritized = if followups.is_empty() {
            None
        } else {
            Some(followups)
        };
    }
}

// ---------------------------------------------------------------------------
// ReferenceStateMachine
// ---------------------------------------------------------------------------

pub struct ZchRef;

impl ReferenceStateMachine for ZchRef {
    type State = ZchModel;
    type Transition = ZchTransition;

    fn init_state() -> BoxedStrategy<Self::State> {
        (arb_cfg(), arb_roots())
            .prop_map(|(cfg, roots)| ZchModel {
                cfg,
                roots,
                enabled: Enabled::Enabled,
                until_enabled: 0,
                visible: vec![],
                prioritized: None,
                last_act_len: 0,
                smart_space_sent: false,
            })
            .prop_filter("must parse", |m| {
                // `Kanata::new_from_str` configures the process-global zippychord
                // state (ZCH). This filter runs during proptest's *generation*
                // phase, outside `init_test`'s guard, so it must take the same lock
                // the sim tests use — otherwise it clobbers ZCH's dictionary while
                // an unrelated sim test is mid-run, which surfaces as that test's
                // chord silently not expanding.
                let _guard = match CFG_PARSE_LOCK.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                let mut fc = FxHashMap::default();
                fc.insert("file".to_string(), m.tsv());
                Kanata::new_from_str(&m.cfg_string(), fc).is_ok()
            })
            .boxed()
    }

    fn transitions(state: &Self::State) -> BoxedStrategy<Self::Transition> {
        // Build chord targets reachable from the current state.
        //
        // Deferred dimension (see ZIPPY_PBT_NOTES.md): when a followup is pending
        // we offer ONLY followup targets, not fresh roots. A fresh root pressed
        // while a followup is pending can, depending on press order, trigger the
        // pending followup mid-hold (erasing the prior word) — an order-dependent
        // corner whose intended semantics are unsettled. Excluding it keeps the
        // reference's per-transition fresh/followup placement exact.
        let mut targets: Vec<(Target, Vec<char>)> = Vec::new();
        if let Some(children) = &state.prioritized {
            for (i, c) in children.iter().enumerate() {
                targets.push((Target::Followup(i), vec![c.key]));
            }
        } else {
            for (i, r) in state.roots.iter().enumerate() {
                let mut keys: Vec<char> = r.keys.iter().copied().collect();
                if r.lead_space {
                    keys.push(' ');
                }
                targets.push((Target::Root(i), keys));
            }
        }

        let chord = proptest::sample::select(targets).prop_flat_map(|(target, keys)| {
            let n = keys.len();
            let press = Just(keys.clone()).prop_shuffle();
            let release = Just(keys).prop_shuffle();
            // Per-event timing. Presses and releases each stay well within the
            // chord deadline (DEADLINE ticks) so the full chord is guaranteed to
            // form and fire; see `ChordExpansion`'s doc comment. Bounds are sized
            // for the max chord (INPUT_ALPHA + leading space) so the cumulative
            // span of either phase cannot reach DEADLINE.
            let press_delays = prop::collection::vec(0u16..=PRESS_GAP_MAX, n);
            let release_delays = prop::collection::vec(0u16..=RELEASE_GAP_MAX, n);
            (Just(target), press, release, press_delays, release_delays).prop_map(
                move |(target, press, release, press_delays, release_delays)| {
                    let mut events = Vec::with_capacity(2 * n);
                    for (k, d) in press.into_iter().zip(press_delays) {
                        events.push((d, KeyAction::Press(k)));
                    }
                    for (i, (k, d)) in release.into_iter().zip(release_delays).enumerate() {
                        // Settle for at least one tick after the final press so the
                        // activation is processed before the first release.
                        let d = if i == 0 { d.max(1) } else { d };
                        events.push((d, KeyAction::Release(k)));
                    }
                    ZchTransition::ChordExpansion { target, events }
                },
            )
        });
        let literal =
            proptest::sample::select(NONCHORD_ALPHA).prop_map(|key| ZchTransition::Literal { key });
        let idle_tiny = (1u16..=3).prop_map(|ms| ZchTransition::Idle { ms });
        let idle_full = (WAIT + 20..=WAIT + 60).prop_map(|ms| ZchTransition::Idle { ms });
        // The PBT discovered that free typing of chord-participating keys
        // incidentally triggers chord activations, which the naive literal oracle
        // mispredicts. So free typing must exclude every key used by any chord;
        // then no combination of free-typed keys can form a chord. (u/v/w are
        // never chord keys, so the free alphabet is always non-empty.)
        let excluded = chord_keys(&state.roots);
        let free_alpha: Vec<char> = FREE_ALPHA
            .iter()
            .copied()
            .filter(|c| !excluded.contains(c))
            .collect();
        let free = prop::collection::btree_set(proptest::sample::select(free_alpha), 1..=3)
            .prop_flat_map(|set| {
                let keys: Vec<char> = set.into_iter().collect();
                let press = Just(keys.clone()).prop_shuffle();
                let release = Just(keys).prop_shuffle();
                (press, release)
                    .prop_map(|(press, release)| ZchTransition::FreeType { press, release })
            });

        prop_oneof![
            6 => chord,
            2 => literal,
            2 => idle_tiny,
            1 => idle_full,
            3 => free,
        ]
        .boxed()
    }

    fn apply(mut state: Self::State, transition: &Self::Transition) -> Self::State {
        match transition {
            ZchTransition::Idle { ms } => {
                if state.enabled == Enabled::WaitEnable {
                    state.until_enabled = state.until_enabled.saturating_sub(*ms);
                    if state.until_enabled == 0 {
                        state.enabled = Enabled::Enabled;
                    }
                }
            }
            ZchTransition::Literal { key } => {
                // A non-chord key: typed literally, disables zippy (-> WaitEnable
                // on release), clears any pending followups.
                state.smart_space_sent = false;
                state.visible.push(*key);
                state.enabled = Enabled::WaitEnable;
                state.until_enabled = WAIT;
                state.prioritized = None;
                state.last_act_len = 0;
            }
            ZchTransition::ChordExpansion { target, events } => {
                if state.enabled == Enabled::Enabled {
                    let (out, followups, is_followup) = state.resolve(target);
                    state.activate(&out, followups, is_followup);
                    // Activation keeps zippy enabled.
                    state.enabled = Enabled::Enabled;
                } else {
                    // Disabled passthrough: the chord does NOT fire; the keys are
                    // typed literally in press order.
                    state.smart_space_sent = false;
                    for k in ZchTransition::press_order(events) {
                        state.visible.push(k);
                    }
                    state.prioritized = None;
                    state.last_act_len = 0;
                    // Release resets the wait countdown.
                    state.enabled = Enabled::WaitEnable;
                    state.until_enabled = WAIT;
                }
            }
            ZchTransition::FreeType { press, .. } => {
                // Naive oracle: predict ordinary literal typing. This is WRONG
                // whenever the typed keys form/complete a chord (the impl will
                // expand) — the PBT is meant to discover exactly that.
                state.smart_space_sent = false;
                for &k in press {
                    state.visible.push(k);
                }
                state.enabled = Enabled::WaitEnable;
                state.until_enabled = WAIT;
                state.prioritized = None;
                state.last_act_len = 0;
            }
        }
        state
    }

    fn preconditions(state: &Self::State, transition: &Self::Transition) -> bool {
        // These guards keep SHRINKING inside valid space: proptest can shrink the
        // dictionary (in init_state) independently of a transition's stored keys,
        // which would otherwise produce inconsistent transitions (e.g. pressing
        // keys that no longer match the target chord, or free-typing keys that
        // became chord keys after the dict shrank) and spurious failures.
        match transition {
            ZchTransition::ChordExpansion { target, events } => {
                let target_keys: Option<BTreeSet<char>> = match target {
                    Target::Root(i) => state.roots.get(*i).map(|r| {
                        let mut k = r.keys.clone();
                        if r.lead_space {
                            k.insert(' ');
                        }
                        k
                    }),
                    Target::Followup(i) => state
                        .prioritized
                        .as_ref()
                        .and_then(|c| c.get(*i))
                        .map(|c| BTreeSet::from([c.key])),
                };
                match target_keys {
                    Some(tk) => {
                        // Every target key is both pressed and released exactly
                        // once, with all presses preceding all releases so the full
                        // chord is held at the last press (guaranteed activation).
                        let pressed: Vec<char> = ZchTransition::press_order(events);
                        let released: Vec<char> = events
                            .iter()
                            .filter_map(|(_, a)| match a {
                                KeyAction::Release(c) => Some(*c),
                                KeyAction::Press(_) => None,
                            })
                            .collect();
                        let last_press = events
                            .iter()
                            .rposition(|(_, a)| matches!(a, KeyAction::Press(_)));
                        let first_release = events
                            .iter()
                            .position(|(_, a)| matches!(a, KeyAction::Release(_)));
                        let ordered = match (last_press, first_release) {
                            (Some(lp), Some(fr)) => lp < fr,
                            _ => true,
                        };
                        ordered
                            && pressed.iter().copied().collect::<BTreeSet<_>>() == tk
                            && released.iter().copied().collect::<BTreeSet<_>>() == tk
                    }
                    None => false,
                }
            }
            ZchTransition::FreeType { press, release } => {
                let chords = chord_keys(&state.roots);
                press.iter().all(|k| !chords.contains(k))
                    && release.iter().all(|k| !chords.contains(k))
            }
            _ => true,
        }
    }
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

fn arb_cfg() -> impl Strategy<Value = ModelCfg> {
    prop_oneof![
        Just(SmartSpace::None),
        Just(SmartSpace::AddOnly),
        Just(SmartSpace::Full),
    ]
    .prop_map(|smart_space| ModelCfg { smart_space })
}

fn arb_out() -> impl Strategy<Value = Vec<OutItem>> {
    // Deferred dimension (ZIPPY_PBT_NOTES.md): `⌫` (backspace) in output — the
    // suffix-chord pattern — interacts with already-committed text and makes the
    // "tail length this activation owns" ambiguous; not modeled yet.
    let item = prop::sample::select(&['a', 'b', 'c', 'A', 'B', ' '][..]).prop_map(OutItem::Char);
    prop::collection::vec(item, 1..=4)
}

fn arb_child(depth: u32) -> BoxedStrategy<Child> {
    let followups = if depth == 0 {
        Just(Vec::new()).boxed()
    } else {
        prop::collection::vec(arb_child(depth - 1), 0..=1).boxed()
    };
    (prop::sample::select(INPUT_ALPHA), arb_out(), followups)
        .prop_map(|(key, out, followups)| Child {
            key,
            out,
            // dedup sibling children by key
            followups: dedup_children(followups),
        })
        .boxed()
}

fn dedup_children(children: Vec<Child>) -> Vec<Child> {
    let mut seen = BTreeSet::new();
    children
        .into_iter()
        .filter(|c| seen.insert(c.key))
        .collect()
}

fn arb_root() -> impl Strategy<Value = Root> {
    (
        any::<bool>(),
        prop::collection::btree_set(prop::sample::select(INPUT_ALPHA), 1..=INPUT_ALPHA.len()),
        arb_out(),
        prop::collection::vec(arb_child(1), 0..=2),
    )
        .prop_map(|(lead_space, keys, out, followups)| Root {
            lead_space,
            keys,
            out,
            followups: dedup_children(followups),
        })
}

fn arb_roots() -> impl Strategy<Value = Vec<Root>> {
    prop::collection::vec(arb_root(), 1..=5).prop_map(|roots| {
        let mut seen = BTreeSet::new();
        roots
            .into_iter()
            .filter(|r| {
                let mut k = r.keys.clone();
                if r.lead_space {
                    k.insert(' ');
                }
                seen.insert(k)
            })
            .collect()
    })
}

// ---------------------------------------------------------------------------
// SUT
// ---------------------------------------------------------------------------

pub struct Sut {
    kanata: Kanata,
    _guard: MutexGuard<'static, ()>,
}

impl Drop for Sut {
    fn drop(&mut self) {
        // Clear the global PRESSED_KEYS so a panic mid-scenario (the assertion
        // failure that drives shrinking) cannot leak held keys into later tests.
        crate::PRESSED_KEYS.lock().clear();
    }
}

fn osc_of(c: char) -> crate::OsCode {
    let tok = if c == ' ' {
        "spc".to_string()
    } else {
        c.to_string()
    };
    str_to_oscode(&tok).expect("valid key")
}

fn pressed_insert(_osc: crate::OsCode) {
    #[cfg(not(all(target_os = "windows", not(feature = "interception_driver"))))]
    crate::PRESSED_KEYS.lock().insert(_osc);
    #[cfg(all(target_os = "windows", not(feature = "interception_driver")))]
    crate::PRESSED_KEYS
        .lock()
        .insert(_osc, web_time::Instant::now());
}

fn pressed_remove(osc: crate::OsCode) {
    crate::PRESSED_KEYS.lock().remove(&osc);
}

fn feed_press(k: &mut Kanata, c: char) {
    let o = osc_of(c);
    k.handle_input_event(&KeyEvent::new(o, KeyValue::Press))
        .unwrap();
    pressed_insert(o);
    k.tick_ms(1, &None).unwrap();
}

fn feed_release(k: &mut Kanata, c: char) {
    let o = osc_of(c);
    k.handle_input_event(&KeyEvent::new(o, KeyValue::Release))
        .unwrap();
    pressed_remove(o);
    k.tick_ms(1, &None).unwrap();
}

/// Output-stream key-state invariant: a key must never be pressed (`out:↓`)
/// twice without an intervening release (`out:↑`). A second down of an
/// already-held key relies on the OS coalescing the two into one held key, which
/// silently drops the second press's effect — this is exactly how a leading-space
/// chord activated space-first loses its smart-space trailing space (the eager
/// participating `Space` is never released before smart-space presses `Space`
/// again). Returns `Err` naming the offending key on the first violation.
///
/// Key *repeat* is a distinct event (not a second `Press`), so it is not a
/// counterexample; the PBT never generates repeats.
pub(super) fn check_no_double_press(events: &str) -> Result<(), String> {
    let mut down: BTreeSet<&str> = BTreeSet::new();
    for tok in events.split_whitespace() {
        if let Some(name) = tok.strip_prefix("out:↓") {
            if !down.insert(name) {
                return Err(format!("key {name} pressed twice without a release"));
            }
        } else if let Some(name) = tok.strip_prefix("out:↑") {
            down.remove(name);
        }
    }
    Ok(())
}

/// Reconstruct net visible text from raw `out:↓X`/`out:↑X` events.
pub(super) fn net_text(events: &str) -> String {
    let mut out: Vec<char> = Vec::new();
    let mut shift = false;
    for tok in events.split_whitespace() {
        if let Some(name) = tok.strip_prefix("out:↓") {
            match name {
                "LShift" | "RShift" => shift = true,
                "BSpace" => {
                    out.pop();
                }
                "Space" => out.push(' '),
                n => {
                    if let Some(c) = key_to_char(n) {
                        out.push(if shift { c.to_ascii_uppercase() } else { c });
                    }
                }
            }
        } else if let Some(name) = tok.strip_prefix("out:↑") {
            if matches!(name, "LShift" | "RShift") {
                shift = false;
            }
        }
    }
    out.into_iter().collect()
}

fn key_to_char(name: &str) -> Option<char> {
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

pub(super) fn sut_net_text(k: &Kanata) -> String {
    let events = k.kbd_out.outputs.events.join(" ");
    net_text(&events)
}

impl StateMachineTest for Sut {
    type SystemUnderTest = Sut;
    type Reference = ZchRef;

    fn init_test(ref_state: &ZchModel) -> Self::SystemUnderTest {
        let guard = match CFG_PARSE_LOCK.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        crate::PRESSED_KEYS.lock().clear();
        let mut fc = FxHashMap::default();
        fc.insert("file".to_string(), ref_state.tsv());
        let kanata =
            Kanata::new_from_str(&ref_state.cfg_string(), fc).expect("generated cfg must parse");
        Sut {
            kanata,
            _guard: guard,
        }
    }

    fn apply(
        mut state: Self::SystemUnderTest,
        ref_state: &ZchModel,
        transition: ZchTransition,
    ) -> Self::SystemUnderTest {
        let k = &mut state.kanata;
        match &transition {
            ZchTransition::Idle { ms } => {
                k.tick_ms(*ms as u128, &None).unwrap();
            }
            ZchTransition::Literal { key } => {
                feed_press(k, *key);
                feed_release(k, *key);
            }
            ZchTransition::ChordExpansion { events, .. } => {
                for (delay, action) in events {
                    if *delay > 0 {
                        k.tick_ms(*delay as u128, &None).unwrap();
                    }
                    match action {
                        KeyAction::Press(c) => feed_press(k, *c),
                        KeyAction::Release(c) => feed_release(k, *c),
                    }
                }
            }
            ZchTransition::FreeType { press, release } => {
                for &c in press {
                    feed_press(k, c);
                }
                k.tick_ms(1, &None).unwrap();
                for &c in release {
                    feed_release(k, c);
                }
            }
        }
        let raw = k.kbd_out.outputs.events.join(" ");
        // Output key-state invariant (independent of the net-text oracle, which
        // is blind to OS key coalescing): no key may be pressed twice without a
        // release in between. Catches the leading-space/smart-space space-first
        // trailing-space loss and any similar double-press defect.
        if let Err(e) = check_no_double_press(&raw) {
            panic!(
                "output key-state invariant violated: {e}\n  transition: {transition:?}\n  cfg: {}\n  dict: {}\n  raw: {raw}",
                ref_state.cfg_string(),
                ref_state.tsv().replace('\n', " | "),
            );
        }
        let got = sut_net_text(k);
        let expected: String = ref_state.visible.iter().collect();
        assert_eq!(
            expected,
            got,
            "\n  transition: {:?}\n  cfg: {}\n  dict: {}\n  raw: {}",
            transition,
            ref_state.cfg_string(),
            ref_state.tsv().replace('\n', " | "),
            raw
        );
        state
    }
}

prop_state_machine_persisted! {
    #![proptest_config(Config { cases: 3000, .. Config::default() })]
    #[test]
    fn zippychord_state_machine(sequential 1..32 => Sut);
}

// ---------------------------------------------------------------------------
// Reference self-consistency tests: drive ZchRef::apply directly and check the
// predicted `visible` against hand-computed expectations. These validate that
// the oracle is trustworthy (so a state-machine failure means a real impl bug,
// not a reference bug).
// ---------------------------------------------------------------------------
#[cfg(test)]
mod reference_tests {
    use super::*;

    fn out(s: &str) -> Vec<OutItem> {
        s.chars().map(OutItem::Char).collect()
    }
    fn root(lead_space: bool, keys: &str, o: &str, followups: Vec<Child>) -> Root {
        Root {
            lead_space,
            keys: keys.chars().collect(),
            out: out(o),
            followups,
        }
    }
    fn child(key: char, o: &str, followups: Vec<Child>) -> Child {
        Child {
            key,
            out: out(o),
            followups,
        }
    }
    fn model(smart_space: SmartSpace, roots: Vec<Root>) -> ZchModel {
        ZchModel {
            cfg: ModelCfg { smart_space },
            roots,
            enabled: Enabled::Enabled,
            until_enabled: 0,
            visible: vec![],
            prioritized: None,
            last_act_len: 0,
            smart_space_sent: false,
        }
    }
    fn chord(target: Target, keys: &str) -> ZchTransition {
        // All presses (delay 0) then all releases — the smart generator's
        // invariant — so the reference's disabled-passthrough press order is well
        // defined. The enabled path ignores the events entirely.
        let mut events: Vec<(u16, KeyAction)> =
            keys.chars().map(|c| (0u16, KeyAction::Press(c))).collect();
        events.extend(keys.chars().map(|c| (0u16, KeyAction::Release(c))));
        ZchTransition::ChordExpansion { target, events }
    }
    fn apply(m: ZchModel, tr: &ZchTransition) -> ZchModel {
        <ZchRef as ReferenceStateMachine>::apply(m, tr)
    }
    fn vis(m: &ZchModel) -> String {
        m.visible.iter().collect()
    }

    #[test]
    fn ref_single_fresh() {
        let m = model(SmartSpace::None, vec![root(false, "ab", "xy", vec![])]);
        let m = apply(m, &chord(Target::Root(0), "ab"));
        assert_eq!("xy", vis(&m));
    }

    #[test]
    fn ref_two_words_append() {
        let m = model(
            SmartSpace::None,
            vec![root(false, "a", "P", vec![]), root(false, "b", "Q", vec![])],
        );
        let m = apply(m, &chord(Target::Root(0), "a"));
        let m = apply(m, &chord(Target::Root(1), "b"));
        assert_eq!("PQ", vis(&m));
    }

    #[test]
    fn ref_target_is_final_chord_output() {
        // Pressing the larger chord's keys yields its output regardless of any
        // smaller subset chord (the reference places the target output).
        let m = model(
            SmartSpace::None,
            vec![
                root(false, "a", "P", vec![]),
                root(false, "ab", "QQ", vec![]),
            ],
        );
        let m = apply(m, &chord(Target::Root(1), "ab"));
        assert_eq!("QQ", vis(&m));
    }

    #[test]
    fn ref_leading_space_swallowed() {
        // " a" -> "a": the participating space is not part of the output.
        let m = model(SmartSpace::None, vec![root(true, "a", "a", vec![])]);
        let m = apply(m, &chord(Target::Root(0), "a "));
        assert_eq!("a", vis(&m));
    }

    #[test]
    fn ref_followup_replaces_prior() {
        let m = model(
            SmartSpace::None,
            vec![root(false, "a", "X", vec![child('b', "Y", vec![])])],
        );
        let m = apply(m, &chord(Target::Root(0), "a"));
        assert_eq!("X", vis(&m));
        let m = apply(m, &chord(Target::Followup(0), "b"));
        assert_eq!("Y", vis(&m));
    }

    #[test]
    fn ref_smart_space_add_only_appends_space() {
        let m = model(SmartSpace::AddOnly, vec![root(false, "a", "X", vec![])]);
        let m = apply(m, &chord(Target::Root(0), "a"));
        assert_eq!("X ", vis(&m));
    }

    #[test]
    fn ref_smart_space_followup_replaces_with_trailing_space() {
        let m = model(
            SmartSpace::AddOnly,
            vec![root(false, "a", "day", vec![child('b', "Monday", vec![])])],
        );
        let m = apply(m, &chord(Target::Root(0), "a"));
        assert_eq!("day ", vis(&m));
        let m = apply(m, &chord(Target::Followup(0), "b"));
        assert_eq!("Monday ", vis(&m));
    }

    #[test]
    fn ref_literal_disables_then_idle_reenables() {
        let m = model(SmartSpace::None, vec![root(false, "a", "X", vec![])]);
        // Type a non-chord literal: appended, zippy goes to WaitEnable.
        let m = apply(m, &ZchTransition::Literal { key: 'z' });
        assert_eq!("z", vis(&m));
        assert_eq!(Enabled::WaitEnable, m.enabled);
        // A chord while WaitEnable does not fire: passthrough of its keys.
        let m = apply(m, &chord(Target::Root(0), "a"));
        assert_eq!("za", vis(&m));
        // A full idle re-enables; now the chord fires.
        let m = apply(m, &ZchTransition::Idle { ms: WAIT + 10 });
        assert_eq!(Enabled::Enabled, m.enabled);
        let m = apply(m, &chord(Target::Root(0), "a"));
        assert_eq!("zaX", vis(&m));
    }
}
