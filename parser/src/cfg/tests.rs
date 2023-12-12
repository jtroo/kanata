use super::*;
#[allow(unused_imports)]
use crate::cfg::sexpr::{parse, Span};
use kanata_keyberon::action::BooleanOperator::*;

use std::sync::Mutex;

static CFG_PARSE_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn sizeof_action_is_two_usizes() {
    assert_eq!(
        std::mem::size_of::<KanataAction>(),
        std::mem::size_of::<usize>() * 2
    );
}

#[test]
fn test_span_absolute_ranges() {
    let s = "(hello world my oyster)\n(row two)";
    let tlevel = parse(s, "test").unwrap();
    assert_eq!(
        &s[tlevel[0].span.start()..tlevel[0].span.end()],
        "(hello world my oyster)"
    );
    assert_eq!(
        &s[tlevel[1].span.start()..tlevel[1].span.end()],
        "(row two)"
    );
}

#[test]
fn span_works_with_unicode_characters() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"(defsrc a) ;; 😊
(deflayer base @😊)
"#;
    let span = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .expect_err("should be an error because @😊 is not defined")
    .span
    .expect("span should be Some");

    assert_eq!(&source[span.start()..span.end()], "@😊");

    assert_eq!(span.start.line, 1);
    assert_eq!(span.end.line, 1);

    assert_eq!("😊".len(), 4);
    assert_eq!("(defsrc a) ;; 😊\n".len(), 19);
    assert_eq!(span.start.line_beginning, 19);
    assert_eq!(span.end.line_beginning, 19);
}

#[test]
fn test_multiline_error_span() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"(defsrc a)
(
  🍍
  🍕
)
(defalias a b)
"#;
    let span = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .expect_err("should error on unknown top level block")
    .span
    .expect("span should be Some");

    assert_eq!(&source[span.start()..span.end()], "(\n  🍍\n  🍕\n)");

    assert_eq!(span.start.line, 1);
    assert_eq!(span.end.line, 4);

    assert_eq!(span.start.line_beginning, "(defsrc a)\n".len());
    assert_eq!(span.end.line_beginning, "(defsrc a)\n(\n  🍍\n  🍕\n".len());
}

#[test]
fn test_span_of_an_unterminated_block_comment_error() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"(defsrc a) |# I'm an unterminated block comment..."#;
    let span = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .expect_err("should be an unterminated comment error")
    .span
    .expect("span should be Some");

    assert_eq!(&source[span.start()..span.end()], "|#");

    assert_eq!(span.start.line, 0);
    assert_eq!(span.end.line, 0);

    assert_eq!(span.start.line_beginning, 0);
    assert_eq!(span.end.line_beginning, 0);
}

#[test]
fn parse_action_vars() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
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
  the (tap-hold-except-keys $one $two $chr $two $three)
  mac (macro $one $two $one $two $chr C-S-$three $one)
  rmc (macro-repeat $one $two $one $two $chr C-S-$three $one)
  dr1 (dynamic-macro-record $one)
  dp1 (dynamic-macro-play $one)
  abc (arbitrary-code $one)
  opf (on-press-fakekey $one $rel)
  orf (on-release-fakekey $one $rel)
  fla $full-action
  frk (fork $one $two $five)
  cpw (caps-word-custom $one $three $four)
  rst (dynamic-macro-record-stop-truncate $one)
  stm (setmouse $one $two)
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
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .unwrap();
}

#[test]
fn parse_delegate_to_default_layer_yes() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defcfg delegate-to-first-layer yes)
(defsrc a)
(deflayer base b)
(deflayer other _)
"#;
    let res = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .unwrap();
    assert_eq!(
        res.3[2][0][OsCode::KEY_A.as_u16() as usize],
        Action::KeyCode(KeyCode::B),
    );
}

#[test]
fn parse_delegate_to_default_layer_yes_but_base_transparent() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defcfg delegate-to-first-layer yes)
(defsrc a)
(deflayer base _)
(deflayer other _)
"#;
    let res = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .unwrap();
    assert_eq!(
        res.3[2][0][OsCode::KEY_A.as_u16() as usize],
        Action::KeyCode(KeyCode::A),
    );
}

