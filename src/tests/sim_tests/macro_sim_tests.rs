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
    let result = simulate(cfg, "d:a t:10 d:c u:c t:10 d:b t:20 d:c t:100").to_ascii();
    assert_eq!(
        "t:1ms dn:Z t:1ms up:Z t:8ms dn:C t:1ms up:C t:10ms \
                dn:X t:1ms up:X t:18ms dn:C t:83ms dn:W t:1ms up:W",
        result
    );
}
