use super::*;
use crate::{anyhow_expr, bail, bail_expr};

#[allow(unused)]
/// The function stored inside `Switch.init_callback`.
pub(crate) type InitFn = dyn Fn() + Send + Sync;
#[allow(unused)]
/// The function type stored inside `Switch.callbacks`.
pub(crate) type CallbackFn = dyn Fn() -> bool + Send + Sync;

#[derive(Default)]
struct OptionalSwitchArgs {
    #[cfg(feature = "cmd")]
    init_cmd: Option<&'static InitFn>,
}

#[cfg(feature = "cmd")]
pub(crate) mod cmd {
    //! Global cmd results for switch processing.
    //! This should not impact runtime which is single threaded in the kanata/keyberon execution
    //! but this impacts possible future testability for parallel unit tests.
    //! In practice at the time of writing the cmd actions are not simulation-tested today.

    use super::*;
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicI32;

    static LATEST_EXIT_CODE: AtomicI32 = AtomicI32::new(0);

    static LATEST_STDOUT: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

    static LATEST_STDERR: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

    struct CmdState {
        // First item is the binary; the remaining (i.e. `&[1..]`) are arguments.
        binary_then_args: &'static [&'static str],
    }

    impl CmdState {
        fn binary(&self) -> &str {
            self.binary_then_args[0]
        }
        fn args(&self) -> &[&str] {
            &self.binary_then_args[1..]
        }
    }

    /// Invariant: binary_then_args must not be empty.
    /// Turn the cmd configuration into a callback used by keyberon.
    pub(crate) fn create_init_fn(
        binary_then_args: Vec<String>,
        s: &ParserState,
    ) -> &'static InitFn {
        assert!(!binary_then_args.is_empty());
        let binary_then_args = binary_then_args
            .into_iter()
            .map(|v| s.a.sref_str(v))
            .collect();
        let binary_then_args = s.a.sref_vec(binary_then_args);

        s.a.sref(|| {
            let cmd_cfg = cmd::CmdState { binary_then_args };
            let mut cmd = std::process::Command::new(cmd_cfg.binary());
            for arg in cmd_cfg.args() {
                cmd.arg(arg);
            }
            match cmd.output() {
                Ok(output) => {
                    let exit_code = output.status.code().unwrap_or(1);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    cmd::LATEST_EXIT_CODE.store(exit_code, std::sync::atomic::Ordering::Release);
                    *cmd::LATEST_STDOUT.lock().expect("unpoisoned") = stdout.into();
                    *cmd::LATEST_STDERR.lock().expect("unpoisoned") = stderr.into();
                }
                Err(e) => {
                    log::error!("Failed to execute program {:?}: {}", cmd.get_program(), e);
                    cmd::LATEST_EXIT_CODE.store(1, std::sync::atomic::Ordering::Release);
                    cmd::LATEST_STDOUT.lock().expect("unpoisoned").clear();
                    cmd::LATEST_STDERR.lock().expect("unpoisoned").clear();
                }
            };
        })
    }

    pub(crate) fn create_exitcode_callback(exitcode: i32, s: &ParserState) -> &'static CallbackFn {
        s.a.sref(move || {
            cmd::LATEST_EXIT_CODE.load(std::sync::atomic::Ordering::Acquire) == exitcode
        })
    }
}

fn parse_optional_arguments<'a>(
    params: &'a [SExpr],
    s: &ParserState,
) -> Result<(OptionalSwitchArgs, &'a [SExpr])> {
    if params.is_empty() {
        return Ok((OptionalSwitchArgs::default(), params));
    }
    let Some((first_atom, params_after_first)) = parse_list_with_first_atom(&params[0], s) else {
        return Ok((OptionalSwitchArgs::default(), params));
    };
    match first_atom {
        "init-cmd" => {
            #[cfg(not(feature = "cmd"))]
            {
                let _ = params_after_first; // suppress unused
                bail_expr!(
                    &params[0],
                    "cmd is not enabled for this kanata executable. \
                     Use a cmd_allowed prebuilt executable or compile with the feature: cmd."
                );
            }
            #[cfg(feature = "cmd")]
            {
                if !s.is_cmd_enabled {
                    bail_expr!(
                        &params[0],
                        "To use cmd you must put in defcfg: danger-enable-cmd yes."
                    );
                }
                let mut binary_then_args = vec![];
                collect_strings(params_after_first, &mut binary_then_args, s);
                if binary_then_args.is_empty() {
                    bail_expr!(
                        &params[0],
                        "cmd must have at least one atom for the binary to execute: <binary> [...args]"
                    );
                }
                Ok((
                    OptionalSwitchArgs {
                        init_cmd: Some(cmd::create_init_fn(binary_then_args, s)),
                    },
                    &params[1..],
                ))
            }
        }
        _ => Ok((OptionalSwitchArgs::default(), params)),
    }
}

