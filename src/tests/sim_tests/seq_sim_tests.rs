use super::*;

#[test]
fn special_nop_keys() {
    let result = simulate(
        "(defcfg sequence-input-mode visible-backspaced)
         (defsrc a b c d e)
         (deflayer base sldr nop0 c nop9 0)
         (defvirtualkeys s1 (macro h i))
         (defseq s1 (nop0 c nop9))
        ",
        "d:b d:d t:50 u:b u:d t:50
         d:a d:b d:c t:50 u:a u:b u:c t:50 d:d t:50",
    );
    assert_eq!(
        "t:102ms\nout:↓C\nt:50ms\nout:↑C\nt:48ms\n\
        out:↓BSpace\nout:↑BSpace\n\
        t:2ms\nout:↓H\nt:1ms\nout:↑H\nt:1ms\nout:↓I\nt:1ms\nout:↑I",
        result
    );
}

#[test]
fn chorded_keys_visible_backspaced() {
    let result = simulate(
        "(defcfg sequence-input-mode visible-backspaced)
         (defsrc 0)
         (deflayer base sldr)
         (defvirtualkeys s1 z)
         (defseq s1 (S-(a b)))
        ",
        "d:0 u:0 d:lsft t:50 d:a d:b t:50 u:lsft u:a u:b t:500
         d:0 u:0 d:rsft t:50 d:a d:b t:50 u:rsft u:a u:b t:500
         d:0 u:0 d:rsft t:50 d:a u:rsft t:50 d:b u:a u:b t:500",
    )
    .no_time()
    .to_ascii();
    assert_eq!(
        // 2nd row is buggy/unexpected! RShift isn't released before outputting Z
        // Workarounds:
        // - remap your rsft key to lsft
        // - use release-key in the macro via virtual keys
        // - accept and use the quirky behaviour; maybe it's what you wanted?
        "dn:LShift dn:A dn:B dn:BSpace up:BSpace dn:BSpace up:BSpace up:LShift up:A up:B dn:Z up:Z \
         dn:RShift dn:A dn:B dn:BSpace up:BSpace dn:BSpace up:BSpace up:A up:B dn:Z up:Z up:RShift \
         dn:RShift dn:A up:RShift dn:B up:A up:B",
        result
    );
}

const OVERLAP_CFG: &str = "
    (defcfg sequence-input-mode visible-backspaced)
    (defsrc 0)
    (deflayer base sldr)
    (defvirtualkeys s1 y)
    (defvirtualkeys s2 z)
    (defvirtualkeys s3 l)
    (defvirtualkeys s4 m)
    (defvirtualkeys s5 n)
    (defvirtualkeys s6 o)
    (defvirtualkeys s7 p)
    (defvirtualkeys s8 q)
    (defseq s1 (O-(a b)))
    (defseq s2 (a b))
    (defseq s3 (O-(c d) e))
    (defseq s4 (c d e))
    (defseq s5 (O-(c d) O-(f g)))
    (defseq s6 (O-(c d) f g))
    (defseq s7 (c d O-(f g)))
    ;; (defseq s8 (c d f g)) KNOWN BUGGY CASE! breaks s6 detection
    ";

#[test]
fn overlapping_activate_overlap() {
    let result = simulate(OVERLAP_CFG, "d:0 d:a d:b t:100 u:a u:b u:0");
    assert_eq!(
        "t:1ms\nout:↓A\nt:1ms\nout:↓B\n\
         out:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nt:1ms\nout:↑A\nout:↑B\n\
         out:↓Y\nt:1ms\nout:↑Y",
        result
    );
}

#[test]
fn overlapping_activate_nonoverlap() {
    let result = simulate(OVERLAP_CFG, "d:0 d:a t:10 u:a t:10 d:b t:10 u:b t:10 u:0");
    assert_eq!(
        "t:1ms\nout:↓A\nt:9ms\nout:↑A\nt:10ms\nout:↓B\n\
        out:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\n\
        t:1ms\nout:↑B\nout:↓Z\nt:1ms\nout:↑Z",
        result
    );
}

#[test]
fn overlapping_then_nonoverlap_activate_overlap() {
    let result = simulate(OVERLAP_CFG, "d:0 d:c d:d d:e t:100 u:c u:d u:e u:0");
    assert_eq!(
        "t:1ms\nout:↓C\nt:1ms\nout:↓D\nt:1ms\nout:↓E\n\
         out:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\n\
         t:1ms\nout:↑C\nout:↑D\nout:↑E\nout:↓L\nt:1ms\nout:↑L",
        result
    );
}

#[test]
fn overlapping_then_nonoverlap_activate_non_overlap() {
    let result = simulate(OVERLAP_CFG, "d:0 d:c u:c d:d d:e t:100 u:d u:e u:0");
    assert_eq!(
        "t:1ms\nout:↓C\nt:1ms\nout:↑C\nt:1ms\nout:↓D\nt:1ms\nout:↓E\n\
         out:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\n\
         t:1ms\nout:↑D\nout:↑E\nout:↓M\nt:1ms\nout:↑M",
        result
    );
}

