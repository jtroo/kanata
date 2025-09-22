use super::*;

#[test]
fn unmod_keys_functionality_works() {
    let result = simulate(
        "
         (defcfg)
         (defsrc f1 1 2 3 4 5 6 7 8 9 0)
         (deflayer base
             (multi lctl rctl lsft rsft lmet rmet lalt ralt)
             (unmod a)
             (unmod (lctl) b)
             (unmod (rctl) c)
             (unmod (lsft) d)
             (unmod (rsft) e)
             (unmod (lmet) f)
             (unmod (rmet) g)
             (unmod (lalt) h)
             (unmod (ralt) i)
             (unmod (lctl lsft lmet lalt) j)
         )
        ",
        "d:f1 t:5 d:1 u:1 t:5 d:2 u:2 t:5 d:3 u:3 t:5 d:4 u:4 t:5 d:5 u:5 t:5 d:6 u:6 t:5
                  d:7 u:7 t:5 d:8 u:8 t:5 d:9 u:9 t:5 d:0 u:0 t:5",
    )
    .no_time()
    .to_ascii();
    assert_eq!(
        "dn:LCtrl dn:RCtrl dn:LShift dn:RShift dn:LGui dn:RGui dn:LAlt dn:RAlt \
         up:LCtrl up:RCtrl up:LShift up:RShift up:LGui up:RGui up:LAlt up:RAlt dn:A up:A \
         dn:LCtrl dn:RCtrl dn:LShift dn:RShift dn:LGui dn:RGui dn:LAlt dn:RAlt \
         up:LCtrl dn:B up:B dn:LCtrl \
         up:RCtrl dn:C up:C dn:RCtrl \
         up:LShift dn:D up:D dn:LShift \
         up:RShift dn:E up:E dn:RShift \
         up:LGui dn:F up:F dn:LGui \
         up:RGui dn:G up:G dn:RGui \
         up:LAlt dn:H up:H dn:LAlt \
         up:RAlt dn:I up:I dn:RAlt \
         up:LCtrl up:LShift up:LGui up:LAlt dn:J up:J dn:LCtrl dn:LShift dn:LGui dn:LAlt",
        result
    );
}

#[test]
#[should_panic]
fn unmod_keys_mod_list_cannot_be_empty() {
    simulate(
        "
         (defcfg)
         (defsrc a)
         (deflayer base (unmod () a))
        ",
        "",
    );
}

#[test]
#[should_panic]
fn unmod_keys_mod_list_cannot_have_nonmod_key() {
    simulate(
        "
         (defcfg)
         (defsrc a)
         (deflayer base (unmod (lmet c) a))
        ",
        "",
    );
}

#[test]
#[should_panic]
fn unmod_keys_mod_list_cannot_have_empty_keys_after_mod_list() {
    simulate(
        "
         (defcfg)
         (defsrc a)
         (deflayer base (unmod (lmet)))
        ",
        "",
    );
}

#[test]
#[should_panic]
fn unmod_keys_mod_list_cannot_have_empty_keys() {
    simulate(
        "
         (defcfg)
         (defsrc a)
         (deflayer base (unmod))
        ",
        "",
    );
}

#[test]
#[should_panic]
fn unmod_keys_mod_list_cannot_have_invalid_keys() {
    simulate(
        "
         (defcfg)
         (defsrc a)
         (deflayer base (unmod invalid-key))
        ",
        "",
    );
}