#[test]
fn parse_delegate_to_default_layer_no() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defcfg delegate-to-first-layer no)
(defsrc a)
(deflayer base b)
(deflayer other _)
"#;
    let res = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .unwrap();
    assert_eq!(
        res.3[2][0][OsCode::KEY_A.as_u16() as usize],
        Action::KeyCode(KeyCode::A),
    );
}

#[test]
fn parse_transparent_default() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let (_, _, layer_strings, layers, _, _) = parse_cfg_raw(
        &std::path::PathBuf::from("./test_cfgs/transparent_default.kbd"),
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
fn parse_multiline_comment() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from(
        "./test_cfgs/multiline_comment.kbd",
    ))
    .unwrap();
}

#[test]
fn disallow_nested_tap_hold() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/nested_tap_hold.kbd"))
        .map_err(|e| format!("{}", e.help().unwrap()))
    {
        Ok(_) => panic!("invalid nested tap-hold in tap action was Ok'd"),
        Err(e) => assert!(e.contains("tap-hold"), "real e: {e}"),
    }
}

#[test]
fn disallow_ancestor_seq() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/ancestor_seq.kbd"))
        .map_err(|e| format!("{e:?}"))
    {
        Ok(_) => panic!("invalid ancestor seq was Ok'd"),
        Err(e) => assert!(e.contains("is contained")),
    }
}

#[test]
fn disallow_descendent_seq() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/descendant_seq.kbd"))
        .map_err(|e| format!("{e:?}"))
    {
        Ok(_) => panic!("invalid descendant seq was Ok'd"),
        Err(e) => assert!(e.contains("contains")),
    }
}

#[test]
fn disallow_multiple_waiting_actions() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/bad_multi.kbd"))
        .map_err(|e| format!("{}", e.help().unwrap()))
    {
        Ok(_) => panic!("invalid multiple waiting actions Ok'd"),
        Err(e) => assert!(e.contains("Cannot combine multiple")),
    }
}

#[test]
fn chord_in_macro_dont_panic() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from(
        "./test_cfgs/macro-chord-dont-panic.kbd",
    ))
    .map(|_| ())
    .expect_err("graceful failure, no panic, also no success");
}

#[test]
fn unknown_defcfg_item_fails() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from(
        "./test_cfgs/unknown_defcfg_opt.kbd",
    ))
    .map(|_| ())
    .expect_err("graceful failure, no panic, also no success");
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

#[test]
fn test_parse_sequence_a_b() {
    let seq = parse_sequence_keys(
        &parse("(a b)", "test").expect("parses")[0].t,
        &ParsedState::default(),
    )
    .expect("parses");
    assert_eq!(seq.len(), 2);
    assert_eq!(&seq[0], &u16::from(OsCode::KEY_A));
    assert_eq!(&seq[1], &u16::from(OsCode::KEY_B));
}

#[test]
fn test_parse_sequence_sa_b() {
    let seq = parse_sequence_keys(
        &parse("(S-a b)", "test").expect("parses")[0].t,
        &ParsedState::default(),
    )
    .expect("parses");
    assert_eq!(seq.len(), 3);
    assert_eq!(&seq[0], &(u16::from(OsCode::KEY_LEFTSHIFT) | 0x8000));
    assert_eq!(&seq[1], &(u16::from(OsCode::KEY_A) | 0x8000));
    assert_eq!(&seq[2], &u16::from(OsCode::KEY_B));
}

#[test]
fn test_parse_sequence_sab() {
    let seq = parse_sequence_keys(
        &parse("(S-(a b))", "test").expect("parses")[0].t,
        &ParsedState::default(),
    )
    .expect("parses");
    assert_eq!(seq.len(), 3);
    assert_eq!(&seq[0], &(u16::from(OsCode::KEY_LEFTSHIFT) | 0x8000));
    assert_eq!(&seq[1], &(u16::from(OsCode::KEY_A) | 0x8000));
    assert_eq!(&seq[2], &(u16::from(OsCode::KEY_B) | 0x8000));
}

