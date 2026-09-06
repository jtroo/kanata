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

#[test]
fn out_of_range_numbers_are_rejected_rather_than_clamped() {
    // In a config an out-of-range value is always a typo, and reading
    // `(deadzone 15)` as `1.0` would leave a stick that never responds.
    for (declaration, expected) in [
        (
            "(defgamepad (stick left (mouse (deadzone 15))))",
            "between 0 and 1",
        ),
        (
            "(defgamepad (stick left (mouse (speed 100001))))",
            "between 0 and 100000",
        ),
        (
            "(defgamepad (stick left (digital (threshold 2))))",
            "between 0 and 1",
        ),
        (
            "(defgamepad (stick left (mouse (speed nope))))",
            "expected a number",
        ),
    ] {
        let message = err("", declaration);
        assert!(message.contains(expected), "{declaration} gave {message}");
    }
}

#[test]
fn duplicate_declarations_are_rejected() {
    // Letting the second one quietly win makes a typo look like it worked.
    for declaration in [
        "(defgamepad (stick left (digital)) (stick left (mouse)))",
        "(defgamepad (trigger right (threshold 0.4)) (trigger right (threshold 0.5)))",
        "(defgamepad (dpad (digital)) (dpad (digital)))",
        "(defgamepad (device 1) (device 1))",
    ] {
        assert!(err("", declaration).contains("duplicate"), "{declaration}");
    }
    // But the two sides of one control are not duplicates.
    ok(
        "",
        "(defgamepad (stick left (digital)) (stick right (digital)))",
    );
    let two = err("", "(defgamepad) (defgamepad (dpad off))");
    assert!(two.contains("Only one defgamepad"), "{two}");
}

#[test]
fn unknown_names_are_reported_with_the_valid_set() {
    for (declaration, expected) in [
        ("(defgamepad (wobble left))", "unknown defgamepad item"),
        ("(defgamepad (stick middle (digital)))", "unknown side"),
        ("(defgamepad (stick left (wiggle)))", "unknown projection"),
        (
            "(defgamepad (stick left (digital (mode 6way))))",
            "unknown mode",
        ),
        (
            "(defgamepad (dpad (digital (socd sometimes))))",
            "unknown socd mode",
        ),
        (
            "(defgamepad (stick left (mouse (curve sigmoid))))",
            "unknown curve",
        ),
        (
            "(defgamepad (stick left (mouse (spede 3))))",
            "unknown mouse option",
        ),
        // An unknown option is reported as one, rather than as an arity error
        // about a name that means nothing here.
        (
            "(defgamepad (stick left (mouse (spede 3 4))))",
            "unknown mouse option",
        ),
        (
            "(defgamepad (stick left (digital (jitter 3))))",
            "unknown digital option",
        ),
        (
            "(defgamepad (trigger left (wobble 3)))",
            "unknown trigger option",
        ),
    ] {
        let message = err("", declaration);
        assert!(message.contains(expected), "{declaration} gave {message}");
    }
    // And an unknown value lists what would have worked.
    let listed = err("", "(defgamepad (stick left (mouse (curve sigmoid))))");
    assert!(listed.contains("linear, quadratic, cubic"), "{listed}");
}

#[test]
fn mapping_a_direction_without_a_projection_says_what_would_fix_it() {
    // These are the failures that otherwise present as "kanata runs, my
    // controller does nothing".
    let message = err("pad-lstick-up", "(defgamepad)");
    assert!(message.contains("no digital projection"), "{message}");
    assert!(message.contains("(stick left (digital))"), "{message}");

    // Motion alone is not a digital projection: a mouse stick presses nothing.
    let motion_only = err("pad-rstick-up", "(defgamepad (stick right (mouse)))");
    assert!(
        motion_only.contains("no digital projection"),
        "{motion_only}"
    );
    // Adding the digital half makes it legal, on the same stick.
    ok(
        "pad-rstick-up",
        "(defgamepad (stick right (mouse) (digital)))",
    );
    // And declaring the left stick must not excuse a mapped right-stick
    // direction.
    let other = err("pad-rstick-up", "(defgamepad (stick left (digital)))");
    assert!(other.contains("right stick"), "{other}");
}

