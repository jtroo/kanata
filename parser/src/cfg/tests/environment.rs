use super::*;

fn parse_cfg_env(
    cfg: &str,
    env_vars: Vec<(String, String)>,
) -> Result<IntermediateCfg> {
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
    parse_cfg_raw_string(
        cfg,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
        Ok(env_vars),
    )
}

#[test]
fn parse_env() {
    parse_cfg_env(
        r#"
        (environment (hello "") (defsrc a))
        (environment (goodbye "") (deflayer 1 (layer-switch 2)))
        (environment (farewell val) (deflayer 2 (layer-switch 1)))
        ;; below would conflict if environment did not cancel
        (environment (hello yea) (defsrc))
        (environment (goodbye yea) (deflayer 1))
        (environment (farewell notval) (deflayer 2))
        "#,
        vec![
            ("goodbye".into(), "".into()),
            ("farewell".into(), "val".into()),
        ],
    )
    .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
    .unwrap();
}
