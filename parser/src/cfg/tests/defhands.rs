use super::*;

#[test]
fn opposite_hand_no_args() {
    let source = "
(defhands (left a s d f) (right j k l ;))
(defsrc a)
(deflayer base (tap-hold-opposite-hand))
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("tap-hold-opposite-hand with zero args should fail");
}

#[test]
fn opposite_hand_one_arg() {
    let source = "
(defhands (left a s d f) (right j k l ;))
(defsrc a)
(deflayer base (tap-hold-opposite-hand 180))
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("tap-hold-opposite-hand with one arg should fail");
}

#[test]
fn opposite_hand_two_args() {
    let source = "
(defhands (left a s d f) (right j k l ;))
(defsrc a)
(deflayer base (tap-hold-opposite-hand 180 a))
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("tap-hold-opposite-hand with two args should fail");
}

#[test]
fn defhands_missing_for_opposite_hand() {
    let source = "
(defsrc a)
(deflayer base (tap-hold-opposite-hand 180 a lctl))
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("tap-hold-opposite-hand without defhands should fail");
}

#[test]
fn defhands_duplicate_blocks() {
    let source = "
(defhands (left a s d f) (right j k l ;))
(defhands (left q w e r))
(defsrc a)
(deflayer base a)
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("duplicate defhands blocks should fail");
}

#[test]
fn defhands_key_in_both_groups() {
    let source = "
(defhands (left a s d f) (right a j k l))
(defsrc a)
(deflayer base a)
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("same key in both left and right should fail");
}

#[test]
fn defhands_duplicate_group_name() {
    let source = "
(defhands (left a s d f) (left q w e r))
(defsrc a)
(deflayer base a)
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("duplicate left group in defhands should fail");
}

#[test]
fn defhands_invalid_group_name() {
    let source = "
(defhands (center a s d f))
(defsrc a)
(deflayer base a)
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("invalid group name 'center' should fail");
}

#[test]
fn opposite_hand_unknown_option() {
    let source = "
(defhands (left a s d f) (right j k l ;))
(defsrc a)
(deflayer base (tap-hold-opposite-hand 180 a lctl (foo bar)))
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("unknown option foo should fail");
}

#[test]
fn opposite_hand_trailing_keyword() {
    let source = "
(defhands (left a s d f) (right j k l ;))
(defsrc a)
(deflayer base (tap-hold-opposite-hand 180 a lctl (timeout)))
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("option list without value should fail");
}

#[test]
fn opposite_hand_invalid_behavior() {
    let source = "
(defhands (left a s d f) (right j k l ;))
(defsrc a)
(deflayer base (tap-hold-opposite-hand 180 a lctl (same-hand maybe)))
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("invalid behavior 'maybe' should fail");
}

#[test]
fn opposite_hand_duplicate_option() {
    let source = "
(defhands (left a s d f) (right j k l ;))
(defsrc a)
(deflayer base (tap-hold-opposite-hand 180 a lctl (timeout tap) (timeout hold)))
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("duplicate option timeout should fail");
}

#[test]
fn opposite_hand_colon_syntax_rejected() {
    let source = "
(defhands (left a s d f) (right j k l ;))
(defsrc a)
(deflayer base (tap-hold-opposite-hand 180 a lctl :timeout hold))
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("colon-style option syntax should fail");
}

#[test]
fn defhands_valid_partial() {
    let source = "
(defhands (left a s d f))
(defsrc a)
(deflayer base (tap-hold-opposite-hand 180 a lctl))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("partial defhands with only left should succeed");
}

#[test]
fn defhands_bare_atom_syntax_rejected() {
    let source = "
(defhands left (a s d f) right (j k l ;))
(defsrc a)
(deflayer base a)
";
    parse_cfg(source)
        .map(|_| ())
        .expect_err("bare atom syntax (left (...)) should fail; use (left ...) instead");
}
