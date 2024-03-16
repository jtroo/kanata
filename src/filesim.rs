use anyhow::{anyhow, bail, Result};
use clap::Parser;
use kanata_parser::keys::str_to_oscode;
use kanata_state_machine::{oskbd::*, *};
use simplelog::*;

use std::path::PathBuf;

#[cfg(test)]
mod tests;

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
///
/// kanata_filesim: a cli tool that helps debug kanata's user configuration by:
/// - reading a text file with a sequence of key events, including key delays
/// - interpreting them with kanata
/// - printing out which actions or key/mouse events kanata would execute if the keys were
/// pressed by a user
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
        doc = r"Simulation file(s) to use with kanata_filesim. If not specified, defaults to
test\sim.txt in the current working directory and
'C:\Users\user\AppData\Roaming\kanata\test\sim.txt'"
    )]
    #[cfg_attr(
        target_os = "macos",
        doc = "Simulation file(s) to use with kanata_filesim. If not specified, defaults to
test/sim.txt in the current working directory and
'$HOME/Library/Application Support/kanata/test/sim.txt.'"
    )]
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows")),
        doc = "Simulation file(s) to use with kanata_filesim. If not specified, defaults to
test/sim.txt in the current working directory and
'$XDG_CONFIG_HOME/kanata/test/sim.txt'"
    )]
    #[arg(short = 's', long, verbatim_doc_comment)]
    sim: Option<Vec<PathBuf>>,
}

fn log_init() {
    let mut log_cfg = ConfigBuilder::new();
    if let Err(e) = log_cfg.set_time_offset_to_local() {
        eprintln!("WARNING: could not set log TZ to local: {e:?}");
    };
    log_cfg.set_time_format_rfc3339();
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        log_cfg.build(),
        TerminalMode::Stderr,
        ColorChoice::AlwaysAnsi,
    )])
    .expect("logger can init");
}

/// Parse CLI arguments
fn cli_init() -> Result<(ValidatedArgs, Vec<PathBuf>)> {
    let args = Args::parse();
    let cfg_paths = args.cfg.unwrap_or_else(default_cfg);
    let sim_paths = args.sim.unwrap_or_else(default_sim);

    log::info!("kanata_filesim v{} starting", env!("CARGO_PKG_VERSION"));
    #[cfg(all(not(feature = "interception_driver"), target_os = "windows"))]
    log::info!("using LLHOOK+SendInput for keyboard IO");
    #[cfg(all(feature = "interception_driver", target_os = "windows"))]
    log::info!("using the Interception driver for keyboard IO");

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
            port: None,
            #[cfg(target_os = "linux")]
            symlink_path: None,
            nodelay: true,
        },
        sim_paths,
    ))
}

fn main_impl() -> Result<()> {
    log_init();
    let (args, sim_paths) = cli_init()?;

    for config_sim_file in &sim_paths {
        let mut k = Kanata::new(&args)?;
        println!("Evaluating simulation file = {:?}", config_sim_file);
        let s = std::fs::read_to_string(config_sim_file)?;
        for l in s.lines() {
            for pair in l.split_whitespace() {
                match pair.split_once(':') {
                    Some((kind, val)) => match kind {
                        "tick" | "🕐" | "t" => {
                            k.tick_ms(str::parse::<u128>(val)?)?;
                        }
                        "press" | "↓" | "d" | "down" => {
                            k.handle_input_event(&KeyEvent {
                                code: str_to_oscode(val)
                                    .ok_or_else(|| anyhow!("unknown key: {val}"))?,
                                value: KeyValue::Press,
                            })?;
                        }
                        "release" | "↑" | "u" | "up" => {
                            k.handle_input_event(&KeyEvent {
                                code: str_to_oscode(val)
                                    .ok_or_else(|| anyhow!("unknown key: {val}"))?,
                                value: KeyValue::Release,
                            })?;
                        }
                        "repeat" | "⟳" | "r" => {
                            k.handle_input_event(&KeyEvent {
                                code: str_to_oscode(val)
                                    .ok_or_else(|| anyhow!("unknown key: {val}"))?,
                                value: KeyValue::Repeat,
                            })?;
                        }
                        _ => bail!("invalid pair prefix: {kind}"),
                    },
                    None => bail!("invalid pair: {l}"),
                }
            }
        }
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
