//! Zippychord config *minimizer*: given a real config (`.kbd` + chord `.tsv`)
//! that reproduces a bug, mechanically reduce both artifacts to the smallest
//! form that still reproduces — a minimal repro for a bug report or a new
//! regression test.
//!
//! The engine is **proptest's shrinker**, driven the way the plan requires: the
//! captured user config is a *constant*; proptest generates and shrinks a
//! *reduction choice* (a keep-mask) over it. The strategy's minimum ==
//! maximally-reduced config, and "shrink" == "reduce more". This is
//! delta-debugging expressed through proptest's native shrinker.
//!
//! Two predicate modes (both supported):
//!   * **Expected-output (differential)** — preserves the *exact* `O_ok`/`O_bad`
//!     divergence captured up front. Comparing against "the correct output"
//!     would be unsound: the shrinker could just delete the chord under test
//!     (making output empty, which differs from correct) and declare victory.
//!   * **Metamorphic press-order** — two press-order permutations *within a
//!     single overlap window* (all keys pressed before any release) must produce
//!     identical net text; a disagreement is the bug. The invariance class is
//!     explicit: only in-window permutations are required to agree, because
//!     sequential (non-overlapping) input is *supposed* to differ from chorded.
//!
//! v1 scope (see the plan's risk flags): no per-key defsrc/deflayer column
//! surgery — only whole-form drops, defzippy per-option resets, and
//! arity-preserving action collapses (`tap-hold`/`multi`/`one-shot`/`fork` →
//! inner). Input must be a single self-contained `.kbd` (includes pre-flattened)
//! plus one `.tsv`. `output-character-mappings` is left un-dropped (conservative,
//! avoids the "Unknown output key name" cascade). The reference graph for
//! whole-form drops is *not* precomputed: `Kanata::new_from_str` is the
//! authoritative validity gate — an invalid drop (e.g. a dangling layer
//! reference) fails to parse and is rejected as "not a repro", so the shrinker
//! never keeps it.

use crate::oskbd::{KeyEvent, KeyValue};
use crate::tests::CFG_PARSE_LOCK;
use crate::{Kanata, str_to_oscode};
use kanata_parser::cfg::sexpr::{SExpr, Spanned, parse};
use kanata_parser::cfg::zch_file_lines;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestError, TestRng, TestRunner};
use rustc_hash::FxHashMap;
use std::sync::MutexGuard;

// ---------------------------------------------------------------------------
// Gestures
// ---------------------------------------------------------------------------

/// A timed key-event stream, mirroring the sim harness's `d:`/`u:`/`t:` model
/// (`mod.rs`): each key is a token (e.g. `"spc"`, `"n"`) resolved via
/// `str_to_oscode` exactly as the harness does; a press/release applies the
/// event with NO implicit tick; `Tick(ms)` advances `ms` milliseconds of
/// processing. (Deliberately *not* the state-machine harness's auto-ticking
/// `feed_press`, so timing-sensitive behaviour matches the recorded sim repros.)
#[derive(Clone, Debug)]
enum Gesture {
    Press(String),
    Release(String),
    Repeat(String),
    Tick(u128),
}

fn osc_of(tok: &str) -> crate::OsCode {
    str_to_oscode(tok).unwrap_or_else(|| panic!("no oscode for key token {tok:?}"))
}

fn pressed_insert(_osc: crate::OsCode) {
    #[cfg(not(all(target_os = "windows", not(feature = "interception_driver"))))]
    crate::PRESSED_KEYS.lock().insert(_osc);
    #[cfg(all(target_os = "windows", not(feature = "interception_driver")))]
    crate::PRESSED_KEYS
        .lock()
        .insert(_osc, web_time::Instant::now());
}

fn feed(k: &mut Kanata, ev: &Gesture) {
    match ev {
        Gesture::Tick(ms) => {
            k.tick_ms(*ms, &None).unwrap();
        }
        Gesture::Press(tok) => {
            let o = osc_of(tok);
            k.handle_input_event(&KeyEvent::new(o, KeyValue::Press))
                .unwrap();
            pressed_insert(o);
        }
        Gesture::Release(tok) => {
            let o = osc_of(tok);
            k.handle_input_event(&KeyEvent::new(o, KeyValue::Release))
                .unwrap();
            crate::PRESSED_KEYS.lock().remove(&o);
        }
        Gesture::Repeat(tok) => {
            let o = osc_of(tok);
            k.handle_input_event(&KeyEvent::new(o, KeyValue::Repeat))
                .unwrap();
        }
    }
}

/// Clears the global `PRESSED_KEYS` on drop so a panic mid-run cannot leak held
/// keys into a later test (mirrors `Sut` in the state-machine PBT).
struct RunGuard {
    _g: MutexGuard<'static, ()>,
}
impl Drop for RunGuard {
    fn drop(&mut self) {
        crate::PRESSED_KEYS.lock().clear();
    }
}

/// Build a `Kanata` from `(kbd, tsv)`, run `gesture`, return the reconstructed
/// net visible text. `None` iff the config fails to parse — that is an *invalid
/// reduction* (e.g. a dropped form left a dangling reference), not a repro, so
/// the caller treats it as "does not reproduce" and the shrinker backtracks.
///
/// `Kanata::new_from_str` mutates the process-global zippychord state (`ZCH`),
/// so this takes `CFG_PARSE_LOCK` for the whole run — mandatory, exactly as the
/// sim harness and the state-machine PBT do.
fn run_gesture(file_name: &str, kbd: &str, tsv: &str, gesture: &[Gesture]) -> Option<String> {
    let guard = match CFG_PARSE_LOCK.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    crate::PRESSED_KEYS.lock().clear();
    let _run_guard = RunGuard { _g: guard };
    let mut fc = FxHashMap::default();
    // Keyed by the exact filename token in the `defzippy` form (e.g. "file" or
    // "./chords.tsv"), which is how the file-content provider resolves it.
    fc.insert(file_name.to_string(), tsv.to_string());
    let mut k = match Kanata::new_from_str(kbd, fc) {
        Ok(k) => k,
        Err(_) => return None,
    };
    for ev in gesture {
        feed(&mut k, ev);
    }
    Some(super::zippychord_state_machine::sut_net_text(&k))
}

