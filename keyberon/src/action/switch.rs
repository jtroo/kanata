//! Handle processing of the switch action for Keyberon.
//!
//! Limitations:
//! - Maximum opcode length: 4095
//! - Maximum boolean expression depth: 8
//! - Maximum key recency: 7, where 0 is the most recent key press
//!
//! The intended use is to build up a `Switch` struct and use that in the `Layout`.
//!
//! The `Layout` will use `Switch::actions` to iterate over the actions that should be activated
//! when the corresponding key is pressed.

use super::*;
use crate::layout::{HistoricalEvent, KCoord};

use crate::key_code::*;

use BooleanOperator::*;
use BreakOrFallthrough::*;

pub const MAX_OPCODE_LEN: u16 = 0x0FFF;
pub const OP_MASK: u16 = 0xF000;
pub const MAX_BOOL_EXPR_DEPTH: usize = 8;
pub const MAX_KEY_RECENCY: u8 = 7;

pub type Case<'a, T> = (&'a [OpCode], &'a Action<'a, T>, BreakOrFallthrough);

#[derive(Debug, Clone, Copy, PartialEq)]
/// Behaviour of a switch action. Each case is a 3-tuple of:
///
/// - the boolean expression (array of opcodes)
/// - the action to evaluate if the expression evaluates to true
/// - whether to break or fallthrough to the next case if the expression evaluates to true
pub struct Switch<'a, T: 'a> {
    pub cases: &'a [Case<'a, T>],
}

// NOTE: have exhausted our opcodes for u16!
//
// Future rewrite: do traditional u8 opcodes, with variable length for the total opcode depending
// on the first one encountered? Or could be lazy and use u32 and have 4 bytes for every opcode.
// This probably isn't that performance-sensitive anyway... it's triggering on every input.

const OR_VAL: u16 = 0x1000;
const AND_VAL: u16 = 0x2000;
const NOT_VAL: u16 = 0x3000;

const INPUT_VAL: u16 = 851;
const HISTORICAL_INPUT_VAL: u16 = 852;
const LAYER_VAL: u16 = 853;
const BASE_LAYER_VAL: u16 = 854;

// Binary values:
// 0b0100 ...
// 0b0110 ...
//
// How-far-back are in bits 12-10 (3 bits)
// Time is compressed in bits 9-0 (10 bits)
const TICKS_SINCE_VAL_GT: u16 = 0x4000;
const TICKS_SINCE_VAL_LT: u16 = 0x6000;

