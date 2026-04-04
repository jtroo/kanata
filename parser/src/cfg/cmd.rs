use super::*;

use crate::bail;
use crate::bail_expr;

pub(crate) enum CmdType {
    /// Execute command in own thread.
    Standard,
    /// Execute command synchronously and output stdout as macro-like SExpr.
    OutputKeys,
    /// Execute command and set clipboard to output. Clipboard content is passed as stdin to the
    /// command.
    ClipboardSet,
    /// Execute command and set clipboard save id to output.
    /// Clipboard save id content is passed as stdin to the command.
    ClipboardSaveSet,
}

// Parse cmd, but there are 2 arguments before specifying normal log and error log
pub(crate) fn parse_cmd_log(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_STR: &str =
        "cmd-log expects at least 3 strings, <log-level> <error-log-level> <cmd...>";
    if !s.is_cmd_enabled {
        bail!(
            "cmd is not enabled for this kanata executable (did you use 'cmd_allowed' variants?), but is set in the configuration"
        );
    }
    if ac_params.len() < 3 {
        bail!(ERR_STR);
    }
    let mut cmd = vec![];
    let log_level =
        if let Some(Ok(input_mode)) = ac_params[0].atom(s.vars()).map(LogLevel::try_from_str) {
            input_mode
        } else {
            bail_expr!(&ac_params[0], "{ERR_STR}\n{}", LogLevel::err_msg());
        };
    let error_log_level =
        if let Some(Ok(input_mode)) = ac_params[1].atom(s.vars()).map(LogLevel::try_from_str) {
            input_mode
        } else {
            bail_expr!(&ac_params[1], "{ERR_STR}\n{}", LogLevel::err_msg());
        };
    collect_strings(&ac_params[2..], &mut cmd, s);
    if cmd.is_empty() {
        bail!(ERR_STR);
    }
    let cmds = cmd.into_iter().map(|v| s.a.sref_str(v)).collect();
    custom(
        CustomAction::CmdLog(log_level, error_log_level, s.a.sref_vec(cmds)),
        &s.a,
    )
}

#[allow(unused_variables)]
pub(crate) fn parse_cmd(
    ac_params: &[SExpr],
    s: &ParserState,
    cmd_type: CmdType,
) -> Result<&'static KanataAction> {
    #[cfg(not(feature = "cmd"))]
    {
        bail!(
            "cmd is not enabled for this kanata executable. Use a cmd_allowed prebuilt executable or compile with the feature: cmd."
        );
    }
    #[cfg(feature = "cmd")]
    {
        if matches!(cmd_type, CmdType::ClipboardSaveSet) {
            const ERR_STR: &str = "expects a save ID and at least one string";
            if !s.is_cmd_enabled {
                bail!("To use cmd you must put in defcfg: danger-enable-cmd yes.");
            }
            if ac_params.len() < 2 {
                bail!("{CLIPBOARD_SAVE_CMD_SET} {ERR_STR}");
            }
            let mut cmd = vec![];
            let save_id = parse_u16(&ac_params[0], s, "clipboard save ID")?;
            collect_strings(&ac_params[1..], &mut cmd, s);
            if cmd.is_empty() {
                bail_expr!(&ac_params[1], "{CLIPBOARD_SAVE_CMD_SET} {ERR_STR}");
            }
            let cmds = cmd.into_iter().map(|v| s.a.sref_str(v)).collect();
            return custom(
                CustomAction::ClipboardSaveCmdSet(save_id, s.a.sref_vec(cmds)),
                &s.a,
            );
        }

        const ERR_STR: &str = "cmd expects at least one string";
        if !s.is_cmd_enabled {
            bail!("To use cmd you must put in defcfg: danger-enable-cmd yes.");
        }
        let mut cmd = vec![];
        collect_strings(ac_params, &mut cmd, s);
        if cmd.is_empty() {
            bail!(ERR_STR);
        }
        let cmds = cmd.into_iter().map(|v| s.a.sref_str(v)).collect();
        let cmds = s.a.sref_vec(cmds);
        custom(
            match cmd_type {
                CmdType::Standard => CustomAction::Cmd(cmds),
                CmdType::OutputKeys => CustomAction::CmdOutputKeys(cmds),
                CmdType::ClipboardSet => CustomAction::ClipboardCmdSet(cmds),
                CmdType::ClipboardSaveSet => unreachable!(),
            },
            &s.a,
        )
    }
}

/// Recurse through all levels of list nesting and collect into a flat list of strings.
/// Recursion is DFS, which matches left-to-right reading of the strings as they appear,
/// if everything was on a single line.
pub(crate) fn collect_strings(params: &[SExpr], strings: &mut Vec<String>, s: &ParserState) {
    for param in params {
        if let Some(a) = param.atom(s.vars()) {
            strings.push(a.trim_atom_quotes().to_owned());
        } else {
            // unwrap: this must be a list, since it's not an atom.
            let l = param.list(s.vars()).unwrap();
            collect_strings(l, strings, s);
        }
    }
}

#[test]
pub(crate) fn test_collect_strings() {
    let params = r#"(gah (squish "squash" (splish splosh) "bah mah") dah)"#;
    let params = sexpr::parse(params, "noexist").unwrap();
    let mut strings = vec![];
    collect_strings(&params[0].t, &mut strings, &ParserState::default());
    assert_eq!(
        &strings,
        &[
            "gah", "squish", "squash", "splish", "splosh", "bah mah", "dah"
        ]
    );
}
