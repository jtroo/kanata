use super::*;

#[test]
fn disallow_same_key_in_defsrc_unmapped_except() {
    let source = "
(defcfg process-unmapped-keys (all-except bspc))
(defsrc bspc)
(deflayermap (name) 0 0)
";
    parse_cfg(source)
        .map(|_| ())
        //.map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect_err("fails");
}

#[test]
fn unmapped_except_keys_cannot_have_dupes() {
    let source = "
(defcfg process-unmapped-keys (all-except bspc bspc))
(defsrc)
(deflayermap (name) 0 0)
";
    parse_cfg(source)
        .map(|_| ())
        //.map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect_err("fails");
}

#[test]
fn unmapped_except_keys_must_be_known() {
    let source = "
(defcfg process-unmapped-keys (all-except notakey))
(defsrc)
(deflayermap (name) 0 0)
";
    parse_cfg(source)
        .map(|_| ())
        //.map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect_err("fails");
}

#[test]
fn unmapped_except_keys_respects_deflocalkeys() {
    let source = "
(deflocalkeys-win         lkey90 555)
(deflocalkeys-winiov2     lkey90 555)
(deflocalkeys-wintercept  lkey90 555)
(deflocalkeys-linux       lkey90 555)
(deflocalkeys-macos       lkey90 555)
(defcfg process-unmapped-keys (all-except lkey90))
(defsrc)
(deflayermap (name) 0 0)
";
    let cfg = parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
    assert!(!cfg.mapped_keys.contains(&OsCode::from(555u16)));
    assert!(cfg.mapped_keys.contains(&OsCode::KEY_ENTER));
    for osc in 0..KEYS_IN_ROW as u16 {
        if let Some(osc) = OsCode::from_u16(osc) {
            match KeyCode::from(osc) {
                KeyCode::No | KeyCode::K555 => {
                    assert!(!cfg.mapped_keys.contains(&osc));
                }
                _ if osc.is_mouse_code() => {
                    assert!(!cfg.mapped_keys.contains(&osc));
                }
                _ => {
                    assert!(cfg.mapped_keys.contains(&osc));
                }
            }
        }
    }
}

#[test]
fn unmapped_except_keys_is_removed_from_mapping() {
    let source = "
(defcfg process-unmapped-keys (all-except 1 2 3))
(defsrc mlft mmid)
(deflayermap (name) 0 0)
";
    let cfg = parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
    assert!(cfg.mapped_keys.contains(&OsCode::KEY_A));
    assert!(cfg.mapped_keys.contains(&OsCode::KEY_0));
    assert!(!cfg.mapped_keys.contains(&OsCode::KEY_1));
    assert!(!cfg.mapped_keys.contains(&OsCode::KEY_2));
    assert!(!cfg.mapped_keys.contains(&OsCode::KEY_3));
    assert!(cfg.mapped_keys.contains(&OsCode::KEY_4));
    for osc in 0..KEYS_IN_ROW as u16 {
        if let Some(osc) = OsCode::from_u16(osc) {
            match KeyCode::from(osc) {
                KeyCode::No | KeyCode::Kb1 | KeyCode::Kb2 | KeyCode::Kb3 => {
                    assert!(!cfg.mapped_keys.contains(&osc));
                }
                // mlft, mmid
                KeyCode::K272 | KeyCode::K274 => {
                    assert!(cfg.mapped_keys.contains(&osc));
                }
                _ if osc.is_mouse_code() => {
                    assert!(!cfg.mapped_keys.contains(&osc));
                }
                _ => {
                    assert!(cfg.mapped_keys.contains(&osc));
                }
            }
        }
    }
}

