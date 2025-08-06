use super::*;
#[allow(unused_imports)]
use crate::cfg::sexpr::{Span, parse};
use kanata_keyberon::action::BooleanOperator::*;

use std::sync::{Mutex, MutexGuard};

mod ambiguous;
mod defcfg;
mod device_detect;
mod environment;
mod macros;

static CFG_PARSE_LOCK: Mutex<()> = Mutex::new(());

fn init_log() {
    use simplelog::*;
    use std::sync::OnceLock;
    static LOG_INIT: OnceLock<()> = OnceLock::new();
    LOG_INIT.get_or_init(|| {
        let mut log_cfg = ConfigBuilder::new();
        if let Err(e) = log_cfg.set_time_offset_to_local() {
            eprintln!("WARNING: could not set log TZ to local: {e:?}");
        };
        log_cfg.set_time_format_rfc3339();
        CombinedLogger::init(vec![TermLogger::new(
            // Note: set to a different level to see logs in tests.
            // Also, not all tests call init_log so you might have to add the call there too.
            LevelFilter::Error,
            log_cfg.build(),
            TerminalMode::Stderr,
            ColorChoice::AlwaysAnsi,
        )])
        .expect("logger can init");
    });
}

fn lock<T>(lk: &Mutex<T>) -> MutexGuard<T> {
    match lk.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn parse_cfg(cfg: &str) -> Result<IntermediateCfg> {
    init_log();
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
    let icfg = parse_cfg_raw_string(
        cfg,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
        Err("env vars not implemented".into()),
    );
    if let Ok(ref icfg) = icfg {
        assert!(icfg.klayers.layers.iter().all(|layer| {
            layer[usize::from(NORMAL_KEY_ROW)]
                .iter()
                .all(|action| *action != DEFAULT_ACTION)
        }));
        #[cfg(any(target_os = "linux", target_os = "unknown"))]
        assert!(icfg.options.linux_opts.linux_device_detect_mode.is_some());
    }
    icfg
}

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
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
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
        Err("env vars not implemented".into()),
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
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
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
        Err("env vars not implemented".into()),
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
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
    let source = r#"(defsrc a) |# I'm an unterminated block comment..."#;
    let span = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
        Err("env vars not implemented".into()),
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
    let source = r#"
(defvirtualkeys
  ctl lctl
  sft lsft
  met lmet
  alt lalt
)
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
  mwua (🖱☸↑ $one $two)
  mwda (🖱☸↓ $one $two)
  mwla (🖱☸← $one $two)
  mwra (🖱☸→ $one $two)
  mmua (🖱↑ $one $two)
  mmda (🖱↓ $one $two)
  mmla (🖱← $one $two)
  mmra (🖱→ $one $two)
  mmu (movemouse-up $one $two)
  mau (movemouse-accel-up $one $two $one $two)
  maua (🖱accel↑ $one $two $one $two)
  mada (🖱accel↓ $one $two $one $two)
  mala (🖱accel← $one $two $one $two)
  mara (🖱accel→ $one $two $one $two)
  ons (one-shot $one $two)
  thd (tap-hold $one $two $chr $two)
  tht (tap-hold-release-timeout $one $two $chr $two $one)
  thk (tap-hold-release-keys $one $two $chr $two $three)
  the (tap-hold-except-keys $one $two $chr $two $three)
  thta (tap⬓↑timeout $one $two $chr $two $one)
  thka (tap⬓↑keys $one $two $chr $two $three)
  thea (tap⬓⤫keys $one $two $chr $two $three)
  mac (macro $one $two $one $two $chr C-S-$three $one)
  rmc (macro-repeat $one $two $one $two $chr C-S-$three $one)
  mrca (macro↑⤫ $one 500 bspc S-1 500 bspc S-2)
  mrra (macro⟳↑⤫ mltp)
  oat (tap⬓↓timeout   200 200 o $one bspc)
  fsta (🖱speed 200)
  psfa (on↓ press-virtualkey   sft)
  rsfa (on↑ release-virtualkey sft)
  os2a (one-shot↓ 2000 lsft)
  os3a (one-shot↑ 2000 lctl)
  os4a (one-shot↓⤫ 2000 lalt)
  os5a (one-shot↑⤫ 2000 lmet)
  oara (tap⬓↓ 200 200 o $two)
  echa (tap⬓↑ 200 200 e $two)
  rmca (macro⟳ $one $two $one $two $chr C-S-$three $one)
  dr1 (dynamic-macro-record $one)
  dp1 (dynamic-macro-play $one)
  abc (arbitrary-code $one)
  opf (on-press-fakekey $one $rel)
  orf (on-release-fakekey $one $rel)
  opfa (on↓fakekey $one $rel)
  orfa (on↑fakekey $one $rel)
  opfda (on↓fakekey-delay 200)
  orfda (on↑fakekey-delay 200)
  relka (key↑ $one)
  rella (layer↑ base)
  fla $full-action
  frk (fork $one $two $five)
  cpw (caps-word-custom $one $three $four)
  cwa (word⇪ 2000)
  cpwa (word⇪custom $one $three $four)
  rst (dynamic-macro-record-stop-truncate $one)
  stm (setmouse $one $two)
  stma (set🖱 $one $two)
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
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_multiline_comment() {
    let _lk = lock(&CFG_PARSE_LOCK);
    new_from_file(&std::path::PathBuf::from(
        "./test_cfgs/multiline_comment.kbd",
    ))
    .unwrap();
}

#[test]
fn parse_all_keys() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from(
        "./test_cfgs/all_keys_in_defsrc.kbd",
    ))
    .unwrap();
}

