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

use crate::key_code::*;

use BooleanOperator::*;
use BreakOrFallthrough::*;

pub const MAX_OPCODE_LEN: u16 = 0x0FFF;
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

const OR_VAL: u16 = 0x1000;
const AND_VAL: u16 = 0x2000;
// Highest bit in u16. Lower 3 bits in the highest nibble are "how far back". This means that
// switch can look back up to 8 keys.
const HISTORICAL_KEYCODE_VAL: u16 = 0x8000;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// Boolean operator. Notably missing today is Not.
pub enum BooleanOperator {
    Or,
    And,
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
    pub fn actions<A, H>(&self, active_keys: A, historical_keys: H) -> SwitchActions<'a, T, A, H>
    where
        A: Iterator<Item = KeyCode> + Clone,
        H: Iterator<Item = KeyCode> + Clone,
    {
        SwitchActions {
            cases: self.cases,
            active_keys,
            historical_keys,
            case_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
/// Iterator returned by `Switch::actions`.
pub struct SwitchActions<'a, T, A, H>
where
    A: Iterator<Item = KeyCode> + Clone,
    H: Iterator<Item = KeyCode> + Clone,
{
    cases: &'a [(&'a [OpCode], &'a Action<'a, T>, BreakOrFallthrough)],
    active_keys: A,
    historical_keys: H,
    case_index: usize,
}

impl<'a, T, A, H> Iterator for SwitchActions<'a, T, A, H>
where
    A: Iterator<Item = KeyCode> + Clone,
    H: Iterator<Item = KeyCode> + Clone,
{
    type Item = &'a Action<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.case_index < self.cases.len() {
            let case = &self.cases[self.case_index];
            if evaluate_boolean(
                case.0,
                self.active_keys.clone(),
                self.historical_keys.clone(),
            ) {
                let ret_ac = case.1;
                match case.2 {
                    Break => self.case_index = self.cases.len(),
                    Fallthrough => self.case_index += 1,
                }
                return Some(ret_ac);
            } else {
                self.case_index += 1;
            }
        }
        None
    }
}

impl BooleanOperator {
    fn to_u16(self) -> u16 {
        match self {
            Or => OR_VAL,
            And => AND_VAL,
        }
    }
}

impl OpCode {
    /// Return a new OpCode that checks if the key active or not.
    pub fn new_key(kc: KeyCode) -> Self {
        assert!((kc as u16) <= MAX_OPCODE_LEN);
        Self(kc as u16 & MAX_OPCODE_LEN)
    }

    /// Return a new OpCode that checks if the n'th most recent key, defined by `key_recency`,
    /// matches the input keycode.
    pub fn new_key_history(kc: KeyCode, key_recency: u8) -> Self {
        assert!((kc as u16) <= MAX_OPCODE_LEN);
        assert!(key_recency <= MAX_KEY_RECENCY);
        Self((kc as u16 & MAX_OPCODE_LEN) | HISTORICAL_KEYCODE_VAL | ((key_recency as u16) << 12))
    }

    /// Return a new OpCode for a boolean operation that ends (non-inclusive) at the specified
    /// index.
    pub fn new_bool(op: BooleanOperator, end_idx: u16) -> Self {
        Self((end_idx & MAX_OPCODE_LEN) + op.to_u16())
    }

    /// Return the interpretation of this `OpCode`.
    fn opcode_type(self) -> OpCodeType {
        if self.0 < MAX_OPCODE_LEN {
            OpCodeType::KeyCode(self.0)
        } else if self.0 & HISTORICAL_KEYCODE_VAL == HISTORICAL_KEYCODE_VAL {
            OpCodeType::HistoricalKeyCode(HistoricalKeyCode {
                key_code: self.0 & 0x0FFF,
                how_far_back: ((self.0 & 0x7000) >> 12) as u8,
            })
        } else {
            OpCodeType::BooleanOp(OperatorAndEndIndex::from(self.0))
        }
    }
}