#[test]
fn test_parse_sequence_bigchord() {
    let seq = parse_sequence_keys(
        &parse("(AG-A-M-C-S-(a b) c)", "test").expect("parses")[0].t,
        &ParsedState::default(),
    )
    .expect("parses");
    assert_eq!(seq.len(), 8);
    assert_eq!(&seq[0], &(u16::from(OsCode::KEY_RIGHTALT) | 0x1000));
    assert_eq!(&seq[1], &(u16::from(OsCode::KEY_LEFTALT) | 0x3000));
    assert_eq!(&seq[2], &(u16::from(OsCode::KEY_LEFTMETA) | 0x3800));
    assert_eq!(&seq[3], &(u16::from(OsCode::KEY_LEFTCTRL) | 0x7800));
    assert_eq!(&seq[4], &(u16::from(OsCode::KEY_LEFTSHIFT) | 0xF800));
    assert_eq!(&seq[5], &(u16::from(OsCode::KEY_A) | 0xF800));
    assert_eq!(&seq[6], &(u16::from(OsCode::KEY_B) | 0xF800));
    assert_eq!(&seq[7], &(u16::from(OsCode::KEY_C)));
}

#[test]
fn test_parse_sequence_inner_chord() {
    let seq = parse_sequence_keys(
        &parse("(S-(a b C-c) d)", "test").expect("parses")[0].t,
        &ParsedState::default(),
    )
    .expect("parses");
    assert_eq!(seq.len(), 6);
    assert_eq!(&seq[0], &(u16::from(OsCode::KEY_LEFTSHIFT) | 0x8000));
    assert_eq!(&seq[1], &(u16::from(OsCode::KEY_A) | 0x8000));
    assert_eq!(&seq[2], &(u16::from(OsCode::KEY_B) | 0x8000));
    assert_eq!(&seq[3], &(u16::from(OsCode::KEY_LEFTCTRL) | 0xC000));
    assert_eq!(&seq[4], &(u16::from(OsCode::KEY_C) | 0xC000));
    assert_eq!(&seq[5], &(u16::from(OsCode::KEY_D)));
}

#[test]
fn test_parse_sequence_earlier_inner_chord() {
    let seq = parse_sequence_keys(
        &parse("(S-(a C-b c) d)", "test").expect("parses")[0].t,
        &ParsedState::default(),
    )
    .expect("parses");
    assert_eq!(seq.len(), 6);
    assert_eq!(&seq[0], &(u16::from(OsCode::KEY_LEFTSHIFT) | 0x8000));
    assert_eq!(&seq[1], &(u16::from(OsCode::KEY_A) | 0x8000));
    assert_eq!(&seq[2], &(u16::from(OsCode::KEY_LEFTCTRL) | 0xC000));
    assert_eq!(&seq[3], &(u16::from(OsCode::KEY_B) | 0xC000));
    assert_eq!(&seq[4], &(u16::from(OsCode::KEY_C) | 0x8000));
    assert_eq!(&seq[5], &(u16::from(OsCode::KEY_D)));
}

#[test]
fn test_parse_sequence_numbers() {
    let seq = parse_sequence_keys(
        &parse("(0 1 2 3 4 5 6 7 8 9)", "test").expect("parses")[0].t,
        &ParsedState::default(),
    )
    .expect("parses");
    assert_eq!(seq.len(), 10);
    assert_eq!(&seq[0], &u16::from(OsCode::KEY_0));
    assert_eq!(&seq[1], &u16::from(OsCode::KEY_1));
    assert_eq!(&seq[2], &u16::from(OsCode::KEY_2));
    assert_eq!(&seq[3], &u16::from(OsCode::KEY_3));
    assert_eq!(&seq[4], &u16::from(OsCode::KEY_4));
    assert_eq!(&seq[5], &u16::from(OsCode::KEY_5));
    assert_eq!(&seq[6], &u16::from(OsCode::KEY_6));
    assert_eq!(&seq[7], &u16::from(OsCode::KEY_7));
    assert_eq!(&seq[8], &u16::from(OsCode::KEY_8));
    assert_eq!(&seq[9], &u16::from(OsCode::KEY_9));
}

