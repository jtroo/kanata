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
    );
    assert_eq!(
        "t:2ms\nout:↓LShift\nt:48ms\nout:↓A\nt:1ms\nout:↓B\nout:↑LShift\nout:↑A\nout:↑B\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nt:1ms\nout:↑LShift\nout:↑A\nout:↑B\nout:↓Z\nt:1ms\nout:↑Z\nt:549ms\nout:↓RShift\nt:48ms\nout:↓A\nt:1ms\nout:↓B\nout:↑RShift\nout:↑A\nout:↑B\nout:↓BSpace\nout:↑BSpace\nout:↓BSpace\nout:↑BSpace\nt:1ms\nout:↑RShift\nout:↑A\nout:↑B\nout:↓Z\nt:1ms\nout:↑Z\nt:549ms\nout:↓RShift\nt:48ms\nout:↓A\nt:1ms\nout:↑RShift\nt:49ms\nout:↓B\nt:1ms\nout:↑A\nt:1ms\nout:↑B",
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
