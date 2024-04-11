use kanata_state_machine::{
    oskbd::{KeyEvent, KeyValue},
    str_to_oscode, Kanata,
};

fn simulate(cfg: &str, sim: &str) -> String {
    let mut k = Kanata::new_from_str(cfg).expect("failed to parse cfg");
    for pair in sim.split_whitespace() {
        match pair.split_once(':') {
            Some((kind, val)) => match kind {
                "t" => {
                    let tick = str::parse::<u128>(val).expect("valid num for tick");
                    k.tick_ms(tick, &None).unwrap();
                }
                "d" => {
                    let key_code = str_to_oscode(val).expect("valid keycode");
                    k.handle_input_event(&KeyEvent {
                        code: key_code,
                        value: KeyValue::Press,
                    })
                    .expect("input handles fine");
                }
                "u" => {
                    let key_code = str_to_oscode(val).expect("valid keycode");
                    k.handle_input_event(&KeyEvent {
                        code: key_code,
                        value: KeyValue::Release,
                    })
                    .expect("input handles fine");
                }
                "r" => {
                    let key_code = str_to_oscode(val).expect("valid keycode");
                    k.handle_input_event(&KeyEvent {
                        code: key_code,
                        value: KeyValue::Repeat,
                    })
                    .expect("input handles fine");
                }
                _ => panic!("invalid item {pair}"),
            },
            None => panic!("invalid item {pair}"),
        }
    }
    k.kbd_out.outputs.events.join("\n")
}

static SIMPLE_NONOVERLAPPING_CHORD_CFG: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes) \
(defsrc) \
(deflayer base) \
(defchordsv2-experimental \
  (a b) c 200 last-release () \
  (b z) d 200 first-release () \
)";

#[test]
fn sim_chord_basic_repeated_last_release() {
    let result = simulate(
        SIMPLE_NONOVERLAPPING_CHORD_CFG,
        "d:a t:50 d:b t:50 u:a t:50 u:b t:50 \
        d:a t:50 d:b t:50 u:a t:50 u:b t:50 ",
    );
    assert_eq!(
        "t:51ms
out:↓C
t:100ms
out:↑C
t:100ms
out:↓C
t:100ms
out:↑C",
        result
    );
}

#[test]
fn sim_chord_min_idle_takes_effect() {
    let result = simulate(
        SIMPLE_NONOVERLAPPING_CHORD_CFG,
        "d:z t:20 d:a t:20 d:b t:20 d:d t:20",
    );
    assert_eq!(
        "t:21ms
out:↓Z
t:1ms
out:↓A
t:39ms
out:↓B
t:1ms
out:↓D",
        result
    );
}

#[test]
fn sim_timeout_hold_key() {
    let result = simulate(SIMPLE_NONOVERLAPPING_CHORD_CFG, "d:z t:201 d:b t:200");
    assert_eq!(
        "t:201ms
out:↓Z
t:1ms
out:↓B",
        result
    );
}

#[test]
fn sim_chord_basic_repeated_first_release() {
    let result = simulate(
        SIMPLE_NONOVERLAPPING_CHORD_CFG,
        "d:z t:50 d:b t:50 u:z t:50 u:b t:50 \
        d:z t:50 d:b t:50 u:z t:50 u:b t:50 ",
    );
    assert_eq!(
        "t:51ms
out:↓D
t:50ms
out:↑D
t:150ms
out:↓D
t:50ms
out:↑D",
        result
    );
}

static SIMPLE_OVERLAPPING_CHORD_CFG: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes)
(defsrc)
(deflayer base)
(defchordsv2-experimental
  (a b) c 200 last-release ()
  (a b z) d 250 first-release ()
  (a b z y) e 400 first-release ()
)";

#[test]
fn sim_chord_overlapping_timeout() {
    let result = simulate(SIMPLE_OVERLAPPING_CHORD_CFG, "d:a d:b t:201 d:z t:300");
    assert_eq!(
        "t:201ms
out:↓C
t:251ms
out:↓Z",
        result
    );
}

#[test]
fn sim_chord_overlapping_release() {
    let result = simulate(
        SIMPLE_OVERLAPPING_CHORD_CFG,
        "d:a d:b t:100 u:a d:z t:300 u:b t:300",
    );
    assert_eq!(
        "t:101ms
out:↓C
t:250ms
out:↓Z
t:50ms
out:↑C",
        result
    );
}

#[test]
fn sim_presses_for_old_chord_repress_into_new_chord() {
    let result = simulate(
        SIMPLE_OVERLAPPING_CHORD_CFG,
        "d:a d:b t:50 u:a t:50 d:z t:50 u:b t:50 d:a d:b t:50 u:a t:50",
    );
    assert_eq!(
        "t:51ms\nout:↓C\nt:100ms\nout:↑C\nt:100ms\nout:↓D\nt:2ms\n\
        out:↑D",
        result
    );
}

#[test]
fn sim_chord_activate_largest_overlapping() {
    let result = simulate(
        SIMPLE_OVERLAPPING_CHORD_CFG,
        "d:a t:50 d:b t:50 d:z t:50 d:y t:50 u:b t:50",
    );
    assert_eq!(
        "t:151ms\nout:↓E\nt:50ms\nout:↑E",
        result
    );
}

static SIMPLE_DISABLED_LAYER_CHORD_CFG: &str = "\
(defcfg process-unmapped-keys yes concurrent-tap-hold yes)
(defsrc)
(deflayermap (1) 2 : (layer-switch 2)
                 3 : (layer-switch 3))
(deflayermap (2) 3 : (layer-while-held 3)
                 1 : (layer-while-held 1))
(deflayermap (3) 2 : (layer-while-held 2)
                 1 : (layer-while-held 1))
(defchordsv2-experimental
  (a b) x 200 last-release (1)
  (c d) y 200 last-release (2)
  (e f) z 200 last-release (3)
)";

#[test]
fn sim_chord_layer_1_switch_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:1ms\nout:↓A\nt:50ms\nout:↓B\nt:100ms\nout:↓Y\nt:100ms\nout:↓Z",
        result
    );
}

#[test]
fn sim_chord_layer_2_switch_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:2 t:50 d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:101ms\nout:↓X\nt:50ms\nout:↓C\nt:50ms\nout:↓D\nt:100ms\nout:↓Z",
        result
    );
}

#[test]
fn sim_chord_layer_3_switch_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:3 t:50 d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:101ms\nout:↓X\nt:100ms\nout:↓Y\nt:50ms\nout:↓E\nt:50ms\nout:↓F",
        result
    );
}

#[test]
fn sim_chord_layer_1_held_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:3 t:50 d:1 t:50 d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:101ms\nout:↓A\nt:50ms\nout:↓B\nt:100ms\nout:↓Y\nt:100ms\nout:↓Z",
        result
    );
}

#[test]
fn sim_chord_layer_2_held_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:3 t:50 d:2 t:50 d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:151ms\nout:↓X\nt:50ms\nout:↓C\nt:50ms\nout:↓D\nt:100ms\nout:↓Z",
        result
    );
}

#[test]
fn sim_chord_layer_3_held_disabled() {
    let result = simulate(
        SIMPLE_DISABLED_LAYER_CHORD_CFG,
        "d:2 t:50 d:3 t:50 d:a t:50 d:b t:50 d:c t:50 d:d t:50 d:e t:50 d:f t:50",
    );
    assert_eq!(
        "t:151ms\nout:↓X\nt:100ms\nout:↓Y\nt:50ms\nout:↓E\nt:50ms\nout:↓F",
        result
    );
}