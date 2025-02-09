use super::*;

static ZIPPY_CFG: &str = "(defsrc lalt)(deflayer base (caps-word 2000))(defzippy file)";
static ZIPPY_FILE_CONTENT: &str = "
dy	day
dy 1	Monday
 abc	Alphabet
pr	pre ⌫
pra	partner
pr q	pull request
r df	recipient
 w  a	Washington
xy	WxYz
rq	request
rqa	request␣assistance
.g	git
.g f p	git fetch -p
12	hi
1234	bye
";

fn simulate_with_zippy_file_content(cfg: &str, input: &str, content: &str) -> String {
    let mut fcontent = FxHashMap::default();
    fcontent.insert("file".into(), content.into());
    simulate_with_file_content(cfg, input, fcontent)
}

#[test]
fn sim_zippychord_capitalize() {
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:a t:10 d:b t:10 d:spc t:10 d:c u:a u:b u:c u:spc t:300 \
         d:a t:10 d:b t:10 d:spc t:10 d:c t:300",
        ZIPPY_FILE_CONTENT,
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
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:d t:10 d:y t:10 u:d u:y t:10 d:1 t:300",
        ZIPPY_FILE_CONTENT,
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
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:r t:10 u:r t:10 d:d d:f t:10 t:300",
        ZIPPY_FILE_CONTENT,
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
fn sim_zippychord_washington() {
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:w d:spc t:10
         u:w u:spc t:10
         d:a d:spc t:10
         u:a u:spc t:300",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:W t:1ms dn:Space t:9ms up:W t:1ms up:Space t:9ms \
         dn:A t:1ms dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
         dn:LShift dn:W up:W up:LShift \
         up:A dn:A dn:S up:S dn:H up:H dn:I up:I dn:N up:N dn:G up:G dn:T up:T dn:O up:O dn:N up:N \
         t:9ms up:A t:1ms up:Space",
        result
    );
}

#[test]
fn sim_zippychord_overlap() {
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:r t:10  d:q t:10 d:a t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:R t:10ms dn:BSpace up:BSpace \
        up:R dn:R dn:E up:E up:Q dn:Q dn:U up:U dn:E up:E dn:S up:S dn:T up:T t:10ms \
        dn:Space up:Space \
        up:A dn:A dn:S up:S dn:S up:S dn:I up:I dn:S up:S dn:T up:T up:A dn:A dn:N up:N dn:C up:C dn:E up:E",
        result
    );
    let result =
        simulate_with_zippy_file_content(ZIPPY_CFG, "d:1 d:2 d:3 d:4 t:20", ZIPPY_FILE_CONTENT)
            .to_ascii();
    assert_eq!(
        "dn:Kb1 t:1ms dn:BSpace up:BSpace dn:H up:H dn:I up:I t:1ms dn:Kb3 t:1ms \
         dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
         dn:B up:B dn:Y up:Y dn:E up:E",
        result
    );
}

#[test]
fn sim_zippychord_lsft() {
    // test lsft behaviour while pressed
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:lsft t:10 d:d t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:LShift t:10ms dn:D t:10ms dn:BSpace up:BSpace up:D dn:D up:LShift dn:A up:A up:Y dn:Y dn:LShift",
        result
    );
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:lsft t:10 d:x t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:LShift t:10ms dn:X t:10ms dn:BSpace up:BSpace \
         dn:W up:W up:LShift up:X dn:X dn:LShift up:Y dn:Y up:LShift dn:Z up:Z dn:LShift",
        result
    );

    // ensure lsft-held behaviour goes away when released
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:lsft t:10 d:d u:lsft t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:LShift t:10ms dn:D t:1ms up:LShift t:9ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y",
        result
    );
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:lsft t:10 d:x u:lsft t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
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
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:rsft t:10 d:d t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:RShift t:10ms dn:D t:10ms dn:BSpace up:BSpace up:D dn:D up:RShift dn:A up:A up:Y dn:Y dn:RShift",
        result
    );
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:rsft t:10 d:x t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:RShift t:10ms dn:X t:10ms dn:BSpace up:BSpace \
         dn:W up:W up:RShift up:X dn:X dn:LShift up:Y dn:Y up:LShift dn:Z up:Z dn:RShift",
        result
    );

    // ensure rsft-held behaviour goes away when released
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:rsft t:10 d:d u:rsft t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:RShift t:10ms dn:D t:1ms up:RShift t:9ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y",
        result
    );
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:rsft t:10 d:x u:rsft t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:RShift t:10ms dn:X t:1ms up:RShift t:9ms dn:BSpace up:BSpace \
         dn:LShift dn:W up:W up:LShift up:X dn:X dn:LShift up:Y dn:Y up:LShift dn:Z up:Z",
        result
    );
}

