use super::*;

static ZIPPY_CFG: &str = "(defsrc)(deflayer base)(defzippy-experimental file)";
static ZIPPY_FILE_CONTENT: &str = "
dy	day
dy 1	Monday
 abc	Alphabet
r df	recipient
 w  a	Washington
";

#[test]
fn sim_zippychord_capitalize() {
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:a t:10 d:b t:10 d:spc t:10 d:c t:300",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:A t:10ms dn:B t:10ms dn:Space t:10ms \
         dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
         up:A dn:LShift dn:A up:A up:LShift up:LShift \
         dn:L up:L dn:P up:P dn:H up:H dn:A up:A up:B dn:B up:B dn:E up:E dn:T up:T",
        result
    );
}

#[test]
fn sim_zippychord_followup_with_prev() {
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:d t:10 d:y t:10 u:d u:y t:10 d:1 t:300",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:D t:10ms dn:BSpace up:BSpace \
        up:D dn:D up:D up:LShift dn:A up:A up:Y dn:Y up:Y \
        t:10ms up:D t:1ms up:Y t:9ms \
        dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
        dn:LShift dn:M up:M up:LShift up:LShift dn:O up:O dn:N up:N dn:D up:D dn:A up:A dn:Y up:Y",
        result
    );
}

#[test]
fn sim_zippychord_followup_no_prev() {
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:r t:10 u:r t:10 d:d d:f t:10 t:300",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "t:10ms up:R t:10ms dn:D t:1ms \
        dn:BSpace up:BSpace dn:BSpace up:BSpace \
        dn:R up:R up:LShift dn:E up:E dn:C up:C dn:I up:I dn:P up:P dn:I up:I dn:E up:E dn:N up:N dn:T up:T",
        result
    );
}