pub fn parse_switch(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_STR: &str =
        "switch expects triples of params: <key match> <action> <break|fallthrough>";

    let (opts, remaining_params) = parse_optional_arguments(ac_params, s)?;

    let mut cases = vec![];
    let mut callbacks = vec![];
    let mut params = remaining_params.iter();

    while let Some(key_match) = params.next() {
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
            parse_switch_case_bool(1, op, &mut ops, &mut callbacks, s)?;
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

    let init_fn = {
        #[cfg(not(feature = "cmd"))]
        {
            let _ = opts; // suppress unused
            None
        }
        #[cfg(feature = "cmd")]
        {
            opts.init_cmd
        }
    };

    Ok(s.a.sref(Action::Switch(s.a.sref(Switch {
        cases: s.a.sref_vec(cases),
        init_fn,
        callbacks: s.a.sref_vec(callbacks),
    }))))
}

pub fn parse_switch_case_bool(
    depth: u8,
    op_expr: &SExpr,
    ops: &mut Vec<OpCode>,
    callbacks: &mut Vec<&'static CallbackFn>,
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
            DeviceHistory,
            CmdExit,
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
                "device-history" => Some(AllowedListOps::DeviceHistory),
                "cmd-exit" => Some(AllowedListOps::CmdExit),
                _ => None,
            })
            .ok_or_else(|| {
                anyhow_expr!(
                    op_expr,
                    "lists inside switch logic must begin with one of:\n\
                    or | and | not | key-history | key-timing\n\
                    | input | input-history | layer | base-layer | device-history",
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
                s.max_key_timing_check
                    .set(std::cmp::max(s.max_key_timing_check.get(), ticks_since));
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
            AllowedListOps::DeviceHistory => {
                if l.len() != 3 {
                    bail_expr!(
                        op_expr,
                        "device-history must have 2 parameters: device-id, device-recency"
                    );
                }
                let id_str = l[1]
                    .atom(s.vars())
                    .ok_or_else(|| anyhow_expr!(&l[1], "device ID must be a number (1-255)"))?;
                let id_num: u8 = id_str
                    .parse()
                    .map_err(|_| anyhow_expr!(&l[1], "device ID must be a number (1-255)"))?;
                let id = std::num::NonZeroU8::new(id_num)
                    .ok_or_else(|| anyhow_expr!(&l[1], "device ID must be nonzero (1-255)"))?;
                if let Some(ref devs) = s.input_devices {
                    if !devs.iter().any(|(did, _)| *did == id) {
                        bail_expr!(
                            &l[1],
                            "device ID {id_num} is not defined in definputdevices"
                        );
                    }
                } else {
                    bail_expr!(
                        &l[1],
                        "cannot use (device-history {id_num} ...) without a definputdevices block"
                    );
                }
                let device_recency = parse_u8_with_range(&l[2], s, "device-recency", 1, 8)? - 1;
                let (op1, op2) = OpCode::new_device_history(id, device_recency);
                ops.extend(&[op1, op2]);
                Ok(())
            }
            AllowedListOps::CmdExit => {
                #[cfg(not(feature = "cmd"))]
                {
                    let _ = callbacks; // suppress unused
                    bail_expr!(
                        op_expr,
                        "cmd is not enabled for this kanata executable. \
                     Use a cmd_allowed prebuilt executable or compile with the feature: cmd."
                    );
                }
                #[cfg(feature = "cmd")]
                {
                    if !s.is_cmd_enabled {
                        bail_expr!(
                            &op_expr,
                            "To use cmd you must put in defcfg: danger-enable-cmd yes."
                        );
                    }
                    if l.len() != 2 {
                        bail_expr!(op_expr, "cmd-exit must have 1 parameter: exit-code");
                    }
                    let exit_code = l[1]
                        .atom(s.vars())
                        .ok_or_else(|| anyhow_expr!(&l[1], "exit code must be a 32 bit integer"))?;
                    let exit_code: i32 = exit_code
                        .parse()
                        .map_err(|_| anyhow_expr!(&l[1], "exit code must be a 32 bit integer"))?;

                    let index = callbacks.len();
                    if index > 60000 {
                        bail_expr!(
                            op_expr,
                            "Exceeded limit of 60000 callback-based switch items"
                        );
                    }
                    let index = u16::try_from(index).expect("checked <=60000 earlier");
                    let callback = cmd::create_exitcode_callback(exit_code, s);
                    callbacks.push(callback);

                    let (op1, op2) = OpCode::new_callback_index(index);
                    ops.extend(&[op1, op2]);
                    Ok(())
                }
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
                    parse_switch_case_bool(depth + 1, op, ops, callbacks, s)?;
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
