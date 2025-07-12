use std::thread::sleep;
use std::time::Duration;

use crate::Kanata;

use web_time::Instant;

#[test]
fn one_second_is_roughly_1000_counted_ticks() {
    let mut k = Kanata::new_from_str("(defsrc)(deflayer base)", Default::default())
        .expect("failed to parse cfg");

    let mut accumulated_ticks = 0;

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(1) {
        sleep(Duration::from_millis(1));
        accumulated_ticks += k.get_ms_elapsed();
    }

    let actually_elapsed_ms = start.elapsed().as_millis();

    // Allow fudge of 1%
    // In practice this is within 1ms purely due to the remainder.
    eprintln!("ticks:{accumulated_ticks}, actual elapsed:{actually_elapsed_ms}");
    assert!(accumulated_ticks < (actually_elapsed_ms + 10));
    assert!(accumulated_ticks > (actually_elapsed_ms - 10));
}
