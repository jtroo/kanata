use super::*;

#[test]
fn block_does_not_block_buttons() {
    let result = simulate(
        "(defcfg process-unmapped-keys yes
                   block-unmapped-keys yes)
        (defsrc)
        (deflayer base)",
        "d:mlft d:mrgt d:mmid d:mbck d:mfwd t:10 d:f1
         u:mlft u:mrgt u:mmid u:mbck u:mfwd t:10 u:f1",
    );
    assert_eq!(
        "out🖰:↓Left\nt:1ms\nout🖰:↓Right\nt:1ms\nout🖰:↓Mid\nt:1ms\nout🖰:↓Backward\n\
               t:1ms\nout🖰:↓Forward\nt:7ms\nout🖰:↑Left\nt:1ms\nout🖰:↑Right\nt:1ms\nout🖰:↑Mid\n\
               t:1ms\nout🖰:↑Backward\nt:1ms\nout🖰:↑Forward",
        result
    );
}

#[test]
fn block_does_not_block_wheel() {
    let result = simulate(
        "(defcfg process-unmapped-keys yes
                   block-unmapped-keys yes)
        (defsrc)
        (deflayer base)",
        "d:mwu d:mwd d:mwl d:mwr t:10 d:f1
         u:mwu u:mwd u:mwl u:mwr t:10 u:f1",
    );
    assert_eq!(
        "scroll:Up,120\nt:1ms\nscroll:Down,120\nt:1ms\nscroll:Left,120\nt:1ms\nscroll:Right,120",
        result
    );
}
