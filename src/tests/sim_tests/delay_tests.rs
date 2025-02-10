use super::*;

#[test]
#[ignore] // timing-based: fails intermittently
fn on_press_delay() {
    let start = std::time::Instant::now();
    let result = simulate(
        "(defsrc) (deflayermap (base) a (on-press-delay 10))",
        "d:a t:50 u:a t:50",
    );
    assert_eq!("", result);
    let end = std::time::Instant::now();
    let duration = end - start;
    assert!(duration > std::time::Duration::from_millis(9));
    assert!(duration < std::time::Duration::from_millis(19));
}

#[test]
#[ignore] // timing-based: fails intermittently
fn on_release_delay() {
    let start = std::time::Instant::now();
    let result = simulate(
        "(defsrc) (deflayermap (base) a (on-release-delay 10))",
        "d:a t:50 u:a t:50",
    );
    assert_eq!("", result);
    let end = std::time::Instant::now();
    let duration = end - start;
    assert!(duration > std::time::Duration::from_millis(9));
    assert!(duration < std::time::Duration::from_millis(19));
}

#[test]
#[ignore] // timing-based: fails intermittently
fn no_delay() {
    let start = std::time::Instant::now();
    let result = simulate("(defsrc) (deflayermap (base) a XX)", "d:a t:50 u:a t:50");
    assert_eq!("", result);
    let end = std::time::Instant::now();
    let duration = end - start;
    assert!(duration < std::time::Duration::from_millis(10));
}
