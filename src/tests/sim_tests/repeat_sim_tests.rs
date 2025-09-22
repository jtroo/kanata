use super::*;

#[test]
fn repeat_standard() {
    let result = simulate(
        "
         (defsrc a)
         (deflayer base b)
        ",
        "
         d:a t:10 r:a t:10 r:a t:10 u:a t:10 r:a
        ",
    );
    assert_eq!(
        "out:↓B\nt:10ms\nout:↓B\nt:10ms\nout:↓B\nt:10ms\nout:↑B",
        result
    );
}

#[test]
fn repeat_layer_while_held() {
    let result = simulate(
        "
         (defsrc a b)
         (deflayer base a (layer-while-held held))
         (deflayer held c b)
        ",
        "
         d:b t:10 r:b t:10 d:a t:10 r:a t:10 r:a t:10 u:a t:10 r:a
        ",
    );
    assert_eq!(
        "t:20ms\nout:↓C\nt:10ms\nout:↓C\nt:10ms\nout:↓C\nt:10ms\nout:↑C",
        result
    );
}

#[test]
fn repeat_layer_switch() {
    let result = simulate(
        "
         (defsrc a b)
         (deflayer base a (layer-switch swtc))
         (deflayer swtc d b)
        ",
        "
         d:b t:10 r:b t:10 d:a t:10 r:a t:10 r:a t:10 u:a t:10 r:a
        ",
    );
    assert_eq!(
        "t:20ms\nout:↓D\nt:10ms\nout:↓D\nt:10ms\nout:↓D\nt:10ms\nout:↑D",
        result
    );
}

#[test]
fn repeat_layer_held_trans() {
    let result = simulate(
        "
         (defsrc a b)
         (deflayer base e (layer-while-held held))
         (deflayer held _ b)
        ",
        "
         d:b t:10 r:b t:10 d:a t:10 r:a t:10 r:a t:10 u:a t:10 r:a
        ",
    );
    assert_eq!(
        "t:20ms\nout:↓E\nt:10ms\nout:↓E\nt:10ms\nout:↓E\nt:10ms\nout:↑E",
        result
    );
}

#[test]
fn repeat_many_layer_held_trans() {
    let result = simulate(
        "
         (defsrc a b c d e)
         (deflayer base e (layer-while-held held1) _ _ _)
         (deflayer held1 f b (layer-while-held held2) _ _)
         (deflayer held2 _ _ _ (layer-while-held held3) _)
         (deflayer held3 _ _ _ _ (layer-while-held held4))
         (deflayer held4 _ _ _ _ _)
        ",
        "
         d:b t:10 r:b t:10
         d:c t:10 r:c t:10
         d:d t:10 r:d t:10
         d:e t:10 r:e t:10
         d:a t:10 r:a t:10 r:a t:10 u:a t:10 r:a
        ",
    );
    assert_eq!(
        "t:80ms\nout:↓F\nt:10ms\nout:↓F\nt:10ms\nout:↓F\nt:10ms\nout:↑F",
        result
    );
}

#[test]
fn repeat_base_layer_trans() {
    let result = simulate(
        "
         (defsrc a)
         (deflayer base _)
        ",
        "
         d:a t:10 r:a t:10 r:a t:10 u:a t:10 r:a
        ",
    );
    assert_eq!(
        "out:↓A\nt:10ms\nout:↓A\nt:10ms\nout:↓A\nt:10ms\nout:↑A",
        result
    );
}

#[test]
fn repeat_delegate_to_base_layer_trans() {
    let result = simulate(
        "
         (defcfg delegate-to-first-layer yes)
         (defsrc a c b)
         (deflayer base e _ (layer-switch swtc))
         (deflayer swtc _ _ _)
        ",
        "
         d:b t:10 r:b t:10
         d:a t:10 r:a t:10 r:a t:10 u:a t:10 r:a
         d:c t:10 r:c t:10 r:c t:10 u:c t:10 r:c
        ",
    );
    assert_eq!(
        "t:20ms\nout:↓E\nt:10ms\nout:↓E\nt:10ms\nout:↓E\nt:10ms\nout:↑E\n\
         t:10ms\nout:↓C\nt:10ms\nout:↓C\nt:10ms\nout:↓C\nt:10ms\nout:↑C",
        result
    );
}
