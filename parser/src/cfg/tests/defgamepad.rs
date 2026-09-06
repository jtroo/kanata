use super::*;

use crate::gamepad::{
    Cardinal, Curve, Digital, DirMode, Directional, GamepadConfig, Motion, MotionKind, Projection,
    Side, Socd, Unit,
};

/// Parse a configuration and return its `defgamepad` result.
///
/// Wraps the declaration in the minimum surrounding config, so each test reads
/// as just the thing it is about.
fn gamepad_of(extra_src: &str, declaration: &str) -> MResult<crate::cfg::Cfg> {
    let _lk = lock(&CFG_PARSE_LOCK);
    let src = format!(
        "(defcfg process-unmapped-keys no)
         (defsrc a {extra_src})
         (deflayer base a {})
         {declaration}",
        vec!["XX"; extra_src.split_whitespace().count()].join(" ")
    );
    new_from_str(&src, HashMap::default())
}

fn ok(extra_src: &str, declaration: &str) -> GamepadConfig {
    gamepad_of(extra_src, declaration)
        .unwrap_or_else(|e| panic!("expected the config to parse, got: {e:?}"))
        .gamepad
        .expect("defgamepad should be retained on Cfg")
}

fn err(extra_src: &str, declaration: &str) -> String {
    match gamepad_of(extra_src, declaration) {
        Ok(_) => panic!("expected this config to be rejected:\n{declaration}"),
        Err(e) => flatten(&e),
    }
}

/// An error message with its whitespace collapsed, so a test can quote it
/// without depending on where the renderer happened to wrap the line.
fn flatten(e: &impl std::fmt::Debug) -> String {
    format!("{e:?}")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn digital(cfg: &GamepadConfig, control: Directional) -> Digital {
    cfg.projection(control)
        .digital
        .unwrap_or_else(|| panic!("expected the {} to be digital", control.as_str()))
}

fn motion(cfg: &GamepadConfig, control: Directional) -> (MotionKind, Motion) {
    for kind in MotionKind::ALL {
        if let Some(motion) = cfg.projection(control).motions.get(kind) {
            return (kind, motion);
        }
    }
    panic!("expected the {} to drive motion", control.as_str())
}

#[test]
fn buttons_and_the_dpad_need_no_declaration_at_all() {
    // Nothing about a button or a d-pad contact has to be decided before it
    // can press a key, so requiring a declaration would be ceremony.
    let cfg = gamepad_of("pad-a pad-l2 pad-dpad-up", "").expect("parses");
    assert_eq!(
        cfg.gamepad,
        Some(GamepadConfig::default()),
        "mapped portable controls must start the backend"
    );

    let keyboard_only = gamepad_of("", "").expect("parses");
    assert_eq!(
        keyboard_only.gamepad, None,
        "a keyboard-only config must not open controller devices"
    );

    let empty = ok("pad-dpad-up", "(defgamepad)");
    assert_eq!(empty, GamepadConfig::default());
    assert!(empty.projection(Directional::Dpad).digital.is_some());
    assert_eq!(empty.projection(Directional::LeftStick), &Projection::OFF);
    assert!(empty.slots.iter().all(Option::is_none));
    // A trigger with nothing said about it must still actuate, at the early
    // point people expect from a trigger.
    assert_eq!(empty.trigger(Side::Right).threshold, Unit::new(0.30));
}

#[test]
fn a_full_declaration_round_trips() {
    let cfg = ok(
        "",
        "(defgamepad
           (stick left  (digital (mode 8way) (threshold 0.60)))
           (stick right (mouse (deadzone 0.20) (speed 40) (curve quadratic) (invert-y no)))
           (dpad (digital (socd neutral)))
           (trigger left  (threshold 0.10))
           (button-slot 0 0x2c0))",
    );
    let left = digital(&cfg, Directional::LeftStick);
    assert_eq!(left.mode, DirMode::EightWay);
    assert_eq!(left.threshold, Unit::new(0.60));

    let (kind, right) = motion(&cfg, Directional::RightStick);
    assert_eq!(
        (kind, right.speed, right.curve),
        (MotionKind::Mouse, 40.0, Curve::Quadratic)
    );
    assert_eq!(right.deadzone, Unit::new(0.20));
    assert_eq!(
        right.invert.y, 1.0,
        "(invert-y no) undoes the mouse default"
    );

    assert_eq!(digital(&cfg, Directional::Dpad).socd, Socd::Neutral);
    assert_eq!(cfg.trigger(Side::Left).threshold, Unit::new(0.10));
    assert_eq!(cfg.slots[0], Some(0x2c0));
    // What was not mentioned keeps its defaults.
    assert_eq!(cfg.trigger(Side::Right).threshold, Unit::new(0.30));
}

