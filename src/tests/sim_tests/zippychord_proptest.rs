//! Deterministic regression shrunk from the zippychord property tests.
//! (The property tests themselves now live in `zippychord_state_machine.rs`.)

use super::*;
use rustc_hash::FxHashMap;

static PROPTEST_CFG: &str =
    "(defsrc lalt)(deflayer base (caps-word 2000))(defzippy file on-first-press-chord-deadline 50)";

/// Reconstruct net visible text from a `to_ascii()` stream: replay key-downs,
/// apply backspaces, honor held shift.
fn net_text(ascii: &str) -> String {
    let mut out: Vec<char> = Vec::new();
    let mut shift = false;
    for tok in ascii.split_ascii_whitespace() {
        if let Some(key) = tok.strip_prefix("dn:") {
            match key {
                "LShift" | "RShift" => shift = true,
                "BSpace" => {
                    out.pop();
                }
                "Space" => out.push(' '),
                k => {
                    if let Some(c) = key_to_char(k) {
                        out.push(if shift { c.to_ascii_uppercase() } else { c });
                    }
                }
            }
        } else if let Some(key) = tok.strip_prefix("up:") {
            if matches!(key, "LShift" | "RShift") {
                shift = false;
            }
        }
    }
    out.into_iter().collect()
}

fn key_to_char(key: &str) -> Option<char> {
    if let Some(d) = key.strip_prefix("Kb") {
        return d.chars().next();
    }
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

fn simulate_zippy(cfg: &str, input: &str, content: &str) -> String {
    let mut fcontent = FxHashMap::default();
    fcontent.insert("file".into(), content.into());
    simulate_with_file_content(cfg, input, fcontent)
}

/// Minimal deterministic reproduction shrunk from the property tests.
///
/// Dictionary: `b`->"c" (exact), ` b`->"cfbcc", ` bd`->"fee ". Pressing b, SPACE,
/// d activates the full chord ` bd` whose output is "fee ". Because `b`->"c"
/// eagerly activates first and "c" is a common prefix of "cfbcc", the backspace
/// accounting for the final ` bd` activation under-counts by one and fails to
/// erase the leading "c", so the result is "cfee " instead of "fee ".
///
/// This is a regression test for the enabled-path backspace under-count: the
/// common-prefix optimization left the kept prefix characters out of the
/// next-activation delete count. Fixed in `zippychord.rs` by seeding that count
/// with the kept-prefix length.
#[test]
fn repro_overlap_underdelete() {
    let content = "\nb\tc\n b\tcfbcc\n bd\tfee \n";
    let ascii = simulate_zippy(
        PROPTEST_CFG,
        "t:600 d:b d:spc d:d t:20 u:spc u:b u:d t:300",
        content,
    )
    .to_ascii();
    assert_eq!("fee ", net_text(&ascii), "stray leading char not erased");
}
