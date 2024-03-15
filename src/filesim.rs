use anyhow::{anyhow, bail, Result};
use clap::Parser;
use kanata_parser::keys::str_to_oscode;
use kanata_state_machine::{oskbd::*, *};
use simplelog::*;

use std::path::PathBuf;

#[cfg(test)]
mod tests;

#[derive(Parser, Debug)]
#[command(author, version, verbatim_doc_comment)]
///
/// kanata_filesim is a helper cli tool that helps debug kanata's user configuration:
/// - it reads a text file with a sequence of key events, including key delays
/// - interprets them with kanata
/// - prints out which key|mouse events/actions kanata would execute if the keys were
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
        TerminalMode::Mixed,
        ColorChoice::AlwaysAnsi,
    )])
    .expect("logger can init");
}


/// Parse CLI arguments
fn cli_init() -> Result<ValidatedArgs> {
    let args = Args::parse();
    let cfg_paths = args.cfg.unwrap_or_else(default_cfg);

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

    Ok(ValidatedArgs {
        paths: cfg_paths,
        #[cfg(feature = "tcp_server")]
        port: None,
        #[cfg(target_os = "linux")]
        symlink_path: None,
        nodelay: true,
    })
}

fn main_impl() -> Result<()> {
    log_init();
    let args = cli_init()?;
    let mut k = Kanata::new(&args)?;

    let s = std::fs::read_to_string("testing/sim.txt")?;
    let send = false; // do not send key/mouse events, just print debug info
    for l in s.lines() {
        match l.split_once(':') {
            Some((kind, val)) => match kind {
                "tick"|"🕐" => {
                    k.tick_ms(str::parse::<u128>(val)?,send)?;
                }
                "press"|"↓" => {
                    k.handle_input_event(&KeyEvent {
                        code: str_to_oscode(val).ok_or_else(|| anyhow!("unknown key: {val}"))?,
                        value: KeyValue::Press,
                    })?;
                }
                "release"|"↑" => {
                    k.handle_input_event(&KeyEvent {
                        code: str_to_oscode(val).ok_or_else(|| anyhow!("unknown key: {val}"))?,
                        value: KeyValue::Release,
                    })?;
                }
                "repeat"|"⟳" => {
                    k.handle_input_event(&KeyEvent {
                        code: str_to_oscode(val).ok_or_else(|| anyhow!("unknown key: {val}"))?,
                        value: KeyValue::Repeat,
                    })?;
                }
                _ => bail!("invalid line prefix: {kind}"),
            },
            None => bail!("invalid line: {l}"),
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let ret = main_impl();
    if let Err(ref e) = ret {
        log::error!("{e}\n");
    }
    eprintln!("\nPress enter to exit");
    let _ = std::io::stdin().read_line(&mut String::new());
    ret
}