#[test]
fn directional_controls_share_projection_modes_but_only_contacts_have_socd() {
    // The d-pad is a stick that only knows nine positions, so projection mode
    // is shared by all three. SOCD is intentionally narrower: only independent
    // contacts can report opposites simultaneously.
    for (item, control) in [
        ("(stick left (digital ", Directional::LeftStick),
        ("(stick right (digital ", Directional::RightStick),
        ("(dpad (digital ", Directional::Dpad),
    ] {
        let cfg = ok("", &format!("(defgamepad {item}(mode 8way))))"));
        assert_eq!(digital(&cfg, control).mode, DirMode::EightWay, "{item}");
    }
    let cfg = ok("", "(defgamepad (dpad (digital (socd last))))");
    assert_eq!(digital(&cfg, Directional::Dpad).socd, Socd::Last);
    let message = err("", "(defgamepad (stick left (digital (socd last))))");
    assert!(message.contains("socd belongs to the d-pad"), "{message}");
}

#[test]
fn a_control_can_press_keys_and_drive_motion_at_once() {
    let cfg = ok(
        "",
        "(defgamepad (stick right (digital (mode 4way)) (mouse (speed 12))))",
    );
    assert_eq!(
        digital(&cfg, Directional::RightStick).mode,
        DirMode::FourWay
    );
    assert_eq!(motion(&cfg, Directional::RightStick).1.speed, 12.0);

    // For the d-pad that means adding a wheel must not take away the keys it
    // presses with no declaration at all. `off` is how you silence it.
    let dpad = ok("", "(defgamepad (dpad (scroll (speed 2))))");
    assert!(dpad.projection(Directional::Dpad).digital.is_some());
    assert_eq!(motion(&dpad, Directional::Dpad).0, MotionKind::Scroll);
    let silent = ok("", "(defgamepad (dpad off))");
    assert_eq!(silent.projection(Directional::Dpad), &Projection::OFF);
}

#[test]
fn mouse_and_scroll_are_independent_projections() {
    let cfg = ok(
        "",
        "(defgamepad
           (stick right (mouse (speed 1200)) (scroll (speed 30)))
           (trigger left
             (mouse right (speed 800))
             (scroll down (speed 20))))",
    );
    let stick = cfg.projection(Directional::RightStick);
    assert_eq!(stick.motions.get(MotionKind::Mouse).unwrap().speed, 1200.0);
    assert_eq!(stick.motions.get(MotionKind::Scroll).unwrap().speed, 30.0);
    let trigger = cfg.trigger(Side::Left);
    assert_eq!(
        trigger.motions.get(MotionKind::Mouse).unwrap().direction,
        Cardinal::Right
    );
    assert_eq!(
        trigger.motions.get(MotionKind::Scroll).unwrap().direction,
        Cardinal::Down
    );

    for declaration in [
        "(defgamepad (stick left (mouse) (mouse)))",
        "(defgamepad (trigger right (scroll up) (scroll down)))",
    ] {
        let message = err("", declaration);
        assert!(
            message.contains("duplicate"),
            "{declaration} gave {message}"
        );
    }
}

#[test]
fn a_trigger_can_drive_the_wheel_in_a_named_direction() {
    let cfg = ok(
        "",
        "(defgamepad (trigger right (scroll down (speed 4) (curve linear))))",
    );
    let projected = cfg
        .trigger(Side::Right)
        .motions
        .get(MotionKind::Scroll)
        .expect("declared");
    assert_eq!(
        (projected.direction, projected.motion.speed),
        (Cardinal::Down, 4.0)
    );
    // The band survives alongside it: a trigger still presses pad-r2.
    assert_eq!(cfg.trigger(Side::Right).threshold, Unit::new(0.30));

    // A trigger is one number, not a vector, so the direction is required.
    let missing = err("", "(defgamepad (trigger right (scroll (speed 4))))");
    assert!(missing.contains("needs a direction"), "{missing}");
    let unknown = err("", "(defgamepad (trigger right (scroll sideways)))");
    assert!(unknown.contains("unknown direction"), "{unknown}");
}

#[test]
fn thresholds_have_one_value_and_reject_the_old_two_value_spelling() {
    let cfg = ok(
        "",
        "(defgamepad (stick left (digital (threshold 0.9))) (trigger left (threshold 0.05)))",
    );
    assert_eq!(
        digital(&cfg, Directional::LeftStick).threshold,
        Unit::new(0.9)
    );
    assert_eq!(cfg.trigger(Side::Left).threshold, Unit::new(0.05));

    for declaration in [
        "(defgamepad (stick left (digital (press 0.5))))",
        "(defgamepad (stick left (digital (release 0.5))))",
        "(defgamepad (trigger right (press 0.2)))",
        "(defgamepad (trigger right (release 0.2)))",
    ] {
        let message = err("", declaration);
        assert!(
            message.contains("threshold"),
            "{declaration} gave {message}"
        );
    }

    // A d-pad has no threshold to set: its contacts are digital before they
    // reach kanata, so a press point would be a knob that did nothing.
    let dpad = err("", "(defgamepad (dpad (digital (threshold 0.5))))");
    assert!(dpad.contains("no threshold"), "{dpad}");
}