// ---------------------------------------------------------------------------
// TSV model: a text-preserving chord tree (NOT SubsetMap — not round-trippable)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct ChordNode {
    input: String,
    output: String,
    followups: Vec<ChordNode>,
}

/// Parse `.tsv` into a round-trippable chord tree. The line grammar (skip blank
/// / `//` lines, split on a tab, `input`/`output` kept **verbatim**) is the
/// shared `zch_file_lines` — the same parser production uses — so this can't
/// drift from how kanata actually reads the file. On top of those lines this
/// groups by string prefix (the inverse of the state-machine PBT's
/// `emit_children`): a line's parent is the earlier line whose `input` equals
/// this input minus its last space-separated token; an empty/absent parent
/// makes it a root. Returns the roots in file order.
fn parse_tsv(text: &str) -> Vec<ChordNode> {
    struct ArenaNode {
        input: String,
        output: String,
        children: Vec<usize>,
    }
    let mut arena: Vec<ArenaNode> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    let mut index: FxHashMap<String, usize> = FxHashMap::default();

    let file_lines = zch_file_lines(text).expect("minimizer input must be a valid chord file");
    for fl in file_lines {
        let parent = fl
            .input
            .rsplit_once(' ')
            .and_then(|(prefix, _)| index.get(prefix).copied());
        let idx = arena.len();
        arena.push(ArenaNode {
            input: fl.input.to_string(),
            output: fl.output.to_string(),
            children: Vec::new(),
        });
        match parent {
            Some(p) => arena[p].children.push(idx),
            None => roots.push(idx),
        }
        // First writer wins for duplicate inputs (matches "the earlier line").
        index.entry(fl.input.to_string()).or_insert(idx);
    }

    fn build(arena: &[ArenaNodeRef], idx: usize) -> ChordNode {
        ChordNode {
            input: arena[idx].0.clone(),
            output: arena[idx].1.clone(),
            followups: arena[idx].2.iter().map(|&c| build(arena, c)).collect(),
        }
    }
    type ArenaNodeRef = (String, String, Vec<usize>);
    let flat: Vec<ArenaNodeRef> = arena
        .into_iter()
        .map(|n| (n.input, n.output, n.children))
        .collect();
    roots.iter().map(|&r| build(&flat, r)).collect()
}

fn emit_chord(n: &ChordNode, lines: &mut Vec<String>) {
    lines.push(format!("{}\t{}", n.input, n.output));
    for c in &n.followups {
        emit_chord(c, lines);
    }
}

/// Serialize the kept roots (each a root + its entire followup subtree) DFS in
/// file order. `keep[i]` selects root `i`.
fn serialize_tsv(roots: &[ChordNode], keep: &[bool]) -> String {
    let mut lines = Vec::new();
    for (i, r) in roots.iter().enumerate() {
        if keep[i] {
            emit_chord(r, &mut lines);
        }
    }
    format!("\n{}\n", lines.join("\n"))
}

// ---------------------------------------------------------------------------
// .kbd model: parsed top-level forms + a hybrid serializer
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct FormModel {
    head: String,
    sexprs: Vec<SExpr>,
    span_start: usize,
    span_end: usize,
}

fn head_of(sexprs: &[SExpr]) -> String {
    match sexprs.first() {
        Some(SExpr::Atom(a)) => a.t.clone(),
        _ => String::new(),
    }
}

fn parse_kbd(text: &str) -> Vec<FormModel> {
    let tops = parse(text, "min.kbd").expect("kbd must lex into s-expressions");
    tops.into_iter()
        .map(|top| FormModel {
            head: head_of(&top.t),
            sexprs: top.t,
            span_start: top.span.start(),
            span_end: top.span.end(),
        })
        .collect()
}

