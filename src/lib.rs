use anyhow::Error;
use anyhow::Result;
use std::path::PathBuf;
use std::str::FromStr;

pub mod kanata;
pub mod oskbd;
pub mod tcp_server;

pub use kanata::*;
pub use tcp_server::TcpServer;

type CfgPath = PathBuf;

pub struct ValidatedArgs {
    pub paths: Vec<CfgPath>,
    #[cfg(feature = "tcp_server")]
    pub port: Option<PortArg>,
    #[cfg(target_os = "linux")]
    pub symlink_path: Option<String>,
    pub nodelay: bool,
}

pub fn default_cfg() -> Vec<PathBuf> {
    let mut cfgs = Vec::new();

    let default = PathBuf::from("kanata.kbd");
    if default.is_file() {
        cfgs.push(default);
    }

    if let Some(config_dir) = dirs::config_dir() {
        let fallback = config_dir.join("kanata").join("kanata.kbd");
        if fallback.is_file() {
            cfgs.push(fallback);
        }
    }

    cfgs
}

#[derive(Clone, Debug)]
pub enum PortArg {
    Number(u16),
    Address(String),
}

impl FromStr for PortArg {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(num) = s.parse::<u16>() {
            Ok(PortArg::Number(num))
        } else if is_address_format(s) {
            Ok(PortArg::Address(s.to_string()))
        } else {
            Err(anyhow::Error::msg(
                "specify either a port number, e.g. 8081 or an address, e.g. 127.0.0.1:8081",
            ))
        }
    }
}

fn is_address_format(addr: &str) -> bool {
    if let Some((host, port)) = addr.split_once(':') {
        if host.is_empty() {
            return false;
        }

        if let Ok(port_num) = port.parse::<u16>() {
            return port_num > 0;
        }
    }
    false
}
