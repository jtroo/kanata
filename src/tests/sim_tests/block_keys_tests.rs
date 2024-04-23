use super::*;

#[test]
fn block_does_not_block_buttons() {
    let result = simulate(
        "(defcfg process-unmapped-keys yes
                   block-unmapped-keys yes)
        (defsrc)
        (deflayer base)",
        "d:mlft u:mlft t:10 d:f1 u:f1 t:10",
    );
    assert_eq!("out🖰:↓Left\nt:1ms\nout🖰:↑Left", result);
}