/// For a collapsible action list, the index of the inner element it collapses
/// to. All targets are a single s-expression, so swapping the list for it keeps
/// the enclosing `deflayer`'s element count intact (the defsrc/deflayer length
/// invariant is never violated). `tap-hold*` → the *tap* action (proving "the
/// bug needs tap-hold on this key" when the collapse cannot be applied).
/// The filename token of the `(defzippy <file> ...)` form, e.g. `"file"` or
/// `"./chords.tsv"`. This is the key the file-content provider resolves the
/// chord file under. Falls back to `"file"` if absent.
fn defzippy_file_name(forms: &[FormModel]) -> String {
    forms
        .iter()
        .find(|f| f.head == "defzippy")
        .and_then(|f| match f.sexprs.get(1) {
            Some(SExpr::Atom(a)) => Some(a.t.trim_matches('"').to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "file".to_string())
}

fn collapse_inner_index(list: &Spanned<Vec<SExpr>>) -> Option<usize> {
    let head = match list.t.first() {
        Some(SExpr::Atom(a)) => a.t.as_str(),
        _ => return None,
    };
    let idx = match head {
        "tap-hold" | "tap-hold-press" | "tap-hold-release" | "tap-hold-except-keys" => 3,
        "multi" => 1,
        "one-shot" => 2,
        "fork" => 1,
        _ => return None,
    };
    (list.t.len() > idx).then_some(idx)
}

/// A single atomic reduction over the captured config. Each maps to one bit of
/// the keep-mask: bit `true` keeps it as-is, bit `false` applies the reduction.
#[derive(Clone, Debug)]
enum ConfigUnit {
    /// Omit an entire optional top-level form.
    DropForm { form: usize },
    /// Drop a `name value` pair from `(defzippy ...)` → that option falls back
    /// to `ZchConfig::default`.
    DropZippyOption { form: usize, name: String },
    /// Replace a collapsible action at `deflayer` element `elem` with its inner
    /// element `inner` (arity-preserving).
    Collapse {
        form: usize,
        elem: usize,
        inner: usize,
    },
    /// Drop a TSV root (and its whole followup subtree).
    DropChord { root: usize },
}

/// Enumerate every reduction unit. Order is stable so a mask index always means
/// the same unit. Core forms (`defsrc`, `defzippy`, the *first* `deflayer`) are
/// never whole-form-dropped; extra forms are droppable and validity is enforced
/// at parse time.
fn build_units(forms: &[FormModel], num_roots: usize) -> Vec<ConfigUnit> {
    let mut units = Vec::new();
    let mut first_deflayer_seen = false;
    for (i, f) in forms.iter().enumerate() {
        match f.head.as_str() {
            "defsrc" => {}
            "defzippy" => {
                let mut j = 2;
                while j + 1 < f.sexprs.len() {
                    if let SExpr::Atom(a) = &f.sexprs[j]
                        && a.t != "output-character-mappings"
                    {
                        units.push(ConfigUnit::DropZippyOption {
                            form: i,
                            name: a.t.clone(),
                        });
                    }
                    j += 2;
                }
            }
            "deflayer" => {
                if first_deflayer_seen {
                    units.push(ConfigUnit::DropForm { form: i });
                } else {
                    first_deflayer_seen = true;
                }
                for (e, el) in f.sexprs.iter().enumerate().skip(2) {
                    if let SExpr::List(l) = el
                        && let Some(inner) = collapse_inner_index(l)
                    {
                        units.push(ConfigUnit::Collapse {
                            form: i,
                            elem: e,
                            inner,
                        });
                    }
                }
            }
            _ => units.push(ConfigUnit::DropForm { form: i }),
        }
    }
    for root in 0..num_roots {
        units.push(ConfigUnit::DropChord { root });
    }
    units
}

fn drop_zippy_option(sexprs: &mut Vec<SExpr>, name: &str) {
    let mut i = 2;
    while i + 1 < sexprs.len() {
        if let SExpr::Atom(a) = &sexprs[i]
            && a.t == name
        {
            sexprs.drain(i..i + 2);
            return;
        }
        i += 2;
    }
}

fn collapse_elem(sexprs: &mut [SExpr], elem: usize, inner: usize) {
    if let SExpr::List(l) = &sexprs[elem] {
        sexprs[elem] = l.t[inner].clone();
    }
}

fn debug_form(sexprs: &[SExpr]) -> String {
    let inner = sexprs
        .iter()
        .map(|s| format!("{s:?}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("({inner})")
}

// ---------------------------------------------------------------------------
// Captured config + predicate
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct ClusterTiming {
    gap: u128,
    hold: u128,
    relgap: u128,
    settle: u128,
}

/// Build a fully co-pressed gesture for one press-order permutation: every key
/// pressed (in `order`) before any release, so all keys are simultaneously held
/// — i.e. a single overlap window. Releases follow the same order.
fn cluster_gesture(order: &[String], t: &ClusterTiming) -> Vec<Gesture> {
    let mut g = Vec::new();
    for (i, tok) in order.iter().enumerate() {
        if i > 0 {
            g.push(Gesture::Tick(t.gap));
        }
        g.push(Gesture::Press(tok.clone()));
    }
    g.push(Gesture::Tick(t.hold));
    for (i, tok) in order.iter().enumerate() {
        if i > 0 {
            g.push(Gesture::Tick(t.relgap));
        }
        g.push(Gesture::Release(tok.clone()));
    }
    g.push(Gesture::Tick(t.settle));
    g
}

fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut res = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let head = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, head.clone());
            res.push(tail);
        }
    }
    res
}

#[derive(Clone)]
enum Predicate {
    /// Differential: the specific divergence captured from the full config.
    Expected {
        trigger: Vec<Gesture>,
        reference: Option<Vec<Gesture>>,
        o_bad: String,
        o_ok: Option<String>,
    },
    /// Press-order permutations within one overlap window are *supposed* to
    /// agree; in the bug they don't. `expected` is the exact per-permutation net
    /// text captured from the FULL config (aligned to `permutations(cluster)`),
    /// at least two entries differing. Requiring the reduced config to reproduce
    /// this *exact* vector — not merely "some divergence" — is the metamorphic
    /// analog of the differential predicate: otherwise the shrinker could delete
    /// the chord, leaving a trivial press-order divergence in literal output
    /// (e.g. " n" vs "n ") that is *expected* behavior, and call it a repro.
    Metamorphic {
        cluster: Vec<String>,
        timing: ClusterTiming,
        expected: Vec<String>,
    },
}

struct Captured {
    kbd_text: String,
    /// The filename token from the `defzippy` form (e.g. "file" or
    /// "./chords.tsv") — the key the file-content provider resolves the chord
    /// file under.
    tsv_file_name: String,
    forms: Vec<FormModel>,
    tsv_roots: Vec<ChordNode>,
    units: Vec<ConfigUnit>,
    predicate: Predicate,
}

impl Captured {
    fn mask_len(&self) -> usize {
        // v1: only config units shrink. Gesture/timing shrinking is deferred —
        // capturing exact per-permutation output (for soundness) pins the
        // timing, so the mask covers config reduction only.
        self.units.len()
    }