// Highest bit in u16. Lower 3 bits in the highest nibble are "how far back". This means that
// switch can look back up to 8 keys.
const HISTORICAL_KEYCODE_VAL: u16 = 0x8000;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// Boolean operator. Notably missing today is Not.
pub enum BooleanOperator {
    Or,
    And,
    Not,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// OpCode for a switch case boolean expression.
pub struct OpCode(u16);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// The more useful interpretion of an OpCode.
enum OpCodeType {
    BooleanOp(OperatorAndEndIndex),
    KeyCode(u16),
    HistoricalKeyCode(HistoricalKeyCode),
    Input(KCoord),
    HistoricalInput(HistoricalInput),
    TicksSinceLessThan(TicksSinceNthKey),
    TicksSinceGreaterThan(TicksSinceNthKey),
    Layer(u16),
    BaseLayer(u16),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// The operation type and the opcode index at which evaluating this type ends.
struct OperatorAndEndIndex {
    pub op: BooleanOperator,
    pub idx: usize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// An op that checks specifically for a key that is a certain number of key presses back in
/// history.
struct HistoricalKeyCode {
    key_code: u16,
    how_far_back: u8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// An op that checks specifically for a key that is a certain number of key presses back in
/// history.
struct HistoricalInput {
    input: KCoord,
    how_far_back: u8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct TicksSinceNthKey {
    nth_key: u8,
    ticks_since: u16,
}

#[derive(Debug, Copy, Clone, PartialEq)]
/// Whether or not a case should break out of the switch if it evaluates to true or fallthrough to
/// the next case.
pub enum BreakOrFallthrough {
    Break,
    Fallthrough,
}

impl<'a, T> Switch<'a, T> {
    /// Iterates over the actions (if any) that are activated in the `Switch` based on its cases,
    /// the currently active keys, and historically pressed keys.
    ///
    /// The `historical_keys` parameter should iterate in the order of most-recent-first.
    pub fn actions<A1, A2, H1, H2, L>(
        &self,
        active_keys: A1,
        active_positions: A2,
        historical_keys: H1,
        historical_positions: H2,
        layers: L,
        default_layer: u16,
    ) -> SwitchActions<'a, T, A1, A2, H1, H2, L>
    where
        A1: Iterator<Item = KeyCode> + Clone,
        A2: Iterator<Item = KCoord> + Clone,
        H1: Iterator<Item = HistoricalEvent<KeyCode>> + Clone,
        H2: Iterator<Item = HistoricalEvent<KCoord>> + Clone,
        L: Iterator<Item = u16> + Clone,
    {
        SwitchActions {
            cases: self.cases,
            active_keys,
            active_positions,
            historical_keys,
            historical_positions,
            layers,
            default_layer,
            case_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
/// Iterator returned by `Switch::actions`.
pub struct SwitchActions<'a, T, A1, A2, H1, H2, L>
where
    A1: Iterator<Item = KeyCode> + Clone,
    A2: Iterator<Item = KCoord> + Clone,
    H1: Iterator<Item = HistoricalEvent<KeyCode>> + Clone,
    H2: Iterator<Item = HistoricalEvent<KCoord>> + Clone,
    L: Iterator<Item = u16> + Clone,
{
    cases: &'a [(&'a [OpCode], &'a Action<'a, T>, BreakOrFallthrough)],
    active_keys: A1,
    active_positions: A2,
    historical_keys: H1,
    historical_positions: H2,
    layers: L,
    default_layer: u16,
    case_index: usize,
}

impl<'a, T, A1, A2, H1, H2, L> Iterator for SwitchActions<'a, T, A1, A2, H1, H2, L>
where
    A1: Iterator<Item = KeyCode> + Clone,
    A2: Iterator<Item = KCoord> + Clone,
    H1: Iterator<Item = HistoricalEvent<KeyCode>> + Clone,
    H2: Iterator<Item = HistoricalEvent<KCoord>> + Clone,
    L: Iterator<Item = u16> + Clone,
{
    type Item = &'a Action<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.case_index < self.cases.len() {
            let case = &self.cases[self.case_index];
            if evaluate_boolean(
                case.0,
                self.active_keys.clone(),
                self.active_positions.clone(),
                self.historical_keys.clone(),
                self.historical_positions.clone(),
                self.layers.clone(),
                self.default_layer,
            ) {
                let ret_ac = case.1;
                match case.2 {
                    Break => self.case_index = self.cases.len(),
                    Fallthrough => self.case_index += 1,
                }
                return Some(ret_ac);
            }
            self.case_index += 1;
        }
        None
    }
}

impl BooleanOperator {
    fn to_u16(self) -> u16 {
        match self {
            Or => OR_VAL,
            And => AND_VAL,
            Not => NOT_VAL,
        }
    }
}
fn lossy_compress_ticks(t: u16) -> u16 {
    match t {
        0..=255 => t,
        256..=2303 => (t - 255) / 8 + 255,
        _ => (t - 2303) / 128 + 511,
    }
}

fn lossy_decompress_ticks(t: u16) -> u16 {
    match t {
        0..=255 => t,
        256..=511 => (t - 255) * 8 + 255,
        _ => (t - 511) * 128 + 2303,
    }
}

impl OpCode {
    /// Return a new OpCode that checks if the key active or not.
    pub fn new_key(kc: KeyCode) -> Self {
        assert!((kc as u16) <= KEY_MAX);
        Self(kc as u16 & MAX_OPCODE_LEN)
    }

    /// Return a new OpCode that checks if the n'th most recent key, defined by `key_recency`,
    /// matches the input keycode.
    pub fn new_key_history(kc: KeyCode, key_recency: u8) -> Self {
        assert!((kc as u16) <= MAX_OPCODE_LEN);
        assert!(key_recency <= MAX_KEY_RECENCY);
        Self((kc as u16 & MAX_OPCODE_LEN) | HISTORICAL_KEYCODE_VAL | ((key_recency as u16) << 12))
    }

    /// Returns a new opcode that returns true if the n'th most recent key was pressed greater
    /// than `ticks_since` ticks ago.
    ///
    /// At 256 ticks or above, this has a resolution of 8ms (rounded down). At 2304 ticks or
    /// above, this has a resolution of 128 ms (rounded down).
    pub fn new_ticks_since_gt(nth_key: u8, ticks_since: u16) -> Self {
        assert!(nth_key <= MAX_KEY_RECENCY);
        Self(TICKS_SINCE_VAL_GT | lossy_compress_ticks(ticks_since) | (u16::from(nth_key) << 10))
    }

    /// Returns a new opcode that returns true if the n'th most recent key was pressed greater
    /// than `ticks_since` ticks ago.
    ///
    /// At 256 ticks or above, this has a resolution of 8ms (rounded down). At 2304 ticks or
    /// above, this has a resolution of 128 ms (rounded down).
    pub fn new_ticks_since_lt(nth_key: u8, ticks_since: u16) -> Self {
        assert!(nth_key <= MAX_KEY_RECENCY);
        Self(TICKS_SINCE_VAL_LT | lossy_compress_ticks(ticks_since) | (u16::from(nth_key) << 10))
    }

