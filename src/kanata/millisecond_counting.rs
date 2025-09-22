use web_time::Instant;

pub struct MillisecondCountResult {
    pub last_tick: Instant,
    pub ms_elapsed: u128,
    pub ms_remainder_in_ns: u128,
}

pub fn count_ms_elapsed(
    last_tick: Instant,
    now: Instant,
    prev_ms_remainder_in_ns: u128,
) -> MillisecondCountResult {
    const NS_IN_MS: u128 = 1_000_000;
    let ns_elapsed = now.duration_since(last_tick).as_nanos();
    let ns_elapsed_with_rem = ns_elapsed + prev_ms_remainder_in_ns;
    let ms_elapsed = ns_elapsed_with_rem / NS_IN_MS;
    let ms_remainder_in_ns = ns_elapsed_with_rem % NS_IN_MS;

    let last_tick = match ms_elapsed {
        0 => last_tick,
        _ => now,
    };
    MillisecondCountResult {
        last_tick,
        ms_elapsed,
        ms_remainder_in_ns,
    }
}

#[test]
fn ms_counts_0_elapsed_correctly() {
    use std::time::Duration;
    let last_tick = Instant::now();
    let now = last_tick + Duration::from_nanos(999999);
    let result = count_ms_elapsed(last_tick, now, 0);
    assert_eq!(0, result.ms_elapsed);
    assert_eq!(last_tick, result.last_tick);
    assert_eq!(999999, result.ms_remainder_in_ns);
}

#[test]
fn ms_counts_1_elapsed_correctly() {
    use std::time::Duration;
    let last_tick = Instant::now();
    let now = last_tick + Duration::from_nanos(1234567);
    let result = count_ms_elapsed(last_tick, now, 0);
    assert_eq!(1, result.ms_elapsed);
    assert_eq!(now, result.last_tick);
    assert_eq!(234567, result.ms_remainder_in_ns);
}

#[test]
fn ms_counts_1_then_2_elapsed_correctly() {
    use std::time::Duration;
    let last_tick = Instant::now();
    let now = last_tick + Duration::from_micros(1750);
    let result = count_ms_elapsed(last_tick, now, 0);
    assert_eq!(1, result.ms_elapsed);
    assert_eq!(now, result.last_tick);
    assert_eq!(750000, result.ms_remainder_in_ns);
    let last_tick = result.last_tick;
    let now = last_tick + Duration::from_micros(1750);
    let result = count_ms_elapsed(last_tick, now, result.ms_remainder_in_ns);
    assert_eq!(2, result.ms_elapsed);
    assert_eq!(now, result.last_tick);
    assert_eq!(500000, result.ms_remainder_in_ns);
}
