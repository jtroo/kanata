use kanata_parser::cfg::parse_mod_prefix;
use kanata_parser::cfg::sexpr::*;
use kanata_parser::keys::*;

// local log prefix
const LP: &str = "cmd-out:";

pub(super) fn run_cmd_in_thread(cmd_and_args: Vec<String>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut args = cmd_and_args.iter();
        let mut cmd = std::process::Command::new(
            args.next()
                .expect("parsing should have forbidden empty cmd"),
        );
        for arg in args {
            cmd.arg(arg);
        }
        match cmd.output() {
            Ok(output) => {
                log::info!(
                    "Successfully ran cmd {}\nstdout:\n{}\nstderr:\n{}",
                    {
                        let mut printable_cmd = Vec::new();
                        printable_cmd.push(format!("{:?}", cmd.get_program()));

                        let printable_cmd = cmd.get_args().fold(printable_cmd, |mut cmd, arg| {
                            cmd.push(format!("{arg:?}"));
                            cmd
                        });
                        printable_cmd.join(" ")
                    },
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(e) => log::error!("Failed to execute cmd: {}", e),
        };
    })
}

pub(super) type Item = (KeyAction, OsCode);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum KeyAction {
    Press,
    Release,
}
use kanata_keyberon::key_code::KeyCode;
use KeyAction::*;

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
                items.push((Press, osc));
                items.push((Release, osc));
                &exprs[1..]
            }
            None => try_parse_chord(&osc.t, exprs, items),
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
                items.push((Press, mod_kc.into()));
            }
            items.push((Press, osc));
            for mod_kc in mods.iter().copied() {
                items.push((Release, mod_kc.into()));
            }
            items.push((Release, osc));
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
                items.push((Press, mod_kc.into()));
            }
            let mut remainder = subexprs.t.as_slice();
            while !remainder.is_empty() {
                remainder = parse_items(remainder, items);
            }
            for mod_kc in mods.iter().copied() {
                items.push((Release, mod_kc.into()));
            }
            &exprs[1..]
        }
    }
}

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
