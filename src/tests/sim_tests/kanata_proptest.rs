//! Stateful property tests for Kanata, using `proptest-state-machine`.
//!
//! This file is growing from a zippychord-only PBT into a general harness that
//! hosts one feature module per Kanata construct. It currently holds three
//! state machines plus a shared invariant catalog:
//!
//! - `zippychord_state_machine` (`KanataRef`/`KanataModel`/`Sut`) — the COUPLED
//!   model. The reference generates a config + chord dictionary AND a layout with
//!   optional tap-hold keys, then a transition stream; the SUT is a real `Kanata`.
//!   The oracle is split: a `ChordExpansion` transition *carries which chord it
//!   activates* (expansion known by construction), while the reference supplies
//!   only *placement* (fresh append / followup replace / disabled passthrough)
//!   from coarse engine state. A tap-hold key's resolved tap/hold output feeds the
//!   SAME zippy enable-state + visible buffer (`type_literal`) — the cross-cutting
//!   coupling. The reference deliberately does NOT reimplement the keystroke-level
//!   eager/overlap/backspace accounting NOR the tap-hold-vs-deadline arbitration —
//!   that is the code under test — so those bugs surface as a mismatch. (This is
//!   how the common-prefix backspace under-count bug was found.) Observable: net
//!   visible text.
//! - `taphold_state_machine` (`ThRef`/`ThModel`/`ThSut`) — tap-hold in ISOLATION,
//!   a pure construction oracle on the lower-level **event stream** (`event_seq`),
//!   with tap output ≠ input. Kept as a fast/isolated slice because it provides an
//!   observable and a coverage region (tap≠self) the coupled net-text model cannot.
//! - `interaction_taphold_zippy_order_independent` — the chord×tap-hold overlap
//!   (a key that is both), judged by a metamorphic determinism oracle. `#[ignore]`d:
//!   RED against the documented press-order bug. The coupled model does NOT
//!   generate this overlap (chord keys and tap-hold inputs are disjoint alphabets).
//!
//! Coverage / deferred dimensions and the oracle/decomposition design are tracked
//! in ZIPPY_PBT_NOTES.md.

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
pub struct KanataModel {
    cfg: ModelCfg,
    roots: Vec<Root>,
    // Tap-hold keys in the layout (coupled feature). Their resolved output feeds
    // the SAME zippy engine state below — the cross-cutting coupling. `default`
    // so older persisted seeds (no tap-hold) still deserialize as empty.
    #[serde(default)]
    taphold: Vec<ThKey>,
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
pub enum KanataTransition {
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
    /// Tap the `i`th tap-hold key (release before the hold timeout). Its tap
    /// output is a non-chord key, so — like a `Literal` — it types that char and
    /// disables zippy. Exercises the layout→zippy coupling.
    TapHoldTap(usize),
    /// Hold the `i`th tap-hold key past the hold timeout. Emits its hold output
    /// (also a non-chord key) and disables zippy.
    TapHoldHold(usize),
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

impl KanataTransition {
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

impl KanataModel {
    fn cfg_string(&self) -> String {
        let ss = match self.cfg.smart_space {
            SmartSpace::None => "none",
            SmartSpace::AddOnly => "add-space-only",
            SmartSpace::Full => "full",
        };
        // Tap-hold keys join the layout; chord keys remain unmapped passthrough
        // (zippychord reads the layout's output). tap output = the key itself so a
        // tap is an ordinary keystroke; hold output is a distinct non-chord key.
        let mut src = String::from("lalt");
        let mut lay = String::from("lalt");
        for k in &self.taphold {
            src.push(' ');
            src.push(k.input);
            lay.push_str(&format!(
                " (tap-hold {TH_TAP_TIMEOUT} {TH_HOLD_TIMEOUT} {} {})",
                k.tap_out, k.hold_out
            ));
        }
        format!(
            "(defsrc {src})(deflayer base {lay})(defzippy file \
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

impl KanataModel {
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

    /// Type one non-chord char literally: appends it and drops zippy into its
    /// post-typing disabled window. Shared by `Literal` and the tap-hold gestures
    /// (whose resolved output reaches zippy as an ordinary keystroke).
    fn type_literal(&mut self, c: char) {
        self.smart_space_sent = false;
        self.visible.push(c);
        self.enabled = Enabled::WaitEnable;
        self.until_enabled = WAIT;
        self.prioritized = None;
        self.last_act_len = 0;
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

pub struct KanataRef;

impl ReferenceStateMachine for KanataRef {
    type State = KanataModel;
    type Transition = KanataTransition;

    fn init_state() -> BoxedStrategy<Self::State> {
        (arb_cfg(), arb_roots(), arb_taphold())
            .prop_map(|(cfg, roots, taphold)| KanataModel {
                cfg,
                roots,
                taphold,
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
                    KanataTransition::ChordExpansion { target, events }
                },
            )
        });
        // Literal types a single plain non-chord key. It must exclude tap-hold
        // input keys: pressed via Literal's brief settle the tap-hold tap stays
        // half-resolved (only TapHoldTap settles past the hold timeout), so the
        // count diverges. Tap-hold tap output is covered by TapHoldTap instead.
        let th_inputs: BTreeSet<char> = state.taphold.iter().map(|k| k.input).collect();
        let literal_alpha: Vec<char> = NONCHORD_ALPHA
            .iter()
            .copied()
            .filter(|c| !th_inputs.contains(c))
            .collect();
        let literal = proptest::sample::select(literal_alpha)
            .prop_map(|key| KanataTransition::Literal { key });
        let idle_tiny = (1u16..=3).prop_map(|ms| KanataTransition::Idle { ms });
        let idle_full = (WAIT + 20..=WAIT + 60).prop_map(|ms| KanataTransition::Idle { ms });
        // The PBT discovered that free typing of chord-participating keys
        // incidentally triggers chord activations, which the naive literal oracle
        // mispredicts. So free typing must exclude every key used by any chord;
        // then no combination of free-typed keys can form a chord. It must ALSO
        // exclude tap-hold input keys: a tap-hold key DELAYS its tap output, so it
        // lands out of press order relative to a plain key in the same gesture
        // (the SUT emits e.g. `wv`, the naive oracle predicts `vw`). The
        // single-key TapHoldTap/Hold transitions cover tap-hold output instead;
        // multi-key free typing over tap-hold keys is a deferred dimension. (w is
        // never a chord or tap-hold input, so the free alphabet stays non-empty.)
        let excluded = chord_keys(&state.roots);
        let free_alpha: Vec<char> = FREE_ALPHA
            .iter()
            .copied()
            .filter(|c| !excluded.contains(c) && !th_inputs.contains(c))
            .collect();
        let free = prop::collection::btree_set(proptest::sample::select(free_alpha), 1..=3)
            .prop_flat_map(|set| {
                let keys: Vec<char> = set.into_iter().collect();
                let press = Just(keys.clone()).prop_shuffle();
                let release = Just(keys).prop_shuffle();
                (press, release)
                    .prop_map(|(press, release)| KanataTransition::FreeType { press, release })
            });

        // Tap-hold gestures, offered only when the keymap has tap-hold keys.
        // Degrades to a tiny idle when there are none so the arm is always a valid
        // strategy (the weight then just adds harmless idles).
        let taphold: BoxedStrategy<KanataTransition> = if state.taphold.is_empty() {
            (1u16..=3).prop_map(|ms| KanataTransition::Idle { ms }).boxed()
        } else {
            let idxs: Vec<usize> = (0..state.taphold.len()).collect();
            let tap = proptest::sample::select(idxs.clone()).prop_map(KanataTransition::TapHoldTap);
            let hold = proptest::sample::select(idxs).prop_map(KanataTransition::TapHoldHold);
            prop_oneof![tap, hold].boxed()
        };

        prop_oneof![
            6 => chord,
            2 => literal,
            2 => idle_tiny,
            1 => idle_full,
            3 => free,
            3 => taphold,
        ]
        .boxed()
    }

    fn apply(mut state: Self::State, transition: &Self::Transition) -> Self::State {
        match transition {
            KanataTransition::Idle { ms } => {
                if state.enabled == Enabled::WaitEnable {
                    state.until_enabled = state.until_enabled.saturating_sub(*ms);
                    if state.until_enabled == 0 {
                        state.enabled = Enabled::Enabled;
                    }
                }
            }
            KanataTransition::Literal { key } => {
                // A non-chord key: typed literally, disables zippy (-> WaitEnable
                // on release), clears any pending followups.
                state.type_literal(*key);
            }
            KanataTransition::TapHoldTap(i) => {
                // The tap-hold key resolves to its tap output (a non-chord key),
                // which reaches zippy exactly as a literal would: types the char
                // and disables zippy. This is the layout->zippy coupling.
                let c = state.taphold[*i].tap_out;
                state.type_literal(c);
            }
            KanataTransition::TapHoldHold(i) => {
                let c = state.taphold[*i].hold_out;
                state.type_literal(c);
            }
            KanataTransition::ChordExpansion { target, events } => {
                if state.enabled == Enabled::Enabled {
                    let (out, followups, is_followup) = state.resolve(target);
                    state.activate(&out, followups, is_followup);
                    // Activation keeps zippy enabled.
                    state.enabled = Enabled::Enabled;
                } else {
                    // Disabled passthrough: the chord does NOT fire; the keys are
                    // typed literally in press order.
                    state.smart_space_sent = false;
                    for k in KanataTransition::press_order(events) {
                        state.visible.push(k);
                    }
                    state.prioritized = None;
                    state.last_act_len = 0;
                    // Release resets the wait countdown.
                    state.enabled = Enabled::WaitEnable;
                    state.until_enabled = WAIT;
                }
            }
            KanataTransition::FreeType { press, .. } => {
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
            KanataTransition::ChordExpansion { target, events } => {
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
                        let pressed: Vec<char> = KanataTransition::press_order(events);
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
            KanataTransition::FreeType { press, release } => {
                let chords = chord_keys(&state.roots);
                let th: BTreeSet<char> = state.taphold.iter().map(|k| k.input).collect();
                let ok = |k: &char| !chords.contains(k) && !th.contains(k);
                press.iter().all(ok) && release.iter().all(ok)
            }
            // Guard against the tap-hold list shrinking out from under a stored
            // index (init_state shrinks independently of transitions).
            KanataTransition::TapHoldTap(i) | KanataTransition::TapHoldHold(i) => {
                *i < state.taphold.len()
            }
            // A literal must stay a plain key; if the dict shrank a key into a
            // tap-hold input, drop it (tap-hold tap is covered by TapHoldTap).
            KanataTransition::Literal { key } => {
                !state.taphold.iter().any(|k| k.input == *key)
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

/// 0..=2 tap-hold keys for the layout. Inputs are drawn from NONCHORD_ALPHA
/// (disjoint from chord keys), tap output = the key itself (an ordinary
/// keystroke), hold output = a distinct non-chord key. Empty is allowed (and is
/// the default for older seeds) — then the model reduces to the pure zippy SM.
fn arb_taphold() -> impl Strategy<Value = Vec<ThKey>> {
    // inputs/tap from the low half (u,v), hold from the high half (x,y) so a hold
    // output is never itself a tap-hold input.
    (0usize..=2).prop_map(|n| {
        (0..n)
            .map(|i| ThKey {
                input: NONCHORD_ALPHA[i],
                tap_out: NONCHORD_ALPHA[i],
                hold_out: NONCHORD_ALPHA[i + 3],
            })
            .collect()
    })
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

// ===========================================================================
// Capability-selected invariant catalog (shared spine)
// ===========================================================================
//
// Decomposition discipline (see ZIPPY_PBT_NOTES.md): kanata's features are
// modelled as the real runtime *components* (capabilities), NOT one cap per
// feature and never a cap named after a property. A test slice declares which
// capabilities its generated keymap exercises; the catalog runs exactly the
// invariants whose need-set is satisfied. `OutputKeyState` is universal, so the
// legality invariants run over every slice for free (coverage is multiplicative:
// a new invariant lights up every slice that has its caps).
//
// Selection rule: runs ⟺ needs_pos ⊆ present ∧ needs_neg ∩ present = ∅. The
// negative half is for degraded-mode twins (e.g. smart-space full vs none).

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Cap {
    /// The serialized output keystroke stream (which keys are down). Universal.
    OutputKeyState,
    /// The reconstructed visible-text buffer (zippychord + smart-space write it).
    VisibleText,
    /// Zippychord engine state (enabled/disabled, pending followups).
    ZippyState,
    /// Layout/layer resolution state (tap-hold, layers, oneshot, tap-dance, ...).
    LayoutState,
}

pub(super) type CapSet = BTreeSet<Cap>;

pub(super) fn capset(caps: &[Cap]) -> CapSet {
    caps.iter().copied().collect()
}

/// One catalog invariant over the raw cumulative event stream, tagged with the
/// component capabilities it requires present (`needs_pos`) and absent
/// (`needs_neg`). `id` is stable so selection-parity checks can name it.
struct Invariant {
    id: &'static str,
    needs_pos: &'static [Cap],
    needs_neg: &'static [Cap],
    check: fn(&str) -> Result<(), String>,
}

impl Invariant {
    fn selected(&self, present: &CapSet) -> bool {
        self.needs_pos.iter().all(|c| present.contains(c))
            && self.needs_neg.iter().all(|c| !present.contains(c))
    }
}

/// The single shared catalog. Authored once; every slice runs the selected
/// subset over the same tick. Add an entry here and it lights up every slice
/// that has its capabilities — no per-slice duplication.
static INVARIANTS: &[Invariant] = &[Invariant {
    id: "no_double_press",
    needs_pos: &[Cap::OutputKeyState],
    needs_neg: &[],
    check: check_no_double_press,
}];

/// Run every selected invariant over one tick's cumulative event stream;
/// returns the offending invariant id + message on the first violation.
pub(super) fn run_invariants(present: &CapSet, raw: &str) -> Result<(), (&'static str, String)> {
    for inv in INVARIANTS {
        if inv.selected(present) {
            (inv.check)(raw).map_err(|e| (inv.id, e))?;
        }
    }
    Ok(())
}

/// Ids of the invariants selected for a slice. Used by the selection self-tests
/// to assert a non-empty, expected footprint — a slice can otherwise be green
/// purely because every property touching its weak spot was deselected.
pub(super) fn selected_ids(present: &CapSet) -> Vec<&'static str> {
    INVARIANTS
        .iter()
        .filter(|i| i.selected(present))
        .map(|i| i.id)
        .collect()
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
    type Reference = KanataRef;

    fn init_test(ref_state: &KanataModel) -> Self::SystemUnderTest {
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
        ref_state: &KanataModel,
        transition: KanataTransition,
    ) -> Self::SystemUnderTest {
        let k = &mut state.kanata;
        match &transition {
            KanataTransition::Idle { ms } => {
                k.tick_ms(*ms as u128, &None).unwrap();
            }
            KanataTransition::Literal { key } => {
                feed_press(k, *key);
                feed_release(k, *key);
            }
            KanataTransition::TapHoldTap(i) => {
                // Press + release before the hold timeout => tap output. The
                // settle tick stays below WAIT so it can't re-enable mid-transition
                // (the reference doesn't decrement until_enabled here); the bimodal
                // idle generator guarantees no idle lands in the ambiguous middle.
                let c = ref_state.taphold[*i].input;
                feed_press(k, c);
                k.tick_ms(4, &None).unwrap();
                feed_release(k, c);
                k.tick_ms((TH_HOLD_TIMEOUT + 50) as u128, &None).unwrap();
            }
            KanataTransition::TapHoldHold(i) => {
                // Hold past the hold timeout => hold output, then release.
                let c = ref_state.taphold[*i].input;
                feed_press(k, c);
                k.tick_ms((TH_HOLD_TIMEOUT + 5) as u128, &None).unwrap();
                feed_release(k, c);
                k.tick_ms(50, &None).unwrap();
            }
            KanataTransition::ChordExpansion { events, .. } => {
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
            KanataTransition::FreeType { press, release } => {
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
        // Capability-selected catalog invariants (independent of the net-text
        // oracle, which is blind to OS key coalescing). This slice's components:
        // the output stream, the visible-text buffer, the zippy engine, and the
        // layout/layer resolution (tap-hold keys).
        let present = capset(&[
            Cap::OutputKeyState,
            Cap::VisibleText,
            Cap::ZippyState,
            Cap::LayoutState,
        ]);
        if let Err((id, e)) = run_invariants(&present, &raw) {
            panic!(
                "invariant `{id}` violated: {e}\n  transition: {transition:?}\n  cfg: {}\n  dict: {}\n  raw: {raw}",
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
// Reference self-consistency tests: drive KanataRef::apply directly and check the
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
    fn model(smart_space: SmartSpace, roots: Vec<Root>) -> KanataModel {
        KanataModel {
            cfg: ModelCfg { smart_space },
            roots,
            taphold: vec![],
            enabled: Enabled::Enabled,
            until_enabled: 0,
            visible: vec![],
            prioritized: None,
            last_act_len: 0,
            smart_space_sent: false,
        }
    }
    fn chord(target: Target, keys: &str) -> KanataTransition {
        // All presses (delay 0) then all releases — the smart generator's
        // invariant — so the reference's disabled-passthrough press order is well
        // defined. The enabled path ignores the events entirely.
        let mut events: Vec<(u16, KeyAction)> =
            keys.chars().map(|c| (0u16, KeyAction::Press(c))).collect();
        events.extend(keys.chars().map(|c| (0u16, KeyAction::Release(c))));
        KanataTransition::ChordExpansion { target, events }
    }
    fn apply(m: KanataModel, tr: &KanataTransition) -> KanataModel {
        <KanataRef as ReferenceStateMachine>::apply(m, tr)
    }
    fn vis(m: &KanataModel) -> String {
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
        let m = apply(m, &KanataTransition::Literal { key: 'z' });
        assert_eq!("z", vis(&m));
        assert_eq!(Enabled::WaitEnable, m.enabled);
        // A chord while WaitEnable does not fire: passthrough of its keys.
        let m = apply(m, &chord(Target::Root(0), "a"));
        assert_eq!("za", vis(&m));
        // A full idle re-enables; now the chord fires.
        let m = apply(m, &KanataTransition::Idle { ms: WAIT + 10 });
        assert_eq!(Enabled::Enabled, m.enabled);
        let m = apply(m, &chord(Target::Root(0), "a"));
        assert_eq!("zaX", vis(&m));
    }
}

// ===========================================================================
// Second feature module: tap-hold (construction oracle)
// ===========================================================================
//
// Demonstrates the state-machine harness generalising beyond zippychord, and is
// the seam toward the real goal — *interaction* testing (see ZIPPY_PBT_NOTES.md).
//
// Two-oracle recap: zippychord splits its oracle (the `ChordExpansion`
// transition carries the expansion; the reference carries placement). Tap-hold
// here uses the other half on its own — a pure **construction oracle**. Each
// transition is a gesture kept in the *unambiguous timing interior*: released
// well before the hold timeout (=> tap) or held well past it (=> hold). So the
// expected output is known by construction with NO timing model — the same
// "stay inside the deadline" trick the chord generator uses, applied to the
// tap/hold decision boundary instead of the chord deadline.
//
// Observable: the normalised output **event stream** — the ordered ↓/↑ of output
// keys (`event_seq`), shared with the zippychord test. Timing is intentionally
// not pinned (it is impl detail); only the key-press order is asserted.
//
// Why tap-hold is the chosen second feature: it is exactly the construct the
// zippychord PBT is blind to. The PBT builds its SUT from a trivial passthrough
// layout, so layout output reaches zippychord immediately and identically
// regardless of press order; once a chord-participating key is a tap-hold (or
// layer) key the layout *delays* that key's tap output by an order/timing-
// dependent amount, which is where the known press-order bugs live. Hosting
// tap-hold in this harness is the prerequisite for generating a keymap where a
// single key is both a tap-hold action and a chord participant (the interaction
// the construction oracles can set up but only the invariants can judge).

const TH_INPUTS: &[char] = &['a', 'b', 'c'];
const TH_TAP_OUTS: &[char] = &['d', 'e', 'f'];
const TH_HOLD_OUTS: &[char] = &['x', 'y', 'z'];
// Fixed timeouts. tap=0 (eager tap) keeps the tap path simple; hold=200 is the
// decision boundary the gestures stay well clear of in both directions.
const TH_TAP_TIMEOUT: u16 = 0;
const TH_HOLD_TIMEOUT: u16 = 200;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ThKey {
    input: char,
    tap_out: char,
    hold_out: char,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ThModel {
    keys: Vec<ThKey>,
    /// Accumulated expected output stream: (is_down, output char), known by
    /// construction from each gesture's tap/hold branch.
    expected: Vec<(bool, char)>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ThTransition {
    /// Press key `i` and release it before the hold timeout => tap output.
    Tap(usize),
    /// Press key `i` and hold past the hold timeout, then release => hold output.
    Hold(usize),
    /// Pure idle; emits nothing.
    Idle(u16),
}

impl ThModel {
    fn cfg_string(&self) -> String {
        let src = self
            .keys
            .iter()
            .map(|k| k.input.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let layer = self
            .keys
            .iter()
            .map(|k| {
                format!(
                    "(tap-hold {TH_TAP_TIMEOUT} {TH_HOLD_TIMEOUT} {} {})",
                    k.tap_out, k.hold_out
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!("(defsrc {src})(deflayer base {layer})")
    }
}

/// Normalise an output event string to the ordered (is_down, char) stream,
/// dropping timing. Output keys are single letters here, so `key_to_char`
/// decodes them directly. Shared observable with the zippychord net-text oracle.
fn event_seq(events: &str) -> Vec<(bool, char)> {
    let mut v = Vec::new();
    for tok in events.split_whitespace() {
        if let Some(name) = tok.strip_prefix("out:↓") {
            if let Some(c) = key_to_char(name) {
                v.push((true, c));
            }
        } else if let Some(name) = tok.strip_prefix("out:↑") {
            if let Some(c) = key_to_char(name) {
                v.push((false, c));
            }
        }
    }
    v
}

pub struct ThRef;

impl ReferenceStateMachine for ThRef {
    type State = ThModel;
    type Transition = ThTransition;

    fn init_state() -> BoxedStrategy<Self::State> {
        (1usize..=TH_INPUTS.len())
            .prop_map(|n| ThModel {
                keys: (0..n)
                    .map(|i| ThKey {
                        input: TH_INPUTS[i],
                        tap_out: TH_TAP_OUTS[i],
                        hold_out: TH_HOLD_OUTS[i],
                    })
                    .collect(),
                expected: vec![],
            })
            .boxed()
    }

    fn transitions(state: &Self::State) -> BoxedStrategy<Self::Transition> {
        let n = state.keys.len();
        let idxs: Vec<usize> = (0..n).collect();
        let tap = proptest::sample::select(idxs.clone()).prop_map(ThTransition::Tap);
        let hold = proptest::sample::select(idxs).prop_map(ThTransition::Hold);
        let idle = (1u16..=5).prop_map(ThTransition::Idle);
        prop_oneof![4 => tap, 4 => hold, 1 => idle].boxed()
    }

    fn apply(mut state: Self::State, transition: &Self::Transition) -> Self::State {
        match transition {
            ThTransition::Tap(i) => {
                let c = state.keys[*i].tap_out;
                state.expected.push((true, c));
                state.expected.push((false, c));
            }
            ThTransition::Hold(i) => {
                let c = state.keys[*i].hold_out;
                state.expected.push((true, c));
                state.expected.push((false, c));
            }
            ThTransition::Idle(_) => {}
        }
        state
    }

    fn preconditions(state: &Self::State, transition: &Self::Transition) -> bool {
        // Guard shrinking: the dictionary (key count) can shrink independently of
        // a transition's stored index, so reject indices that fell out of range.
        match transition {
            ThTransition::Tap(i) | ThTransition::Hold(i) => *i < state.keys.len(),
            ThTransition::Idle(_) => true,
        }
    }
}

pub struct ThSut {
    kanata: Kanata,
    _guard: MutexGuard<'static, ()>,
}

impl Drop for ThSut {
    fn drop(&mut self) {
        crate::PRESSED_KEYS.lock().clear();
    }
}

impl ThSut {
    fn press(&mut self, c: char) {
        let o = osc_of(c);
        self.kanata
            .handle_input_event(&KeyEvent::new(o, KeyValue::Press))
            .unwrap();
        pressed_insert(o);
    }
    fn release(&mut self, c: char) {
        let o = osc_of(c);
        self.kanata
            .handle_input_event(&KeyEvent::new(o, KeyValue::Release))
            .unwrap();
        pressed_remove(o);
    }
    fn tick(&mut self, ms: u16) {
        self.kanata.tick_ms(ms as u128, &None).unwrap();
    }
}

impl StateMachineTest for ThSut {
    type SystemUnderTest = ThSut;
    type Reference = ThRef;

    fn init_test(ref_state: &ThModel) -> Self::SystemUnderTest {
        let guard = match CFG_PARSE_LOCK.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        crate::PRESSED_KEYS.lock().clear();
        let kanata = Kanata::new_from_str(&ref_state.cfg_string(), FxHashMap::default())
            .expect("generated tap-hold cfg must parse");
        ThSut {
            kanata,
            _guard: guard,
        }
    }

    fn apply(
        mut state: Self::SystemUnderTest,
        ref_state: &ThModel,
        transition: ThTransition,
    ) -> Self::SystemUnderTest {
        match &transition {
            ThTransition::Tap(i) => {
                let c = ref_state.keys[*i].input;
                state.press(c);
                state.tick(5); // released well before hold timeout => tap
                state.release(c);
                state.tick(TH_HOLD_TIMEOUT + 50); // settle past the boundary
            }
            ThTransition::Hold(i) => {
                let c = ref_state.keys[*i].input;
                state.press(c);
                state.tick(TH_HOLD_TIMEOUT + 5); // held past hold timeout => hold
                state.release(c);
                state.tick(50);
            }
            ThTransition::Idle(ms) => {
                state.tick(*ms);
            }
        }
        let raw = state.kanata.kbd_out.outputs.events.join(" ");
        // Same shared catalog as the zippychord slice, selected by this slice's
        // components: the output stream and the layout/layer resolution state.
        let present = capset(&[Cap::OutputKeyState, Cap::LayoutState]);
        if let Err((id, e)) = run_invariants(&present, &raw) {
            panic!(
                "invariant `{id}` violated: {e}\n  transition: {transition:?}\n  cfg: {}\n  raw: {raw}",
                ref_state.cfg_string(),
            );
        }
        // Construction oracle: the accumulated output stream must match exactly.
        let got = event_seq(&raw);
        assert_eq!(
            ref_state.expected, got,
            "\n  transition: {:?}\n  cfg: {}\n  raw: {}",
            transition,
            ref_state.cfg_string(),
            raw
        );
        state
    }
}

prop_state_machine_persisted! {
    #![proptest_config(Config { cases: 256, .. Config::default() })]
    #[test]
    fn taphold_state_machine(sequential 1..16 => ThSut);
}

// ===========================================================================
// Interaction prototype: tap-hold x zippychord, judged by a determinism oracle
// ===========================================================================
//
// The two feature modules above each predict their isolated output. Their
// *interaction* is where the construction oracles run out: when a single key is
// both a tap-hold action and a zippychord chord participant, the layout delays
// that key's tap output by an order/timing-dependent amount, and the combined
// output is no longer a pure function of the gesture that either module can
// state. The framework still tests it — with a *metamorphic* oracle that needs
// no prediction: a chord is a set, so pressing its keys in either micro-order is
// the same physical gesture and MUST produce the same visible text.
//
// This is the harness auto-discovering the documented press-order bug (the
// pinned regression `zippychord_sim_tests::sim_zippy_taphold_chord_press_order_dependent`):
// space is a 200ms tap-hold thumb key, the chord is leading-space ` n`->`no`,
// and `n`-first vs `space`-first diverge. The test is `#[ignore]`d because it is
// RED against current behaviour (it asserts the *intended* invariant, not the
// buggy status quo); run it with `--ignored` to see the shrunk counterexample.
// It marks exactly the deferred interaction dimension this prototype exists to
// reach; un-ignore it once the press-order bug is fixed.
fn sim_zippy_file(cfg: &str, input: &str, content: &str) -> String {
    let mut fc = FxHashMap::default();
    fc.insert("file".to_string(), content.to_string());
    super::simulate_with_file_content(cfg, input, fc)
}

proptest! {
    #![proptest_config(Config { cases: 64, .. Config::default() })]
    #[test]
    #[ignore = "RED: framework reproduction of the tap-hold x zippychord press-order bug; un-ignore when fixed"]
    fn interaction_taphold_zippy_order_independent(
        deadline in 10u16..=80,
        hold_gap in 5u16..=40,
    ) {
        // `spc` is a tap-hold thumb key (its tap output is delayed); it also
        // participates in the leading-space chord ` n`->`no`. `n` is plain.
        let cfg = format!(
            "(defsrc spc n)\
             (deflayer base (tap-hold 200 200 spc (layer-while-held l2)) n)\
             (deflayer l2 spc n)\
             (defzippy file on-first-press-chord-deadline {deadline} \
              idle-reactivate-time 100 smart-space full)"
        );
        let content = "\n n\tno\n";

        let space_first = net_text(&sim_zippy_file(
            &cfg,
            &format!("d:spc d:n t:{hold_gap} u:spc u:n t:300"),
            content,
        ));
        let n_first = net_text(&sim_zippy_file(
            &cfg,
            &format!("d:n d:spc t:{hold_gap} u:n u:spc t:300"),
            content,
        ));

        // A chord is a set: the two press orders are the same physical gesture
        // and must yield the same visible text.
        prop_assert_eq!(
            &space_first, &n_first,
            "press-order changed the chord output (deadline={}, hold_gap={}): \
             space-first={:?} n-first={:?}",
            deadline, hold_gap, space_first, n_first
        );
    }
}

// ---------------------------------------------------------------------------
// Catalog selection self-tests: guard against a slice going green only because
// every property touching its weak spot was deselected. Assert each slice's
// selected set is non-empty and as expected, no invariant has an empty positive
// footprint, and a narrow slice's set is a subset of a wider one's.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod catalog_selection_tests {
    use super::*;

    // The component sets each live slice declares (kept in sync with the SUTs).
    // The coupled `Sut` hosts zippy + tap-hold over one keymap, so it declares all
    // four; the standalone `ThSut` is tap-hold only.
    fn coupled_caps() -> CapSet {
        capset(&[
            Cap::OutputKeyState,
            Cap::VisibleText,
            Cap::ZippyState,
            Cap::LayoutState,
        ])
    }
    fn taphold_caps() -> CapSet {
        capset(&[Cap::OutputKeyState, Cap::LayoutState])
    }

    #[test]
    fn no_invariant_has_empty_positive_footprint() {
        for inv in INVARIANTS {
            assert!(
                !inv.needs_pos.is_empty(),
                "invariant `{}` observes nothing real (empty positive footprint)",
                inv.id
            );
        }
    }

    #[test]
    fn each_slice_selects_nonempty_expected() {
        for (name, caps) in [("coupled", coupled_caps()), ("taphold", taphold_caps())] {
            let ids = selected_ids(&caps);
            assert!(!ids.is_empty(), "slice `{name}` selected no invariants");
            assert!(
                ids.contains(&"no_double_press"),
                "slice `{name}` must select the universal key-state invariant"
            );
        }
    }

    #[test]
    fn empty_slice_selects_nothing() {
        // No components present => no output to judge => nothing selected.
        assert!(selected_ids(&capset(&[])).is_empty());
    }

    #[test]
    fn planted_violation_is_caught_when_selected_and_honestly_skipped_when_absent() {
        // A double-press the key-state invariant must reject.
        let bad = "out:↓A out:↓A";
        // Selected (OutputKeyState present) => caught with teeth.
        assert!(
            run_invariants(&coupled_caps(), bad).is_err(),
            "selected key-state invariant failed to catch a planted double-press"
        );
        // Absent (no OutputKeyState) => the invariant is *deselected*, so the run
        // is vacuously ok. This is honest non-selection, NOT a stubbed/faked pass:
        // a slice without the output-stream component genuinely cannot judge it.
        assert!(run_invariants(&capset(&[Cap::VisibleText]), bad).is_ok());
    }

    #[test]
    fn output_only_slice_is_subset_of_feature_slices() {
        // The universal read-only slice's selection must be a subset of any
        // richer slice's (adding components can only add invariants).
        let base = selected_ids(&capset(&[Cap::OutputKeyState]));
        for caps in [coupled_caps(), taphold_caps()] {
            let wide: BTreeSet<_> = selected_ids(&caps).into_iter().collect();
            assert!(
                base.iter().all(|id| wide.contains(id)),
                "narrowing changed selection non-monotonically"
            );
        }
    }

    #[test]
    fn coupled_slice_subsumes_standalone_taphold_catalog() {
        // The deletion safety-gate for the standalone `taphold_state_machine`:
        // the coupled slice must select at least every catalog invariant the
        // standalone does. It does (LayoutState ⊆ coupled caps). NOTE the
        // standalone is nonetheless KEPT — its teeth are not in the catalog but in
        // its distinct observable (the lower-level event stream, `event_seq`) and
        // its tap≠self coverage, which the coupled net-text model does not provide.
        // If those are ever folded into the coupled model, this gate licenses the
        // deletion.
        let coupled: BTreeSet<_> = selected_ids(&coupled_caps()).into_iter().collect();
        for id in selected_ids(&taphold_caps()) {
            assert!(
                coupled.contains(id),
                "coupled slice does not subsume standalone catalog invariant `{id}`"
            );
        }
    }
}
