//! Handle processing of the switch action for Keyberon.
//!
//! Limitations:
//! - Maximum opcode length: 4095
//! - Maximum boolean expression depth: 8
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
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// The operation type and the opcode index at which evaluating this type ends.
struct OperatorAndEndIndex {
    pub op: BooleanOperator,
    pub idx: usize,
}

#[derive(Debug, Copy, Clone, PartialEq)]
/// Whether or not a case should break out of the switch if it evaluates to true or fallthrough to
/// the next case.
pub enum BreakOrFallthrough {
    Break,
    Fallthrough,
}

impl<'a, T> Switch<'a, T> {
    /// Iterates over the actions (if any) that are activated in the `Switch` based on its cases
    /// and the currently active keys.
    pub fn actions<T2>(&self, active_keys: T2) -> SwitchActions<'a, T, T2>
    where
        T2: Iterator<Item = KeyCode> + Clone,
    {
        SwitchActions {
            cases: self.cases,
            active_keys,
            case_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
/// Iterator returned by `Switch::actions`.
pub struct SwitchActions<'a, T, T2>
where
    T2: Iterator<Item = KeyCode> + Clone,
{
    cases: &'a [(&'a [OpCode], &'a Action<'a, T>, BreakOrFallthrough)],
    active_keys: T2,
    case_index: usize,
}

impl<'a, T, T2> Iterator for SwitchActions<'a, T, T2>
where
    T2: Iterator<Item = KeyCode> + Clone,
{
    type Item = &'a Action<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.case_index < self.cases.len() {
            let case = &self.cases[self.case_index];
            if evaluate_boolean(case.0, self.active_keys.clone()) {
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

    /// Return a new OpCode for a boolean operation that ends (non-inclusive) at the specified
    /// index.
    pub fn new_bool(op: BooleanOperator, end_idx: u16) -> Self {
        Self((end_idx & MAX_OPCODE_LEN) + op.to_u16())
    }
    /// Return the interpretation of this `OpCode`.
    fn opcode_type(self) -> OpCodeType {
        if self.0 < MAX_OPCODE_LEN {
            OpCodeType::KeyCode(self.0)
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
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        true
    );
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
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        true
    );
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
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        false
    );
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
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        false
    );
}

#[test]
fn bool_evaluation_test_4() {
    let opcodes = [];
    let keycodes = [];
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        true
    );
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
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        true
    );
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
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        true
    );
}

#[test]
fn bool_evaluation_test_7() {
    let opcodes = [OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16)];
    let keycodes = [KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        false
    );
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
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        true
    );
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
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        false
    );
}

#[test]
fn bool_evaluation_test_11() {
    let opcodes = [
        OpCode(0x1003),
        OpCode(KeyCode::A as u16),
        OpCode(KeyCode::B as u16),
    ];
    let keycodes = [KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        false
    );
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
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        true
    );
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
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        true
    );
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
    assert_eq!(
        evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()),
        true
    );
}

#[test]
fn switch_fallthrough() {
    let sw = Switch {
        cases: &[
            (&[], &Action::<()>::KeyCode(KeyCode::A), Fallthrough),
            (&[], &Action::<()>::KeyCode(KeyCode::B), Fallthrough),
        ],
    };
    let mut actions = sw.actions([].iter().copied());
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
    let mut actions = sw.actions([].iter().copied());
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
    let mut actions = sw.actions([].iter().copied());
    assert_eq!(actions.next(), None);
}