#[test]
fn test_parse_macro_numbers() {
    // Note, can't test zero in this way because a delay of 0 isn't allowed by the parsing.
    let exprs = parse("(1 2 3 4 5 6 7 8 9)", "test").expect("parses")[0]
        .t
        .clone();
    let mut expr_rem = exprs.as_slice();
    let mut i = 1;
    while !expr_rem.is_empty() {
        let (macro_events, expr_rem_tmp) =
            parse_macro_item(expr_rem, &ParsedState::default()).expect("parses");
        expr_rem = expr_rem_tmp;
        assert_eq!(macro_events.len(), 1);
        match &macro_events[0] {
            SequenceEvent::Delay { duration } => assert_eq!(duration, &i),
            ev => panic!("expected delay, {ev:?}"),
        }
        i += 1;
    }

    let exprs = parse("(0)", "test").expect("parses")[0].t.clone();
    parse_macro_item(exprs.as_slice(), &ParsedState::default()).expect_err("errors");
}

#[test]
fn test_include_good() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from("./test_cfgs/include-good.kbd")).unwrap();
}

#[test]
fn test_include_bad_has_filename_included() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let err = format!(
        "{:?}",
        new_from_file(
            &std::path::Path::new(".")
                .join("test_cfgs")
                .join("include-bad.kbd")
        )
        .map(|_| ())
        .unwrap_err()
    );
    assert!(err.contains("included-bad.kbd"));
    assert!(!err.contains(&format!(
        "test_cfgs{}include-bad.kbd",
        std::path::MAIN_SEPARATOR
    )));
}

#[test]
fn test_include_bad2_has_original_filename() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let err = format!(
        "{:?}",
        new_from_file(
            &std::path::Path::new(".")
                .join("test_cfgs")
                .join("include-bad2.kbd")
        )
        .map(|_| ())
        .unwrap_err()
    );
    assert!(!err.contains(&format!(
        "test_cfgs{}included-bad2.kbd",
        std::path::MAIN_SEPARATOR
    )));
    assert!(err.contains(&format!(
        "test_cfgs{}include-bad2.kbd",
        std::path::MAIN_SEPARATOR
    )));
}

#[test]
fn parse_bad_submacro() {
    // Test exists since it used to crash. It should not crash.
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defsrc a)
(deflayer base
  (macro M-s-())
)
"#;
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .map_err(|_e| {
        // uncomment to see what this looks like when running test
        // eprintln!("{:?}", _e);
        ""
    })
    .unwrap_err();
}

#[test]
fn parse_bad_submacro_2() {
    // Test exists since it used to crash. It should not crash.
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defsrc a)
(deflayer base
  (macro M-s-g)
)
"#;
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .map_err(|_e| {
        // uncomment to see what this looks like when running test
        // eprintln!("{:?}", _e);
        ""
    })
    .unwrap_err();
}

#[test]
fn parse_nested_macro() {
    // Test exists since it used to crash. It should not crash.
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defvar m1 (a b c))
(defsrc a b)
(deflayer base
  (macro $m1)
  (macro bspc bspc $m1)
)
"#;
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .map_err(|e| {
        eprintln!("{:?}", miette::Error::from(e));
        ""
    })
    .unwrap();
}

