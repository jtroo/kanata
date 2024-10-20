use super::*;

static ZIPPY_CFG: &str =
    "(defsrc lalt)(deflayer base (caps-word 2000))(defzippy-experimental file)";
static ZIPPY_FILE_CONTENT: &str = "
dy	day
dy 1	Monday
 abc	Alphabet
r df	recipient
 w  a	Washington
xy	WxYz
rq	request
rqa	request␣assistance
";

#[test]
fn sim_zippychord_capitalize() {
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:a t:10 d:b t:10 d:spc t:10 d:c u:a u:b u:c u:spc t:300 \
         d:a t:10 d:b t:10 d:spc t:10 d:c t:300",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:A t:10ms dn:B t:10ms dn:Space t:10ms \
         dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
         dn:LShift up:A dn:A up:LShift \
         dn:L up:L dn:P up:P dn:H up:H up:A dn:A up:B dn:B dn:E up:E dn:T up:T \
         t:1ms up:A t:1ms up:B t:1ms up:C t:1ms up:Space t:296ms \
         dn:A t:10ms dn:B t:10ms dn:Space t:10ms \
         dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
         dn:LShift up:A dn:A up:LShift \
         dn:L up:L dn:P up:P dn:H up:H up:A dn:A up:B dn:B dn:E up:E dn:T up:T",
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
         up:D dn:D dn:A up:A up:Y dn:Y \
         t:10ms up:D t:1ms up:Y t:9ms \
         dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
         dn:LShift dn:M up:M up:LShift dn:O up:O dn:N up:N dn:D up:D dn:A up:A dn:Y up:Y",
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
        "dn:R t:10ms up:R t:10ms dn:D t:1ms \
        dn:BSpace up:BSpace dn:BSpace up:BSpace \
        dn:R up:R dn:E up:E dn:C up:C dn:I up:I dn:P up:P dn:I up:I dn:E up:E dn:N up:N dn:T up:T",
        result
    );
}

#[test]
fn sim_zippychord_overlap() {
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:r t:10  d:q t:10 d:a t:10",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:R t:10ms dn:BSpace up:BSpace \
        up:R dn:R dn:E up:E up:Q dn:Q dn:U up:U dn:E up:E dn:S up:S dn:T up:T t:10ms \
        dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
        dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
        up:R dn:R dn:E up:E up:Q dn:Q dn:U up:U dn:E up:E dn:S up:S dn:T up:T \
        dn:Space up:Space \
        up:A dn:A dn:S up:S dn:S up:S dn:I up:I dn:S up:S dn:T up:T up:A dn:A dn:N up:N dn:C up:C dn:E up:E",
        result
    );
}

#[test]
fn sim_zippychord_lsft() {
    // test lsft behaviour while pressed
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:lsft t:10 d:d t:10 d:y t:10",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:LShift t:10ms dn:D t:10ms dn:BSpace up:BSpace up:D dn:D up:LShift dn:A up:A up:Y dn:Y dn:LShift",
        result
    );
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:lsft t:10 d:x t:10 d:y t:10",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:LShift t:10ms dn:X t:10ms dn:BSpace up:BSpace \
         dn:W up:W up:LShift up:X dn:X dn:LShift up:Y dn:Y up:LShift dn:Z up:Z dn:LShift",
        result
    );

    // ensure lsft-held behaviour goes away when released
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:lsft t:10 d:d u:lsft t:10 d:y t:10",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:LShift t:10ms dn:D t:1ms up:LShift t:9ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y",
        result
    );
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:lsft t:10 d:x u:lsft t:10 d:y t:10",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:LShift t:10ms dn:X t:1ms up:LShift t:9ms dn:BSpace up:BSpace \
         dn:LShift dn:W up:W up:LShift up:X dn:X dn:LShift up:Y dn:Y up:LShift dn:Z up:Z",
        result
    );
}

#[test]
fn sim_zippychord_rsft() {
    // test rsft behaviour while pressed
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:rsft t:10 d:d t:10 d:y t:10",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:RShift t:10ms dn:D t:10ms dn:BSpace up:BSpace up:D dn:D up:RShift dn:A up:A up:Y dn:Y dn:RShift",
        result
    );
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:rsft t:10 d:x t:10 d:y t:10",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:RShift t:10ms dn:X t:10ms dn:BSpace up:BSpace \
         dn:W up:W up:RShift up:X dn:X dn:LShift up:Y dn:Y up:LShift dn:Z up:Z dn:RShift",
        result
    );

    // ensure rsft-held behaviour goes away when released
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:rsft t:10 d:d u:rsft t:10 d:y t:10",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:RShift t:10ms dn:D t:1ms up:RShift t:9ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y",
        result
    );
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:rsft t:10 d:x u:rsft t:10 d:y t:10",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "dn:RShift t:10ms dn:X t:1ms up:RShift t:9ms dn:BSpace up:BSpace \
         dn:LShift dn:W up:W up:LShift up:X dn:X dn:LShift up:Y dn:Y up:LShift dn:Z up:Z",
        result
    );
}

#[test]
fn sim_zippychord_caps_word() {
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:lalt u:lalt t:10 d:d t:10 d:y t:10 u:d u:y t:10 d:spc u:spc t:10 d:d d:y t:10",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "t:10ms dn:LShift dn:D t:10ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y \
         t:10ms up:D t:1ms up:LShift up:Y t:9ms dn:Space t:1ms up:Space \
         t:9ms dn:D t:1ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y",
        result
    );
    let result = simulate_with_file_content(
        ZIPPY_CFG,
        "d:lalt t:10 d:y t:10 d:x t:10 u:x u:y t:10 d:spc u:spc t:10 d:y d:x t:10",
        Some(ZIPPY_FILE_CONTENT),
    )
    .to_ascii();
    assert_eq!(
        "t:10ms dn:LShift dn:Y t:10ms dn:BSpace up:BSpace \
         dn:W up:W up:X dn:X up:Y dn:Y dn:Z up:Z \
         t:10ms up:X t:1ms up:LShift up:Y t:9ms dn:Space t:1ms up:Space \
         t:9ms dn:Y t:1ms dn:BSpace up:BSpace dn:LShift dn:W up:W up:LShift \
         up:X dn:X dn:LShift up:Y dn:Y up:LShift dn:Z up:Z",
        result
    );
}
