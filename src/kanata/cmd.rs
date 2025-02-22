#![cfg_attr(feature = "simulated_output", allow(dead_code, unused_imports))]

use std::fmt::Write;

use kanata_parser::cfg::parse_mod_prefix;
use kanata_parser::cfg::sexpr::*;
use kanata_parser::keys::*;

// local log prefix
const LP: &str = "cmd-out:";

#[cfg(not(feature = "simulated_output"))]
pub(super) fn run_cmd_in_thread(
    cmd_and_args: Vec<String>,
    log_level: Option<log::Level>,
    error_log_level: Option<log::Level>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut args = cmd_and_args.iter();
        let mut printable_cmd = String::new();
        let executable = args
            .next()
            .expect("parsing should have forbidden empty cmd");
        write!(
            printable_cmd,
            "Program: {}, Arguments:",
            executable.as_str()
        )
        .expect("write to string should succeed");
        let mut cmd = std::process::Command::new(executable);
        for arg in args {
            cmd.arg(arg);
            printable_cmd.push(' ');
            printable_cmd.push_str(arg.as_str());
        }
        if let Some(level) = log_level {
            log::log!(level, "Running cmd: {}", printable_cmd);
        }
        match cmd.output() {
            Ok(output) => {
                if let Some(level) = log_level {
                    log::log!(
                        level,
                        "Successfully ran cmd: {}\nstdout:\n{}\nstderr:\n{}",
                        printable_cmd,
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                };
            }
            Err(e) => {
                if let Some(level) = error_log_level {
                    log::log!(
                        level,
                        "Failed to execute program {:?}: {}",
                        cmd.get_program(),
                        e
                    )
                }
            }
        };
    })
}

pub(super) type Item = KeyAction;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(super) enum KeyAction {
    Press(OsCode),
    Release(OsCode),
    Delay(u16),
}
use KeyAction::*;
use kanata_keyberon::key_code::KeyCode;

fn empty() -> std::vec::IntoIter<Item> {
    vec![].into_iter()
}

fn from_sexpr(sexpr: Vec<SExpr>) -> std::vec::IntoIter<Item> {
    let mut items = vec![];
    let mut remainder = sexpr.as_slice();
    while !remainder.is_empty() {
        remainder = parse_items(remainder, &mut items);
    }
    items.into_iter()
}

fn parse_items<'a>(exprs: &'a [SExpr], items: &mut Vec<Item>) -> &'a [SExpr] {
    match &exprs[0] {
        SExpr::Atom(osc) => match str_to_oscode(&osc.t) {
            Some(osc) => {
                items.push(Press(osc));
                items.push(Release(osc));
                &exprs[1..]
            }
            None => {
                use std::str::FromStr;
                match u16::from_str(&osc.t) {
                    Ok(delay) => {
                        items.push(Delay(delay));
                        &exprs[1..]
                    }
                    Err(_) => try_parse_chord(&osc.t, exprs, items),
                }
            }
        },
        SExpr::List(sexprs) => {
            let mut remainder = sexprs.t.as_slice();
            while !remainder.is_empty() {
                remainder = parse_items(remainder, items);
            }
            &exprs[1..]
        }
    }
}

fn try_parse_chord<'a>(chord: &str, exprs: &'a [SExpr], items: &mut Vec<Item>) -> &'a [SExpr] {
    match parse_mod_prefix(chord) {
        Ok((mods, osc)) => match osc.is_empty() {
            true => try_parse_chorded_list(&mods, chord, &exprs[1..], items),
            false => {
                try_parse_chorded_key(&mods, osc, chord, items);
                &exprs[1..]
            }
        },
        Err(e) => {
            log::warn!("{LP} found invalid chord {chord}: {}", e.msg);
            &exprs[1..]
        }
    }
}

fn try_parse_chorded_key(mods: &[KeyCode], osc: &str, chord: &str, items: &mut Vec<Item>) {
    if mods.is_empty() {
        log::warn!("{LP} found invalid key: {osc}");
        return;
    }
    match str_to_oscode(osc) {
        Some(osc) => {
            for mod_kc in mods.iter().copied() {
                items.push(Press(mod_kc.into()));
            }
            items.push(Press(osc));
            for mod_kc in mods.iter().copied() {
                items.push(Release(mod_kc.into()));
            }
            items.push(Release(osc));
        }
        None => {
            log::warn!("{LP} found chord {chord} with invalid key: {osc}");
        }
    };
}

fn try_parse_chorded_list<'a>(
    mods: &[KeyCode],
    chord: &str,
    exprs: &'a [SExpr],
    items: &mut Vec<Item>,
) -> &'a [SExpr] {
    if exprs.is_empty() {
        log::warn!(
            "{LP} found chord modifiers with no attached key or list - ignoring it: {chord}"
        );
        return exprs;
    }
    match &exprs[0] {
        SExpr::Atom(osc) => {
            log::warn!("{LP} expected list after {chord}, got string {}", &osc.t);
            exprs
        }
        SExpr::List(subexprs) => {
            for mod_kc in mods.iter().copied() {
                items.push(Press(mod_kc.into()));
            }
            let mut remainder = subexprs.t.as_slice();
            while !remainder.is_empty() {
                remainder = parse_items(remainder, items);
            }
            for mod_kc in mods.iter().copied() {
                items.push(Release(mod_kc.into()));
            }
            &exprs[1..]
        }
    }
}

#[cfg(not(feature = "simulated_output"))]
pub(super) fn keys_for_cmd_output(cmd_and_args: &[String]) -> impl Iterator<Item = Item> {
    let mut args = cmd_and_args.iter();
    let mut cmd = std::process::Command::new(
        args.next()
            .expect("parsing should have forbidden empty cmd"),
    );
    for arg in args {
        cmd.arg(arg);
    }
    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            log::error!("Failed to execute cmd: {e}");
            return empty();
        }
    };
    log::debug!("{LP} stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    match parse(&stdout, "cmd") {
        Ok(lists) => match lists.len() {
            0 => {
                log::warn!("{LP} got zero top-level S-expression from cmd, expected 1:\n{stdout}");
                empty()
            }
            1 => from_sexpr(lists.into_iter().next().expect("len 1").t),
            _ => {
                log::warn!(
                    "{LP} got multiple top-level S-expression from cmd, expected 1:\n{stdout}"
                );
                empty()
            }
        },
        Err(e) => {
            log::warn!(
                "{LP} could not parse an S-expression from cmd:\n{stdout}\n{}",
                e.msg
            );
            empty()
        }
    }
}

#[cfg(feature = "simulated_output")]
pub(super) fn keys_for_cmd_output(cmd_and_args: &[String]) -> impl Iterator<Item = Item> {
    println!("cmd-keys:{cmd_and_args:?}");
    [].iter().copied()
}

#[cfg(feature = "simulated_output")]
pub(super) fn run_cmd_in_thread(
    cmd_and_args: Vec<String>,
    _log_level: Option<log::Level>,
    _error_log_level: Option<log::Level>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        println!("cmd:{cmd_and_args:?}");
    })
}