#[test]
fn parse_switch() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defvar var1 a)
(defsrc a)
(deflayer base
  (switch
    ((and a b (or c d) (or e f))) XX break
    () _ fallthrough
    (a b c) $var1 fallthrough
    ((or (or (or (or (or (or (or (or))))))))) $var1 fallthrough
    ((key-history a 1) (key-history b 5) (key-history c 8)) $var1 fallthrough
  )
)
"#;
    let res = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .unwrap();
    assert_eq!(
        res.3[0][0][OsCode::KEY_A.as_u16() as usize],
        Action::Switch(&Switch {
            cases: &[
                (
                    &[
                        OpCode::new_bool(And, 9),
                        OpCode::new_key(KeyCode::A),
                        OpCode::new_key(KeyCode::B),
                        OpCode::new_bool(Or, 6),
                        OpCode::new_key(KeyCode::C),
                        OpCode::new_key(KeyCode::D),
                        OpCode::new_bool(Or, 9),
                        OpCode::new_key(KeyCode::E),
                        OpCode::new_key(KeyCode::F),
                    ],
                    &Action::NoOp,
                    BreakOrFallthrough::Break
                ),
                (&[], &Action::Trans, BreakOrFallthrough::Fallthrough),
                (
                    &[
                        OpCode::new_key(KeyCode::A),
                        OpCode::new_key(KeyCode::B),
                        OpCode::new_key(KeyCode::C),
                    ],
                    &Action::KeyCode(KeyCode::A),
                    BreakOrFallthrough::Fallthrough
                ),
                (
                    &[
                        OpCode::new_bool(Or, 8),
                        OpCode::new_bool(Or, 8),
                        OpCode::new_bool(Or, 8),
                        OpCode::new_bool(Or, 8),
                        OpCode::new_bool(Or, 8),
                        OpCode::new_bool(Or, 8),
                        OpCode::new_bool(Or, 8),
                        OpCode::new_bool(Or, 8),
                    ],
                    &Action::KeyCode(KeyCode::A),
                    BreakOrFallthrough::Fallthrough
                ),
                (
                    &[
                        OpCode::new_key_history(KeyCode::A, 0),
                        OpCode::new_key_history(KeyCode::B, 4),
                        OpCode::new_key_history(KeyCode::C, 7),
                    ],
                    &Action::KeyCode(KeyCode::A),
                    BreakOrFallthrough::Fallthrough
                ),
            ]
        })
    );
}

#[test]
fn parse_switch_exceed_depth() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defsrc a)
(deflayer base
  (switch
    ((or (or (or (or (or (or (or (or (or)))))))))) XX break
  )
)
"#;
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .map_err(|_e| {
        // uncomment to see what this looks like when running test
        // eprintln!("{:?}", _e);
        ""
    })
    .unwrap_err();
}

#[test]
fn parse_on_idle_fakekey() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defvar var1 a)
(defsrc a)
(deffakekeys hello a)
(deflayer base
  (on-idle-fakekey hello tap 200)
)
"#;
    let res = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .map_err(|_e| {
        eprintln!("{:?}", _e);
        ""
    })
    .unwrap();
    assert_eq!(
        res.3[0][0][OsCode::KEY_A.as_u16() as usize],
        Action::Custom(
            &[&CustomAction::FakeKeyOnIdle(FakeKeyOnIdle {
                coord: Coord { x: 1, y: 0 },
                action: FakeKeyAction::Tap,
                idle_duration: 200
            })]
            .as_ref()
        ),
    );
}

#[test]
fn parse_on_idle_fakekey_errors() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defvar var1 a)
(defsrc a)
(deffakekeys hello a)
(deflayer base
  (on-idle-fakekey hello bap 200)
)
"#;
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .map_err(|_e| {
        // comment out to see what this looks like when running test
        // eprintln!("{:?}", _e);
        ""
    })
    .unwrap_err();

    let source = r#"
(defvar var1 a)
(defsrc a)
(deffakekeys hello a)
(deflayer base
  (on-idle-fakekey jello tap 200)
)
"#;
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .map_err(|_e| {
        // uncomment to see what this looks like when running test
        // eprintln!("{:?}", _e);
        ""
    })
    .unwrap_err();

    let source = r#"
(defvar var1 a)
(defsrc a)
(deffakekeys hello a)
(deflayer base
  (on-idle-fakekey (hello) tap 200)
)
"#;
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .map_err(|_e| {
        // uncomment to see what this looks like when running test
        // eprintln!("{:?}", _e);
        ""
    })
    .unwrap_err();

    let source = r#"
