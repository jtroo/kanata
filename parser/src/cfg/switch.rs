use super::sexpr::*;
use super::*;
use crate::{anyhow_expr, bail, bail_expr};

pub fn parse_switch(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_STR: &str =
        "switch expects triples of params: <key match> <action> <break|fallthrough>";

    let mut cases = vec![];

    let mut params = ac_params.iter();
    loop {
        let Some(key_match) = params.next() else {
            break;
        };
        let Some(action) = params.next() else {
            bail!("{ERR_STR}\nMissing <action> and <break|fallthrough> for the final triple");
        };
        let Some(break_or_fallthrough_expr) = params.next() else {
            bail!("{ERR_STR}\nMissing <break|fallthrough> for the final triple");
        };

        let Some(key_match) = key_match.list(s.vars()) else {
            bail_expr!(key_match, "{ERR_STR}\n<key match> must be a list")
        };
        let mut ops = vec![];
        for op in key_match.iter() {
            parse_switch_case_bool(1, op, &mut ops, s)?;
        }

        let action = parse_action(action, s)?;

        let Some(break_or_fallthrough) = break_or_fallthrough_expr.atom(s.vars()) else {
            bail_expr!(
                break_or_fallthrough_expr,
                "{ERR_STR}\nthis must be one of: break, fallthrough"
            );
        };
        let break_or_fallthrough = match break_or_fallthrough {
            "break" => BreakOrFallthrough::Break,
            "fallthrough" => BreakOrFallthrough::Fallthrough,
            _ => bail_expr!(
                break_or_fallthrough_expr,
                "{ERR_STR}\nthis must be one of: break, fallthrough"
            ),
        };
        cases.push((s.a.sref_vec(ops), action, break_or_fallthrough));
    }
    Ok(s.a.sref(Action::Switch(s.a.sref(Switch {
        cases: s.a.sref_vec(cases),
    }))))
}