#[test]
fn overlapping_then_overlap_activate_overlap1() {
    let result = simulate(OVERLAP_CFG, "d:0 d:c d:d d:f d:g t:100");
    assert_eq!(
        "t:1ms\nout:↓C\nt:1ms\nout:↓D\nt:1ms\nout:↓F\nt:1ms\nout:↓G\n\
         out:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\n\
         t:1ms\nout:↑C\nout:↑D\nout:↑F\nout:↑G\nout:↓N\nt:1ms\nout:↑N",
        result
    );
}

#[test]
fn overlapping_then_overlap_activate_overlap2() {
    let result = simulate(OVERLAP_CFG, "d:0 d:c d:d u:c u:d d:f d:g t:100");
    assert_eq!(
        "t:1ms\nout:↓C\nt:1ms\nout:↓D\nt:1ms\nout:↑C\nt:1ms\nout:↑D\nt:1ms\nout:↓F\nt:1ms\nout:↓G\n\
         out:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\n\
         t:1ms\nout:↑F\nout:↑G\nout:↓N\nt:1ms\nout:↑N",
        result
    );
}

#[test]
fn overlapping_then_overlap_activate_overlap3() {
    let result = simulate(OVERLAP_CFG, "d:0 d:c d:d u:c u:d t:10 d:f d:g t:100");
    assert_eq!(
        "t:1ms\nout:↓C\nt:1ms\nout:↓D\nt:1ms\nout:↑C\nt:1ms\nout:↑D\nt:6ms\nout:↓F\nt:1ms\nout:↓G\n\
         out:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\n\
         t:1ms\nout:↑F\nout:↑G\nout:↓N\nt:1ms\nout:↑N",
        result
    );
}

#[test]
fn overlapping_then_overlap_activate_nonoverlap() {
    let result = simulate(OVERLAP_CFG, "d:0 d:c d:d u:c u:d t:10 d:f u:f d:g t:100");
    assert_eq!(
        "t:1ms\nout:↓C\nt:1ms\nout:↓D\nt:1ms\nout:↑C\nt:1ms\nout:↑D\nt:6ms\nout:↓F\nt:1ms\nout:↑F\nt:1ms\nout:↓G\n\
         out:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\n\
         t:1ms\nout:↑G\nout:↓O\nt:1ms\nout:↑O",
        result
    );
}

#[test]
fn non_overlapping_then_overlap_activate_overlap() {
    let result = simulate(OVERLAP_CFG, "d:0 d:c u:c d:d u:d d:f d:g t:100");
    assert_eq!(
        "t:1ms\nout:↓C\nt:1ms\nout:↑C\nt:1ms\nout:↓D\nt:1ms\nout:↑D\nt:1ms\nout:↓F\nt:1ms\nout:↓G\n\
         out:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\n\
         t:1ms\nout:↑F\nout:↑G\nout:↓P\nt:1ms\nout:↑P",
        result
    );
}

#[test]
fn non_overlapping_then_overlap_activate_nothing() {
    let result = simulate(OVERLAP_CFG, "d:0 d:c u:c d:d u:d d:f u:f d:g t:100");
    assert_eq!(
        "t:1ms\nout:↓C\nt:1ms\nout:↑C\nt:1ms\nout:↓D\nt:1ms\nout:↑D\nt:1ms\nout:↓F\nt:1ms\nout:↑F\nt:1ms\nout:↓G",
        result
    );
}

/* BUG: chorded_hidden_delay_type
 *
 * Enable this test when fixing.
 *
 * Backtracking currently destroys information about held modifiers before finally outputting the
 * invalid sequence characters. There is also no logic to keep modifier keys held for the
 * appropriate duration according to the modifier bits information, even if the information was
 * preserved. Seems like a complicated low-value edge-case bug to fix so for now will just document
 * it... Nobody has reported it yet anyway. And visible-backspaced seems preferable in most cases
 * anyway.
#[test]
fn chorded_keys_hidden_delaytype() {
    let result = simulate(
        "(defcfg sequence-input-mode hidden-delay-type)
         (defsrc 0)
         (deflayer base sldr)
         (defvirtualkeys s1 z)
         (defseq s1 (S-(a b)))
        ",
        "d:0 u:0 d:lsft t:50 d:a d:b t:50 u:lsft u:a u:b t:500
         d:0 u:0 d:rsft t:50 d:a d:b t:50 u:rsft u:a u:b t:500
         d:0 u:0 d:rsft t:50 d:a u:rsft t:50 d:b u:a u:b t:500",
    );
    assert_eq!(
        "",
        result
    );
}
*/

