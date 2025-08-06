use anyhow::Result;
use anyhow::{anyhow, bail};
use clap::Parser;
use kanata_state_machine::{oskbd::*, *};
use simplelog::{format_description, *};
use std::path::PathBuf;

pub fn default_sim() -> Vec<PathBuf> {
    let mut cfgs = Vec::new();

    let default = PathBuf::from("test/sim.txt");
    if default.is_file() {
        cfgs.push(default);
    }

    if let Some(config_dir) = dirs::config_dir() {
        let fallback = config_dir.join("kanata").join("test").join("sim.txt");
        if fallback.is_file() {
            cfgs.push(fallback);
        }
    }

    cfgs
}

#[derive(Parser, Debug)]
#[command(author, version, verbatim_doc_comment)]
/// kanata_simulated_input: a cli tool that helps debug kanata's user configuration by:
/// - reading a text file with a sequence of key events, including key delays
/// - interpreting them with kanata
/// - printing out which actions or key/mouse events kanata would execute if the keys were
///   pressed by a user
/// - (optionally) saving the result to a file for reference
struct Args {
    // Display different platform specific paths based on the target OS
    #[cfg_attr(
        target_os = "windows",
        doc = r"Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'C:\Users\user\AppData\Roaming\kanata\kanata.kbd'"
    )]
    #[cfg_attr(
        target_os = "macos",
        doc = "Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'$HOME/Library/Application Support/kanata/kanata.kbd.'"
    )]
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows")),
        doc = "Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'$XDG_CONFIG_HOME/kanata/kanata.kbd'"
    )]
    #[arg(short, long, verbatim_doc_comment)]
    cfg: Option<Vec<PathBuf>>,

    // Display different platform specific paths based on the target OS
    #[cfg_attr(
        target_os = "windows",
        doc = r"Simulation file(s) to use with kanata_simulated_input. If not specified, defaults to
test\sim.txt in the current working directory and
'C:\Users\user\AppData\Roaming\kanata\test\sim.txt'"
    )]
    #[cfg_attr(
        target_os = "macos",
        doc = "Simulation file(s) to use with kanata_simulated_input. If not specified, defaults to
test/sim.txt in the current working directory and
'$HOME/Library/Application Support/kanata/test/sim.txt.'"
    )]
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows")),
        doc = "Simulation file(s) to use with kanata_simulated_input. If not specified, defaults to
test/sim.txt in the current working directory and
'$XDG_CONFIG_HOME/kanata/test/sim.txt'"
    )]
    #[arg(short = 's', long, verbatim_doc_comment)]
    sim: Option<Vec<PathBuf>>,
    /// Save output to the simulation file's path with its name appended by the value of this argument.
    /// This flag generates an error if the binary is compiled without simulated output.
    #[arg(short = 'o', long, verbatim_doc_comment)]
    out: Option<String>,
}

fn log_init() {
    let mut log_cfg = ConfigBuilder::new();
    if let Err(e) = log_cfg.set_time_offset_to_local() {
        eprintln!("WARNING: could not set log TZ to local: {e:?}");
    };
    log_cfg.set_time_format_custom(format_description!(
        version = 2,
        "[hour]:[minute]:[second].[subsecond digits:4]"
    ));
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        log_cfg.build(),
        TerminalMode::Stderr,
        ColorChoice::AlwaysAnsi,
    )])
    .expect("logger can init");
}

/// Parse CLI arguments
fn cli_init_fsim() -> Result<(ValidatedArgs, Vec<PathBuf>, Option<String>)> {
    let args = Args::parse();
    let cfg_paths = args.cfg.unwrap_or_else(default_cfg);
    let sim_paths = args.sim.unwrap_or_else(default_sim);
    let sim_appendix = args.out;

    log::info!(
        "kanata_simulated_input v{} starting",
        env!("CARGO_PKG_VERSION")
    );

    if let Some(config_file) = cfg_paths.first() {
        if !config_file.exists() {
            bail!(
                "Could not find the config file ({})\nFor more info, pass the `-h` or `--help` flags.",
                cfg_paths[0].to_str().unwrap_or("?")
            )
        }
    } else {
        bail!("No config files provided\nFor more info, pass the `-h` or `--help` flags.");
    }
    if let Some(config_sim_file) = sim_paths.first() {
        if !config_sim_file.exists() {
            bail!(
                "Could not find the simulation file ({})\nFor more info, pass the `-h` or `--help` flags.",
                sim_paths[0].to_str().unwrap_or("?")
            )
        }
    } else {
        bail!("No simulation files provided\nFor more info, pass the `-h` or `--help` flags.");
    }

    Ok((
        ValidatedArgs {
            paths: cfg_paths,
            #[cfg(feature = "tcp_server")]
            tcp_server_address: None::<SocketAddrWrapper>,
            #[cfg(target_os = "linux")]
            symlink_path: None,
            nodelay: true,
            #[cfg(feature = "watch")]
            watch: false,
        },
        sim_paths,
        sim_appendix,
    ))
}

