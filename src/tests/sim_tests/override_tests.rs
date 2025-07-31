use super::*;

#[test]
fn override_with_unmod() {
    let result = simulate(
        "
(defoverrides
 (a) (b)
 (b) (a)
)

(defalias
 b (unshift b)
 a (unshift a)
)
(defsrc a b)
(deflayer base @a @b)
        ",
        "d:lsft t:50 d:a t:50 u:a t:50 d:b t:50 u:b t:50",
    )
    .to_ascii()
    .no_time();
    assert_eq!(
        "dn:LShift up:LShift dn:B up:B dn:LShift up:LShift dn:A up:A dn:LShift",
        result
    );
}

#[test]
fn override_release_mod_change_key() {
    let cfg = "
(defsrc)
(deflayer base)
(defoverrides
  (lsft a) (lsft 9)
  (lsft 1) (lctl 2))
        ";
    let result =
        simulate(cfg, "d:lsft t:10 d:a t:10 u:lsft t:10 u:a t:10").to_ascii();
    assert_eq!("dn:LShift t:10ms dn:Kb9 t:10ms up:LShift up:Kb9", result);
    let result =
        simulate(cfg, "d:lsft t:10 d:a t:10 u:a t:10 u:lsft t:10").to_ascii();
    assert_eq!(
        "dn:LShift t:10ms dn:Kb9 t:10ms up:Kb9 t:10ms up:LShift",
        result
    );
    let result = simulate(cfg, "d:lsft t:10 d:a t:10 d:c t:10").to_ascii();
    assert_eq!("dn:LShift t:10ms dn:Kb9 t:10ms up:Kb9 dn:C", result);
    let result = simulate(cfg, "d:lsft t:10 d:1 t:10 d:c t:10").to_ascii();
    assert_eq!(
        "dn:LShift t:10ms up:LShift dn:LCtrl dn:Kb2 t:10ms up:LCtrl up:Kb2 dn:LShift dn:C",
        result
    );
}

#[test]
fn override_eagerly_releases() {
    let result = simulate(
        "
(defcfg override-release-on-activation yes)
(defsrc)
(deflayer base)
(defoverrides (lsft a) (lsft 9))
        ",
        "d:lsft t:10 d:a t:10 u:lsft t:10 u:a t:10",
    )
    .to_ascii();
    assert_eq!(
        "dn:LShift t:10ms dn:Kb9 t:1ms up:Kb9 t:9ms up:LShift",
        result
    );
}