    /// Return a new OpCode for a boolean operation that ends (non-inclusive) at the specified
    /// index.
    pub fn new_bool(op: BooleanOperator, end_idx: u16) -> Self {
        assert!(end_idx <= MAX_OPCODE_LEN);
        Self((end_idx & MAX_OPCODE_LEN) + op.to_u16())
    }

    /// Return OpCodes specifying an active input check.
    pub fn new_active_input(input: KCoord) -> (Self, Self) {
        assert!(input.0 < 4);
        assert!(input.1 < 0x0400);
        (
            Self(INPUT_VAL),
            Self((u16::from(input.0 & 3) << 14) + input.1),
        )
    }

    /// Return OpCodes specifying an active input check.
    pub fn new_historical_input(input: KCoord, key_recency: u8) -> (Self, Self) {
        assert!(input.0 < 4);
        assert!(input.1 < 0x0400);
        assert!(key_recency < 0x8);
        (
            Self(HISTORICAL_INPUT_VAL),
            Self((u16::from(input.0 & 3) << 14) + (u16::from(key_recency) << 11) + input.1),
        )
    }

    /// Return OpCodes specifying an active layer check.
    pub fn new_layer(layer: u16) -> (Self, Self) {
        assert!(usize::from(layer) < crate::layout::MAX_LAYERS);
        (Self(LAYER_VAL), Self(layer))
    }

    /// Return OpCodes specifying an base layer check.
    pub fn new_base_layer(base_layer: u16) -> (Self, Self) {
        assert!(usize::from(base_layer) < crate::layout::MAX_LAYERS);
        (Self(BASE_LAYER_VAL), Self(base_layer))
    }