    /// Reconstruct `(kbd, tsv)` from the config bits (`bits.len() ==
    /// units.len()`).
    fn reconstruct_config(&self, bits: &[bool]) -> (String, String) {
        // Working copies; mutated forms are re-emitted via Debug, untouched ones
        // via their original span slice (lossless: preserves quotes/comments).
        let mut dropped = vec![false; self.forms.len()];
        let mut mutated = vec![false; self.forms.len()];
        let mut work: Vec<Vec<SExpr>> = self.forms.iter().map(|f| f.sexprs.clone()).collect();
        let mut keep_root = vec![true; self.tsv_roots.len()];

        for (unit, &keep) in self.units.iter().zip(bits) {
            if keep {
                continue;
            }
            match unit {
                ConfigUnit::DropForm { form } => dropped[*form] = true,
                ConfigUnit::DropZippyOption { form, name } => {
                    drop_zippy_option(&mut work[*form], name);
                    mutated[*form] = true;
                }
                ConfigUnit::Collapse { form, elem, inner } => {
                    collapse_elem(&mut work[*form], *elem, *inner);
                    mutated[*form] = true;
                }
                ConfigUnit::DropChord { root } => keep_root[*root] = false,
            }
        }

        let mut kbd = String::new();
        for (i, f) in self.forms.iter().enumerate() {
            if dropped[i] {
                continue;
            }
            if !kbd.is_empty() {
                kbd.push('\n');
            }
            if mutated[i] {
                kbd.push_str(&debug_form(&work[i]));
            } else {
                kbd.push_str(&self.kbd_text[f.span_start..f.span_end]);
            }
        }
        let tsv = serialize_tsv(&self.tsv_roots, &keep_root);
        (kbd, tsv)
    }