#[test]
fn sim_zippychord_ralt() {
    // test ralt behaviour while pressed
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:ralt t:10 d:d t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:RAlt t:10ms dn:D t:10ms dn:BSpace up:BSpace up:RAlt up:D dn:D dn:A up:A up:Y dn:Y dn:RAlt",
        result
    );
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:ralt t:10 d:x t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:RAlt t:10ms dn:X t:10ms dn:BSpace up:BSpace \
         up:RAlt dn:LShift dn:W up:W up:LShift up:X dn:X dn:LShift up:Y dn:Y up:LShift dn:Z up:Z dn:RAlt",
        result
    );

    // ensure rsft-held behaviour goes away when released
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:ralt t:10 d:d u:ralt t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:RAlt t:10ms dn:D t:1ms up:RAlt t:9ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y",
        result
    );
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:ralt t:10 d:x u:ralt t:10 d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:RAlt t:10ms dn:X t:1ms up:RAlt t:9ms dn:BSpace up:BSpace \
         dn:LShift dn:W up:W up:LShift up:X dn:X dn:LShift up:Y dn:Y up:LShift dn:Z up:Z",
        result
    );
}

#[test]
fn sim_zippychord_caps_word() {
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:lalt u:lalt t:10 d:d t:10 d:y t:10 u:d u:y t:10 d:spc u:spc t:2000 d:d d:y t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "t:10ms dn:LShift dn:D t:10ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y \
         t:10ms up:D t:1ms up:LShift up:Y t:9ms dn:Space t:1ms up:Space \
         t:1999ms dn:D t:1ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y",
        result
    );
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:lalt t:10 d:y t:10 d:x t:10 u:x u:y t:10 d:spc u:spc t:1000 d:y d:x t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "t:10ms dn:LShift dn:Y t:10ms dn:BSpace up:BSpace \
         dn:W up:W up:X dn:X up:Y dn:Y dn:Z up:Z \
         t:10ms up:X t:1ms up:LShift up:Y t:9ms dn:Space t:1ms up:Space \
         t:999ms dn:Y t:1ms dn:BSpace up:BSpace dn:LShift dn:W up:W up:LShift \
         up:X dn:X dn:LShift up:Y dn:Y up:LShift dn:Z up:Z",
        result
    );
}

#[test]
fn sim_zippychord_triple_combo() {
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:. d:g t:10 u:. u:g d:f t:10 u:f d:p t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:Dot t:1ms dn:BSpace up:BSpace up:G dn:G dn:I up:I dn:T up:T t:9ms up:Dot t:1ms up:G \
         t:1ms dn:F t:8ms up:F t:1ms \
         dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
         dn:G up:G dn:I up:I dn:T up:T dn:Space up:Space \
         dn:F up:F dn:E up:E dn:T up:T dn:C up:C dn:H up:H dn:Space up:Space \
         dn:Minus up:Minus up:P dn:P",
        result
    );
}

#[test]
fn sim_zippychord_disabled_by_typing() {
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:v u:v t:10 d:d d:y t:100",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!("dn:V t:1ms up:V t:9ms dn:D t:1ms dn:Y", result);
}

#[test]
fn sim_zippychord_prefix() {
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:p d:r u:p u:r t:10 d:q u:q t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:P t:1ms dn:BSpace up:BSpace up:P dn:P up:R dn:R dn:E up:E dn:Space up:Space \
         dn:BSpace up:BSpace t:1ms up:P t:1ms up:R t:7ms \
         dn:BSpace up:BSpace dn:BSpace up:BSpace \
         dn:U up:U dn:L up:L dn:L up:L dn:Space up:Space \
         dn:R up:R dn:E up:E up:Q dn:Q dn:U up:U dn:E up:E dn:S up:S dn:T up:T t:1ms up:Q",
        result
    );
    let result = simulate_with_zippy_file_content(
        ZIPPY_CFG,
        "d:p d:r d:a t:10 u:d u:r u:a",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii()
    .no_time()
    .no_releases();
    assert_eq!(
        "dn:P dn:BSpace \
         dn:P dn:R dn:E dn:Space dn:BSpace \
         dn:BSpace dn:BSpace dn:A dn:R dn:T dn:N dn:E dn:R",
        result
    );
}