#[test]
fn parse_file_with_utf8_bom() {
    let _lk = lock(&CFG_PARSE_LOCK);
    new_from_file(&std::path::PathBuf::from("./test_cfgs/utf8bom.kbd")).unwrap();
}

#[test]
#[cfg(feature = "zippychord")]
fn parse_zippychord_file() {
    let _lk = lock(&CFG_PARSE_LOCK);
    new_from_file(&std::path::PathBuf::from("./test_cfgs/testzch.kbd")).unwrap();
}

#[test]
fn disallow_nested_tap_hold() {
    let _lk = lock(&CFG_PARSE_LOCK);
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/nested_tap_hold.kbd"))
        .map_err(|e| format!("{}", e.help().unwrap()))
    {
        Ok(_) => panic!("invalid nested tap-hold in tap action was Ok'd"),
        Err(e) => assert!(e.contains("tap-hold"), "real e: {e}"),
    }
}

#[test]
fn disallow_ancestor_seq() {
    let _lk = lock(&CFG_PARSE_LOCK);
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/ancestor_seq.kbd"))
        .map_err(|e| format!("{e:?}"))
    {
        Ok(_) => panic!("invalid ancestor seq was Ok'd"),
        Err(e) => assert!(e.contains("is contained")),
    }
}

#[test]
fn disallow_descendent_seq() {
    let _lk = lock(&CFG_PARSE_LOCK);
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/descendant_seq.kbd"))
        .map_err(|e| format!("{e:?}"))
    {
        Ok(_) => panic!("invalid descendant seq was Ok'd"),
        Err(e) => assert!(e.contains("contains")),
    }
}

#[test]
fn disallow_multiple_waiting_actions() {
    let _lk = lock(&CFG_PARSE_LOCK);
    match new_from_file(&std::path::PathBuf::from("./test_cfgs/bad_multi.kbd"))
        .map_err(|e| format!("{}", e.help().unwrap()))
    {
        Ok(_) => panic!("invalid multiple waiting actions Ok'd"),
        Err(e) => assert!(e.contains("Cannot combine multiple")),
    }
}

#[test]
fn chord_in_macro_dont_panic() {
    let _lk = lock(&CFG_PARSE_LOCK);
    new_from_file(&std::path::PathBuf::from(
        "./test_cfgs/macro-chord-dont-panic.kbd",
    ))
    .map(|_| ())
    .expect_err("graceful failure, no panic, also no success");
}

#[test]
fn unknown_defcfg_item_fails() {
    let _lk = lock(&CFG_PARSE_LOCK);
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
    let s = ParserState::default();
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
        &ParserState::default(),
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
        &ParserState::default(),
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
        &ParserState::default(),
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
        &ParserState::default(),
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
        &ParserState::default(),
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
        &ParserState::default(),
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
        &ParserState::default(),
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
            parse_macro_item(expr_rem, &ParserState::default()).expect("parses");
        expr_rem = expr_rem_tmp;
        assert_eq!(macro_events.len(), 1);
        match &macro_events[0] {
            SequenceEvent::Delay { duration } => assert_eq!(duration, &i),
            ev => panic!("expected delay, {ev:?}"),
        }
        i += 1;
    }

    let exprs = parse("(0)", "test").expect("parses")[0].t.clone();
    parse_macro_item(exprs.as_slice(), &ParserState::default()).expect_err("errors");
}

