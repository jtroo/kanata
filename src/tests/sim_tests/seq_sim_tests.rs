use super::*;

#[test]
fn special_seq_keys() {
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