    /// Return the interpretation of this `OpCode`.
    fn opcode_type(self, next: Option<OpCode>) -> OpCodeType {
        if self.0 < KEY_MAX {
            OpCodeType::KeyCode(self.0)
        } else if self.0 <= MAX_OPCODE_LEN {
            let op2 = next.expect("next should be some for opcode {self:?}");
            match self.0 {
                INPUT_VAL => OpCodeType::Input((((op2.0 >> 14) & 0x3) as u8, op2.0 & 0x3FF)),
                HISTORICAL_INPUT_VAL => OpCodeType::HistoricalInput(HistoricalInput {
                    input: (((op2.0 >> 14) & 0x3) as u8, op2.0 & 0x3FF),
                    how_far_back: (op2.0 >> 11) as u8 & 0x7,
                }),
                LAYER_VAL => OpCodeType::Layer(op2.0),
                BASE_LAYER_VAL => OpCodeType::BaseLayer(op2.0),
                _ => unreachable!("unexpected opcode {self:?}"),
            }
        } else {
            match self.0 & 0xE000 {
                TICKS_SINCE_VAL_LT => OpCodeType::TicksSinceLessThan(TicksSinceNthKey {
                    nth_key: ((self.0 & 0x1C00) >> 10) as u8,
                    ticks_since: lossy_decompress_ticks(self.0 & 0x03FF),
                }),
                TICKS_SINCE_VAL_GT => OpCodeType::TicksSinceGreaterThan(TicksSinceNthKey {
                    nth_key: ((self.0 & 0x1C00) >> 10) as u8,
                    ticks_since: lossy_decompress_ticks(self.0 & 0x03FF),
                }),
                0x8000..=0xF000 => OpCodeType::HistoricalKeyCode(HistoricalKeyCode {
                    key_code: self.0 & 0x0FFF,
                    how_far_back: ((self.0 & 0x7000) >> 12) as u8,
                }),
                _ => OpCodeType::BooleanOp(OperatorAndEndIndex::from(self.0)),
            }
        }
    }
}

impl From<u16> for OperatorAndEndIndex {
    fn from(value: u16) -> Self {
        Self {
            op: match value & OP_MASK {
                OR_VAL => Or,
                AND_VAL => And,
                NOT_VAL => Not,
                _ => unreachable!("public interface should protect from this"),
            },
            idx: usize::from(value & MAX_OPCODE_LEN),
        }
    }
}

/// Evaluate the return value of an expression evaluated on the given key codes.
fn evaluate_boolean(
    bool_expr: &[OpCode],
    key_codes: impl Iterator<Item = KeyCode> + Clone,
    inputs: impl Iterator<Item = KCoord> + Clone,
    historical_keys: impl Iterator<Item = HistoricalEvent<KeyCode>> + Clone,
    historical_inputs: impl Iterator<Item = HistoricalEvent<KCoord>> + Clone,
    layers: impl Iterator<Item = u16> + Clone,
    default_layer: u16,
) -> bool {
    let mut ret = true;
    let mut current_index = 0;
    let mut current_end_index = bool_expr.len();
    let mut current_op = Or;
    let mut stack: arraydeque::ArrayDeque<
        OperatorAndEndIndex,
        MAX_BOOL_EXPR_DEPTH,
        arraydeque::behavior::Saturating,
    > = Default::default();
    while current_index < bool_expr.len() {
        if current_index >= current_end_index {
            match stack.pop_back() {
                Some(operator) => {
                    (current_op, current_end_index) = (operator.op, operator.idx);
                }
                None => break,
            }
            // Short-circuiting logic
            if matches!((ret, current_op), (true, Or | Not) | (false, And))
                || current_index >= current_end_index
            {
                if current_op == Not {
                    ret = false;
                }
                current_index = current_end_index;
                continue;
            }
        }
        match bool_expr[current_index].opcode_type(bool_expr.get(current_index + 1).copied()) {
            OpCodeType::BooleanOp(operator) => {
                let res = stack.push_back(OperatorAndEndIndex {
                    op: current_op,
                    idx: current_end_index,
                });
                assert!(
                    res.is_ok(),
                    "exceeded boolean op depth {MAX_BOOL_EXPR_DEPTH}"
                );
                (current_op, current_end_index) = (operator.op, operator.idx);
                current_index += 1;
                continue;
            }
            OpCodeType::KeyCode(kc) => {
                ret = key_codes.clone().any(|kc_input| kc_input as u16 == kc);
            }
            OpCodeType::HistoricalKeyCode(hkc) => {
                ret = historical_keys
                    .clone()
                    .nth(hkc.how_far_back as usize)
                    .map(|he| he.event as u16 == hkc.key_code)
                    .unwrap_or(false);
            }
            OpCodeType::TicksSinceLessThan(tsnk) => {
                ret = historical_keys
                    .clone()
                    .nth(tsnk.nth_key.into())
                    .map(|he| he.ticks_since_occurrence <= tsnk.ticks_since)
                    .unwrap_or(false);
            }
            OpCodeType::TicksSinceGreaterThan(tsnk) => {
                ret = historical_keys
                    .clone()
                    .nth(tsnk.nth_key.into())
                    .map(|he| he.ticks_since_occurrence > tsnk.ticks_since)
                    .unwrap_or(false);
            }
            OpCodeType::Input(coord) => {
                // opcode has size 2
                current_index += 1;
                ret = inputs.clone().any(|c| c == coord)
            }
            OpCodeType::HistoricalInput(hki) => {
                // opcode has size 2
                current_index += 1;
                ret = historical_inputs
                    .clone()
                    .nth(hki.how_far_back as usize)
                    .map(|he| he.event == hki.input)
                    .unwrap_or(false)
            }
            OpCodeType::Layer(layer) => {
                // opcode has size 2
                current_index += 1;
                ret = layers.clone().next().map(|l| l == layer).unwrap_or(false)
            }
            OpCodeType::BaseLayer(base_layer) => {
                // opcode has size 2
                current_index += 1;
                ret = default_layer == base_layer;
            }
        };
        if current_op == Not {
            ret = !ret;
        }
        if matches!((ret, current_op), (true, Or) | (false, And | Not)) {
            current_index = current_end_index;
            continue;
        }
        current_index += 1;
    }
    while let Some(OperatorAndEndIndex { op, .. }) = stack.pop_back() {
        if op == Not {
            ret = !ret;
        }
    }
    ret
}

#[cfg(test)]
fn evaluate_bool_test(opcodes: &[OpCode], keycodes: impl Iterator<Item = KeyCode> + Clone) -> bool {
    evaluate_boolean(
        opcodes,
        keycodes,
        [].iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        0,
    )
}

#[test]
fn bool_evaluation_test_0() {
    let opcodes = [
        OpCode::new_bool(And, 9),
        OpCode::new_key(KeyCode::A),
        OpCode::new_key(KeyCode::B),
        OpCode::new_bool(Or, 6),
        OpCode::new_key(KeyCode::C),
        OpCode::new_key(KeyCode::D),
        OpCode::new_bool(Or, 9),
        OpCode::new_key(KeyCode::E),
        OpCode::new_key(KeyCode::F),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_1() {
    let opcodes = [
        OpCode::new_bool(And, 9),
        OpCode::new_key(KeyCode::A),
        OpCode::new_key(KeyCode::B),
        OpCode::new_bool(Or, 6),
        OpCode::new_key(KeyCode::C),
        OpCode::new_key(KeyCode::D),
        OpCode::new_bool(Or, 9),
        OpCode::new_key(KeyCode::E),
        OpCode::new_key(KeyCode::F),
    ];
    let keycodes = [
        KeyCode::A,
        KeyCode::B,
        KeyCode::C,
        KeyCode::D,
        KeyCode::E,
        KeyCode::F,
    ];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_2() {
    let opcodes = [
        OpCode(0x2009),
        OpCode(KeyCode::A as u16),
        OpCode(KeyCode::B as u16),
        OpCode(0x1006),
        OpCode(KeyCode::C as u16),
        OpCode(KeyCode::D as u16),
        OpCode(0x1009),
        OpCode(KeyCode::E as u16),
        OpCode(KeyCode::F as u16),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::E, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_3() {
    let opcodes = [
        OpCode(0x2009),
        OpCode(KeyCode::A as u16),
        OpCode(KeyCode::B as u16),
        OpCode(0x1006),
        OpCode(KeyCode::C as u16),
        OpCode(KeyCode::D as u16),
        OpCode(0x1009),
        OpCode(KeyCode::E as u16),
        OpCode(KeyCode::F as u16),
    ];
    let keycodes = [KeyCode::B, KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_4() {
    let opcodes = [];
    let keycodes = [];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_5() {
    let opcodes = [];
    let keycodes = [
        KeyCode::A,
        KeyCode::B,
        KeyCode::C,
        KeyCode::D,
        KeyCode::E,
        KeyCode::F,
    ];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_6() {
    let opcodes = [OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16)];
    let keycodes = [
        KeyCode::A,
        KeyCode::B,
        KeyCode::C,
        KeyCode::D,
        KeyCode::E,
        KeyCode::F,
    ];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_7() {
    let opcodes = [OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16)];
    let keycodes = [KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_9() {
    let opcodes = [
        OpCode(0x2003),
        OpCode(KeyCode::A as u16),
        OpCode(KeyCode::B as u16),
        OpCode(KeyCode::C as u16),
    ];
    let keycodes = [KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_10() {
    let opcodes = [
        OpCode(0x2004),
        OpCode(KeyCode::A as u16),
        OpCode(KeyCode::B as u16),
        OpCode(KeyCode::C as u16),
    ];
    let keycodes = [KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_11() {
    let opcodes = [
        OpCode(0x1003),
        OpCode(KeyCode::A as u16),
        OpCode(KeyCode::B as u16),
    ];
    let keycodes = [KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_12() {
    let opcodes = [
        OpCode(0x1005),
        OpCode(0x2004),
        OpCode(KeyCode::A as u16),
        OpCode(KeyCode::B as u16),
        OpCode(KeyCode::C as u16),
    ];
    let keycodes = [KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_max_depth_does_not_panic() {
    let opcodes = [
        OpCode(0x1008),
        OpCode(0x1008),
        OpCode(0x1008),
        OpCode(0x1008),
        OpCode(0x1008),
        OpCode(0x1008),
        OpCode(0x1008),
        OpCode(0x1008),
    ];
    let keycodes = [];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
#[should_panic]
fn bool_evaluation_test_more_than_max_depth_panics() {
    let opcodes = [
        OpCode(0x1009),
        OpCode(0x1009),
        OpCode(0x1009),
        OpCode(0x1009),
        OpCode(0x1009),
        OpCode(0x1009),
        OpCode(0x1009),
        OpCode(0x1009),
        OpCode(0x1009),
    ];
    let keycodes = [];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn switch_fallthrough() {
    let sw = Switch {
        cases: &[
            (&[], &Action::<()>::KeyCode(KeyCode::A), Fallthrough),
            (&[], &Action::<()>::KeyCode(KeyCode::B), Fallthrough),
        ],
    };
    let mut actions = sw.actions(
        [].iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        0,
    );
    assert_eq!(actions.next(), Some(&Action::<()>::KeyCode(KeyCode::A)));
    assert_eq!(actions.next(), Some(&Action::<()>::KeyCode(KeyCode::B)));
    assert_eq!(actions.next(), None);
}

#[test]
fn switch_break() {
    let sw = Switch {
        cases: &[
            (&[], &Action::<()>::KeyCode(KeyCode::A), Break),
            (&[], &Action::<()>::KeyCode(KeyCode::B), Break),
        ],
    };
    let mut actions = sw.actions(
        [].iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        0,
    );
    assert_eq!(actions.next(), Some(&Action::<()>::KeyCode(KeyCode::A)));
    assert_eq!(actions.next(), None);
}

#[test]
fn switch_no_actions() {
    let sw = Switch {
        cases: &[
            (
                &[OpCode::new_key(KeyCode::A)],
                &Action::<()>::KeyCode(KeyCode::A),
                Break,
            ),
            (
                &[OpCode::new_key(KeyCode::A)],
                &Action::<()>::KeyCode(KeyCode::B),
                Break,
            ),
        ],
    };
    let mut actions = sw.actions(
        [].iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        0,
    );
    assert_eq!(actions.next(), None);
}

#[test]
fn switch_historical_1() {
    let opcode_true = [OpCode(0x8000 | KeyCode::A as u16)];
    let opcode_true2 = [OpCode(0xF000 | KeyCode::H as u16)];
    let opcode_false = [OpCode(0x9000 | KeyCode::A as u16)];
    let opcode_false2 = [OpCode(0xE000 | KeyCode::H as u16)];
    assert_eq!(
        OpCode::new_key_history(KeyCode::A, 0),
        OpCode(0x8000 | KeyCode::A as u16)
    );
    assert_eq!(
        OpCode::new_key_history(KeyCode::H, 7),
        OpCode(0xF000 | KeyCode::H as u16)
    );
    let hist_keycodes = [
        HistoricalEvent {
            event: KeyCode::A,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::B,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::C,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::D,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::E,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::F,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::G,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::H,
            ticks_since_occurrence: 0,
        },
    ];
    assert!(evaluate_boolean(
        opcode_true.as_slice(),
        [].iter().copied(),
        [].iter().copied(),
        hist_keycodes.iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        0,
    ));
    assert!(evaluate_boolean(
        opcode_true2.as_slice(),
        [].iter().copied(),
        [].iter().copied(),
        hist_keycodes.iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        0,
    ));
    assert!(!evaluate_boolean(
        opcode_false.as_slice(),
        [].iter().copied(),
        [].iter().copied(),
        hist_keycodes.iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        0,
    ));
    assert!(!evaluate_boolean(
        opcode_false2.as_slice(),
        [].iter().copied(),
        [].iter().copied(),
        hist_keycodes.iter().copied(),
        [].iter().copied(),
        [].iter().copied(),
        0,
    ));
}

#[test]
fn switch_historical_bools() {
    let opcodes_true_and = [
        OpCode::new_bool(And, 3),
        OpCode::new_key_history(KeyCode::A, 0),
        OpCode::new_key_history(KeyCode::B, 1),
    ];
    let opcodes_false_and1 = [
        OpCode::new_bool(And, 3),
        OpCode::new_key_history(KeyCode::A, 0),
        OpCode::new_key_history(KeyCode::B, 2),
    ];
    let opcodes_false_and2 = [
        OpCode::new_bool(And, 3),
        OpCode::new_key_history(KeyCode::B, 2),
        OpCode::new_key_history(KeyCode::A, 0),
    ];
    let opcodes_true_or1 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_key_history(KeyCode::A, 0),
        OpCode::new_key_history(KeyCode::B, 1),
    ];
    let opcodes_true_or2 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_key_history(KeyCode::A, 0),
        OpCode::new_key_history(KeyCode::B, 2),
    ];
    let opcodes_true_or3 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_key_history(KeyCode::B, 2),
        OpCode::new_key_history(KeyCode::A, 0),
    ];
    let opcodes_false_or = [
        OpCode::new_bool(Or, 3),
        OpCode::new_key_history(KeyCode::A, 1),
        OpCode::new_key_history(KeyCode::B, 2),
    ];
    let hist_keycodes = [
        HistoricalEvent {
            event: KeyCode::A,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::B,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::C,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::D,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::E,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::F,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::G,
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: KeyCode::H,
            ticks_since_occurrence: 0,
        },
    ];

    let test = |opcodes: &[OpCode], expectation: bool| {
        assert_eq!(
            evaluate_boolean(
                opcodes,
                [].iter().copied(),
                [].iter().copied(),
                hist_keycodes.iter().copied(),
                [].iter().copied(),
                [].iter().copied(),
                0,
            ),
            expectation
        );
    };
    test(&opcodes_true_and, true);
    test(&opcodes_true_or1, true);
    test(&opcodes_true_or2, true);
    test(&opcodes_true_or3, true);
    test(&opcodes_false_and1, false);
    test(&opcodes_false_and2, false);
    test(&opcodes_false_or, false);
}

#[test]
fn switch_historical_ticks_since() {
    let opcodes_true_and = [
        OpCode::new_bool(And, 3),
        OpCode::new_ticks_since_gt(0, 99),
        OpCode::new_ticks_since_lt(0, 101),
    ];
    let opcodes_false_and1 = [
        OpCode::new_bool(And, 3),
        OpCode::new_ticks_since_gt(1, 200),
        OpCode::new_ticks_since_lt(1, 240),
    ];
    let opcodes_false_and2 = [
        OpCode::new_bool(And, 3),
        OpCode::new_ticks_since_gt(2, 300),
        OpCode::new_ticks_since_lt(2, 300),
    ];
    let opcodes_true_or1 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_ticks_since_gt(3, 500),
        OpCode::new_ticks_since_lt(3, 510),
    ];
    let opcodes_true_or2 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_ticks_since_gt(4, 500),
        OpCode::new_ticks_since_lt(4, 511),
    ];
    let opcodes_true_or3 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_ticks_since_gt(5, 980),
        OpCode::new_ticks_since_lt(5, 999),
    ];
    let opcodes_false_or1 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_ticks_since_gt(6, 40200),
        OpCode::new_ticks_since_lt(6, 39999),
    ];
    let opcodes_false_or2 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_ticks_since_gt(5, 1030),
        OpCode::new_ticks_since_lt(5, 999),
    ];
    let opcodes_false_or3 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_ticks_since_gt(4, 520),
        OpCode::new_ticks_since_lt(4, 511),
    ];
    let opcodes_false_or4 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_ticks_since_gt(3, 520),
        OpCode::new_ticks_since_lt(3, 510),
    ];
    let opcodes_false_or5 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_ticks_since_gt(2, 265),
        OpCode::new_ticks_since_lt(2, 255),
    ];
    let opcodes_false_or6 = [
        OpCode::new_bool(Or, 3),
        OpCode::new_ticks_since_gt(1, 256),
        OpCode::new_ticks_since_lt(1, 254),
    ];
    let hist_keycodes = [
        HistoricalEvent {
            event: KeyCode::A,
            ticks_since_occurrence: 100,
        },
        HistoricalEvent {
            event: KeyCode::B,
            ticks_since_occurrence: 255,
        },
        HistoricalEvent {
            event: KeyCode::C,
            ticks_since_occurrence: 256,
        },
        HistoricalEvent {
            event: KeyCode::D,
            ticks_since_occurrence: 511,
        },
        HistoricalEvent {
            event: KeyCode::E,
            ticks_since_occurrence: 512,
        },
        HistoricalEvent {
            event: KeyCode::F,
            ticks_since_occurrence: 1000,
        },
        HistoricalEvent {
            event: KeyCode::G,
            ticks_since_occurrence: 40000,
        },
    ];

    let test = |opcodes: &[OpCode], expectation: bool| {
        assert_eq!(
            evaluate_boolean(
                opcodes,
                [].iter().copied(),
                [].iter().copied(),
                hist_keycodes.iter().copied(),
                [].iter().copied(),
                [].iter().copied(),
                0,
            ),
            expectation
        );
    };
    test(&opcodes_true_and, true);
    test(&opcodes_true_or1, true);
    test(&opcodes_true_or2, true);
    test(&opcodes_true_or3, true);
    test(&opcodes_false_and1, false);
    test(&opcodes_false_and2, false);
    test(&opcodes_false_or1, false);
    test(&opcodes_false_or2, false);
    test(&opcodes_false_or3, false);
    test(&opcodes_false_or4, false);
    test(&opcodes_false_or5, false);
    test(&opcodes_false_or6, false);
}

#[test]
fn bool_evaluation_test_not_0() {
    // Full inverse of a previous test
    let opcodes = [
        OpCode::new_bool(Not, 10),
        OpCode::new_bool(And, 10),
        OpCode::new_key(KeyCode::A),
        OpCode::new_key(KeyCode::B),
        OpCode::new_bool(Or, 7),
        OpCode::new_key(KeyCode::C),
        OpCode::new_key(KeyCode::D),
        OpCode::new_bool(Or, 10),
        OpCode::new_key(KeyCode::E),
        OpCode::new_key(KeyCode::F),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_not_1() {
    // Both A and B exist, should be false
    let opcodes = [
        OpCode::new_bool(Not, 3),
        OpCode::new_key(KeyCode::A),
        OpCode::new_key(KeyCode::B),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_not_2() {
    // Neither X nor Y exist, should be false
    let opcodes = [
        OpCode::new_bool(Not, 3),
        OpCode::new_key(KeyCode::X),
        OpCode::new_key(KeyCode::Y),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_not_3() {
    let opcodes = [
        OpCode::new_key(KeyCode::C),
        OpCode::new_bool(Not, 3),
        OpCode::new_key(KeyCode::D),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_not_4() {
    let opcodes = [
        OpCode::new_bool(And, 10),
        OpCode::new_key(KeyCode::A),
        OpCode::new_key(KeyCode::B),
        OpCode::new_bool(Or, 7),
        OpCode::new_key(KeyCode::C),
        OpCode::new_bool(Not, 7),
        OpCode::new_key(KeyCode::D),
        OpCode::new_bool(Or, 10),
        OpCode::new_key(KeyCode::E),
        OpCode::new_key(KeyCode::F),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_not_5() {
    let opcodes = [
        OpCode::new_bool(Not, 4),
        OpCode::new_key(KeyCode::C),
        OpCode::new_bool(Not, 4),
        OpCode::new_key(KeyCode::D),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_not_6() {
    // C does not exist, D does. Ensure C nonexistence does not short-circuit
    // and existence of D is checked.
    let opcodes = [
        OpCode::new_bool(Not, 3),
        OpCode::new_key(KeyCode::C),
        OpCode::new_key(KeyCode::D),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_or_equivalency_not_6() {
    let opcodes = [
        OpCode::new_bool(Not, 4),
        OpCode::new_bool(Or, 4),
        OpCode::new_key(KeyCode::C),
        OpCode::new_key(KeyCode::D),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_not_7() {
    // A exists, make sure this short-circuits, and E nonexistence does not override the return.
    let opcodes = [
        OpCode::new_bool(Not, 3),
        OpCode::new_key(KeyCode::A),
        OpCode::new_key(KeyCode::E),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_or_equivalency_not_7() {
    let opcodes = [
        OpCode::new_bool(Not, 4),
        OpCode::new_bool(Or, 4),
        OpCode::new_key(KeyCode::A),
        OpCode::new_key(KeyCode::E),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_not_8() {
    let opcodes = [
        OpCode::new_bool(Not, 4),
        OpCode::new_bool(Not, 4),
        OpCode::new_bool(Not, 4),
        OpCode::new_key(KeyCode::A),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(!evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn bool_evaluation_test_not_9() {
    let opcodes = [
        OpCode::new_bool(Not, 4),
        OpCode::new_bool(Not, 4),
        OpCode::new_bool(Not, 4),
        OpCode::new_key(KeyCode::C),
    ];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::D, KeyCode::F];
    assert!(evaluate_bool_test(
        opcodes.as_slice(),
        keycodes.iter().copied(),
    ));
}

#[test]
fn switch_inputs() {
    let (op1, op2) = OpCode::new_active_input((0, 1));
    let (op3, op4) = OpCode::new_active_input((1, 2));
    let (op5, op6) = OpCode::new_active_input((1, 3));
    let (op7, op8) = OpCode::new_active_input((3, 3));
    let opcodes_true_and = [OpCode::new_bool(And, 5), op1, op2, op3, op4];
    let opcodes_false_and1 = [OpCode::new_bool(And, 5), op1, op2, op5, op6];
    let opcodes_false_and2 = [OpCode::new_bool(And, 5), op5, op6, op1, op2];
    let opcodes_false_or = [OpCode::new_bool(Or, 5), op7, op8, op5, op6];
    let opcodes_true_or1 = [OpCode::new_bool(Or, 5), op1, op2, op5, op6];
    let opcodes_true_or2 = [OpCode::new_bool(Or, 5), op7, op8, op3, op4];
    let active_inputs = [(0, 1), (1, 2), (2, 3), (3, 4)];
    let test = |opcodes: &[OpCode], expectation: bool| {
        assert_eq!(
            evaluate_boolean(
                opcodes,
                [].iter().copied(),
                active_inputs.iter().copied(),
                [].iter().copied(),
                [].iter().copied(),
                [].iter().copied(),
                0,
            ),
            expectation
        );
    };
    test(&opcodes_true_and, true);
    test(&opcodes_false_and1, false);
    test(&opcodes_false_and2, false);
    test(&opcodes_false_or, false);
    test(&opcodes_true_or1, true);
    test(&opcodes_true_or2, true);
}

#[test]
fn switch_historical_inputs() {
    let (op1, op2) = OpCode::new_historical_input((0, 0), 0);
    let (op3, op4) = OpCode::new_historical_input((3, 750), 7);
    let (op5, op6) = OpCode::new_historical_input((1, 3), 0);
    let (op7, op8) = OpCode::new_historical_input((3, 3), 7);
    let opcodes_true_and = [OpCode::new_bool(And, 5), op1, op2, op3, op4];
    let opcodes_false_and1 = [OpCode::new_bool(And, 5), op1, op2, op5, op6];
    let opcodes_false_and2 = [OpCode::new_bool(And, 5), op5, op6, op1, op2];
    let opcodes_false_or = [OpCode::new_bool(Or, 5), op7, op8, op5, op6];
    let opcodes_true_or1 = [OpCode::new_bool(Or, 5), op1, op2, op5, op6];
    let opcodes_true_or2 = [OpCode::new_bool(Or, 5), op7, op8, op3, op4];
    let historical_inputs = [
        HistoricalEvent {
            event: (0, 0),
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: (1, 750),
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: (2, 1),
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: (3, 749),
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: (0, 1),
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: (1, 2),
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: (2, 3),
            ticks_since_occurrence: 0,
        },
        HistoricalEvent {
            event: (3, 750),
            ticks_since_occurrence: 0,
        },
    ];
    let test = |opcodes: &[OpCode], expectation: bool| {
        assert_eq!(
            evaluate_boolean(
                opcodes,
                [].iter().copied(),
                [].iter().copied(),
                [].iter().copied(),
                historical_inputs.iter().copied(),
                [].iter().copied(),
                0,
            ),
            expectation
        );
    };
    test(&opcodes_true_and, true);
    test(&opcodes_false_and1, false);
    test(&opcodes_false_and2, false);
    test(&opcodes_false_or, false);
    test(&opcodes_true_or1, true);
    test(&opcodes_true_or2, true);
}
