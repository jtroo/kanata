use anyhow::{anyhow, bail, Result};
use kanata_parser::keys::str_to_oscode;
use kanata_state_machine::{oskbd::*, *};
use simplelog::*;

#[cfg(test)]
mod tests;

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

fn arg_init() -> Result<ValidatedArgs> {
    let cfg_paths = default_cfg();

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
    let args = arg_init()?;
    let mut k = Kanata::new(&args)?;

    let s = std::fs::read_to_string("testing/sim.txt")?;
    for l in s.lines() {
        match l.split_once(':') {
            Some((kind, val)) => match kind {
                "tick" => {
                    k.tick_ms(str::parse::<u128>(val)?)?;
                }
                "press" => {
                    k.handle_input_event(&KeyEvent {
                        code: str_to_oscode(val).ok_or_else(|| anyhow!("unknown key: {val}"))?,
                        value: KeyValue::Press,
                    })?;
                }
                "release" => {
                    k.handle_input_event(&KeyEvent {
                        code: str_to_oscode(val).ok_or_else(|| anyhow!("unknown key: {val}"))?,
                        value: KeyValue::Release,
                    })?;
                }
                "repeat" => {
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
