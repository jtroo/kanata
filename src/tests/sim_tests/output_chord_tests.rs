use super::*;

#[test]
fn output_chord_samekey_has_release() {
    let config = "
        (defsrc a b)
        (deflayer _ S-= =)
        ";
    let result = simulate(config, "d:a t:10 d:b t:10").to_ascii();
    assert_eq!(
        "dn:LShift dn:Equal t:10ms up:LShift up:Equal t:1ms dn:Equal",
        result
    );
    let result = simulate(config, "d:b t:10 d:a t:10").to_ascii();
    assert_eq!("dn:Equal t:10ms up:Equal dn:LShift t:1ms dn:Equal", result);
}

#[test]
fn output_chord_follows_processing_delay_config() {
    let result = simulate(
        "
        (defsrc)
        (deflayermap (base)
         a S-9
         b S-0)
        ",
        "d:a t:10 d:b t:10 u:b t:10 u:a t:10",
    )
    .to_ascii();
    assert_eq!(
        "dn:LShift dn:Kb9 t:10ms up:Kb9 dn:Kb0 t:10ms up:LShift up:Kb0",
        result
    );
}
