//! Controller controls driven through the real processing loop.
//!
//! The projector and the engine are unit-tested on their own. What these cover
//! is the claim `defgamepad` is built around: a controller control is an
//! ordinary kanata input, so every action in the language already works on it
//! with nothing added. If that ever stops being true these break, and no
//! amount of testing the gamepad module in isolation would have noticed.

use super::*;

const PAD: &str = "\
(defcfg process-unmapped-keys no)
(defgamepad
  (stick left (digital (mode 4way)))
  (trigger left (threshold 0.30)))
(defsrc pad-a pad-b pad-l2 pad-lstick-up a)
";

#[test]
fn a_controller_control_is_an_ordinary_input() {
    let cfg = format!("{PAD}(deflayer base x y z w q)");
    assert_eq!(
        simulate(
            cfg.clone(),
            "d:pad-a t:10 u:pad-a t:10 d:pad-b t:10 u:pad-b t:10".into()
        )
        .to_ascii(),
        "dn:X t:10ms up:X t:10ms dn:Y t:10ms up:Y"
    );
    // A projected stick direction is no different from a button.
    assert_eq!(
        simulate(cfg, "d:pad-lstick-up t:10 u:pad-lstick-up t:10".into()).to_ascii(),
        "dn:W t:10ms up:W"
    );
}

#[test]
fn tap_hold_works_on_a_controller_button() {
    // The headline claim: nothing in tap-hold knows a controller exists.
    let cfg = format!("{PAD}(deflayer base (tap-hold 100 100 spc lctl) y z w q)");
    assert_eq!(
        simulate(cfg.clone(), "d:pad-a t:30 u:pad-a t:50".into()).to_ascii(),
        "t:30ms dn:Space t:6ms up:Space",
        "a short press should tap"
    );
    assert_eq!(
        simulate(cfg, "d:pad-a t:150 u:pad-a t:10".into()).to_ascii(),
        "t:100ms dn:LCtrl t:50ms up:LCtrl",
        "a long press should hold"
    );
}

#[test]
fn a_controller_button_can_reach_a_layer_that_a_key_then_uses() {
    // A pad control and a keyboard key composing in one chord is the thing
    // that would break if controller input took a separate path.
    let result = simulate(
        format!(
            "{PAD}\
             (deflayer base x y z w (layer-while-held nav))\
             (deflayer nav 1 2 3 4 _)"
        ),
        "d:a t:10 d:pad-a t:10 u:pad-a t:10 u:a t:10 d:pad-a t:10 u:pad-a t:10".into(),
    )
    .to_ascii();
    assert_eq!(
        result,
        "t:10ms dn:Kb1 t:10ms up:Kb1 t:20ms dn:X t:10ms up:X"
    );
}

#[test]
fn a_controller_control_never_reaches_the_os_as_a_scancode() {
    // `use-defsrc` builds its output from the defsrc position rather than from
    // a key name, so it is the one path that can hand a pad code to the output
    // layer. It must produce nothing rather than an invalid scancode.
    let result = simulate(
        format!("{PAD}(deflayer base use-defsrc use-defsrc use-defsrc use-defsrc use-defsrc)"),
        "d:pad-a t:10 u:pad-a t:10 d:a t:10 u:a t:10".into(),
    )
    .to_ascii();
    assert_eq!(result, "t:20ms dn:A t:10ms up:A");
}

#[test]
fn controller_controls_are_not_swept_in_by_process_unmapped_keys() {
    // A controller is a separate device the user opted into with defsrc.
    // Sweeping its controls in would claim a device kanata was never asked to
    // touch.
    let parsed = Kanata::new_from_str(
        "(defcfg process-unmapped-keys yes)\n(defsrc a)\n(deflayer base b)\n",
        Default::default(),
    )
    .expect("parses");
    let mapped = crate::kanata::MAPPED_KEYS.lock();
    assert!(mapped.contains(&str_to_oscode("b").unwrap()));
    assert!(
        !mapped.iter().any(|osc| osc.is_gamepad_code()),
        "process-unmapped-keys claimed a controller control"
    );
    drop(parsed);
}

/// Everything below needs the controller engine, which only exists in a build
/// with the backend. The tests above do not: a controller control is an
/// ordinary `OsCode`, so `d:pad-a` works in any build — which is the whole
/// point of parsing `defgamepad` everywhere.
mod motion {
    use super::*;

