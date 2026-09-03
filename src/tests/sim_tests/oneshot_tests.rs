use super::*;

#[test]
fn oneshot_pause() {
    let result = simulate(
        "
(defsrc a lmet rmet)
(deflayer base
  1 @lme @rme)
(deflayer numbers
  2 @lme @rme)
(deflayer navigation
  (one-shot 2000 lalt) @lme @rme)
(deflayer symbols
  4 @lme @rme)

(defvirtualkeys
  callum (switch
    ((and nop1 nop2)) (layer-while-held numbers) break
    (nop1) (layer-while-held navigation) break
    (nop2) (layer-while-held symbols) break)
  activate-callum (multi
   (one-shot-pause-processing 5)
   (switch
    ((or nop1 nop2))
     (multi (on-press release-vkey callum)
            (on-press press-vkey callum))
     break
    () (on-press release-vkey callum) break)))

(defalias
  lme (multi nop1
             (on-press tap-vkey activate-callum)
             (on-release tap-vkey activate-callum))
  rme (multi nop2
             (on-press tap-vkey activate-callum)
             (on-release tap-vkey activate-callum)))
        ",
        "d:lmet t:10 d:a u:a t:10 u:lmet t:10 d:a u:a t:10",
    )
    .to_ascii();
    assert_eq!("t:10ms dn:LAlt t:20ms dn:Kb1 t:5ms up:LAlt up:Kb1", result);
}

#[test]
fn oneshot_multi_with_chord() {
    let result = simulate(
        "(defsrc a)
         (deflayer test (multi a (one-shot 1000 C-z)))",
        "d:KeyA t:10 u:KeyA t:200 d:KeyC t:1000",
    )
    .to_ascii();
    assert_eq!(
        "dn:A dn:LCtrl dn:Z t:10ms up:A t:200ms dn:C t:5ms up:LCtrl up:Z",
        result
    );
}

#[test]
fn oneshot_multi_with_layer() {
    let result = simulate(
        "(defsrc a)
         (deflayer l1 (multi a (one-shot 100 (layer-while-held l2))))
         (deflayer l2 b)
        ",
        "d:KeyA t:10 u:KeyA t:10 d:KeyA t:10 u:KeyA t:1000",
    )
    .to_ascii();
    // Known bug:
    // The 5ms should be 10ms.
    // The B is released (with instant action delay) even when it shouldn't
    // because one-shot completion is releasing everything
    // that is on the same coordinate as it.
    //   issue: -------------------------------.
    //                                         v
    assert_eq!("dn:A t:10ms up:A t:10ms dn:B t:5ms up:B", result);
}

#[test]
fn oneshot_release_activator_repressed_on_other_layer() {
    // Regression test for #2049.
    // `tab` momentarily activates the `nav` layer where `d` is a
    // `one-shot-release lalt`. After leaving `nav`, re-pressing the same
    // physical `d` key (now a normal key on `base`) and releasing it must
    // terminate the one-shot. Previously LAlt leaked onto every subsequent
    // key (here `p`) until the 2000ms timeout.
    let result = simulate(
        "(defsrc tab d p)
         (deflayer base (layer-while-held nav) d p)
         (deflayer nav _ (one-shot-release 2000 lalt) _)",
        "d:Tab t:10 d:KeyD t:10 u:KeyD t:10 u:Tab t:10 \
         d:KeyD t:10 u:KeyD t:10 d:KeyP t:10 u:KeyP t:10",
    )
    .to_ascii();
    assert_eq!(
        "t:10ms dn:LAlt t:30ms dn:D t:10ms up:LAlt up:D t:10ms dn:P t:10ms up:P",
        result
    );
}
