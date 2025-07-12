use super::*;

#[test]
fn unsupported_action_in_macro_triggers_error() {
    let source = r#"
(defsrc)
(deflayer base)
(defalias a (macro (multi a b c))) "#;
    parse_cfg(source)
        .map(|_| ())
        .map_err(|e| log::info!("{:?}", miette::Error::from(e)))
        .expect_err("errors");
}

#[test]
fn incorrectly_configured_supported_action_in_macro_triggers_useful_error() {
    let source = r#"
(defsrc)
(deflayer base)
(defalias a (macro (on-press press-vkey does-not-exist))) "#;
    parse_cfg(source)
        .map(|_| ())
        .map_err(|e| {
            let e = miette::Error::from(e);
            let msg = format!("{e:?}");
            log::info!("{msg}");
            assert!(msg.contains("unknown virtual key name: does-not-exist"));
        })
        .expect_err("errors");
}