    /// True == "the specific bug still reproduces".
    fn reproduces(&self, mask: &[bool]) -> bool {
        let nc = self.units.len();
        let (kbd, tsv) = self.reconstruct_config(&mask[..nc]);
        match &self.predicate {
            Predicate::Expected {
                trigger,
                reference,
                o_bad,
                o_ok,
            } => {
                let fname = &self.tsv_file_name;
                if run_gesture(fname, &kbd, &tsv, trigger).as_deref() != Some(o_bad.as_str()) {
                    return false;
                }
                match (reference, o_ok) {
                    (Some(r), Some(ok)) => {
                        run_gesture(fname, &kbd, &tsv, r).as_deref() == Some(ok.as_str())
                    }
                    _ => true,
                }
            }
            Predicate::Metamorphic {
                cluster,
                timing,
                expected,
            } => {
                for (order, want) in permutations(cluster).iter().zip(expected) {
                    match run_gesture(&self.tsv_file_name, &kbd, &tsv, &cluster_gesture(order, timing))
                    {
                        Some(got) if &got == want => {}
                        _ => return false,
                    }
                }
                true
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reduction engine: proptest shrinker over the keep-mask
// ---------------------------------------------------------------------------

/// 1<<20: makes a generated bit `true` with probability ≈ `BIG/(BIG+1)`, so the
/// first generated mask is ≈ all-keep (the full config) and reproduces
/// immediately; shrinking then drives bits `true→false` (reduce more).
const BIG: u32 = 1 << 20;
/// Fixed seed → a given minimization run is reproducible in CI (proptest minima
/// are RNG-dependent in general).
const SEED: [u8; 32] = *b"kanata-zippychord-minimizer-seed";

#[derive(Debug)]
struct Minimized {
    kbd: String,
    tsv: String,
    kept_units: Vec<String>,
    mask: Vec<bool>,
}

fn minimize(captured: &Captured) -> Minimized {
    let n = captured.mask_len();

    // Pre-flight guard: the full (all-keep) config MUST reproduce, and for the
    // differential mode O_ok != O_bad must already hold (asserted at capture).
    // A false negative here means the predicate/repro spec is wrong, not that
    // reduction failed — fail loudly rather than emit confidently-wrong output.
    let all_keep = vec![true; n];
    assert!(
        captured.reproduces(&all_keep),
        "pre-flight: the full config must reproduce the bug (predicate or repro spec is wrong)"
    );

    let strategy = prop::collection::vec(
        prop_oneof![1 => Just(false), BIG => Just(true)],
        n..=n,
    );
    let config = Config {
        cases: 64,
        // Sized for tiny per-candidate parse cost; not 1e6. Each iter is a
        // couple of `new_from_str` + short gestures.
        max_shrink_iters: 4096,
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner = TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &SEED));

    // Inverted semantics: a test *failure* means the bug *reproduces*, so
    // proptest shrinks toward the smallest reproducing config.
    let result = runner.run(&strategy, |mask| {
        if captured.reproduces(&mask) {
            Err(TestCaseError::fail("reproduces"))
        } else {
            Ok(())
        }
    });

    let mask = match result {
        Err(TestError::Fail(_, minimal)) => minimal,
        other => panic!("expected a reproducing minimum from the shrinker, got: {other:?}"),
    };

    let nc = captured.units.len();
    let (kbd, tsv) = captured.reconstruct_config(&mask[..nc]);
    let kept_units = captured
        .units
        .iter()
        .zip(&mask)
        .filter(|(_, keep)| **keep)
        .map(|(u, _)| format!("{u:?}"))
        .collect();
    Minimized {
        kbd,
        tsv,
        kept_units,
        mask,
    }
}

// ---------------------------------------------------------------------------
// Capture helpers
// ---------------------------------------------------------------------------

/// Parse a sim-harness gesture string ("d:n t:5 d:spc t:10 u:n ...") into a
/// `Gesture` stream, exactly as `simulate_with_file_content` (`mod.rs`) reads it:
/// `d:` press, `u:` release, `t:` tick; the key value is a token resolved by
/// `str_to_oscode` (e.g. `spc`, `n`), kept verbatim here.
fn parse_sim_gesture(s: &str) -> Vec<Gesture> {
    let mut g = Vec::new();
    for tok in s.split_whitespace() {
        let (kind, val) = tok.split_once(':').expect("gesture token must be kind:val");
        match kind {
            "t" => g.push(Gesture::Tick(val.parse().expect("tick ms"))),
            "d" => g.push(Gesture::Press(val.to_string())),
            "u" => g.push(Gesture::Release(val.to_string())),
            "r" => g.push(Gesture::Repeat(val.to_string())),
            other => panic!("unknown gesture kind: {other}"),
        }
    }
    g
}

fn build_captured_expected(
    kbd_text: String,
    tsv_text: &str,
    trigger: Vec<Gesture>,
    reference: Option<Vec<Gesture>>,
) -> Captured {
    let forms = parse_kbd(&kbd_text);
    let tsv_file_name = defzippy_file_name(&forms);
    let tsv_roots = parse_tsv(tsv_text);
    let units = build_units(&forms, tsv_roots.len());

    // Capture the divergence from the FULL config.
    let full_tsv = serialize_tsv(&tsv_roots, &vec![true; tsv_roots.len()]);
    let o_bad = run_gesture(&tsv_file_name, &kbd_text, &full_tsv, &trigger)
        .expect("full config must parse for the trigger gesture");
    let o_ok = reference.as_ref().map(|r| {
        run_gesture(&tsv_file_name, &kbd_text, &full_tsv, r)
            .expect("full config must parse for the reference gesture")
    });
    if let Some(ok) = &o_ok {
        assert_ne!(
            ok, &o_bad,
            "repro spec is wrong: reference and trigger produce the same output (no divergence to preserve)"
        );
    }
    Captured {
        kbd_text,
        tsv_file_name,
        forms,
        tsv_roots,
        units,
        predicate: Predicate::Expected {
            trigger,
            reference,
            o_bad,
            o_ok,
        },
    }
}

fn build_captured_metamorphic(
    kbd_text: String,
    tsv_text: &str,
    cluster: Vec<String>,
    timing: ClusterTiming,
) -> Captured {
    assert!(
        cluster.len() >= 2 && cluster.len() <= 5,
        "metamorphic cluster must have 2..=5 keys (factorial permutations)"
    );
    let forms = parse_kbd(&kbd_text);
    let tsv_file_name = defzippy_file_name(&forms);
    let tsv_roots = parse_tsv(tsv_text);
    let units = build_units(&forms, tsv_roots.len());

    // Capture the exact per-permutation outputs from the FULL config and assert
    // there really is a divergence (the bug); if the orders all agree, the repro
    // spec is wrong — abort rather than emit confidently-wrong output.
    let full_tsv = serialize_tsv(&tsv_roots, &vec![true; tsv_roots.len()]);
    let expected: Vec<String> = permutations(&cluster)
        .iter()
        .map(|order| {
            run_gesture(&tsv_file_name, &kbd_text, &full_tsv, &cluster_gesture(order, &timing))
                .expect("full config must parse for every permutation")
        })
        .collect();
    assert!(
        !expected.windows(2).all(|w| w[0] == w[1]),
        "repro spec is wrong: all press orders agree, so there is no divergence to preserve"
    );

    Captured {
        kbd_text,
        tsv_file_name,
        forms,
        tsv_roots,
        units,
        predicate: Predicate::Metamorphic {
            cluster,
            timing,
            expected,
        },
    }
}

// ---------------------------------------------------------------------------
// Builtin repro: the tap-hold press-order bug (mirrors
// `sim_zippy_taphold_chord_press_order_dependent`).
// ---------------------------------------------------------------------------

static BUILTIN_KBD: &str = "(defsrc spc n)\n\
    (deflayer base (tap-hold 200 200 spc (layer-while-held l2)) n)\n\
    (deflayer l2 spc n)\n\
    (defzippy file on-first-press-chord-deadline 20 idle-reactivate-time 100 smart-space full)";
static BUILTIN_TSV: &str = "\n n\tno\n";

fn builtin_captured() -> Captured {
    build_captured_metamorphic(
        BUILTIN_KBD.to_string(),
        BUILTIN_TSV,
        vec!["spc".to_string(), "n".to_string()],
        // Mirrors the recorded gesture: `d:_ t:5 d:_ t:10 u:_ t:5 u:_ t:300`.
        ClusterTiming {
            gap: 5,
            hold: 10,
            relgap: 5,
            settle: 300,
        },
    )
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

fn env_ms(name: &str, default: u128) -> u128 {
    std::env::var(name)
        .ok()
        .map(|v| v.parse().unwrap_or_else(|_| panic!("{name} must be a number")))
        .unwrap_or(default)
}

/// External entry: point env vars at a real (self-contained) repro and run the
/// minimizer. No-op when `KANATA_MIN_KBD` is unset, so it is inert in normal CI.
///
///   KANATA_MIN_KBD   path to the `.kbd`
///   KANATA_MIN_TSV   path to the chord `.tsv`
///   KANATA_MIN_MODE  "expected" (default) | "metamorphic"
///   expected:    KANATA_MIN_INPUT = trigger sim string,
///                KANATA_MIN_EXPECTED = reference sim string (optional)
///   metamorphic: KANATA_MIN_INPUT = space-separated cluster keys (e.g. "spc n")
///                KANATA_MIN_GAP / _HOLD / _RELGAP / _SETTLE = cluster timing ms
///                (defaults 5 / 10 / 5 / 300)
///   KANATA_MIN_PROBE=1  (metamorphic only) just print each press-order's net
///                       text and stop — use to find a divergence-producing
///                       gesture before committing to a minimize run.
#[test]
fn minimize_external() {
    let Ok(kbd_path) = std::env::var("KANATA_MIN_KBD") else {
        eprintln!("minimize_external: KANATA_MIN_KBD unset; skipping");
        return;
    };
    let tsv_path = std::env::var("KANATA_MIN_TSV").expect("KANATA_MIN_TSV required");
    let kbd_text = std::fs::read_to_string(&kbd_path).expect("read KANATA_MIN_KBD");
    let tsv_text = std::fs::read_to_string(&tsv_path).expect("read KANATA_MIN_TSV");
    let input = std::env::var("KANATA_MIN_INPUT").expect("KANATA_MIN_INPUT required");
    let mode = std::env::var("KANATA_MIN_MODE").unwrap_or_else(|_| "expected".to_string());

    let captured = match mode.as_str() {
        "metamorphic" => {
            let cluster: Vec<String> = input.split_whitespace().map(str::to_string).collect();
            let timing = ClusterTiming {
                gap: env_ms("KANATA_MIN_GAP", 5),
                hold: env_ms("KANATA_MIN_HOLD", 10),
                relgap: env_ms("KANATA_MIN_RELGAP", 5),
                settle: env_ms("KANATA_MIN_SETTLE", 300),
            };

            // Probe mode: run each press order on the FULL config and print the
            // net text, without the divergence assertion. Lets us search the
            // timing space for a reproducing gesture.
            if std::env::var("KANATA_MIN_PROBE").is_ok() {
                let forms = parse_kbd(&kbd_text);
                let fname = defzippy_file_name(&forms);
                let roots = parse_tsv(&tsv_text);
                let full_tsv = serialize_tsv(&roots, &vec![true; roots.len()]);
                println!("probe: file={fname:?} cluster={cluster:?} timing={timing:?}");
                let run_all = |t: &ClusterTiming| -> Vec<Option<String>> {
                    permutations(&cluster)
                        .iter()
                        .map(|o| run_gesture(&fname, &kbd_text, &full_tsv, &cluster_gesture(o, t)))
                        .collect()
                };
                // KANATA_MIN_SWEEP="knob:lo:hi" (knob = gap|hold|relgap) sweeps
                // that knob over [lo, hi], printing every value where the press
                // orders disagree (a candidate reproducing gesture); otherwise
                // just run the single configured timing.
                if let Ok(spec) = std::env::var("KANATA_MIN_SWEEP") {
                    let parts: Vec<&str> = spec.split(':').collect();
                    let (knob, lo, hi) = match parts.as_slice() {
                        [knob, lo, hi] => (*knob, lo.parse::<u128>().unwrap(), hi.parse::<u128>().unwrap()),
                        _ => panic!("KANATA_MIN_SWEEP=knob:lo:hi"),
                    };
                    let mut found = 0;
                    for v in lo..=hi {
                        let mut t = timing.clone();
                        match knob {
                            "gap" => t.gap = v,
                            "hold" => t.hold = v,
                            "relgap" => t.relgap = v,
                            other => panic!("unknown sweep knob {other}"),
                        }
                        let outs = run_all(&t);
                        if !outs.windows(2).all(|w| w[0] == w[1]) {
                            println!("  DIVERGE {knob}={v}: {outs:?}");
                            found += 1;
                        }
                    }
                    println!("  sweep done: {found} diverging {knob} value(s) in {lo}..={hi}");
                    return;
                }
                for (order, out) in permutations(&cluster).iter().zip(run_all(&timing)) {
                    println!("  order {order:?} -> {out:?}");
                }
                return;
            }

            build_captured_metamorphic(kbd_text, &tsv_text, cluster, timing)
        }
        "expected" => {
            let trigger = parse_sim_gesture(&input);
            let reference = std::env::var("KANATA_MIN_EXPECTED")
                .ok()
                .map(|s| parse_sim_gesture(&s));

            // Probe mode: run the trigger (and reference, if any) sim-string
            // gestures on the FULL config and print net text, no minimize/assert.
            // Full control over timing AND release order via the sim strings.
            if std::env::var("KANATA_MIN_PROBE").is_ok() {
                let forms = parse_kbd(&kbd_text);
                let fname = defzippy_file_name(&forms);
                let roots = parse_tsv(&tsv_text);
                let full_tsv = serialize_tsv(&roots, &vec![true; roots.len()]);
                println!("probe expected: file={fname:?}");

                // KANATA_MIN_GRID="k0 k1": brute-force a grid of press order ×
                // release order × (gap, hold, relgap) over the two keys, printing
                // every distinct net text with one example gesture. Parses once.
                if let Ok(keys) = std::env::var("KANATA_MIN_GRID") {
                    let kk: Vec<String> = keys.split_whitespace().map(str::to_string).collect();
                    assert_eq!(kk.len(), 2, "KANATA_MIN_GRID needs exactly 2 keys");
                    let mut seen: std::collections::BTreeMap<String, String> = Default::default();
                    let times = [1u128, 5, 20, 40, 100, 210];
                    for porder in [[0usize, 1], [1, 0]] {
                        for rorder in [[0usize, 1], [1, 0]] {
                            for &gap in &times {
                                for &hold in &times {
                                    for &relgap in &[1u128, 5, 20] {
                                        let p = [&kk[porder[0]], &kk[porder[1]]];
                                        let r = [&kk[rorder[0]], &kk[rorder[1]]];
                                        let mut g = Vec::new();
                                        g.push(Gesture::Press(p[0].clone()));
                                        g.push(Gesture::Tick(gap));
                                        g.push(Gesture::Press(p[1].clone()));
                                        g.push(Gesture::Tick(hold));
                                        g.push(Gesture::Release(r[0].clone()));
                                        g.push(Gesture::Tick(relgap));
                                        g.push(Gesture::Release(r[1].clone()));
                                        g.push(Gesture::Tick(300));
                                        let out = run_gesture(&fname, &kbd_text, &full_tsv, &g)
                                            .unwrap_or_else(|| "<parse-fail>".into());
                                        let desc = format!(
                                            "press[{},{}] rel[{},{}] gap={gap} hold={hold} relgap={relgap}",
                                            p[0], p[1], r[0], r[1]
                                        );
                                        seen.entry(out).or_insert(desc);
                                    }
                                }
                            }
                        }
                    }
                    println!("  distinct outputs ({}):", seen.len());
                    for (out, desc) in &seen {
                        println!("    {out:?}  <=  {desc}");
                    }
                    return;
                }

                println!("  trigger {input:?} -> {:?}", run_gesture(&fname, &kbd_text, &full_tsv, &trigger));
                if let Some(r) = &reference {
                    let rs = std::env::var("KANATA_MIN_EXPECTED").unwrap();
                    println!("  reference {rs:?} -> {:?}", run_gesture(&fname, &kbd_text, &full_tsv, r));
                }
                return;
            }

            build_captured_expected(kbd_text, &tsv_text, trigger, reference)
        }
        other => panic!("unknown KANATA_MIN_MODE: {other}"),
    };

    let min = minimize(&captured);
    println!("=== minimized .kbd ===\n{}\n=== minimized .tsv ===\n{}", min.kbd, min.tsv);
    println!("kept units: {:?}", min.kept_units);
}

/// CI exercise of the whole pipeline on a small in-repo repro. Asserts
/// **properties** of the minimum, not exact identity (proptest minima are not
/// stable across versions/inputs): the triggering chord survives, a `tap-hold`
/// on the chord key survives (collapsing it kills the bug, so the differential/
/// metamorphic predicate forbids it), and the kept unit count is small.
#[test]
fn minimize_builtin_taphold_repro() {
    let captured = builtin_captured();
    let min = minimize(&captured);

    println!("=== minimized .kbd ===\n{}\n=== minimized .tsv ===\n{}", min.kbd, min.tsv);
    println!("kept units: {:?}", min.kept_units);

    assert!(
        min.tsv.contains("\tno"),
        "the triggering chord (` n`->`no`) must survive; got tsv: {:?}",
        min.tsv
    );
    assert!(
        min.kbd.contains("tap-hold"),
        "a tap-hold on the chord key must survive (collapsing it removes the bug); got kbd: {}",
        min.kbd
    );
    let kept = min.mask.iter().filter(|&&b| b).count();
    assert!(
        kept <= 8,
        "expected a small minimum (<= 8 kept units), got {kept}: {:?}",
        min.kept_units
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tsv_tests {
    use super::*;

    #[test]
    fn parse_skips_blank_and_comments() {
        let roots = parse_tsv("\n// a comment\ndy\tday\n\n");
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].input, "dy");
        assert_eq!(roots[0].output, "day");
    }

    #[test]
    fn leading_space_input_preserved() {
        let roots = parse_tsv("\n abc\tAlphabet\n");
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].input, " abc");
    }

    #[test]
    fn interior_double_space_is_a_root() {
        // " w  a".rsplit_once(' ') == (" w ", "a"); no node " w " exists => root.
        let roots = parse_tsv("\n w  a\tWashington\n");
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].input, " w  a");
        assert!(roots[0].followups.is_empty());
    }

