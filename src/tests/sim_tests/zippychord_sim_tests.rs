use super::*;

#[test]
fn sim_zippychord_capitalize() {
    let result = simulate_with_file_content(
        "(defsrc)(deflayer base)(defzippy-experimental file)",
        "d:a t:10 d:b t:10 d:c t:300",
        Some("abc\tAlphabet"),
    )
    .to_ascii();
    assert_eq!(
        "dn:A t:10ms dn:B t:10ms \
         dn:BSpace up:BSpace dn:BSpace up:BSpace \
         up:A dn:LShift dn:A up:A up:LShift up:LShift \
         dn:L up:L dn:P up:P dn:H up:H dn:A up:A up:B dn:B up:B dn:E up:E dn:T up:T",
        result
    );
}