(defvar var1 a)
(defsrc a)
(deffakekeys hello a)
(deflayer base
  (on-idle-fakekey hello tap -1)
)
"#;
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .map_err(|_e| {
        // uncomment to see what this looks like when running test
        // eprintln!("{:?}", _e);
        ""
    })
    .unwrap_err();
}

#[test]
fn parse_fake_keys_errors_on_too_many() {
    let mut s = ParsedState::default();
    let mut checked_for_err = false;
    for n in 0..1000 {
        let exprs = [&vec![
            SExpr::Atom(Spanned {
                t: "deffakekeys".to_string(),
                span: Default::default(),
            }),
            SExpr::Atom(Spanned {
                t: "a".repeat(n),
                span: Default::default(),
            }),
            SExpr::Atom(Spanned {
                t: "a".to_string(),
                span: Default::default(),
            }),
        ]];
        if n < 500 {
            // fill up fake keys, expect first bunch to succeed
            parse_fake_keys(&exprs, &mut s).unwrap();
        } else if n < 999 {
            // at some point they start failing, ignore result
            let _ = parse_fake_keys(&exprs, &mut s);
        } else {
            // last iteration, check for error. probably happened before this, but just check here
            let _ = parse_fake_keys(&exprs, &mut s).unwrap_err();
            checked_for_err = true;
        }
    }
    assert!(checked_for_err);
}

#[test]
fn parse_deflocalkeys_overridden() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let source = r#"
(deflocalkeys-win
+   300
[   301
]   302
{   303
}   304
/   305
;   306
`   307
=   308
-   309
'   310
,   311
.   312
\   313
yen 314
¥   315
new 316
)
(deflocalkeys-wintercept
+   300
[   301
]   302
{   303
}   304
/   305
;   306
`   307
=   308
-   309
'   310
,   311
.   312
\   313
yen 314
¥   315
new 316
)
(deflocalkeys-linux
+   300
[   301
]   302
{   303
}   304
/   305
;   306
`   307
=   308
-   309
'   310
,   311
.   312
\   313
yen 314
¥   315
new 316
)
(deflocalkeys-macos
+   300
[   301
]   302
{   303
}   304
/   305
;   306
`   307
=   308
-   309
'   310
,   311
.   312
\   313
yen 314
¥   315
new 316
)
(defsrc + [  ]  {  }  /  ;  `  =  -  '  ,  .  \  yen ¥ new)
(deflayer base + [  ]  {  }  /  ;  `  =  -  '  ,  .  \  yen ¥ new)
"#;
    let mut s = ParsedState::default();
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .expect("succeeds");
}

#[test]
fn use_default_overridable_mappings() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let source = r#"
(defsrc + [  ]  a  b  /  ;  `  =  -  '  ,  .  9  yen ¥  )
(deflayer base + [  ]  {  }  /  ;  `  =  -  '  ,  .  \  yen ¥  )
"#;
    let mut s = ParsedState::default();
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .expect("succeeds");
}

#[test]
fn list_action_not_in_list_error_message_is_good() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defsrc a)
(defalias hello
  one-shot 1 2
)
(deflayer base hello)
"#;
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .map_err(|e| {
        assert_eq!(
            e.msg,
            "This is a list action and must be in parentheses: (one-shot ...)"
        );
    })
    .unwrap_err();
}

#[test]
fn parse_device_paths() {
    assert_eq!(parse_colon_separated_text(""), [""]);
    assert_eq!(parse_colon_separated_text("device1"), ["device1"]);
    assert_eq!(parse_colon_separated_text("h:w"), ["h", "w"]);
    assert_eq!(parse_colon_separated_text("h\\:w"), ["h:w"]);
    assert_eq!(parse_colon_separated_text("h\\:w\\"), ["h:w\\"]);
}