    #[test]
    fn followup_chain_nests() {
        let roots = parse_tsv("\ndy\tday\ndy 1\tMonday\n");
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].followups.len(), 1);
        assert_eq!(roots[0].followups[0].input, "dy 1");
        assert_eq!(roots[0].followups[0].output, "Monday");
    }

    #[test]
    fn absent_parent_makes_root() {
        // ".g f p" minus "p" is ".g f", which does not exist => root.
        let roots = parse_tsv("\n.g\tgit\n.g f p\tgit fetch -p\n");
        assert_eq!(roots.len(), 2);
    }

    #[test]
    fn roundtrip_serialize_keeps_all() {
        let src = "\ndy\tday\ndy 1\tMonday\npr\tpre\n";
        let roots = parse_tsv(src);
        let out = serialize_tsv(&roots, &vec![true; roots.len()]);
        let reparsed = parse_tsv(&out);
        assert_eq!(roots, reparsed);
    }

    #[test]
    fn drop_root_drops_its_subtree() {
        let roots = parse_tsv("\ndy\tday\ndy 1\tMonday\npr\tpre\n");
        // roots: [dy(+dy 1), pr]
        let out = serialize_tsv(&roots, &[false, true]);
        assert!(!out.contains("day"));
        assert!(!out.contains("Monday"));
        assert!(out.contains("pre"));
    }
}

