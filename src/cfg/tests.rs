use super::*;
use serial_test::serial;

#[test]
fn sizeof_action_is_two_usizes() {
    assert_eq!(
        std::mem::size_of::<KanataAction>(),
        std::mem::size_of::<usize>() * 2
    );
}

#[test]
fn span_works() {
    let s = "(hello world my oyster)\n(row two)";
    let tlevel = parse(s).unwrap();
    assert_eq!(
        &s[tlevel[0].span.start..tlevel[0].span.end],
        "(hello world my oyster)"
    );
    assert_eq!(&s[tlevel[1].span.start..tlevel[1].span.end], "(row two)");
}

#[test]
#[serial]
fn parse_simple() {
    new_from_file(&std::path::PathBuf::from("./cfg_samples/simple.kbd")).unwrap();
}

#[test]
#[serial]
fn parse_minimal() {
    new_from_file(&std::path::PathBuf::from("./cfg_samples/minimal.kbd")).unwrap();
}

#[test]
#[serial]
fn parse_default() {
    new_from_file(&std::path::PathBuf::from("./cfg_samples/kanata.kbd")).unwrap();
}

#[test]
#[serial]
fn parse_jtroo() {
    let cfg = new_from_file(&std::path::PathBuf::from("./cfg_samples/jtroo.kbd")).unwrap();
    assert_eq!(cfg.layer_info.len(), 16);
}

#[test]
#[serial]
fn parse_f13_f24() {
    new_from_file(&std::path::PathBuf::from("./cfg_samples/f13_f24.kbd")).unwrap();
}

#[test]
#[serial]
fn parse_action_vars() {
    let mut s = ParsedState::default();
    let source = r#"
(defvar
  one 1
  two 2
  a a
  base base
  three ($a b c)
  chr C-S-v
  td ($a b $chr)
  four (lctl d)
)
(defvar
  five (lsft e)
  rel release
  e  example
  e2 example2
  chord1 (chord $e $one)
  chord2 (chord $e2 $one)
  1 (1)
  full-action (tap-dance $one $three)
)
(defalias
  tdl (tap-dance $two $td)
  tde (tap-dance-eager $two $td)
  unc (unicode $one)
  rlk (release-key $one)
  mul (multi $two $one)
  mwu (mwheel-up $one $two)
  mmu (movemouse-up $one $two)
  mau (movemouse-accel-up $one $two $one $two)
  ons (one-shot $one $two)
  thd (tap-hold $one $two $chr $two)
  tht (tap-hold-release-timeout $one $two $chr $two $one)
  thk (tap-hold-release-keys $one $two $chr $two $three)
  mac (macro $one $two $one $two $chr C-S-$three $one)
  dr1 (dynamic-macro-record $one)
  dp1 (dynamic-macro-play $one)
  abc (arbitrary-code $one)
  opf (on-press-fakekey $one $rel)
  orf (on-release-fakekey $one $rel)
  fla $full-action
  frk (fork $one $two $five)
)
(defsrc a b c d)
(deflayer base $chord1 $chord2 $chr @tdl)
(defoverrides
  ($two) ($one)
  ($one) $four
  $five ($two)
  $four $five
)
(deffakekeys
  $one $two
)
(defseq $one $three)
(defchords $e $one $1 $two)
(defchords $e2 $one ($one) $two)
"#;
    s.cfg_text = source.into();
    parse_cfg_raw_string(source.into(), &mut s)
        .map_err(|e| {
            eprintln!("{:?}", error_with_source(e.into(), &s));
            ""
        })
        .unwrap();
}

