use super::*;

const CFG: &str = r##"
 (defcfg)
 (defsrc 7 8 9 0)
 (deflayer base
     (caps-word 1000)
     (caps-word-custom 200 (a) (b))
     (caps-word-toggle 1000)
     (caps-word-custom-toggle 200 (a) (b))
 )
"##;

#[test]
fn caps_word_behaves_correctly() {
    let result = simulate(
        CFG,
        "d:7 u:7 d:a u:a d:1 u:1 d:a u:a d:spc u:spc d:a u:a t:1000",
    )
    .no_time();
    assert_eq!(
        "out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓Kb1 out:↑Kb1 out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓Space out:↑Space out:↓A out:↑A",
        result
    );
}

#[test]
fn caps_word_custom_behaves_correctly() {
    let result = simulate(
        CFG,
        "d:8 u:8 d:a u:a d:b u:b d:a u:a d:1 u:1 d:a u:a t:1000",
    )
    .no_time();
    assert_eq!(
        "out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓B out:↑B out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓Kb1 out:↑Kb1 out:↓A out:↑A",
        result
    );
}

#[test]
fn caps_word_times_out() {
    let result =
        simulate(CFG, "d:7 u:7 d:a u:a t:500 d:a u:a t:1001 d:a u:a t:10")
            .no_time();
    assert_eq!(
        "out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓A out:↑A",
        result
    );
}

#[test]
fn caps_word_custom_times_out() {
    let result =
        simulate(CFG, "d:8 u:8 d:a u:a t:100 d:a u:a t:201 d:a u:a t:10")
            .no_time();
    assert_eq!(
        "out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓A out:↑A",
        result
    );
}

#[test]
fn caps_word_does_not_toggle() {
    let result =
        simulate(CFG, "d:7 u:7 d:a u:a t:100 d:7 u:7 t:100 d:a u:a t:10")
            .no_time();
    assert_eq!(
        "out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓LShift out:↓A out:↑LShift out:↑A",
        result
    );
}

#[test]
fn caps_word_custom_does_not_toggle() {
    let result =
        simulate(CFG, "d:8 u:8 d:a u:a t:100 d:8 u:8 t:100 d:a u:a t:10")
            .no_time();
    assert_eq!(
        "out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓LShift out:↓A out:↑LShift out:↑A",
        result
    );
}

#[test]
fn caps_word_toggle_does_toggle() {
    let result =
        simulate(CFG, "d:9 u:9 d:a u:a t:100 d:9 u:9 t:100 d:a u:a t:10")
            .no_time();
    assert_eq!(
        "out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓A out:↑A",
        result
    );
}

#[test]
fn caps_word_custom_toggle_does_toggle() {
    let result =
        simulate(CFG, "d:0 u:0 d:a u:a t:100 d:0 u:0 t:100 d:a u:a t:10")
            .no_time();
    assert_eq!(
        "out:↓LShift out:↓A out:↑LShift out:↑A \
         out:↓A out:↑A",
        result
    );
}