fn split_at_1(s: &str) -> (&str, &str) {
    match s.chars().next() {
        Some(c) => s.split_at(c.len_utf8()),
        None => s.split_at(0),
    }
}
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LogFmtT {
    // partial dupe of @simulated since otherwise no-features clippy fails when features are disabled since even though the function body is conditional and doesn't use the enum, the ↓ function signature still uses it, so will warn
    InKeyUp,
    InKeyDown,
    InKeyRep,
    InTick,
}
fn kbd_out_log(
    _kbd_out: &mut KbdOut,
    _log_type: LogFmtT,
    _key_code: Option<OsCode>,
    _tick: Option<u128>,
) {
    let a = LogFmtT::InTick;
    if a == LogFmtT::InTick {
        println!("{}", 42)
    };
    #[cfg(all(
        not(feature = "simulated_input"),
        not(feature = "passthru_ahk"),
        feature = "simulated_output"
    ))]
    {
        match _log_type {
            LogFmtT::InTick => {
                if let Some(tick) = _tick {
                    _kbd_out.log.in_tick(tick);
                }
            }
            LogFmtT::InKeyUp => {
                if let Some(key_code) = _key_code {
                    _kbd_out.log.in_release_key(key_code);
                }
            }
            LogFmtT::InKeyDown => {
                if let Some(key_code) = _key_code {
                    _kbd_out.log.in_press_key(key_code);
                }
            }
            LogFmtT::InKeyRep => {
                if let Some(key_code) = _key_code {
                    _kbd_out.log.in_repeat_key(key_code);
                }
            }
        }
    }
}
fn main_impl() -> Result<()> {
    log_init();
    let (args, sim_paths, _sim_appendix) = cli_init_fsim()?;
    #[cfg(not(feature = "simulated_output"))]
    {
        if _sim_appendix.is_some() {
            bail!(
                "The program was compiled without simulated output. The -o|--out flag is unsupported"
            );
        }
    }

    for config_sim_file in &sim_paths {
        let mut k = Kanata::new(&args)?;
        log::info!("Evaluating simulation file = {:?}", config_sim_file);
        let s = std::fs::read_to_string(config_sim_file)?;
        for l in s.lines() {
            for pair in l.split_whitespace() {
                match pair.split_once(':') {
                    Some((kind, val)) => match kind {
                        "tick" | "🕐" | "t" => {
                            let tick = str::parse::<u128>(val)?;
                            kbd_out_log(&mut k.kbd_out, LogFmtT::InTick, None, Some(tick));
                            k.tick_ms(tick, &None)?;
                        }
                        "press" | "↓" | "d" | "down" => {
                            let key_code =
                                str_to_oscode(val).ok_or_else(|| anyhow!("unknown key: {val}"))?;
                            kbd_out_log(&mut k.kbd_out, LogFmtT::InKeyDown, Some(key_code), None);
                            k.handle_input_event(&KeyEvent {
                                code: key_code,
                                value: KeyValue::Press,
                            })?;
                        }
                        "release" | "↑" | "u" | "up" => {
                            let key_code =
                                str_to_oscode(val).ok_or_else(|| anyhow!("unknown key: {val}"))?;
                            kbd_out_log(&mut k.kbd_out, LogFmtT::InKeyUp, Some(key_code), None);
                            k.handle_input_event(&KeyEvent {
                                code: key_code,
                                value: KeyValue::Release,
                            })?;
                        }
                        "repeat" | "⟳" | "r" => {
                            let key_code =
                                str_to_oscode(val).ok_or_else(|| anyhow!("unknown key: {val}"))?;
                            kbd_out_log(&mut k.kbd_out, LogFmtT::InKeyRep, Some(key_code), None);
                            k.handle_input_event(&KeyEvent {
                                code: key_code,
                                value: KeyValue::Repeat,
                            })?;
                        }
                        _ => bail!("invalid pair prefix: {kind}"),
                    },
                    None => {
                        let (kind, val) = split_at_1(pair);
                        match kind {
                            //allow skipping : separator for unique non-key symbols
                            "🕐" => {
                                let tick = str::parse::<u128>(val)?;
                                kbd_out_log(&mut k.kbd_out, LogFmtT::InTick, None, Some(tick));
                                k.tick_ms(tick, &None)?;
                            }
                            "↓" => {
                                let key_code = str_to_oscode(val)
                                    .ok_or_else(|| anyhow!("unknown key: {val}"))?;
                                kbd_out_log(
                                    &mut k.kbd_out,
                                    LogFmtT::InKeyDown,
                                    Some(key_code),
                                    None,
                                );
                                k.handle_input_event(&KeyEvent {
                                    code: key_code,
                                    value: KeyValue::Press,
                                })?;
                            }
                            "↑" => {
                                let key_code = str_to_oscode(val)
                                    .ok_or_else(|| anyhow!("unknown key: {val}"))?;
                                kbd_out_log(&mut k.kbd_out, LogFmtT::InKeyUp, Some(key_code), None);
                                k.handle_input_event(&KeyEvent {
                                    code: key_code,
                                    value: KeyValue::Release,
                                })?;
                            }
                            "⟳" => {
                                let key_code = str_to_oscode(val)
                                    .ok_or_else(|| anyhow!("unknown key: {val}"))?;
                                kbd_out_log(
                                    &mut k.kbd_out,
                                    LogFmtT::InKeyRep,
                                    Some(key_code),
                                    None,
                                );
                                k.handle_input_event(&KeyEvent {
                                    code: key_code,
                                    value: KeyValue::Repeat,
                                })?;
                            }
                            _ => bail!("invalid pair: {l}"),
                        }
                    }
                }
            }
        }
        #[cfg(all(
            not(feature = "simulated_input"),
            not(feature = "passthru_ahk"),
            feature = "simulated_output"
        ))]
        println!("{}", k.kbd_out.outputs.events.join("\n"));
        #[cfg(all(
            not(feature = "simulated_input"),
            not(feature = "passthru_ahk"),
            feature = "simulated_output"
        ))]
        k.kbd_out.log.end(config_sim_file, _sim_appendix.clone());
    }

    Ok(())
}

fn main() -> Result<()> {
    let ret = main_impl();
    if let Err(ref e) = ret {
        log::error!("{e}\n");
    }
    ret
}
