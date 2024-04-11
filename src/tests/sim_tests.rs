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
(defcfg process-unmapped-keys yes concurrent-tap-hold yes) \
(defsrc) \
(deflayer base) \
(defchordsv2-experimental \
  (a b) c 200 last-release () \
  (a b z) d 250 first-release () \
)";

#[test]
fn sim_overlapping_timeout() {
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
fn sim_overlapping_release() {
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
        "t:51ms\nout:↓C\nt:100ms\nout:↑C\nt:50ms\nout:↓D\nt:50ms\n\
        out:↑D",
        result
    );
}
