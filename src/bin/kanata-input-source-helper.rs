#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode};

    let _ = TermLogger::init(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    );

    kanata_state_machine::macos_input_source::serve_helper_forever()
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("kanata-input-source-helper is only supported on macOS");
    std::process::exit(1);
}