#[test]
fn four_way_mode_rejects_a_mapped_diagonal_and_eight_way_accepts_both() {
    let message = err(
        "pad-lstick-upleft",
        "(defgamepad (stick left (digital (mode 4way))))",
    );
    assert!(
        message.contains("4way") && message.contains("(mode 8way)"),
        "{message}"
    );
    ok(
        "pad-lstick-upleft pad-lstick-up",
        "(defgamepad (stick left (digital (mode 8way))))",
    );
    // The same rule reaches the d-pad, which is the point of sharing a type.
    let dpad = err("pad-dpad-upleft", "(defgamepad)");
    assert!(dpad.contains("4way"), "{dpad}");
    ok(
        "pad-dpad-upleft",
        "(defgamepad (dpad (digital (mode 8way))))",
    );
}

#[test]
fn slots_must_be_bound_and_unique() {
    let unbound = err("pad-button-0", "(defgamepad)");
    assert!(unbound.contains("(button-slot 0"), "{unbound}");
    assert_eq!(
        ok("pad-button-0", "(defgamepad (button-slot 0 0x2c1))").slots[0],
        Some(0x2c1)
    );
    assert_eq!(
        ok("", "(defgamepad (button-slot 0 705))").slots[0],
        Some(705)
    );

    for (declaration, expected) in [
        (
            "(defgamepad (button-slot 0 1) (button-slot 0 2))",
            "duplicate button-slot",
        ),
        (
            "(defgamepad (button-slot 0 1) (button-slot 1 1))",
            "already bound",
        ),
        ("(defgamepad (button-slot 99 1))", "slot index must be 0-15"),
        ("(defgamepad (button-slot 0))", "button-slot takes"),
    ] {
        let message = err("", declaration);
        assert!(message.contains(expected), "{declaration} gave {message}");
    }
}

#[test]
fn a_device_reference_must_match_definputdevices() {
    let _lk = lock(&CFG_PARSE_LOCK);
    let with_devices = |id: &str| {
        format!(
            "(defcfg process-unmapped-keys no)
             (definputdevices 1 ((name \"DualSense\")))
             (defsrc a) (deflayer base a)
             (defgamepad (device {id}))"
        )
    };
    assert!(new_from_str(&with_devices("1"), HashMap::default()).is_ok());
    let message = match new_from_str(&with_devices("2"), HashMap::default()) {
        Ok(_) => panic!("an undeclared device ID should be rejected"),
        Err(e) => flatten(&e),
    };
    assert!(
        message.contains("definputdevices has no entry"),
        "{message}"
    );
}

#[test]
fn digital_values_can_be_written_as_variables() {
    // defgamepad is parsed after defvar, so this has to work.
    let cfg = ok(
        "",
        "(defvar threshold 0.75 curve linear)
         (defgamepad (stick left (digital (threshold $threshold)) (mouse (curve $curve))))",
    );
    assert_eq!(
        digital(&cfg, Directional::LeftStick).threshold,
        Unit::new(0.75)
    );
    assert_eq!(motion(&cfg, Directional::LeftStick).1.curve, Curve::Linear);
}

#[test]
fn every_reserved_code_is_named_and_input_only() {
    // The reserved range and the name table are written separately, so a code
    // with no spelling would be a layout slot nothing could ever occupy. And
    // no OS can be asked to emit one, so an output position must be refused.
    let _lk = lock(&CFG_PARSE_LOCK);
    for index in 0..OsCode::GAMEPAD_COUNT {
        let osc = OsCode::from_gamepad_index(index).expect("in range");
        let name = format!("{osc:?}").to_lowercase().replace('_', "-");
        assert_eq!(str_to_oscode(&name), Some(osc), "{osc} has no name");
    }
    for output in ["pad-a", "C-pad-a", "(macro pad-a)", "(unmod pad-a)"] {
        let src = format!("(defcfg process-unmapped-keys no) (defsrc a) (deflayer base {output})");
        let message = match new_from_str(&src, HashMap::default()) {
            Ok(_) => panic!("{output} should be rejected as an output"),
            Err(e) => flatten(&e),
        };
        assert!(
            message.contains("can only be used as an input"),
            "{message}"
        );
    }
}

#[test]
fn pad_names_accept_their_vendor_aliases() {
    // One positional control, several spellings, so a config written for a
    // DualSense reads on an Xbox pad.
    for (canonical, alias) in [
        ("pad-south", "pad-cross"),
        ("pad-south", "pad-a"),
        ("pad-l1", "pad-lb"),
        ("pad-select", "pad-share"),
        ("pad-lstick-up", "pad-ls-up"),
        ("pad-dpad-left", "pad-left"),
        ("pad-button-3", "pb3"),
    ] {
        assert_eq!(
            str_to_oscode(canonical),
            str_to_oscode(alias),
            "{canonical} and {alias} should be one control"
        );
    }
}
