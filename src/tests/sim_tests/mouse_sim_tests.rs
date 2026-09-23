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

#[test]
fn movemouse_accel_resets_on_direction_reversal() {
    // Regression test for #2142.
    // With `movemouse-inherit-accel-state yes`, reversing direction on the
    // same axis (accel-left held, then accel-right pressed) must reset the
    // acceleration back to the minimum distance instead of inheriting the
    // maxed-out speed, matching the fixed QMK behavior.
    let result = simulate(
        "(defcfg movemouse-inherit-accel-state yes)
         (defsrc a b)
         (deflayermap (base)
           a (movemouse-accel-left 1 3 1 4)
           b (movemouse-accel-right 1 3 1 4))",
        "d:a t:6 d:b t:6 u:a u:b t:2",
    )
    .to_ascii();
    assert_eq!(
        "out🖰:move Left,1 t:1ms out🖰:move Left,2 t:1ms out🖰:move Left,3 t:1ms out🖰:move Left,4 t:1ms out🖰:move Left,4 t:1ms out🖰:move Left,4 t:1ms out🖰:move Right,1 t:1ms out🖰:move Right,2 t:1ms out🖰:move Right,3 t:1ms out🖰:move Right,4 t:1ms out🖰:move Right,4 t:1ms out🖰:move Right,4 t:1ms out🖰:move Right,4",
        result
    );
}

#[test]
fn movemouse_accel_reversal_keeps_other_axis_inheritance() {
    // With `movemouse-inherit-accel-state yes`, a same-axis reversal must
    // still let the *other* axis be inherited from. Up is held and maxed
    // out first, then left is pressed (cross-axis inherit from up, so it
    // starts maxed), then right is pressed while left is still held (a
    // same-axis reversal on left/right). Right must reset relative to
    // left, but it can still inherit the still-active, still-maxed up
    // state, so it also starts maxed rather than ramping from the minimum.
    let result = simulate(
        "(defcfg movemouse-inherit-accel-state yes)
         (defsrc a b c)
         (deflayermap (base)
           a (movemouse-accel-up 1 3 1 4)
           b (movemouse-accel-left 1 3 1 4)
           c (movemouse-accel-right 1 3 1 4))",
        "d:a t:6 d:b t:6 d:c t:2 u:a u:b u:c t:2",
    )
    .to_ascii();
    assert_eq!(
        "out🖰:move Up,1 t:1ms out🖰:move Up,2 t:1ms out🖰:move Up,3 t:1ms out🖰:move Up,4 t:1ms out🖰:move Up,4 t:1ms out🖰:move Up,4 t:1ms out🖰:move Up,4 out🖰:move Left,4 t:1ms out🖰:move Up,4 out🖰:move Left,4 t:1ms out🖰:move Up,4 out🖰:move Left,4 t:1ms out🖰:move Up,4 out🖰:move Left,4 t:1ms out🖰:move Up,4 out🖰:move Left,4 t:1ms out🖰:move Up,4 out🖰:move Left,4 t:1ms out🖰:move Up,4 out🖰:move Right,4 t:1ms out🖰:move Up,4 out🖰:move Right,4 t:1ms out🖰:move Right,4 t:1ms out🖰:move Right,4",
        result
    );
}

#[test]
fn movemouse_release_resumes_other_held_direction() {
    // An axis tracks one movement at a time, so releasing the key that owns
    // that state used to stop the axis outright even when another movement
    // key on it was still held, leaving the held key doing nothing until it
    // was re-pressed. The still-held movement has to be resumed instead.

    // Reversed direction: left is held, right takes the axis over, and
    // releasing right goes back to moving left.
    let result = simulate(
        "(defsrc a b)
         (deflayermap (base)
           a (movemouse-left 1 5)
           b (movemouse-right 1 7))",
        "d:a t:3 d:b t:3 u:b t:3 u:a t:2",
    )
    .to_ascii();
    assert_eq!(
        "out🖰:move Left,5 t:1ms out🖰:move Left,5 t:1ms out🖰:move Left,5 t:1ms out🖰:move Right,7 t:1ms out🖰:move Right,7 t:1ms out🖰:move Right,7 t:1ms out🖰:move Left,5 t:1ms out🖰:move Left,5 t:1ms out🖰:move Left,5",
        result
    );

    // Same direction at a different distance: the slower movement is still
    // held underneath the faster one and resumes at its own distance.
    let result = simulate(
        "(defsrc a c)
         (deflayermap (base)
           a (movemouse-left 1 5)
           c (movemouse-left 1 7))",
        "d:a t:3 d:c t:3 u:c t:3 u:a t:2",
    )
    .to_ascii();
    assert_eq!(
        "out🖰:move Left,5 t:1ms out🖰:move Left,5 t:1ms out🖰:move Left,5 t:1ms out🖰:move Left,7 t:1ms out🖰:move Left,7 t:1ms out🖰:move Left,7 t:1ms out🖰:move Left,5 t:1ms out🖰:move Left,5 t:1ms out🖰:move Left,5",
        result
    );
}

#[test]
fn movemouse_accel_release_resumes_other_held_direction_from_minimum() {
    // Same resumption for accelerated movement, and the resumed direction
    // ramps from its minimum distance again rather than picking up the speed
    // built up by the movement that was just released.
    let result = simulate(
        "(defsrc a b)
         (deflayermap (base)
           a (movemouse-accel-left 1 3 1 4)
           b (movemouse-accel-right 1 3 1 4))",
        "d:a t:4 d:b t:4 u:b t:5 u:a t:2",
    )
    .to_ascii();
    assert_eq!(
        "out🖰:move Left,1 t:1ms out🖰:move Left,2 t:1ms out🖰:move Left,3 t:1ms out🖰:move Left,4 t:1ms out🖰:move Right,1 t:1ms out🖰:move Right,2 t:1ms out🖰:move Right,3 t:1ms out🖰:move Right,4 t:1ms out🖰:move Left,1 t:1ms out🖰:move Left,2 t:1ms out🖰:move Left,3 t:1ms out🖰:move Left,4 t:1ms out🖰:move Left,4",
        result
    );
}

#[test]
fn movemouse_release_leaves_the_other_axis_alone() {
    // Resuming is per axis: releasing a horizontal movement must not pick up
    // a still-held vertical one, and must not disturb it either.
    let result = simulate(
        "(defsrc u l)
         (deflayermap (base)
           u (movemouse-up 1 3)
           l (movemouse-left 1 5))",
        "d:u t:2 d:l t:2 u:l t:3 u:u t:2",
    )
    .to_ascii();
    assert_eq!(
        "out🖰:move Up,3 t:1ms out🖰:move Up,3 t:1ms out🖰:move Up,3 out🖰:move Left,5 t:1ms out🖰:move Up,3 out🖰:move Left,5 t:1ms out🖰:move Up,3 t:1ms out🖰:move Up,3 t:1ms out🖰:move Up,3",
        result
    );
}