#[test]
fn sim_zippychord_smartspace_full() {
    let result = simulate_with_zippy_file_content(
        "(defsrc)(deflayer base)(defzippy file
         smart-space full)",
        "d:d d:y t:10 u:d u:y t:100 d:. t:10 u:. t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:D t:1ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y dn:Space up:Space \
         t:9ms up:D t:1ms up:Y t:99ms dn:BSpace up:BSpace dn:Dot t:10ms up:Dot",
        result
    );

    // Test that prefix works as intended.
    let result = simulate_with_zippy_file_content(
        "(defsrc)(deflayer base)(defzippy file
         smart-space add-space-only)",
        "d:p d:r t:10 u:p u:r t:100 d:. t:10 u:. t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:P t:1ms dn:BSpace up:BSpace up:P dn:P up:R dn:R dn:E up:E \
         dn:Space up:Space dn:BSpace up:BSpace \
         t:9ms up:P t:1ms up:R t:99ms dn:Dot t:10ms up:Dot",
        result
    );
}

#[test]
fn sim_zippychord_smartspace_spaceonly() {
    let result = simulate_with_zippy_file_content(
        "(defsrc)(deflayer base)(defzippy file
         smart-space add-space-only)",
        "d:d d:y t:10 u:d u:y t:100 d:. t:10 u:. t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:D t:1ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y dn:Space up:Space \
         t:9ms up:D t:1ms up:Y t:99ms dn:Dot t:10ms up:Dot",
        result
    );

    // Test that prefix works as intended.
    let result = simulate_with_zippy_file_content(
        "(defsrc)(deflayer base)(defzippy file
         smart-space add-space-only)",
        "d:p d:r t:10 u:p u:r t:100 d:. t:10 u:. t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:P t:1ms dn:BSpace up:BSpace up:P dn:P up:R dn:R dn:E up:E \
         dn:Space up:Space dn:BSpace up:BSpace \
         t:9ms up:P t:1ms up:R t:99ms dn:Dot t:10ms up:Dot",
        result
    );
}

#[test]
fn sim_zippychord_smartspace_none() {
    let result = simulate_with_zippy_file_content(
        "(defsrc)(deflayer base)(defzippy file
         smart-space none)",
        "d:d d:y t:10 u:d u:y t:100 d:. t:10 u:. t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:D t:1ms dn:BSpace up:BSpace up:D dn:D dn:A up:A up:Y dn:Y \
         t:9ms up:D t:1ms up:Y t:99ms dn:Dot t:10ms up:Dot",
        result
    );

    // Test that prefix works as intended.
    let result = simulate_with_zippy_file_content(
        "(defsrc)(deflayer base)(defzippy file
         smart-space add-space-only)",
        "d:p d:r t:10 u:p u:r t:100 d:. t:10 u:. t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:P t:1ms dn:BSpace up:BSpace up:P dn:P up:R dn:R dn:E up:E \
         dn:Space up:Space dn:BSpace up:BSpace \
         t:9ms up:P t:1ms up:R t:99ms dn:Dot t:10ms up:Dot",
        result
    );
}

#[test]
fn sim_zippychord_smartspace_overlap() {
    let result = simulate_with_zippy_file_content(
        "(defsrc)(deflayer base)(defzippy file
         smart-space full)",
        "d:r t:10 d:q t:10 d:a t:10",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:R t:10ms dn:BSpace up:BSpace \
        up:R dn:R dn:E up:E up:Q dn:Q dn:U up:U dn:E up:E dn:S up:S dn:T up:T dn:Space up:Space t:10ms \
        dn:BSpace up:BSpace dn:Space up:Space \
        up:A dn:A dn:S up:S dn:S up:S dn:I up:I dn:S up:S dn:T up:T up:A dn:A dn:N up:N dn:C up:C dn:E up:E \
        dn:Space up:Space",
        result
    );
    let result = simulate_with_zippy_file_content(
        "(defsrc)(deflayer base)(defzippy file
         smart-space full)",
        "d:1 d:2 d:3 d:4 t:20",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:Kb1 t:1ms dn:BSpace up:BSpace dn:H up:H dn:I up:I dn:Space up:Space \
         t:1ms dn:Kb3 t:1ms \
         dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
         dn:B up:B dn:Y up:Y dn:E up:E dn:Space up:Space",
        result
    );
}