    // These drive an analog control through the tick loop, which is the only
    // place a continuous projection becomes real pointer or wheel movement.
    // Everything upstream of the `pad:` verb is unit-tested; this is the seam.

    /// A stick fast enough to see, with the curve and inversion taken out of
    /// the way so the arithmetic under test is only the deadzone and the speed.
    const MOUSE: &str = "\
    (defcfg process-unmapped-keys no)
    (defgamepad (stick right (mouse (deadzone 0.1) (speed 10000) (curve linear) (invert-y no))))
    (defsrc a)
    (deflayer base a)
    ";

    /// Total pointer travel in one direction across a simulation.
    fn travelled(result: &str, direction: &str) -> u32 {
        let needle = format!("out🖰:move {direction},");
        result
            .split('\n')
            .filter_map(|line| line.strip_prefix(needle.as_str()))
            .map(|d| d.parse::<u32>().expect("a distance"))
            .sum()
    }

    #[test]
    fn a_pushed_stick_moves_the_pointer_and_centering_stops_it() {
        let result = simulate(
            MOUSE.to_string(),
            "pad:right:1.0,0.0 t:3 pad:right:0.0,0.0 t:5".to_string(),
        );
        // Full deflection at 10,000 px/s (10 px/tick) for three ticks and then nothing: the
        // five ticks after centering would show up as another 50 pixels.
        assert_eq!(travelled(&result, "Right"), 30, "{result}");

        // A stick held still reports no further events, so the movement above
        // only happens if a held stick keeps the processing loop awake.
        // Without that, the first tick moves and the pointer then freezes.
        let held = simulate(MOUSE.to_string(), "pad:right:1.0,0.0 t:20".to_string());
        assert_eq!(
            travelled(&held, "Right"),
            200,
            "the pointer stalled: {held}"
        );
    }

    #[test]
    fn both_axes_move_together_rather_than_as_two_steps() {
        // A diagonal delivered as two independent steps would stair-step
        // visibly; `move_mouse_many` is what keeps them in one call.
        let result = simulate(MOUSE.to_string(), "pad:right:1.0,1.0 t:1".to_string());
        assert_eq!(travelled(&result, "Right"), 7, "{result}");
        assert_eq!(travelled(&result, "Down"), 7, "{result}");
    }

    #[test]
    fn the_deadzone_holds_the_pointer_still_but_does_not_round_slow_pushes_away() {
        // A resting controller walking the cursor across the screen all day is
        // the failure on one side; making slow movement impossible rather than
        // merely slow is the failure on the other.
        let resting = simulate(MOUSE.to_string(), "pad:right:0.05,0.05 t:50".to_string());
        assert!(!resting.contains("move"), "the pointer drifted: {resting}");

        // 0.115 is barely past the 0.1 deadzone: about a sixth of a pixel per
        // tick, which only accumulates into movement if the remainder carries.
        let crawl = simulate(MOUSE.to_string(), "pad:right:0.115,0.0 t:20".to_string());
        let moved = travelled(&crawl, "Right");
        assert!((1..=5).contains(&moved), "crept {moved} pixels: {crawl}");
    }

    #[test]
    fn movemouse_speed_scales_controller_movement_too() {
        // A controller pointer is still kanata's pointer, so the existing
        // speed modifier has to reach it.
        let cfg = "\
    (defcfg process-unmapped-keys no)
    (defgamepad (stick right (mouse (speed 10000) (curve linear) (invert-y no))))
    (defsrc a)
    (deflayer base (movemouse-speed 200))
    ";
        let result = simulate(cfg.to_string(), "d:a t:1 pad:right:1.0,0.0 t:1".to_string());
        assert_eq!(
            travelled(&result, "Right"),
            20,
            "200% of 10 px/tick: {result}"
        );
    }

    #[test]
    fn a_scroll_projection_turns_the_wheel_rather_than_moving_the_pointer() {
        let cfg = "\
    (defcfg process-unmapped-keys no)
    (defgamepad (stick left (scroll (speed 2000) (curve linear))))
    (defsrc a)
    (deflayer base a)
    ";
        let result = simulate(cfg.to_string(), "pad:left:0.0,1.0 t:2".to_string());
        assert!(!result.contains("move"), "scroll must not move the pointer");
        assert!(
            result.contains("scroll:Up,2"),
            "a stick pushed away should scroll up: {result}"
        );
    }