#[cfg(test)]
mod kbd_tests {
    use super::*;

    #[test]
    fn untouched_roundtrip_is_lossless_modulo_join() {
        // No-mutation pass: span-slice reassembly re-parses to the same forms.
        let src = "(defsrc a b)\n(deflayer base a b) ;; comment\n(defcfg)";
        let forms = parse_kbd(src);
        let captured = Captured {
            kbd_text: src.to_string(),
            tsv_file_name: "file".to_string(),
            forms: forms.clone(),
            tsv_roots: vec![],
            units: vec![],
            predicate: Predicate::Metamorphic {
                cluster: vec!["a".to_string(), "b".to_string()],
                timing: ClusterTiming {
                    gap: 0,
                    hold: 0,
                    relgap: 0,
                    settle: 0,
                },
                expected: vec![],
            },
        };
        let (kbd, _) = captured.reconstruct_config(&[]);
        let reparsed = parse_kbd(&kbd);
        assert_eq!(reparsed.len(), forms.len());
        for (a, b) in forms.iter().zip(&reparsed) {
            assert_eq!(a.head, b.head);
        }
    }

    #[test]
    fn mutation_collapse_preserves_quoted_atom_and_reparses() {
        // Layer-(b) mutation pass: collapse a tap-hold inside a form that ALSO
        // contains a quoted/whitespace-bearing atom and a comment. The Debug
        // serializer must round-trip the quoted atom (it retains quotes in `.t`,
        // which is why call sites use `trim_atom_quotes`).
        let src = "(deftemplate t (x) (tap-hold 200 200 a b) \"quoted atom\") ;; cmt";
        let forms = parse_kbd(src);
        // element indices: 0 deftemplate, 1 t, 2 (x), 3 (tap-hold ...), 4 "quoted atom"
        let inner = collapse_inner_index(match &forms[0].sexprs[3] {
            SExpr::List(l) => l,
            _ => panic!("expected list"),
        })
        .expect("tap-hold collapses");
        assert_eq!(inner, 3);
        let mut sexprs = forms[0].sexprs.clone();
        collapse_elem(&mut sexprs, 3, inner);
        let emitted = debug_form(&sexprs);
        // tap-hold collapsed to its tap action `a`.
        assert!(!emitted.contains("tap-hold"), "collapsed: {emitted}");
        assert!(emitted.contains("\"quoted atom\""), "quotes survived: {emitted}");
        // Re-parses with intact structure.
        let reparsed = parse_kbd(&emitted);
        assert_eq!(reparsed.len(), 1);
        let SExpr::Atom(a) = &reparsed[0].sexprs[3] else {
            panic!("collapsed element should be the atom `a`");
        };
        assert_eq!(a.t, "a");
    }