#[test]
fn noerase() {
    let result = simulate(
        "(defcfg sequence-input-mode visible-backspaced)
         (defsrc)
         (deflayermap (base)
           0 sldr
           u (t! maybe-noerase u)
         )
         (deftemplate maybe-noerase (char)
            (multi
             (switch
              ((key-history ' 1)) (sequence-noerase 1) fallthrough
              () $char break
            ))
         )
         (defvirtualkeys s1 z)
         (defseq s1 (' u))
        ",
        "d:0 u:0 d:' t:50 d:u t:500",
    )
    .no_time()
    .to_ascii();
    assert_eq!(
        "dn:Quote dn:U dn:BSpace up:BSpace up:Quote up:U dn:Z up:Z",
        result,
    );
}

#[test]
fn tap_hold_pending() {
    let result = simulate(
        "
(defalias md     (tap-hold 200 200 s S-s))
(defsrc s d j)
(deflayer base @md d sldr)
(deffakekeys _u  (unicode μ))
(defseq _u     (s))
        ",
        "
d:KeyJ t:10 u:KeyJ t:10
d:KeyS t:10 u:KeyS t:10 d:KeyD t:10 u:KeyD t:10

d:KeyJ t:10 u:KeyJ t:10
d:KeyS t:10 d:KeyD t:10 u:KeyS t:10 u:KeyD t:10",
    )
    .no_time()
    .no_releases()
    .to_ascii();
    assert_eq!("outU:μ dn:D outU:μ dn:D", result,);
}

#[cfg(feature = "tcp_server")]
mod tcp_seq_state {
    use super::*;
    use kanata_tcp_protocol::ServerMessage;

    const CFG: &str = "
        (defcfg danger-tcp-allow-keylogging yes sequence-timeout 100)
        (defsrc 0 1)
        (deflayer base sldr scnl)
        (defvirtualkeys s1 z)
        (defvirtualkeys s2 y)
        (defvirtualkeys s3 x)
        (defvirtualkeys s4 w)
        (defseq s1 (g r) s2 (g t) s3 (S-(a b)) s4 (c d))
    ";

    /// The sequence notifications that kanata sends for the simulated input,
    /// as `Phase [prefix] [candidates]`.
    fn seq_state_messages(cfg: &str, sim: &str) -> Vec<String> {
        let (tx, rx) = std::sync::mpsc::sync_channel(100);
        simulate_notifying(cfg, sim, Default::default(), &Some(tx));
        rx.try_iter()
            .map(|message| match message {
                ServerMessage::SeqState {
                    prefix,
                    candidates,
                    phase,
                } => format!("{phase:?} [{prefix}] {candidates:?}"),
                other => panic!("unexpected message: {other:?}"),
            })
            .collect()
    }

    const ALL: &str = r#"["S-a S-b", "c d", "g r", "g t"]"#;

    #[test]
    fn started_advanced_completed() {
        assert_eq!(
            [
                format!("Started [] {ALL}"),
                r#"Advanced [g] ["g r", "g t"]"#.to_string(),
                "Completed [g r] []".to_string(),
            ],
            seq_state_messages(CFG, "d:0 u:0 t:10 d:g u:g t:10 d:r u:r t:10").as_slice(),
        );
    }

    #[test]
    fn invalid_key_cancels() {
        assert_eq!(
            [format!("Started [] {ALL}"), "Cancelled [] []".to_string()],
            seq_state_messages(CFG, "d:0 u:0 t:10 d:x u:x t:10").as_slice(),
        );
    }

    #[test]
    fn timeout_keeps_the_typed_prefix() {
        assert_eq!(
            [
                format!("Started [] {ALL}"),
                r#"Advanced [g] ["g r", "g t"]"#.to_string(),
                "TimedOut [g] []".to_string(),
            ],
            seq_state_messages(CFG, "d:0 u:0 t:10 d:g u:g t:200").as_slice(),
        );
    }

    #[test]
    fn cancel_action_and_rearming() {
        assert_eq!(
            [
                format!("Started [] {ALL}"),
                r#"Advanced [g] ["g r", "g t"]"#.to_string(),
                format!("Started [] {ALL}"),
                "Cancelled [] []".to_string(),
                format!("Started [] {ALL}"),
                r#"Advanced [g] ["g r", "g t"]"#.to_string(),
                "Cancelled [g] []".to_string(),
            ],
            seq_state_messages(
                CFG,
                "d:0 u:0 t:10 d:g u:g t:10 d:0 u:0 t:10 d:1 u:1 t:10 \
                 d:0 u:0 t:10 d:g u:g t:10 d:1 u:1 t:10"
            )
            .as_slice(),
        );
    }

    #[test]
    fn chorded_keys_are_written_with_modifier_prefixes() {
        assert_eq!(
            [
                format!("Started [] {ALL}"),
                r#"Advanced [S-a] ["S-a S-b"]"#.to_string(),
                "Completed [S-a S-b] []".to_string(),
            ],
            seq_state_messages(
                CFG,
                "d:0 u:0 t:10 d:lsft t:10 d:a t:10 d:b t:10 u:lsft u:a u:b t:10"
            )
            .as_slice(),
        );
    }

    #[test]
    fn nothing_is_sent_without_allowing_keylogging() {
        let cfg = CFG.replace("danger-tcp-allow-keylogging yes", "");
        assert!(seq_state_messages(&cfg, "d:0 u:0 t:10 d:g u:g t:10 d:r u:r t:10").is_empty());
    }
}
