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
(defsrc)
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
