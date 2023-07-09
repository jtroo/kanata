//! Handle processing of the switch action for Keyberon.
//!
//! Limitations:
//! - Maximum opcode length: 4095
//! - Maximum boolean expression depth: 8

use crate::key_code::*;

pub const MAX_OPCODE_LEN: usize = 0x0FFF;
pub const MAX_BOOL_EXPR_DEPTH: usize = 8;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// Operator for the opcode, inluding its end index.
/// The maximum end index is 0x0FFF, or 4095.
pub enum BooleanOperator {
    Or,
    And,
}

use BooleanOperator::*;
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OpCode(u16);

impl OpCode {
    /// Return the interpretation of this `OpCode`.
    pub fn opcode_type(self) -> OpCodeType {
        if self.0 < (MAX_OPCODE_LEN as u16) {
            OpCodeType::KeyCode(self.0)
        } else {
            OpCodeType::BooleanOp(OperatorAndEndIndex::from(self.0))
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// Interpretion of an OpCode.
pub enum OpCodeType {
    BooleanOp(OperatorAndEndIndex),
    KeyCode(u16),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct OperatorAndEndIndex {
    pub op: BooleanOperator,
    pub idx: usize,
}

impl From<u16> for OperatorAndEndIndex {
    fn from(value: u16) -> Self {
        Self {
            op: match value & 0xF000 {
                0x2000 => And,
                0x1000 => Or,
                _ => unreachable!("invalid opcode: {}", value),
            },
            idx: usize::from(value & (MAX_OPCODE_LEN as u16)),
        }
    }
}

/// Evaluate the return value of an expression evaluated on the given key codes.
pub fn evaluate_boolean(
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
                if let Err(_) = stack.push_back(OperatorAndEndIndex {
                    op: current_op,
                    idx: current_end_index,
                }) {
                    panic!("exceeded boolean op depth {}", MAX_BOOL_EXPR_DEPTH);
                }
                (current_op, current_end_index) = (operator.op, operator.idx);
            }
        };
        current_index += 1;
    }
    ret
}

#[test]
fn bool_evaluation_test_1() {
    let opcodes = [OpCode(0x2009), OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16), OpCode(0x1006), OpCode(KeyCode::C as u16), OpCode(KeyCode::D as u16), OpCode(0x1009), OpCode(KeyCode::E as u16), OpCode(KeyCode::F as u16)];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert_eq!(evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()), true);
}

#[test]
fn bool_evaluation_test_2() {
    let opcodes = [OpCode(0x2009), OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16), OpCode(0x1006), OpCode(KeyCode::C as u16), OpCode(KeyCode::D as u16), OpCode(0x1009), OpCode(KeyCode::E as u16), OpCode(KeyCode::F as u16)];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::E, KeyCode::F];
    assert_eq!(evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()), false);
}

#[test]
fn bool_evaluation_test_3() {
    let opcodes = [OpCode(0x2009), OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16), OpCode(0x1006), OpCode(KeyCode::C as u16), OpCode(KeyCode::D as u16), OpCode(0x1009), OpCode(KeyCode::E as u16), OpCode(KeyCode::F as u16)];
    let keycodes = [KeyCode::B, KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert_eq!(evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()), false);
}

#[test]
fn bool_evaluation_test_4() {
    let opcodes = [];
    let keycodes = [];
    assert_eq!(evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()), true);
}

#[test]
fn bool_evaluation_test_5() {
    let opcodes = [];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert_eq!(evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()), true);
}

#[test]
fn bool_evaluation_test_6() {
    let opcodes = [ OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16)];
    let keycodes = [KeyCode::A, KeyCode::B, KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert_eq!(evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()), true);
}

#[test]
fn bool_evaluation_test_7() {
    let opcodes = [ OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16)];
    let keycodes = [ KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert_eq!(evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()), false);
}

#[test]
fn bool_evaluation_test_9() {
    let opcodes = [ OpCode(0x2003), OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16), OpCode(KeyCode::C as u16)];
    let keycodes = [ KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert_eq!(evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()), true);
}

#[test]
fn bool_evaluation_test_10() {
    let opcodes = [ OpCode(0x2004), OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16), OpCode(KeyCode::C as u16)];
    let keycodes = [ KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert_eq!(evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()), false);
}

#[test]
fn bool_evaluation_test_11() {
    let opcodes = [ OpCode(0x1003), OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16)];
    let keycodes = [ KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert_eq!(evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()), false);
}

#[test]
fn bool_evaluation_test_12() {
    let opcodes = [ OpCode(0x1005), OpCode(0x2004), OpCode(KeyCode::A as u16), OpCode(KeyCode::B as u16), OpCode(KeyCode::C as u16)];
    let keycodes = [ KeyCode::C, KeyCode::D, KeyCode::E, KeyCode::F];
    assert_eq!(evaluate_boolean(opcodes.as_slice(), keycodes.iter().copied()), true);
}