pub fn parse_switch_case_bool(
    depth: u8,
    op_expr: &SExpr,
    ops: &mut Vec<OpCode>,
    s: &ParserState,
) -> Result<()> {
    if ops.len() > MAX_OPCODE_LEN as usize {
        bail_expr!(
            op_expr,
            "maximum key match size of {MAX_OPCODE_LEN} items is exceeded"
        );
    }
    if usize::from(depth) > MAX_BOOL_EXPR_DEPTH {
        bail_expr!(
            op_expr,
            "maximum key match expression depth {MAX_BOOL_EXPR_DEPTH} is exceeded"
        );
    }
    if let Some(a) = op_expr.atom(s.vars()) {
        let osc = str_to_oscode(a).ok_or_else(|| anyhow_expr!(op_expr, "invalid key name"))?;
        ops.push(OpCode::new_key(osc.into()));
        Ok(())
    } else {
        let l = op_expr
            .list(s.vars())
            .expect("must be a list, checked atom");
        if l.is_empty() {
            bail_expr!(op_expr, "switch logic cannot contain empty lists inside");
        }
        #[derive(PartialEq)]
        enum AllowedListOps {
            Or,
            And,
            Not,
            KeyHistory,
            KeyTiming,
            Input,
            InputHistory,
            Layer,
            BaseLayer,
        }
        #[derive(Copy, Clone)]
        enum InputType {
            Real,
            Virtual,
        }
        impl InputType {
            fn to_row(self) -> u8 {
                match self {
                    InputType::Real => 0,
                    InputType::Virtual => 1,
                }
            }
        }
        let op = l[0]
            .atom(s.vars())
            .and_then(|s| match s {
                "or" => Some(AllowedListOps::Or),
                "and" => Some(AllowedListOps::And),
                "not" => Some(AllowedListOps::Not),
                "key-history" => Some(AllowedListOps::KeyHistory),
                "key-timing" => Some(AllowedListOps::KeyTiming),
                "input" => Some(AllowedListOps::Input),
                "input-history" => Some(AllowedListOps::InputHistory),
                "layer" => Some(AllowedListOps::Layer),
                "base-layer" => Some(AllowedListOps::BaseLayer),
                _ => None,
            })
            .ok_or_else(|| {
                anyhow_expr!(
                    op_expr,
                    "lists inside switch logic must begin with one of:\n\
                    or | and | not | key-history | key-timing\n\
                    | input | input-history | layer | base-layer",
                )
            })?;

        match op {
            AllowedListOps::KeyHistory => {
                if l.len() != 3 {
                    bail_expr!(
                        op_expr,
                        "key-history must have 2 parameters: key, key-recency"
                    );
                }
                let osc = l[1]
                    .atom(s.vars())
                    .and_then(str_to_oscode)
                    .ok_or_else(|| anyhow_expr!(&l[1], "invalid key name"))?;
                let key_recency = parse_u8_with_range(&l[2], s, "key-recency", 1, 8)? - 1;
                ops.push(OpCode::new_key_history(osc.into(), key_recency));
                Ok(())
            }
            AllowedListOps::Input => {
                if l.len() != 3 {
                    bail_expr!(
                        op_expr,
                        "input must have 2 parameters: key-type(virtual|real), key"
                    );
                }

                let input_type = match l[1]
                    .atom(s.vars())
                    .ok_or_else(|| anyhow_expr!(&l[1], "key-type must be virtual|real"))?
                {
                    "real" => InputType::Real,
                    "fake" | "virtual" => InputType::Virtual,
                    _ => bail_expr!(op_expr, "key-type must be virtual|real"),
                };
                let input = match input_type {
                    InputType::Real => {
                        let key = l[2].atom(s.vars()).ok_or_else(|| {
                            anyhow_expr!(&l[2], "input key name must not be a list")
                        })?;
                        u16::from(
                            str_to_oscode(key)
                                .ok_or_else(|| anyhow_expr!(&l[2], "invalid input key name"))?,
                        )
                    }
                    InputType::Virtual => parse_vkey_coord(&l[2], s)?.y,
                };
                let (op1, op2) = OpCode::new_active_input((input_type.to_row(), input));
                ops.extend(&[op1, op2]);
                Ok(())
            }
            AllowedListOps::InputHistory => {
                if l.len() != 4 {
                    bail_expr!(
                        op_expr,
                        "input-history must have 3 parameters: key-type(virtual|real), key, key-recency"
                    );
                }

                let input_type = match l[1]
                    .atom(s.vars())
                    .ok_or_else(|| anyhow_expr!(&l[1], "key-type must be virtual|real"))?
                {
                    "real" => InputType::Real,
                    "fake" | "virtual" => InputType::Virtual,
                    _ => bail_expr!(&l[1], "key-type must be virtual|real"),
                };
                let input = match input_type {
                    InputType::Real => {
                        let key = l[2].atom(s.vars()).ok_or_else(|| {
                            anyhow_expr!(&l[2], "input key name must not be a list")
                        })?;
                        u16::from(
                            str_to_oscode(key)
                                .ok_or_else(|| anyhow_expr!(&l[2], "invalid input key name"))?,
                        )
                    }
                    InputType::Virtual => parse_vkey_coord(&l[2], s)?.y,
                };
                let key_recency = parse_u8_with_range(&l[3], s, "key-recency", 1, 8)? - 1;
                let (op1, op2) =
                    OpCode::new_historical_input((input_type.to_row(), input), key_recency);
                ops.extend(&[op1, op2]);
                Ok(())
            }
            AllowedListOps::KeyTiming => {
                if l.len() != 4 {
                    bail_expr!(
                        op_expr,
                        "key-timing must have 3 parameters: key-recency, lt|gt|less-than|greater-than, milliseconds (0-65535)"
                    );
                }
                let nth_key = parse_u8_with_range(&l[1], s, "key-recency", 1, 8)? - 1;
                let ticks_since = parse_u16(&l[3], s, "milliseconds")?;
                match l[2].atom(s.vars()).ok_or_else(|| {
                    anyhow_expr!(
                        &l[2],
                        "key-timing 2nd parameter must be one of: lt|gt|less-than|greater-than"
                    )
                })? {
                    "less-than" | "lt" => {
                        ops.push(OpCode::new_ticks_since_lt(nth_key, ticks_since));
                    }
                    "greater-than" | "gt" => {
                        ops.push(OpCode::new_ticks_since_gt(nth_key, ticks_since));
                    }
                    _ => {
                        bail_expr!(
                            &l[2],
                            "key-timing 2nd parameter must be one of: lt|gt|less-than|greater-than"
                        );
                    }
                };
                s.switch_max_key_timing
                    .set(std::cmp::max(s.switch_max_key_timing.get(), ticks_since));
                Ok(())
            }
            AllowedListOps::Layer | AllowedListOps::BaseLayer => {
                if l.len() != 2 {
                    bail_expr!(
                        op_expr,
                        "{} must have 1 parameter: layer-name",
                        match op {
                            AllowedListOps::Layer => "layer",
                            AllowedListOps::BaseLayer => "base-layer",
                            _ => unreachable!(),
                        }
                    );
                }
                let layer = l[1]
                    .atom(s.vars())
                    .and_then(|atom| s.layer_idxs.get(atom))
                    .map(|idx| {
                        assert!(*idx < MAX_LAYERS);
                        *idx as u16
                    })
                    .ok_or_else(|| anyhow_expr!(&l[1], "not a known layer name"))?;
                let (op1, op2) = match op {
                    AllowedListOps::Layer => OpCode::new_layer(layer),
                    AllowedListOps::BaseLayer => OpCode::new_base_layer(layer),
                    _ => unreachable!(),
                };
                ops.extend(&[op1, op2]);
                Ok(())
            }
            AllowedListOps::Or | AllowedListOps::And | AllowedListOps::Not => {
                let op = match op {
                    AllowedListOps::Or => BooleanOperator::Or,
                    AllowedListOps::And => BooleanOperator::And,
                    AllowedListOps::Not => BooleanOperator::Not,
                    _ => unreachable!(),
                };
                // insert a placeholder for now, don't know the end index yet.
                let placeholder_index = ops.len() as u16;
                ops.push(OpCode::new_bool(op, placeholder_index));
                for op in l.iter().skip(1) {
                    parse_switch_case_bool(depth + 1, op, ops, s)?;
                }
                if ops.len() > usize::from(MAX_OPCODE_LEN) {
                    bail_expr!(op_expr, "switch logic length has been exceeded");
                }
                ops[placeholder_index as usize] = OpCode::new_bool(op, ops.len() as u16);
                Ok(())
            }
        }
    }
}
