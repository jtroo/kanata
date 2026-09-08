use super::*;

#[test]
fn multi_mouse_button_does_multi_click_release_single_hold() {
    let result = simulate(
        "(defsrc) (deflayermap (base) a (multi mmid mmid mrgt mlft))",
        "d:a t:50 u:a t:50",
    )
    .to_ascii();
    assert_eq!(
        "out🖰:↓Mid out🖰:↑Mid t:1ms out🖰:↓Mid out🖰:↑Mid t:1ms out🖰:↓Right out🖰:↑Right t:1ms out🖰:↓Left t:50ms out🖰:↑Left",
        result
    );
}

#[test]
fn mwheel_accel_stops_decel_on_mod_press() {
    for (cfg_direction, result_direction) in [
        ("up", "Up"),
        ("down", "Down"),
        ("left", "Left"),
        ("right", "Right"),
    ]
    .iter()
    {
        // Pressing `lctl` which is a modifier immediately stops acceleration.
        let cfg = format!(
            "(defsrc)
         (deflayermap (base)
          a (mwheel-accel-{} 10 50 1.15 0.93)
          b lctl)",
            cfg_direction
        );
        let result = simulate(cfg.as_str(), "d:a t:50 u:a t:2 d:b u:b t:200").to_ascii();
        assert_eq!(
            format!(
                "scroll:{0},11 t:16ms scroll:{0},13 t:16ms scroll:{0},15 t:16ms scroll:{0},17 t:4ms \
         dn:LCtrl t:1ms up:LCtrl",
                &result_direction
            ),
            result
        );

        // Pressing `b` which is not a modifier retains stanard deceleration behaviour.
        // Pressing `lctl` which is a modifier immediately stops acceleration.
        let cfg = format!(
            "(defsrc)
         (deflayermap (base)
          a (mwheel-accel-{} 10 50 1.15 0.93))",
            cfg_direction
        )
        .to_ascii();
        let result = simulate(cfg.as_str(), "d:a t:50 u:a t:2 d:b u:b t:200").to_ascii();
        assert_eq!(
            format!(
                "scroll:{0},11 t:16ms scroll:{0},13 t:16ms scroll:{0},15 t:16ms scroll:{0},17 t:4ms \
        dn:B t:1ms up:B \
        t:11ms scroll:{0},16 t:16ms scroll:{0},15 t:16ms scroll:{0},14 t:16ms scroll:{0},13 t:16ms scroll:{0},12 t:16ms scroll:{0},11 t:16ms scroll:{0},10 t:16ms scroll:{0},9 t:16ms scroll:{0},9 t:16ms scroll:{0},8 t:16ms scroll:{0},7 t:16ms scroll:{0},7",
                result_direction
            ),
            result
        );
    }
}
