use kanata_parser::cfg::*;
use std::sync::Mutex;

#[cfg(all(
    feature = "simulated_output",
    not(feature = "simulated_input"),
    not(target_os = "macos"),
    not(feature = "interception_driver")
))]
mod sim_tests;

static CFG_PARSE_LOCK: Mutex<()> = Mutex::new(());

fn init_log() {
    use simplelog::*;
    use std::sync::OnceLock;
    static LOG_INIT: OnceLock<()> = OnceLock::new();
    LOG_INIT.get_or_init(|| {
        let mut log_cfg = ConfigBuilder::new();
        if let Err(e) = log_cfg.set_time_offset_to_local() {
            eprintln!("WARNING: could not set log TZ to local: {e:?}");
        };
        log_cfg.set_time_format_rfc3339();
        CombinedLogger::init(vec![TermLogger::new(
            // Note: set to a different level to see logs in tests.
            // Also, not all tests call init_log so you might have to add the call there too.
            LevelFilter::Off,
            log_cfg.build(),
            TerminalMode::Stderr,
            ColorChoice::AlwaysAnsi,
        )])
        .expect("logger can init");
    });
}

#[test]
fn parse_simple() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from("./cfg_samples/simple.kbd")).unwrap();
}

#[test]
fn parse_minimal() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from("./cfg_samples/minimal.kbd")).unwrap();
}

#[test]
fn parse_deflayermap() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from("./cfg_samples/deflayermap.kbd")).unwrap();
}

#[test]
fn parse_default() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from("./cfg_samples/kanata.kbd")).unwrap();
}

#[test]
fn parse_jtroo() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let cfg = new_from_file(&std::path::PathBuf::from("./cfg_samples/jtroo.kbd")).unwrap();
    assert_eq!(cfg.layer_info.len(), 8);
}

#[test]
fn parse_f13_f24() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from("./cfg_samples/f13_f24.kbd")).unwrap();
}

#[test]
fn parse_home_row_mods() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from(
        "./cfg_samples/home-row-mod-basic.kbd",
    ))
    .unwrap();
    new_from_file(&std::path::PathBuf::from(
        "./cfg_samples/home-row-mod-advanced.kbd",
    ))
    .unwrap();
}

#[test]
fn parse_press_release_toggle_vkeys() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from(
        "./cfg_samples/key-toggle_press-only_release-only.kbd",
    ))
    .unwrap();
}

#[test]
fn parse_automousekeys_only() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from(
        "./cfg_samples/automousekeys-only.kbd",
    ))
    .unwrap();
}

#[test]
fn parse_automousekeys_full_map() {
    init_log();
    let _lk = match CFG_PARSE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    new_from_file(&std::path::PathBuf::from(
        "./cfg_samples/automousekeys-full-map.kbd",
    ))
    .unwrap();
}

#[test]
#[cfg(target_pointer_width = "64")]
fn sizeof_state() {
    init_log();
    assert_eq!(
        std::mem::size_of::<
            kanata_keyberon::layout::State<
                &'static &'static [&'static kanata_parser::custom_action::CustomAction],
            >,
        >(),
        2 * std::mem::size_of::<usize>()
    );
}
