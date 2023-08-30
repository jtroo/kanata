use kanata_parser::cfg::*;
use std::sync::Mutex;

static CFG_PARSE_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn parse_simple() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from("./cfg_samples/simple.kbd")).unwrap();
}

#[test]
fn parse_minimal() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from("./cfg_samples/minimal.kbd")).unwrap();
}

#[test]
fn parse_default() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from("./cfg_samples/kanata.kbd")).unwrap();
}

#[test]
fn parse_jtroo() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let cfg = new_from_file(&std::path::PathBuf::from("./cfg_samples/jtroo.kbd")).unwrap();
    assert_eq!(cfg.layer_info.len(), 16);
}

#[test]
fn parse_f13_f24() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from("./cfg_samples/f13_f24.kbd")).unwrap();
}

#[test]
fn parse_all_keys() {
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from(
        "./cfg_samples/all_keys_in_defsrc.kbd",
    ))
    .unwrap();
}

#[test]
fn sizeof_state() {
    assert_eq!(
        std::mem::size_of::<
            kanata_keyberon::layout::State<
                &'static &'static [&'static kanata_parser::custom_action::CustomAction],
            >,
        >(),
        2 * std::mem::size_of::<usize>()
    );
}