    #[test]
    fn a_trigger_can_drive_the_wheel_by_how_hard_it_is_squeezed() {
        // A trigger is one number, so it is told which way to push. Squeezing
        // harder scrolls faster, which a digital control cannot express.
        let cfg = "\
    (defcfg process-unmapped-keys no)
    (defgamepad (trigger right (scroll down (deadzone 0) (speed 4000) (curve linear))))
    (defsrc a)
    (deflayer base a)
    ";
        let half = simulate(cfg.to_string(), "pad:rt:0.5 t:1".to_string());
        assert!(half.contains("scroll:Down,2"), "{half}");
        let full = simulate(cfg.to_string(), "pad:rt:1.0 t:1".to_string());
        assert!(full.contains("scroll:Down,4"), "{full}");
    }

    #[test]
    fn one_stick_can_press_keys_and_move_the_pointer_at_the_same_time() {
        // The threshold and the displacement read the same value without
        // disturbing each other, so there is no reason to make the user pick.
        let cfg = "\
    (defcfg process-unmapped-keys no)
    (defgamepad
      (stick right (digital (threshold 0.5)) (mouse (deadzone 0) (speed 10000) (curve linear))))
    (defsrc pad-rstick-right)
    (deflayer base x)
    ";
        let result = simulate(cfg.to_string(), "pad:right:1.0,0.0 t:2".to_string());
        assert_eq!(travelled(&result, "Right"), 20, "{result}");
        assert!(
            result.contains("↓X"),
            "the threshold key never pressed: {result}"
        );
    }

    #[test]
    fn a_digital_projection_uses_raw_deflection_through_the_whole_stack() {
        // The config says 0.5, so 0.45 must be silent and 0.55 must press, and
        // no deadzone anywhere is able to move that point.
        let cfg = "\
    (defcfg process-unmapped-keys no)
    (defgamepad (stick left (digital (threshold 0.5))))
    (defsrc pad-lstick-up)
    (deflayer base x)
    ";
        let below = simulate(cfg.to_string(), "pad:left:0.0,0.45 t:5".to_string());
        assert!(below.is_empty(), "0.45 is below the threshold: {below}");
        assert_eq!(
            simulate(cfg.to_string(), "pad:left:0.0,0.55 t:5".to_string()).to_ascii(),
            "dn:X",
            "0.55 is above it"
        );
    }
}

mod from_the_tick {
    use super::*;
    use kanata_parser::gamepad::{
        Cardinal, CardinalSet, Dir, PadButton, PadInput, Side, StickPosition,
    };

    // A live reload and a state clean both produce controller releases from
    // *inside* the tick, on the very thread that drains the input channel.
    // Pushing them down that channel would be a self-send that drops a release
    // when the queue is full; they are applied directly instead, and these
    // cover that path and the engine reset that goes with it.

    /// A `Kanata` with a controller connected but no backend thread.
    fn with_pad(cfg: &str) -> Kanata {
        let mut k = Kanata::new_from_str(cfg, Default::default()).expect("the config parses");
        let handle = crate::gamepad::GamepadHandle::new(
            k.gamepad_config.expect("defgamepad"),
            k.input_devices.as_deref(),
        );
        handle.connect(SIM_PAD, Default::default());
        k.gamepad = Some(handle);
        k
    }

    /// Feed one reading through the engine, as the backend would, and deliver
    /// whatever edges come back.
    fn feed(k: &mut Kanata, input: PadInput) {
        let mut edges = Vec::new();
        k.gamepad
            .as_ref()
            .expect("a controller")
            .feed(SIM_PAD, input, &mut edges);
        k.apply_gamepad_edges(&edges).expect("edges apply");
    }

    fn south(pressed: bool) -> PadInput {
        PadInput::Button {
            button: PadButton::South,
            pressed,
        }
    }

    fn stick(side: Side, x: f32, y: f32) -> PadInput {
        PadInput::Stick {
            side,
            value: StickPosition::new(x, y),
        }
    }