#[test]
fn sim_zippychord_smartspace_followup() {
    let result = simulate_with_zippy_file_content(
        "(defsrc)(deflayer base)(defzippy file
         smart-space full)",
        "d:d t:10 d:y t:10 u:d u:y t:10 d:1 t:300",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:D t:10ms dn:BSpace up:BSpace \
         up:D dn:D dn:A up:A up:Y dn:Y dn:Space up:Space \
         t:10ms up:D t:1ms up:Y t:9ms \
         dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
         dn:LShift dn:M up:M up:LShift dn:O up:O dn:N up:N dn:D up:D dn:A up:A dn:Y up:Y dn:Space up:Space",
        result
    );
}

const CUSTOM_PUNC_CFG: &str = "\
(defsrc)
(deflayer base)
(defzippy file
 smart-space full
 smart-space-punctuation (z ! ® *)
 output-character-mappings (
   ® AG-r
   * S-AG-v
   ! S-1))";

#[test]
fn sim_zippychord_smartspace_custom_punc() {
    // 1 without lsft: no smart-space-erase
    let result = simulate_with_zippy_file_content(
        CUSTOM_PUNC_CFG,
        "d:d t:10 d:y t:10 u:d u:y t:10 d:1 t:300",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:D t:10ms dn:BSpace up:BSpace \
         up:D dn:D dn:A up:A up:Y dn:Y dn:Space up:Space \
         t:10ms up:D t:1ms up:Y t:9ms \
         dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace dn:BSpace up:BSpace \
         dn:LShift dn:M up:M up:LShift dn:O up:O dn:N up:N dn:D up:D dn:A up:A dn:Y up:Y dn:Space up:Space",
        result
    );

    // S-1 = !: smart-space-erase
    let result = simulate_with_zippy_file_content(
        CUSTOM_PUNC_CFG,
        "d:1 d:2 t:10 u:1 u:2 t:10 d:lsft d:1 u:1 u:lsft t:300",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:Kb1 t:1ms dn:BSpace up:BSpace \
         dn:H up:H dn:I up:I dn:Space up:Space t:9ms \
         up:Kb1 t:1ms up:Kb2 t:9ms \
         dn:LShift t:1ms dn:BSpace up:BSpace dn:Kb1 t:1ms up:Kb1 t:1ms up:LShift",
        result
    );

    // z: smart-space-erase
    let result = simulate_with_zippy_file_content(
        CUSTOM_PUNC_CFG,
        "d:1 d:2 t:10 u:1 u:2 t:10 d:z u:z t:300",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:Kb1 t:1ms dn:BSpace up:BSpace \
         dn:H up:H dn:I up:I dn:Space up:Space t:9ms \
         up:Kb1 t:1ms up:Kb2 t:9ms \
         dn:BSpace up:BSpace dn:Z t:1ms up:Z",
        result
    );

    // r no altgr: no smart-space-erase
    let result = simulate_with_zippy_file_content(
        CUSTOM_PUNC_CFG,
        "d:1 d:2 t:10 u:1 u:2 t:10 d:r u:r t:300",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:Kb1 t:1ms dn:BSpace up:BSpace \
         dn:H up:H dn:I up:I dn:Space up:Space t:9ms \
         up:Kb1 t:1ms up:Kb2 t:9ms \
         dn:R t:1ms up:R",
        result
    );

    // r with altgr: smart-space-erase
    let result = simulate_with_zippy_file_content(
        CUSTOM_PUNC_CFG,
        "d:1 d:2 t:10 u:1 u:2 t:10 d:ralt d:r u:r u:ralt t:300",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:Kb1 t:1ms dn:BSpace up:BSpace \
         dn:H up:H dn:I up:I dn:Space up:Space t:9ms \
         up:Kb1 t:1ms up:Kb2 t:9ms \
         dn:RAlt t:1ms dn:BSpace up:BSpace dn:R t:1ms up:R t:1ms up:RAlt",
        result
    );

    // v with altgr+lsft: smart-space-erase
    let result = simulate_with_zippy_file_content(
        CUSTOM_PUNC_CFG,
        "d:1 d:2 t:10 u:1 u:2 t:10 d:ralt d:lsft d:v u:v u:ralt u:lsft t:300",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:Kb1 t:1ms dn:BSpace up:BSpace \
         dn:H up:H dn:I up:I dn:Space up:Space t:9ms \
         up:Kb1 t:1ms up:Kb2 t:9ms \
         dn:RAlt t:1ms dn:LShift t:1ms dn:BSpace up:BSpace dn:V t:1ms up:V t:1ms up:RAlt t:1ms up:LShift",
        result
    );
}