#[test]
fn non_applicable_os_deflocalkeys_always_succeeds_parsing() {
    let source = "
(deflocalkeys-linux å 26 ' 43)
(defsrc)
(deflayer base)
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
}

#[test]
fn tap_hold_require_prior_idle_parses_valid_value() {
    let source = "
(defcfg tap-hold-require-prior-idle 150)
(defsrc a)
(deflayer base a)
";
    let cfg = parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
    assert_eq!(cfg.options.tap_hold_require_prior_idle, 150);
}

#[test]
fn tap_hold_require_prior_idle_allows_zero() {
    let source = "
(defcfg tap-hold-require-prior-idle 0)
(defsrc a)
(deflayer base a)
";
    let cfg = parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
    assert_eq!(cfg.options.tap_hold_require_prior_idle, 0);
}

#[test]
fn tap_hold_require_prior_idle_rejects_non_numeric() {
    let source = "
(defcfg tap-hold-require-prior-idle nope)
(defsrc a)
(deflayer base a)
";
    parse_cfg(source)
        .map(|_| ())
        //.map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect_err("fails");
}

#[test]
fn per_action_require_prior_idle_parses() {
    let source = "
(defsrc a)
(deflayer base @a)
(defalias a (tap-hold 200 200 a lctl (require-prior-idle 100)))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
}

#[test]
fn per_action_require_prior_idle_zero_parses() {
    let source = "
(defsrc a)
(deflayer base @a)
(defalias a (tap-hold 200 200 a lctl (require-prior-idle 0)))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
}

#[test]
fn per_action_require_prior_idle_rejects_non_numeric() {
    let source = "
(defsrc a)
(deflayer base @a)
(defalias a (tap-hold 200 200 a lctl (require-prior-idle nope)))
";
    parse_cfg(source).map(|_| ()).expect_err("fails");
}

#[test]
fn per_action_require_prior_idle_rejects_unknown_option() {
    let source = "
(defsrc a)
(deflayer base @a)
(defalias a (tap-hold 200 200 a lctl (unknown-option 100)))
";
    parse_cfg(source).map(|_| ()).expect_err("fails");
}

#[test]
fn per_action_require_prior_idle_rejects_duplicate() {
    let source = "
(defsrc a)
(deflayer base @a)
(defalias a (tap-hold 200 200 a lctl (require-prior-idle 100) (require-prior-idle 50)))
";
    parse_cfg(source).map(|_| ()).expect_err("fails");
}

#[test]
fn per_action_require_prior_idle_on_tap_hold_press() {
    let source = "
(defsrc a)
(deflayer base @a)
(defalias a (tap-hold-press 200 200 a lctl (require-prior-idle 100)))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
}

#[test]
fn per_action_require_prior_idle_on_tap_hold_release() {
    let source = "
(defsrc a)
(deflayer base @a)
(defalias a (tap-hold-release 200 200 a lctl (require-prior-idle 100)))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
}

#[test]
fn per_action_require_prior_idle_on_tap_hold_release_timeout() {
    let source = "
(defsrc a)
(deflayer base @a)
(defalias a (tap-hold-release-timeout 200 200 a lctl lalt (require-prior-idle 100)))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
}

#[test]
fn per_action_require_prior_idle_on_tap_hold_press_timeout() {
    let source = "
(defsrc a)
(deflayer base @a)
(defalias a (tap-hold-press-timeout 200 200 a lctl lalt (require-prior-idle 100)))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
}

#[test]
fn per_action_require_prior_idle_on_tap_hold_release_keys() {
    let source = "
(defsrc a b)
(deflayer base @a b)
(defalias a (tap-hold-release-keys 200 200 a lctl (b) (require-prior-idle 100)))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
}

#[test]
fn per_action_require_prior_idle_on_tap_hold_except_keys() {
    let source = "
(defsrc a b)
(deflayer base @a b)
(defalias a (tap-hold-except-keys 200 200 a lctl (b) (require-prior-idle 100)))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
}

#[test]
fn per_action_require_prior_idle_on_tap_hold_opposite_hand() {
    let source = "
(defhands (left a s d f g) (right h j k l ;))
(defsrc a)
(deflayer base @a)
(defalias a (tap-hold-opposite-hand 200 a lctl (require-prior-idle 100)))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
}

#[test]
fn per_action_require_prior_idle_on_tap_hold_release_tap_keys_release() {
    let source = "
(defsrc a b c)
(deflayer base @a b c)
(defalias a (tap-hold-release-tap-keys-release 200 200 a lctl (b) (c) (require-prior-idle 100)))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("passes");
}