#[test]
#[cfg(any(target_os = "linux", target_os = "unknown"))]
fn test_parse_dev() {
    // The old colon separated devices format
    assert_eq!(
        parse_dev(&SExpr::Atom(Spanned {
            t: "\"Keyboard2:Input Device 1:pci-0000\\:00\\:14.0-usb-0\\:1\\:1.0-event\""
                .to_string(),
            span: Span::default(),
        }))
        .expect("succeeds"),
        [
            "Keyboard2",
            "Input Device 1",
            "pci-0000:00:14.0-usb-0:1:1.0-event"
        ]
    );
    parse_dev(&SExpr::Atom(Spanned {
        t: "\"\"".to_string(),
        span: Span::default(),
    }))
    .expect_err("'' is not a valid device name/path, this should fail");

    // The new device list format
    assert_eq!(
        parse_dev(&SExpr::List(Spanned {
            t: vec![
                SExpr::Atom(Spanned {
                    t: "Keyboard2".to_string(),
                    span: Span::default(),
                }),
                SExpr::Atom(Spanned {
                    t: "\"Input Device 1\"".to_string(),
                    span: Span::default(),
                }),
                SExpr::Atom(Spanned {
                    t: "pci-0000:00:14.0-usb-0:1:1.0-event".to_string(),
                    span: Span::default(),
                }),
                SExpr::Atom(Spanned {
                    t: r"backslashes\do\not\escape\:\anything".to_string(),
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        }))
        .expect("succeeds"),
        [
            "Keyboard2",
            "Input Device 1",
            "pci-0000:00:14.0-usb-0:1:1.0-event",
            r"backslashes\do\not\escape\:\anything"
        ]
    );
    parse_dev(&SExpr::List(Spanned {
        t: vec![
            SExpr::Atom(Spanned {
                t: "Device1".to_string(),
                span: Span::default(),
            }),
            SExpr::List(Spanned {
                t: vec![],
                span: Span::default(),
            }),
        ],
        span: Span::default(),
    }))
    .expect_err("nested lists in path list shouldn't be allowed");
}

#[test]
fn parse_all_defcfg() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let source = r#"
(defcfg
  process-unmapped-keys yes
  danger-enable-cmd yes
  sequence-timeout 2000
  sequence-input-mode visible-backspaced
  sequence-backtrack-modcancel no
  log-layer-changes no
  delegate-to-first-layer yes
  movemouse-inherit-accel-state yes
  movemouse-smooth-diagonals yes
  dynamic-macro-max-presses 1000
  linux-dev /dev/input/dev1:/dev/input/dev2
  linux-dev-names-include "Name 1:Name 2"
  linux-dev-names-exclude "Name 3:Name 4"
  linux-continue-if-no-devs-found yes
  linux-unicode-u-code v
  linux-unicode-termination space
  linux-x11-repeat-delay-rate 400,50
  windows-altgr add-lctl-release
  windows-interception-mouse-hwid "70, 0, 60, 0"
)
(defsrc a)
(deflayer base a)
"#;
    let mut s = ParsedState::default();
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .expect("succeeds");
}

#[test]
fn parse_unmod() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let source = r#"
(defsrc a b c d)
(deflayer base
  (unmod a)
  (unmod a b)
  (unshift a)
  (unshift a b)
)
"#;
    let mut s = ParsedState::default();
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .expect("succeeds");
}

#[test]
fn using_parentheses_in_deflayer_directly_fails_with_custom_message() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defsrc a b)
(deflayer base ( ))
"#;
    let err = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .expect_err("should err");
    assert!(err
        .msg
        .contains("You can't put parentheses in deflayer directly"));
}

#[test]
fn using_escaped_parentheses_in_deflayer_fails_with_custom_message() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut s = ParsedState::default();
    let source = r#"
(defsrc a b)
(deflayer base \( \))
"#;
    let err = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .expect_err("should err");
    assert!(err
        .msg
        .contains("Escaping shifted characters with `\\` is currently not supported"));
}

#[test]
#[cfg(feature = "cmd")]
fn parse_cmd() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let source = r#"
(defcfg danger-enable-cmd yes)
(defsrc a)
(deflayer base a)
(defvar
    x blah
    y (nyoom)
    z (squish squash (splish splosh))
)
(defalias
    1 (cmd hello world)
    2 (cmd (hello world))
    3 (cmd $x $y ($z))
)
"#;
    let mut s = ParsedState::default();
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
    )
    .expect("succeeds");
}