    #[test]
    fn drop_zippy_option_falls_back_to_default() {
        // Dropping one `name value` pair leaves the rest parsing.
        let src = "(defzippy file on-first-press-chord-deadline 20 smart-space full)";
        let forms = parse_kbd(src);
        let mut sexprs = forms[0].sexprs.clone();
        drop_zippy_option(&mut sexprs, "smart-space");
        let emitted = debug_form(&sexprs);
        assert!(emitted.contains("on-first-press-chord-deadline"));
        assert!(!emitted.contains("smart-space"));
        // Still a valid zippy config.
        let mut fc = FxHashMap::default();
        fc.insert("file".to_string(), "\n n\tno\n".to_string());
        assert!(Kanata::new_from_str(&emitted_full(&emitted), fc).is_ok());
    }

    // A defzippy alone needs a defsrc+deflayer to be a valid kanata config.
    fn emitted_full(defzippy: &str) -> String {
        format!("(defsrc n)(deflayer base n){defzippy}")
    }

    #[test]
    fn build_units_classifies_builtin() {
        let forms = parse_kbd(BUILTIN_KBD);
        let units = build_units(&forms, 1);
        let mut drop_forms = 0;
        let mut options = 0;
        let mut collapses = 0;
        let mut chords = 0;
        for u in &units {
            match u {
                ConfigUnit::DropForm { .. } => drop_forms += 1,
                ConfigUnit::DropZippyOption { .. } => options += 1,
                ConfigUnit::Collapse { .. } => collapses += 1,
                ConfigUnit::DropChord { .. } => chords += 1,
            }
        }
        // l2 deflayer is droppable; 3 zippy options; 1 tap-hold collapse; 1 chord.
        assert_eq!(drop_forms, 1, "extra deflayer l2 is droppable");
        assert_eq!(options, 3, "deadline, idle-reactivate, smart-space");
        assert_eq!(collapses, 1, "the tap-hold on spc");
        assert_eq!(chords, 1);
    }
}

#[cfg(test)]
mod predicate_tests {
    use super::*;

    #[test]
    fn builtin_full_config_diverges() {
        // Pre-flight sanity: the full builtin config reproduces (the two press
        // orders disagree).
        let captured = builtin_captured();
        let mask = vec![true; captured.mask_len()];
        assert!(captured.reproduces(&mask));
    }

    /// Slippage guard. A naive "any press-order divergence ⇒ bug" predicate is
    /// unsound: with the chord deleted (and the tap-hold collapsed so the config
    /// still parses) the two orders type the held keys literally in press order
    /// — " n" vs "n " — a divergence that is *expected* behavior, not the bug.
    /// The naive predicate accepts that reduction (dropping the bug itself); the
    /// exact-output differential predicate rejects it. This is why the
    /// differential capture matters.
    #[test]
    fn slippage_naive_predicate_would_drop_chord() {
        let captured = builtin_captured();
        let nc = captured.units.len();
        // Reduce away the chord AND the tap-hold (keeps the config parse-valid).
        let mut mask = vec![true; captured.mask_len()];
        for (i, u) in captured.units.iter().enumerate() {
            if matches!(
                u,
                ConfigUnit::DropChord { .. } | ConfigUnit::Collapse { .. }
            ) {
                mask[i] = false;
            }
        }
        let (kbd, tsv) = captured.reconstruct_config(&mask[..nc]);
        assert!(!tsv.contains("\tno"), "this reduction drops the chord");

        // Naive predicate: "any in-window divergence" — WRONGLY true here.
        let Predicate::Metamorphic { cluster, timing, .. } = &captured.predicate else {
            unreachable!()
        };
        let outs: Vec<String> = permutations(cluster)
            .iter()
            .map(|o| {
                run_gesture(&captured.tsv_file_name, &kbd, &tsv, &cluster_gesture(o, timing))
                    .expect("parses")
            })
            .collect();
        let naive_reproduces = !outs.windows(2).all(|w| w[0] == w[1]);
        assert!(
            naive_reproduces,
            "naive any-divergence predicate is fooled by literal press-order ({outs:?})"
        );

        // Differential predicate (the real one): correctly rejects this drop.
        assert!(
            !captured.reproduces(&mask),
            "exact-output predicate must NOT accept a config with the chord deleted"
        );
    }

    #[test]
    fn collapsing_taphold_removes_the_bug() {
        // The slippage guard's core claim: if the tap-hold is collapsed, the two
        // press orders agree again (bug gone) — which is exactly why the
        // minimizer must NOT collapse it.
        let captured = builtin_captured();
        let mut mask = vec![true; captured.mask_len()];
        // Find the Collapse unit and turn it off (collapse the tap-hold).
        let collapse_idx = captured
            .units
            .iter()
            .position(|u| matches!(u, ConfigUnit::Collapse { .. }))
            .expect("a collapse unit exists");
        mask[collapse_idx] = false;
        assert!(
            !captured.reproduces(&mask),
            "collapsing the tap-hold should remove the divergence"
        );
    }
}