#[test]
#[serial]
fn parse_transparent_default() {
    let mut s = ParsedState::default();
    let (_, _, layer_strings, layers, _, _) = parse_cfg_raw(
        &std::path::PathBuf::from("./cfg_samples/transparent_default.kbd"),
        &mut s,
    )
    .unwrap();

    assert_eq!(layer_strings.len(), 4);

    assert_eq!(
        layers[0][0][usize::from(OsCode::KEY_F13)],
        Action::KeyCode(KeyCode::F13)
    );
    assert_eq!(
        layers[0][0][usize::from(OsCode::KEY_F14)],
        Action::DefaultLayer(2)
    );
    assert_eq!(layers[0][0][usize::from(OsCode::KEY_F15)], Action::Layer(3));
    assert_eq!(layers[1][0][usize::from(OsCode::KEY_F13)], Action::Trans);
    assert_eq!(
        layers[1][0][usize::from(OsCode::KEY_F14)],
        Action::DefaultLayer(2)
    );
    assert_eq!(layers[1][0][usize::from(OsCode::KEY_F15)], Action::Layer(3));
    assert_eq!(
        layers[2][0][usize::from(OsCode::KEY_F13)],
        Action::DefaultLayer(0)
    );
    assert_eq!(layers[2][0][usize::from(OsCode::KEY_F14)], Action::Layer(1));
    assert_eq!(
        layers[2][0][usize::from(OsCode::KEY_F15)],
        Action::KeyCode(KeyCode::F15)
    );
    assert_eq!(
        layers[3][0][usize::from(OsCode::KEY_F13)],
        Action::DefaultLayer(0)
    );
    assert_eq!(layers[3][0][usize::from(OsCode::KEY_F14)], Action::Layer(1));
    assert_eq!(layers[3][0][usize::from(OsCode::KEY_F15)], Action::Trans);
}

#[test]
#[serial]
fn parse_all_keys() {
    new_from_file(&std::path::PathBuf::from(
        "./cfg_samples/all_keys_in_defsrc.kbd",
    ))
    .unwrap();
}

#[test]
#[serial]
fn parse_multiline_comment() {
    new_from_file(&std::path::PathBuf::from(
        "./test_cfgs/multiline_comment.kbd",
    ))
    .unwrap();
}

#[test]
#[serial]
fn disallow_nested_tap_hold() {
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/nested_tap_hold.kbd"))
        .map_err(|e| format!("{e:?}"))
    {
        Ok(_) => panic!("invalid nested tap-hold in tap action was Ok'd"),
        Err(e) => assert!(e.contains("tap-hold"), "real e: {e}"),
    }
}

#[test]
#[serial]
fn disallow_ancestor_seq() {
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/ancestor_seq.kbd"))
        .map_err(|e| format!("{e:?}"))
    {
        Ok(_) => panic!("invalid ancestor seq was Ok'd"),
        Err(e) => assert!(e.contains("is contained")),
    }
}

#[test]
#[serial]
fn disallow_descendent_seq() {
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/descendant_seq.kbd"))
        .map_err(|e| format!("{e:?}"))
    {
        Ok(_) => panic!("invalid descendant seq was Ok'd"),
        Err(e) => assert!(e.contains("contains")),
    }
}

#[test]
#[serial]
fn disallow_multiple_waiting_actions() {
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/bad_multi.kbd"))
        .map_err(|e| format!("{e:?}"))
    {
        Ok(_) => panic!("invalid multiple waiting actions Ok'd"),
        Err(e) => assert!(e.contains("Cannot combine multiple")),
    }
}

#[test]
fn recursive_multi_is_flattened() {
    macro_rules! atom {
        ($e:expr) => {
            SExpr::Atom(Spanned::new($e.into(), Span::default()))
        };
    }
    macro_rules! list {
        ($e:tt) => {
            SExpr::List(Spanned::new(vec! $e, Span::default()))
        };
    }
    use sexpr::*;
    let params = [
        atom!("a"),
        atom!("mmtp"),
        list!([
            atom!("multi"),
            atom!("b"),
            atom!("mltp"),
            list!([atom!("multi"), atom!("c"), atom!("mrtp"),])
        ]),
    ];
    let s = ParsedState::default();
    if let KanataAction::MultipleActions(parsed_multi) = parse_multi(&params, &s).unwrap() {
        assert_eq!(parsed_multi.len(), 4);
        assert_eq!(parsed_multi[0], Action::KeyCode(KeyCode::A));
        assert_eq!(parsed_multi[1], Action::KeyCode(KeyCode::B));
        assert_eq!(parsed_multi[2], Action::KeyCode(KeyCode::C));
        assert_eq!(
            parsed_multi[3],
            Action::Custom(
                &&[
                    &CustomAction::MouseTap(Btn::Mid),
                    &CustomAction::MouseTap(Btn::Left),
                    &CustomAction::MouseTap(Btn::Right),
                ][..]
            )
        );
    } else {
        panic!("multi did not parse into multi");
    }
}
