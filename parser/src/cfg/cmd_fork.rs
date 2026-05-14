use super::*;

use crate::bail;

#[allow(unused_variables)]
pub(crate) fn parse_cmd_fork(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    #[cfg(not(feature = "cmd"))]
    {
        bail!(
            "cmd is not enabled for this kanata executable. \
             Use a cmd_allowed prebuilt executable or compile with the feature: cmd."
        );
    }
    #[cfg(feature = "cmd")]
    {
        const ERR_STR: &str =
            "cmd-fork expects at least 3 params: \
             (cmd-fork <action-if-0> <action-if-nonzero> <cmd> [args...])";
        if !s.is_cmd_enabled {
            bail!("To use cmd-fork you must put in defcfg: danger-enable-cmd yes.");
        }
        if ac_params.len() < 3 {
            bail!(
                "{ERR_STR}\nFound {} params instead of at least 3",
                ac_params.len()
            );
        }

        let action_true = parse_action(&ac_params[0], s)?;
        let action_false = parse_action(&ac_params[1], s)?;

        let mut cmd = vec![];
        collect_strings(&ac_params[2..], &mut cmd, s);
        if cmd.is_empty() {
            bail!("{ERR_STR}\nCommand must not be empty");
        }

        let coord_true = allocate_cmd_fork_vk(s, action_true)?;
        let coord_false = allocate_cmd_fork_vk(s, action_false)?;

        let cmds = cmd.into_iter().map(|v| s.a.sref_str(v)).collect();
        let cmd_slice = s.a.sref_vec(cmds);

        custom(
            CustomAction::CmdFork {
                cmd: cmd_slice,
                coord_true,
                coord_false,
            },
            &s.a,
        )
    }
}

#[cfg(feature = "cmd")]
fn allocate_cmd_fork_vk(
    s: &ParserState,
    action: &'static KanataAction,
) -> Result<crate::custom_action::Coord> {
    use crate::layers::KEYS_IN_ROW;
    let base = s.virtual_keys.len();
    let mut vkeys = s.cmd_fork_vkeys.borrow_mut();
    let idx = base + vkeys.len();
    if idx >= KEYS_IN_ROW {
        bail!(
            "Maximum number of virtual keys ({KEYS_IN_ROW}) exceeded by cmd-fork"
        );
    }
    vkeys.push((idx, action));
    let (x, y) = get_fake_key_coords(idx);
    Ok(Coord { x, y })
}
