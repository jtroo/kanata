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
