use super::*;

#[test]
fn macro_cancel_on_press() {
    let cfg = "\
(defsrc a b c)
(deflayer base (macro-cancel-on-press z 100 y) (macro x 100 w) c)";
    test_on_press(cfg);
    let cfg = "\
(defsrc a b c)
(deflayer base (macro-repeat-cancel-on-press z 100 y 100) (macro x 100 w) c)";
    test_on_press(cfg);
}

fn test_on_press(cfg: &str) {
    // Cancellation should happen.
    let result = simulate(cfg, "d:a t:50 d:c t:100").to_ascii();
    assert_eq!("t:1ms dn:Z t:1ms up:Z t:48ms dn:C", result);
    // Macro should complete if allowed to.
    let result = simulate(cfg, "d:a u:a t:150 d:c t:100").to_ascii();
    assert_eq!(
        "t:1ms dn:Z t:1ms up:Z t:101ms dn:Y t:1ms up:Y t:46ms dn:C",
        result
    );
    // The window for macro cancellation should not persist to a new macro that is not cancellable.
    let result = simulate(cfg, "d:a t:120 d:b t:20 d:c t:100").to_ascii();
    assert_eq!(
        "t:1ms dn:Z t:1ms up:Z t:101ms dn:Y t:1ms up:Y \
                t:17ms dn:X t:1ms up:X t:18ms dn:C t:83ms dn:W t:1ms up:W",
        result
    );
    let result =
        simulate(cfg, "d:a t:10 d:c u:c t:10 d:b t:20 d:c t:100").to_ascii();
    assert_eq!(
        "t:1ms dn:Z t:1ms up:Z t:8ms dn:C t:1ms up:C t:10ms \
                dn:X t:1ms up:X t:18ms dn:C t:83ms dn:W t:1ms up:W",
        result
    );
}

#[test]
fn macro_release_cancel_and_cancel_on_press() {
    let cfg = "\
(defsrc a b c)
(deflayer base (macro-release-cancel-and-cancel-on-press z 100 y 100) (macro x 100 w) c)";
    test_release_and_on_press(cfg);
    let cfg = "\
(defsrc a b c)
(deflayer base (macro-repeat-release-cancel-and-cancel-on-press z 100 y 100) (macro x 100 w) c)";
    test_release_and_on_press(cfg);
}

fn test_release_and_on_press(cfg: &str) {
    // Cancellation should happen for press.
    let result = simulate(cfg, "d:a t:50 d:c t:100").to_ascii();
    assert_eq!("t:1ms dn:Z t:1ms up:Z t:48ms dn:C", result);
    // Cancellation should happen for release
    let result = simulate(cfg, "d:a u:a t:150 d:c t:100").to_ascii();
    assert_eq!("t:1ms dn:Z t:1ms up:Z t:148ms dn:C", result);
    // Macro should complete if allowed to.
    let result = simulate(cfg, "d:a t:150 d:c t:100").to_ascii();
    assert_eq!(
        "t:1ms dn:Z t:1ms up:Z t:101ms dn:Y t:1ms up:Y t:46ms dn:C",
        result
    );
    // The window for macro cancellation should not persist to a new macro that is not cancellable.
    let result = simulate(cfg, "d:a t:120 d:b t:20 d:c t:100").to_ascii();
    assert_eq!(
        "t:1ms dn:Z t:1ms up:Z t:101ms dn:Y t:1ms up:Y \
                t:17ms dn:X t:1ms up:X t:18ms dn:C t:83ms dn:W t:1ms up:W",
        result
    );
    let result =
        simulate(cfg, "d:a t:10 d:c u:c t:10 d:b t:20 d:c t:100").to_ascii();
    assert_eq!(
        "t:1ms dn:Z t:1ms up:Z t:8ms dn:C t:1ms up:C t:10ms \
                dn:X t:1ms up:X t:18ms dn:C t:83ms dn:W t:1ms up:W",
        result
    );
    let result =
        simulate(cfg, "d:a u:a t:10 t:10 d:b u:b t:20 d:c t:100").to_ascii();
    assert_eq!(
        "t:1ms dn:Z t:1ms up:Z t:19ms \
         dn:X t:1ms up:X t:18ms dn:C t:83ms dn:W t:1ms up:W",
        result
    );
}

#[test]
fn macro_repeat() {
    let cfg = "\
(defsrc a b c d)
(deflayer base
 (macro-repeat Digit1 50)
 (macro-repeat-release-cancel Digit1 50)
 (macro-repeat-cancel-on-press Digit1 50)
 (macro-repeat-release-cancel-and-cancel-on-press Digit1 50))";
    let result = simulate(cfg, "d:a t:125 u:a").to_ascii();
    assert_eq!(
        "t:1ms dn:Kb1 t:1ms up:Kb1 t:52ms dn:Kb1 t:1ms up:Kb1 t:52ms dn:Kb1 t:1ms up:Kb1",
        result
    );
    let result = simulate(cfg, "d:b t:125 u:b").to_ascii();
    assert_eq!(
        "t:1ms dn:Kb1 t:1ms up:Kb1 t:52ms dn:Kb1 t:1ms up:Kb1 t:52ms dn:Kb1 t:1ms up:Kb1",
        result
    );
    let result = simulate(cfg, "d:c t:125 u:c").to_ascii();
    assert_eq!(
        "t:1ms dn:Kb1 t:1ms up:Kb1 t:52ms dn:Kb1 t:1ms up:Kb1 t:52ms dn:Kb1 t:1ms up:Kb1",
        result
    );
    let result = simulate(cfg, "d:d t:125 u:d").to_ascii();
    assert_eq!(
        "t:1ms dn:Kb1 t:1ms up:Kb1 t:52ms dn:Kb1 t:1ms up:Kb1 t:52ms dn:Kb1 t:1ms up:Kb1",
        result
    );
}

#[test]
fn macro_release_cancel() {
    let cfg = "\
(defsrc a b c)
(deflayer base (macro-release-cancel z 100 y 100) (macro x 100 w) c)";
    test_release(cfg);
    let cfg = "\
(defsrc a b c)
(deflayer base (macro-repeat-release-cancel z 100 y 100) (macro x 100 w) c)";
    test_release(cfg);
}

fn test_release(cfg: &str) {
    // Cancellation should not happen for press.
    let result = simulate(cfg, "d:a t:50 d:c t:100").to_ascii();
    assert_eq!(
        "t:1ms dn:Z t:1ms up:Z t:48ms dn:C t:53ms dn:Y t:1ms up:Y",
        result
    );
    // Cancellation should happen for release
    let result = simulate(cfg, "d:a u:a t:150 d:c t:100").to_ascii();
    assert_eq!("t:1ms dn:Z t:1ms up:Z t:148ms dn:C", result);
    // Macro should complete if allowed to.
    let result = simulate(cfg, "d:a t:150 d:c t:20").to_ascii();
    assert_eq!(
        "t:1ms dn:Z t:1ms up:Z t:101ms dn:Y t:1ms up:Y t:46ms dn:C",
        result
    );
}