#[test]
fn sim_zippychord_non_followup_subsequent_with_potential_followups_available() {
    let result = simulate_with_zippy_file_content(
        "(defsrc)(deflayer base)(defzippy file
         smart-space full)",
        "d:g d:. t:10 u:g u:. t:1000 d:g d:. t:10 u:g u:. t:1000",
        ZIPPY_FILE_CONTENT,
    )
    .to_ascii();
    assert_eq!(
        "dn:G t:1ms dn:BSpace up:BSpace up:G dn:G dn:I up:I dn:T up:T dn:Space up:Space t:9ms \
         up:G t:1ms up:Dot t:999ms \
         dn:G t:1ms dn:BSpace up:BSpace up:G dn:G dn:I up:I dn:T up:T dn:Space up:Space t:9ms \
         up:G t:1ms up:Dot",
        result
    );
}

const DEAD_KEYS_CFG: &str = "\
(defsrc)
(deflayer base)
(defzippy file
 smart-space full
 output-character-mappings (
   ’ (no-erase ')
   ‘ (no-erase `)
   é (single-output ' e)
   è (single-output ` e)
 ))";
static DEAD_KEYS_FILE_CONTENT: &str = "
by	h’elo
bye	by‘e
by d	ft‘a’ng
by d a	aye
cy	hélo
cye	byè
cy d	ftéèng
cy d a	aye
";

#[test]
fn sim_zippychord_noerase() {
    let result = simulate_with_zippy_file_content(
        DEAD_KEYS_CFG,
        "d:b d:y t:100 d:e u:b u:y u:e t:1000",
        DEAD_KEYS_FILE_CONTENT,
    )
    .no_releases()
    .no_time()
    .to_ascii();
    assert_eq!(
        "dn:B dn:BSpace dn:H dn:Quote dn:E dn:L dn:O dn:Space \
         dn:BSpace dn:BSpace dn:BSpace dn:BSpace dn:BSpace \
         dn:B dn:Y dn:Grave dn:E dn:Space",
        result,
    );

    let result = simulate_with_zippy_file_content(
        DEAD_KEYS_CFG,
        "d:b d:y t:100 u:b u:y d:d t:10 u:d d:a t:10 u:a t:1000",
        DEAD_KEYS_FILE_CONTENT,
    )
    .no_releases()
    .no_time()
    .to_ascii();
    assert_eq!(
        "dn:B dn:BSpace dn:H dn:Quote dn:E dn:L dn:O dn:Space \
         dn:BSpace dn:BSpace dn:BSpace dn:BSpace dn:BSpace \
         dn:F dn:T dn:Grave dn:A dn:Quote dn:N dn:G dn:Space \
         dn:BSpace dn:BSpace dn:BSpace dn:BSpace dn:BSpace dn:BSpace \
         dn:A dn:Y dn:E dn:Space",
        result,
    );
}

#[test]
fn sim_zippychord_single_output() {
    let result = simulate_with_zippy_file_content(
        DEAD_KEYS_CFG,
        "d:c d:y t:100 d:e u:c u:y u:e t:1000",
        DEAD_KEYS_FILE_CONTENT,
    )
    .no_releases()
    .no_time()
    .to_ascii();
    assert_eq!(
        "dn:C dn:BSpace dn:H dn:Quote dn:E dn:L dn:O dn:Space \
         dn:BSpace dn:BSpace dn:BSpace dn:BSpace dn:BSpace \
         dn:B dn:Y dn:Grave dn:E dn:Space",
        result,
    );

    let result = simulate_with_zippy_file_content(
        DEAD_KEYS_CFG,
        "d:c d:y t:100 u:c u:y d:d t:10 u:d d:a t:10 u:a t:1000",
        DEAD_KEYS_FILE_CONTENT,
    )
    .no_releases()
    .no_time()
    .to_ascii();
    assert_eq!(
        "dn:C dn:BSpace dn:H dn:Quote dn:E dn:L dn:O dn:Space \
         dn:BSpace dn:BSpace dn:BSpace dn:BSpace dn:BSpace \
         dn:F dn:T dn:Quote dn:E dn:Grave dn:E dn:N dn:G dn:Space \
         dn:BSpace dn:BSpace dn:BSpace dn:BSpace dn:BSpace dn:BSpace dn:BSpace \
         dn:A dn:Y dn:E dn:Space",
        result,
    );
}