#[test]
fn test_include_good() {
    let _lk = lock(&CFG_PARSE_LOCK);
    new_from_file(&std::path::PathBuf::from("./test_cfgs/include-good.kbd")).unwrap();
}

#[test]
fn test_include_bad_has_filename_included() {
    let _lk = lock(&CFG_PARSE_LOCK);
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
    let _lk = lock(&CFG_PARSE_LOCK);
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
fn test_include_ignore_optional_filename() {
    let _lk = lock(&CFG_PARSE_LOCK);
    new_from_file(&std::path::PathBuf::from(
        "./test_cfgs/include-good-optional-absent.kbd",
    ))
    .unwrap();
}

#[test]
fn parse_bad_submacro() {
    // Test exists since it used to crash. It should not crash.
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
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
        Err("env vars not implemented".into()),
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
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
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
        Err("env vars not implemented".into()),
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
    let source = r#"
(defvar m1 (a b c))
(defsrc a b)
(deflayer base
  (macro $m1)
  (macro bspc bspc $m1)
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_switch() {
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
    let source = r#"
(defvar var1 a)
(defsrc a)
(deffakekeys vk1 XX)
(defvirtualkeys vk2 XX)
(deflayer base
  (switch
    ((and a b (or c d) (or e f))) XX break
    ((not (and a b (not (or c (not d))) (or e f)))) XX break
    () _ fallthrough
    (a b c) $var1 fallthrough
    ((or (or (or (or (or (or (or (or))))))))) $var1 fallthrough
    ((key-history a 1) (key-history b 5) (key-history c 8)) $var1 fallthrough
    ((not
      (key-timing 1 less-than 200)
      (key-timing 4 greater-than 500)
      (key-timing 7 lt 1000)
      (key-timing 8 gt 20000)
    )) $var1 fallthrough
    ((input virtual vk1)) $var1 break
    ((input real lctl)) $var1 break
    ((input-history virtual vk2 1)) $var1 break
    ((input-history real lsft 8)) $var1 break
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
        Err("env vars not implemented".into()),
    )
    .unwrap();
    let (op1, op2) = OpCode::new_active_input((FAKE_KEY_ROW, 0));
    let (op3, op4) = OpCode::new_active_input((NORMAL_KEY_ROW, u16::from(OsCode::KEY_LEFTCTRL)));
    let (op5, op6) = OpCode::new_historical_input((FAKE_KEY_ROW, 1), 0);
    let (op7, op8) =
        OpCode::new_historical_input((NORMAL_KEY_ROW, u16::from(OsCode::KEY_LEFTSHIFT)), 7);
    let (klayers, _) = res.klayers.get();
    assert_eq!(
        klayers[0][0][OsCode::KEY_A.as_u16() as usize],
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
                (
                    &[
                        OpCode::new_bool(Not, 12),
                        OpCode::new_bool(And, 12),
                        OpCode::new_key(KeyCode::A),
                        OpCode::new_key(KeyCode::B),
                        OpCode::new_bool(Not, 9),
                        OpCode::new_bool(Or, 9),
                        OpCode::new_key(KeyCode::C),
                        OpCode::new_bool(Not, 9),
                        OpCode::new_key(KeyCode::D),
                        OpCode::new_bool(Or, 12),
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
                (
                    &[
                        OpCode::new_bool(Not, 5),
                        OpCode::new_ticks_since_lt(0, 200),
                        OpCode::new_ticks_since_gt(3, 500),
                        OpCode::new_ticks_since_lt(6, 1000),
                        OpCode::new_ticks_since_gt(7, 20000),
                    ],
                    &Action::KeyCode(KeyCode::A),
                    BreakOrFallthrough::Fallthrough
                ),
                (
                    &[op1, op2],
                    &Action::KeyCode(KeyCode::A),
                    BreakOrFallthrough::Break
                ),
                (
                    &[op3, op4],
                    &Action::KeyCode(KeyCode::A),
                    BreakOrFallthrough::Break
                ),
                (
                    &[op5, op6],
                    &Action::KeyCode(KeyCode::A),
                    BreakOrFallthrough::Break
                ),
                (
                    &[op7, op8],
                    &Action::KeyCode(KeyCode::A),
                    BreakOrFallthrough::Break
                ),
            ]
        })
    );
}

#[test]
fn parse_switch_exceed_depth() {
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
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
        Err("env vars not implemented".into()),
    )
    .map_err(|_e| {
        // uncomment to see what this looks like when running test
        // eprintln!("{:?}", _e);
        ""
    })
    .unwrap_err();
}

#[test]
fn parse_virtualkeys() {
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
    let source = r#"
(defvar var1 a)
(defsrc a b c d e f g h i j k l m n o p)
(deffakekeys hello a)
(defvirtualkeys bye a)
(deflayer base
  (on-press   press-virtualkey hello)
  (on-press release-virtualkey hello)
  (on-press  toggle-virtualkey hello)
  (on-press     tap-virtualkey hello)
  (on-press   press-vkey bye)
  (on-press release-vkey bye)
  (on-press  toggle-vkey bye)
  (on-press     tap-vkey bye)
  (on-release   press-virtualkey hello)
  (on-release release-virtualkey hello)
  (on-release  toggle-virtualkey hello)
  (on-release     tap-virtualkey hello)
  (on-release   press-vkey bye)
  (on-release release-vkey bye)
  (on-release  toggle-vkey bye)
  (on-release     tap-vkey bye)
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
        Err("env vars not implemented".into()),
    )
    .map_err(|e| {
        eprintln!("{:?}", miette::Error::from(e));
        ""
    })
    .unwrap();
    let (klayers, _) = res.klayers.get();
    assert_eq!(
        klayers[0][0][OsCode::KEY_A.as_u16() as usize],
        Action::Custom(
            &[&CustomAction::FakeKey {
                coord: Coord { x: 1, y: 0 },
                action: FakeKeyAction::Press,
            }]
            .as_ref()
        ),
    );
    assert_eq!(
        klayers[0][0][OsCode::KEY_F.as_u16() as usize],
        Action::Custom(
            &[&CustomAction::FakeKey {
                coord: Coord { x: 1, y: 1 },
                action: FakeKeyAction::Release,
            }]
            .as_ref()
        ),
    );
    assert_eq!(
        klayers[0][0][OsCode::KEY_K.as_u16() as usize],
        Action::Custom(
            &[&CustomAction::FakeKeyOnRelease {
                coord: Coord { x: 1, y: 0 },
                action: FakeKeyAction::Toggle,
            }]
            .as_ref()
        ),
    );
    assert_eq!(
        klayers[0][0][OsCode::KEY_P.as_u16() as usize],
        Action::Custom(
            &[&CustomAction::FakeKeyOnRelease {
                coord: Coord { x: 1, y: 1 },
                action: FakeKeyAction::Tap,
            }]
            .as_ref()
        ),
    );
}

#[test]
fn parse_on_idle_fakekey() {
    let source = r#"
(defvar var1 a)
(defsrc a b c d e f g h i)
(deffakekeys hello a)
(defvirtualkeys bye a)
(deflayer base
  (on-idle-fakekey hello tap 200)
  (on-idle 100   press-virtualkey hello)
  (on-idle 100 release-virtualkey hello)
  (on-idle 100  toggle-virtualkey hello)
  (on-idle 100     tap-virtualkey hello)
  (on-idle 100   press-vkey bye)
  (on-idle 100 release-vkey bye)
  (on-idle 100  toggle-vkey bye)
  (on-idle 200     tap-vkey bye)
)
"#;
    let res = parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
    let (klayers, _) = res.klayers.get();
    assert_eq!(
        klayers[0][0][OsCode::KEY_A.as_u16() as usize],
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
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
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
        Err("env vars not implemented".into()),
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
        Err("env vars not implemented".into()),
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
        Err("env vars not implemented".into()),
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
        Err("env vars not implemented".into()),
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
    let mut s = ParserState::default();
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
(deflocalkeys-winiov2
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
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn use_default_overridable_mappings() {
    let source = r#"
(defsrc + [  ]  a  b  /  ;  `  =  -  '  ,  .  9  yen ¥  )
(deflayer base + [  ]  {  }  /  ;  `  =  -  '  ,  .  \  yen ¥  )
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn list_action_not_in_list_error_message_is_good() {
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
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
        Err("env vars not implemented".into()),
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
  override-release-on-activation yes
  dynamic-macro-max-presses 1000
  concurrent-tap-hold yes
  rapid-event-delay 5
  linux-dev /dev/input/dev1:/dev/input/dev2
  linux-dev-names-include "Name 1:Name 2"
  linux-dev-names-exclude "Name 3:Name 4"
  linux-continue-if-no-devs-found yes
  linux-unicode-u-code v
  linux-unicode-termination space
  linux-x11-repeat-delay-rate 400,50
  linux-use-trackpoint-property yes
  linux-output-device-name "Kanata Test"
  linux-output-device-bus-type USB
  tray-icon symbols.ico
  icon-match-layer-name no
  tooltip-layer-changes yes
  tooltip-show-blank yes
  tooltip-no-base yes
  tooltip-duration 300
  tooltip-size 24,24
  notify-cfg-reload yes
  notify-cfg-reload-silent no
  notify-error yes
  windows-altgr add-lctl-release
  windows-interception-mouse-hwid "70, 0, 60, 0"
  windows-interception-mouse-hwids ("0, 0, 0" "1, 1, 1")
  windows-interception-keyboard-hwids ("0, 0, 0" "1, 1, 1")
)
(defsrc a)
(deflayer base a)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_defcfg_linux_output_bus() {
    let source = r#"
(defcfg linux-output-device-bus-type USB)
(defsrc a)
(deflayer base a)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
    let source = r#"
(defcfg linux-output-device-bus-type I8042)
(defsrc a)
(deflayer base a)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
    let source = r#"
(defcfg linux-output-device-bus-type INVALID)
(defsrc a)
(deflayer base a)
"#;
    let err = parse_cfg(source).expect_err("should err");
    assert!(
        err.msg
            .contains("Invalid value for linux-output-device-bus-type")
    );
}

#[test]
fn parse_unmod() {
    let source = r#"
(defsrc a b c d e)
(deflayer base
  (unmod a)
  (unmod a b)
  (unshift a)
  (un⇧ a)
  (unshift a b)
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn using_parentheses_in_deflayer_directly_fails_with_custom_message() {
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
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
        Err("env vars not implemented".into()),
    )
    .expect_err("should err");
    assert!(
        err.msg
            .contains("You can't put parentheses in deflayer directly")
    );
}

#[test]
fn using_escaped_parentheses_in_deflayer_fails_with_custom_message() {
    let _lk = lock(&CFG_PARSE_LOCK);
    let mut s = ParserState::default();
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
        Err("env vars not implemented".into()),
    )
    .expect_err("should err");
    assert!(
        err.msg
            .contains("Escaping shifted characters with `\\` is currently not supported")
    );
}

#[test]
#[cfg(feature = "cmd")]
fn parse_cmd() {
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
    4 (clipboard-cmd-set powershell.exe -c "echo 'hello world'")
    5 (clipboard-save-cmd-set 0 bash -c "echo 'goodbye'")
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
#[cfg(feature = "cmd")]
fn parse_cmd_log() {
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
    1 (cmd-log debug debug hello world)
    2 (cmd-log error warn (hello world))
    3 (cmd-log info debug $x $y ($z))
    4 (cmd-log none none hello world)
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_defvar_concat() {
    let _lk = lock(&CFG_PARSE_LOCK);
    let source = r#"
(defsrc a)
(deflayer base a)
(defvar
    x (concat a b c)
    y (concat d (e f))
    z (squish squash (splish splosh))
    xx (concat $x $y)
    xy (concat $x ($y))
    xz (notconcat a b " " c " d")
    yx (concat a b " " c " d" ("efg" " hij ") "kl")
    yz (concat "abc"def"ghi""jkl")

    rootpath "/home/myuser/mysubdir"
    ;; $otherpath will be the string: /home/myuser/mysubdir/helloworld
    otherpath (concat $rootpath "/helloworld")
)
"#;
    let mut s = ParserState::default();
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
        Err("env vars not implemented".into()),
    )
    .expect("succeeds");
    match s.vars().unwrap().get("x").unwrap() {
        SExpr::Atom(a) => assert_eq!(&a.t, "abc"),
        SExpr::List(l) => panic!("expected string not list: {l:?}"),
    }
    match s.vars().unwrap().get("y").unwrap() {
        SExpr::Atom(a) => assert_eq!(&a.t, "def"),
        SExpr::List(l) => panic!("expected string not list: {l:?}"),
    }
    match s.vars().unwrap().get("z").unwrap() {
        SExpr::Atom(a) => panic!("expected list not string: {a:?}"),
        SExpr::List(_) => {}
    }
    match s.vars().unwrap().get("xx").unwrap() {
        SExpr::Atom(a) => assert_eq!(&a.t, "abcdef"),
        SExpr::List(l) => panic!("expected string not list: {l:?}"),
    }
    match s.vars().unwrap().get("xy").unwrap() {
        SExpr::Atom(a) => assert_eq!(&a.t, "abcdef"),
        SExpr::List(l) => panic!("expected string not list: {l:?}"),
    }
    match s.vars().unwrap().get("xz").unwrap() {
        SExpr::Atom(a) => panic!("expected list not string {a:?}"),
        SExpr::List(_) => {}
    }
    match s.vars().unwrap().get("yx").unwrap() {
        SExpr::Atom(a) => assert_eq!(&a.t, "ab c defg hij kl"),
        SExpr::List(l) => panic!("expected string not list: {l:?}"),
    }
    match s.vars().unwrap().get("yz").unwrap() {
        SExpr::Atom(a) => assert_eq!(&a.t, "abcdefghijkl"),
        SExpr::List(l) => panic!("expected string not list: {l:?}"),
    }
    match s.vars().unwrap().get("otherpath").unwrap() {
        SExpr::Atom(a) => assert_eq!(&a.t, "/home/myuser/mysubdir/helloworld"),
        SExpr::List(l) => panic!("expected string not list: {l:?}"),
    }
}

#[test]
fn parse_template_1() {
    let source = r#"
(deftemplate home-row (j-behaviour)
  a s d f g h $j-behaviour k l ; '
)

(defsrc
  grv  1    2    3    4    5    6    7    8    9    0    -    =    bspc
  tab  q    w    e    r    t    y    u    i    o    p    [    ]    \
  caps (template-expand home-row j)                            ret
  lsft z    x    c    v    b    n    m    ,    .    /    rsft
  lctl lmet lalt           spc            ralt rmet rctl
)

(deflayer base
  grv  1    2    3    4    5    6    7    8    9    0    -    =    bspc
  tab  q    w    e    r    t    y    u    i    o    p    [    ]    \
  caps (template-expand home-row (tap-hold 200 200 j lctl))    ret
  lsft z    x    c    v    b    n    m    ,    .    /    rsft
  lctl lmet lalt           spc            ralt rmet rctl
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_template_2() {
    let source = r#"
(defvar chord-timeout 200)
(defcfg process-unmapped-keys yes)

;; This template defines a chord group and aliases that use the chord group.
;; The purpose is to easily define the same chord position behaviour
;; for multiple layers that have different underlying keys.
(deftemplate left-hand-chords (chordgroupname k1 k2 k3 k4 alias1 alias2 alias3 alias4)
  (defalias
    $alias1 (chord $chordgroupname $k1)
    $alias2 (chord $chordgroupname $k2)
    $alias3 (chord $chordgroupname $k3)
    $alias4 (chord $chordgroupname $k4)
  )
  (defchords $chordgroupname $chord-timeout
    ($k1) $k1
    ($k2) $k2
    ($k3) $k3
    ($k4) $k4
    ($k1 $k2) lctl
    ($k3 $k4) lsft
  )
)

(template-expand left-hand-chords qwerty a s d f qwa qws qwd qwf)
(template-expand left-hand-chords dvorak a o e u dva dvo dve dvu)

(defsrc a s d f)
(deflayer dvorak @dva @dvo @dve @dvu)
(deflayer qwerty @qwa @qws @qwd @qwf)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_template_3() {
    let source = r#"
(deftemplate home-row (version)
  a s d f g h
  (if-equal $version v1 j)
  (if-equal $version v2 (tap-hold 200 200 j (if-equal $version v2 k)))
   k l ; '
)

(defsrc
  grv  1    2    3    4    5    6    7    8    9    0    -    =    bspc
  tab  q    w    e    r    t    y    u    i    o    p    [    ]    \
  caps (template-expand home-row v1)                            ret
  lsft z    x    c    v    b    n    m    ,    .    /    rsft
  lctl lmet lalt           spc            ralt rmet rctl
)

(deflayer base
  grv  1    2    3    4    5    6    7    8    9    0    -    =    bspc
  tab  q    w    e    r    t    y    u    i    o    p    [    ]    \
  caps (template-expand home-row v2)                            ret
  lsft z    x    c    v    b    n    m    ,    .    /    rsft
  lctl lmet lalt           spc            ralt rmet rctl
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_template_4() {
    let source = r#"
(deftemplate home-row (version)
  a s d f g h
  (if-not-equal $version v2 j)
  (if-not-equal $version v1 (tap-hold 200 200 j (if-not-equal $version v1 k)))
   k l ; '
)

(defsrc
  (template-expand home-row v1)
)

(deflayer base
  (template-expand home-row v2)
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_template_5() {
    let source = r#"
(deftemplate home-row (version)
  a s d f g h
  (if-in-list $version (v0 v3 v1 v4) j)
  (if-in-list $version (v0 v2 v3 v4) (tap-hold 200 200 j (if-in-list $version (v0 v3 v4 v2) k)))
   k l ; '
)

(defsrc
  (template-expand home-row v1)
)

(deflayer base
  (template-expand home-row v2)
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_template_6() {
    let source = r#"
(deftemplate home-row (version)
  a s d f g h
  (if-not-in-list $version (v2 v3 v4) j)
  (if-not-in-list $version (v1 v3 v4) (tap-hold 200 200 j (if-not-in-list $version (v1 v3 v4) k)))
   k l ; '
)

(defsrc
  (template-expand home-row v1)
)

(deflayer base
  (template-expand home-row v2)
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_template_7() {
    let source = r#"
(deftemplate home-row (version)
  a s d f g h
  (if-in-list $version (v0 v3 (concat v (((1)))) v4) (concat j))
  (if-in-list $version ((concat v 0) (concat v (2)) v3 v4) (tap-hold 200 200 (concat j) (if-in-list $version (v0 v3 v4 v2) (concat "k"))))
   k l ; '
)

(defsrc
  (template-expand home-row v1)
)

(deflayer base
  (template-expand home-row v2)
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_template_8() {
    let _lk = lock(&CFG_PARSE_LOCK);

    let source = r#"
(deftemplate home-row (version)
  a s d f g h
  (if-in-list $version (v0 v3 (concat v (((1)))) v4) (concat j))
  (if-in-list $version ((concat v 0) (concat v (2)) v3 v4) (tap-hold 200 200 (concat j) (if-in-list $version (v0 v3 v4 v2) (concat "k"))))
   k l ; '
)
(deftemplate var () (defvar num 200))
(defsrc
  (t! home-row v1)
)
(deflayer base
  (t! home-row v2)
)
(t! var)
(defalias a (tap-dance $num (a)))
"#;
    let mut s = ParserState::default();
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
        Err("env vars not implemented".into()),
    )
    .map_err(|e| {
        eprintln!("{:?}", miette::Error::from(e));
    })
    .expect("parses");
}

#[test]
fn test_deflayermap() {
    let source = r#"
(defsrc a b l)
(deflayermap (blah)
  d   (macro a b c)
  e   e
  f   0
  j   1
  k   2
  l   3
  m   4
  =   a
  a   =
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn test_defaliasenvcond() {
    let _lk = lock(&CFG_PARSE_LOCK);
    let source = r#"
(defsrc spc)
(deflayer base _)
(defaliasenvcond (ENV_TEST 1) a b)
"#;

    let env_var_err = "env vars not implemented";
    let mut s = ParserState::default();
    let parse_err = parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
        Err(env_var_err.into()),
    )
    .expect_err("should err");
    assert_eq!(parse_err.msg, env_var_err);

    // now test with env vars implemented

    let mut s = ParserState::default();
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
        Ok(vec![("ENV_TEST".into(), "1".into())]),
    )
    .map_err(|e| {
        eprintln!("{:?}", miette::Error::from(e));
        ""
    })
    .expect("parses");
    assert!(s.aliases["a"].key_codes().eq(vec![KeyCode::B]));

    // test env var set but to a different value

    let mut s = ParserState::default();
    parse_cfg_raw_string(
        source,
        &mut s,
        &PathBuf::from("test"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| unimplemented!(),
        },
        DEF_LOCAL_KEYS,
        Ok(vec![("ENV_TEST".into(), "asdf".into())]),
    )
    .map_err(|e| {
        eprintln!("{:?}", miette::Error::from(e));
        ""
    })
    .expect("parses");
    assert!(!s.aliases.contains_key("a"));
}

#[test]
fn parse_platform_specific() {
    let source = r#"
(platform () (invalid config but is not used anywhere))
(defsrc)
(deflayermap (base)
  a (layer-switch 2)
)
;; layer 2 must exist on all platforms, all in one list
(platform (win winiov2 wintercept linux macos)
  (deflayermap (2)
    a (layer-switch 3)
  )
)
;; layer 3 must exist on all platforms, in individual lists
;; Tests for no duplication.
(platform (win)
  (deflayermap (3)
    a (layer-switch 3)
  )
)
(platform (winiov2)
  (deflayermap (3)
    a (layer-switch 3)
  )
)
(platform (wintercept)
  (deflayermap (3)
    a (layer-switch 3)
  )
)
(platform (linux)
  (deflayermap (3)
    a (layer-switch 3)
  )
)
(platform (macos)
  (deflayermap (3)
    a (layer-switch 3)
  )
)
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_defseq_overlap_limits() {
    let source = r#"
(defsrc)
(deflayer base)
(defvirtualkeys v v)
(defseq v (O-(a b c d e f)))
(defseq v (O-(a b)))
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parses");
}

#[test]
fn parse_defseq_overlap_too_many() {
    let source = r#"
(defsrc)
(deflayer base)
(defvirtualkeys v v)
(defseq v (O-(a b c d e f g)))
"#;
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect_err("fails");
}

#[test]
fn parse_layer_opts_icon() {
    let _lk = lock(&CFG_PARSE_LOCK);
    new_from_file(&std::path::PathBuf::from("./test_cfgs/icon_good.kbd")).unwrap();
}

#[test]
fn disallow_dupe_layer_opts_icon_layernonmap() {
    let _lk = lock(&CFG_PARSE_LOCK);
    new_from_file(&std::path::PathBuf::from("./test_cfgs/icon_bad_dupe.kbd"))
        .map(|_| ())
        .expect_err("fails");
}

#[test]
fn disallow_dupe_layer_opts_icon_layermap() {
    let source = "
(defcfg)
(defsrc)
(deflayermap (base icon base.png 🖻 n.ico) 0 0)
";
    parse_cfg(source).map(|_| ()).expect_err("fails");
}

#[test]
fn layer_name_allows_var() {
    let source = "
(defvar l1name base)
(defvar l2name l2)
(defvar l3name (concat $l1name $l2name))
(defvar l4name (concat $l3name actually-4))
(defsrc a)
(deflayer $l1name (layer-while-held $l2name))
(deflayermap ($l2name)
  a (layer-while-held $l3name))
(deflayer ($l3name) (layer-while-held $l4name))
(deflayer ($l4name icon icon.ico) (layer-while-held $l1name))
";
    parse_cfg(source)
        .map_err(|e| eprintln!("{:?}", miette::Error::from(e)))
        .expect("parse succeeds");
}

#[test]
fn disallow_whitespace_in_tooltip_size() {
    let source = "
(defcfg
  tooltip-size 24 24	;; should be 24,24
)
(defsrc 1)
(deflayer test 1)
";
    parse_cfg(source).map(|_| ()).expect_err("fails");
}

#[test]
fn reverse_release_order_must_be_within_multi() {
    let source = "
(defsrc a)
(deflayer base reverse-release-order)
";
    let e = parse_cfg(source).map(|_| ()).expect_err("fails");
    assert_eq!(
        e.msg,
        "reverse-release-order is only allowed inside of a (multi ...) action list"
    );
}

#[test]
fn parse_clipboard_actions() {
    let source = "
(defsrc)
(deflayermap (base)
 a (clipboard-set       clip)
 b (clipboard-save      0)
 c (clipboard-restore   0)
 d (clipboard-save-swap 0 65535)
)
";
    parse_cfg(source).map(|_| ()).expect("success");
}