    fn lock_cfg() -> impl Drop {
        match CFG_PARSE_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    const ONE_BUTTON: &str = "\
    (defcfg process-unmapped-keys no)
    (defgamepad (stick left (digital)))
    (defsrc pad-a)
    (deflayer base x)
    ";

    /// Drive a d-pad contact trace and report the key events the OS saw.
    ///
    /// Timing is dropped: what these are about is which keys came out and in
    /// what order, and one contact change can produce two edges, which the
    /// layout then spreads over as many ticks.
    fn dpad_output(cfg: &str, trace: &[(Dir, bool)]) -> String {
        let mut k = with_pad(cfg);
        let mut contacts = CardinalSet::EMPTY;
        for (dir, pressed) in trace {
            contacts.set(
                Cardinal::try_from(*dir).expect("test trace uses cardinals"),
                *pressed,
            );
            feed(&mut k, PadInput::Contacts(contacts));
            k.tick_ms(5, &None).expect("ticks fine");
        }
        k.kbd_out
            .outputs
            .events
            .join(" ")
            .to_ascii()
            .split_whitespace()
            .filter(|event| event.starts_with("dn:") || event.starts_with("up:"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn socd_resolves_a_dpad_conflict_all_the_way_to_the_output() {
        // An opposed pair of contacts is something real hardware reports, and
        // what the OS sees has to be the resolved answer rather than two keys
        // held at once.
        let _lk = lock_cfg();
        let out = dpad_output(
            "(defcfg process-unmapped-keys no)
             (defgamepad (dpad (digital (socd neutral))))
             (defsrc pad-dpad-up pad-dpad-down)
             (deflayer base w s)",
            &[
                (Dir::Up, true),
                // Down arrives while Up is still held: under neutral both go
                // away, so the W that was down has to be released.
                (Dir::Down, true),
                // Letting go of Up leaves Down unopposed, and S becomes real
                // without the user having touched Down again.
                (Dir::Up, false),
                (Dir::Down, false),
            ],
        );
        assert_eq!(
            out, "dn:W up:W dn:S up:S",
            "an opposed pair must leave nothing held, and hand over on release"
        );

        // The default is to report both, because on a d-pad an opposed pair is
        // the user pressing two buttons and kanata's job is to say so.
        let both = dpad_output(
            "(defcfg process-unmapped-keys no)
             (defsrc pad-dpad-up pad-dpad-down)
             (deflayer base w s)
             (defgamepad)",
            &[(Dir::Up, true), (Dir::Down, true)],
        );
        assert_eq!(both, "dn:W dn:S");
    }

    #[test]
    fn an_eight_way_dpad_presses_the_diagonal_all_the_way_to_the_output() {
        // The d-pad shares the sticks' projection, so eight-way reaches it for
        // free — and a diagonal has to replace its components, not join them.
        let _lk = lock_cfg();
        let out = dpad_output(
            "(defcfg process-unmapped-keys no)
             (defgamepad (dpad (digital (mode 8way))))
             (defsrc pad-dpad-up pad-dpad-right pad-dpad-upright)
             (deflayer base w d e)",
            &[(Dir::Up, true), (Dir::Right, true), (Dir::Up, false)],
        );
        assert_eq!(out, "dn:W up:W dn:E up:E dn:D");
    }

    #[test]
    fn a_release_produced_inside_the_tick_reaches_the_layout() {
        // The delivery path that replaced the self-send. Nothing here touches
        // the input channel, and the release must still land.
        let _lk = lock_cfg();
        let mut k = with_pad(ONE_BUTTON);
        feed(&mut k, south(true));
        k.tick_ms(5, &None).expect("ticks fine");
        assert!(
            k.kbd_out.outputs.events.join("\n").contains("↓X"),
            "the press should have landed"
        );

        let mut edges = Vec::new();
        k.gamepad
            .as_mut()
            .expect("a controller")
            .release_all(&mut edges);
        assert!(!edges.is_empty(), "there was something to release");
        k.apply_gamepad_edges(&edges).expect("edges apply");
        k.tick_ms(5, &None).expect("ticks fine");

        let out = k.kbd_out.outputs.events.join("\n");
        assert!(out.contains("↑X"), "the release was dropped: {out}");
    }

    /// Reload `k` from a config written to a temporary file.
    fn reload(k: &mut Kanata, name: &str, cfg: &str) {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, cfg).expect("a writable config");
        k.cfg_paths = vec![path.clone()];
        k.cur_cfg_idx = 0;
        let outcome = k.do_live_reload(&None);
        let _ = std::fs::remove_file(&path);
        outcome.expect("the reload succeeds");
        k.tick_ms(5, &None).expect("ticks fine");
    }

    #[test]
    fn a_live_reload_resets_the_engine_so_a_held_control_works_again() {
        // A controller held across a reload has already been counted as
        // pressed. Without the reset the next press is swallowed as a repeat
        // and the control is dead until it is released and pressed again,
        // which the user has no way to know they must do.
        let _lk = lock_cfg();
        let mut k = with_pad(ONE_BUTTON);
        feed(&mut k, south(true));
        k.tick_ms(5, &None).expect("ticks fine");
        reload(&mut k, "kanata-gamepad-reload.kbd", ONE_BUTTON);

        let before = k.kbd_out.outputs.events.len();
        feed(&mut k, south(true));
        k.tick_ms(5, &None).expect("ticks fine");
        let after: Vec<_> = k.kbd_out.outputs.events[before..].to_vec();
        assert!(
            after.iter().any(|e| e.contains("↓X")),
            "the control was dead after the reload: {after:?}"
        );
    }

    #[test]
    fn a_reload_that_drops_defgamepad_stops_the_projections_it_declared() {
        // Removing the declaration has to remove what it declared, not merely
        // release it: the deleted stick must stop projecting entirely, or it
        // keeps producing edges under thresholds no longer in the file. The
        // fixture uses a *mouse* stick so `drives_motion` starts out true.
        let _lk = lock_cfg();
        let mut k = with_pad(
            "(defcfg process-unmapped-keys no)
             (defgamepad
               (stick left (digital))
               (stick right (mouse (deadzone 0.0) (speed 20000) (curve linear))))
             (defsrc pad-a pad-lstick-up)
             (deflayer base x w)",
        );
        feed(&mut k, south(true));
        feed(&mut k, stick(Side::Left, 0.0, 1.0));
        k.tick_ms(5, &None).expect("ticks fine");
        assert!(
            k.gamepad.as_ref().expect("a controller").drives_motion(),
            "the fixture has to start out driving the pointer, or the check \
             below proves nothing"
        );

        reload(
            &mut k,
            "kanata-gamepad-drop.kbd",
            "(defcfg process-unmapped-keys no)\n(defsrc a)\n(deflayer base y)\n",
        );

        assert!(
            !k.gamepad
                .as_ref()
                .expect("the handle stays up")
                .drives_motion(),
            "a dropped declaration must stop the tick loop sampling"
        );
        // The stick that was declared is gone, so pushing it produces nothing
        // at all: not an edge under the old thresholds, and no motion demand.
        feed(&mut k, stick(Side::Left, 0.0, -1.0));
        feed(&mut k, stick(Side::Right, 1.0, 0.0));
        assert!(
            k.gamepad.as_ref().expect("a controller").demand().is_idle(),
            "the deleted mouse projection is still asking for movement"
        );

        let before = k.kbd_out.outputs.events.len();
        k.handle_input_event(&KeyEvent::new(
            str_to_oscode("a").expect("a key"),
            KeyValue::Press,
        ))
        .expect("the press applies");
        k.tick_ms(5, &None).expect("ticks fine");
        let after: Vec<_> = k.kbd_out.outputs.events[before..].to_vec();
        assert!(
            after.iter().any(|e| e.contains("↓Y")),
            "the keyboard stopped working after the reload: {after:?}"
        );
    }

    #[test]
    fn a_state_clean_lets_go_without_disabling_the_pointer() {
        // `clean_state` cleans state, not configuration. Switching the pointer
        // off here would leave a mouse stick dead for the rest of the process,
        // with nothing to say why and no reload to put it back.
        let _lk = lock_cfg();
        let mut k = with_pad(
            "(defcfg process-unmapped-keys no)
             (defgamepad (stick right (mouse (deadzone 0.0) (speed 20000) (curve linear))))
             (defsrc pad-a)
             (deflayer base x)",
        );
        feed(&mut k, south(true));
        feed(&mut k, stick(Side::Right, 1.0, 0.0));

        let mut edges = Vec::new();
        k.gamepad
            .as_mut()
            .expect("a controller")
            .release_all(&mut edges);
        k.apply_gamepad_edges(&edges).expect("edges apply");

        assert!(
            k.gamepad.as_ref().expect("a controller").drives_motion(),
            "a release must not switch the pointer off for good"
        );
        // And the stick still drives it: the projection was never the thing
        // being released.
        feed(&mut k, stick(Side::Right, 1.0, 0.0));
        assert!(!k.gamepad.as_ref().expect("a controller").demand().is_idle());
    }
}