impl From<u16> for OperatorAndEndIndex {
    fn from(value: u16) -> Self {
        Self {
            op: match value & 0xF000 {
                OR_VAL => Or,
                AND_VAL => And,
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
    historical_keys: impl Iterator<Item = KeyCode> + Clone,
) -> bool {
    let mut ret = true;
    let mut current_index = 0;
    let mut current_end_index = bool_expr.len();
    let mut current_op = Or;
    let mut stack: arraydeque::ArrayDeque<
        [OperatorAndEndIndex; MAX_BOOL_EXPR_DEPTH],
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
            if matches!((ret, current_op), (true, Or) | (false, And)) {
                current_index = current_end_index;
                continue;
            }
        }
        match bool_expr[current_index].opcode_type() {
            OpCodeType::KeyCode(kc) => {
                ret = key_codes.clone().any(|kc_input| kc_input as u16 == kc);
                if matches!((ret, current_op), (true, Or) | (false, And)) {
                    current_index = current_end_index;
                    continue;
                }
            }
            OpCodeType::HistoricalKeyCode(hkc) => {
                ret = historical_keys
                    .clone()
                    .nth(hkc.how_far_back as usize)
                    .map(|kc| kc as u16 == hkc.key_code)
                    .unwrap_or(false);
                if matches!((ret, current_op), (true, Or) | (false, And)) {
                    current_index = current_end_index;
                    continue;
                }
            }
            OpCodeType::BooleanOp(operator) => {
                let res = stack.push_back(OperatorAndEndIndex {
                    op: current_op,
                    idx: current_end_index,
                });
                assert!(
                    res.is_ok(),
                    "exceeded boolean op depth {}",
                    MAX_BOOL_EXPR_DEPTH
                );
                (current_op, current_end_index) = (operator.op, operator.idx);
            }
        };
        current_index += 1;
    }
    ret
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
    assert!(evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    assert!(evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    assert!(!evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    assert!(!evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
    ));
}

#[test]
fn bool_evaluation_test_4() {
    let opcodes = [];
    let keycodes = [];
    assert!(evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    assert!(evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    assert!(evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
    ));
}

#[test]
fn bool_evaluation_test_7() {
    let opcodes = [OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16)];
    let keycodes = [KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert!(!evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    assert!(evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    assert!(!evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    assert!(!evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    assert!(evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    assert!(evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    assert!(evaluate_boolean(
        opcodes.as_slice(),
        keycodes.iter().copied(),
        [].iter().copied()
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
    let mut actions = sw.actions([].iter().copied(), [].iter().copied());
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
    let mut actions = sw.actions([].iter().copied(), [].iter().copied());
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
    let mut actions = sw.actions([].iter().copied(), [].iter().copied());
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
        KeyCode::A,
        KeyCode::B,
        KeyCode::C,
        KeyCode::D,
        KeyCode::E,
        KeyCode::F,
        KeyCode::G,
        KeyCode::H,
    ];
    assert!(evaluate_boolean(
        opcode_true.as_slice(),
        [].iter().copied(),
        hist_keycodes.iter().copied(),
    ));
    assert!(evaluate_boolean(
        opcode_true2.as_slice(),
        [].iter().copied(),
        hist_keycodes.iter().copied(),
    ));
    assert!(!evaluate_boolean(
        opcode_false.as_slice(),
        [].iter().copied(),
        hist_keycodes.iter().copied(),
    ));
    assert!(!evaluate_boolean(
        opcode_false2.as_slice(),
        [].iter().copied(),
        hist_keycodes.iter().copied(),
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
        KeyCode::A,
        KeyCode::B,
        KeyCode::C,
        KeyCode::D,
        KeyCode::E,
        KeyCode::F,
        KeyCode::G,
        KeyCode::H,
    ];

    let test = |opcodes: &[OpCode], expectation: bool| {
        assert_eq!(
            evaluate_boolean(opcodes, [].iter().copied(), hist_keycodes.iter().copied(),),
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
