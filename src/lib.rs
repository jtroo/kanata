use anyhow::{Error, Result, anyhow};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

#[cfg(feature = "watch")]
pub mod file_watcher;
#[cfg(all(target_os = "windows", feature = "gui"))]
pub mod gui;
pub mod kanata;
pub mod oskbd;
pub mod tcp_server;
#[cfg(test)]
pub mod tests;

pub use kanata::*;
pub use tcp_server::TcpServer;

type CfgPath = PathBuf;

pub struct ValidatedArgs {
    pub paths: Vec<CfgPath>,
    #[cfg(feature = "tcp_server")]
    pub tcp_server_address: Option<SocketAddrWrapper>,
    #[cfg(target_os = "linux")]
    pub symlink_path: Option<String>,
    pub nodelay: bool,
    #[cfg(feature = "watch")]
    pub watch: bool,
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

#[derive(Debug, Clone)]
pub struct SocketAddrWrapper(SocketAddr);

impl FromStr for SocketAddrWrapper {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut address = s.to_string();
        if let Ok(port) = s.parse::<u16>() {
            address = format!("127.0.0.1:{port}");
        }
        address
            .parse::<SocketAddr>()
            .map(SocketAddrWrapper)
            .map_err(|e| anyhow!("Please specify either a port number, e.g. 8081 or an address, e.g. 127.0.0.1:8081.\n{e}"))
    }
}

impl SocketAddrWrapper {
    pub fn into_inner(self) -> SocketAddr {
        self.0
    }
    pub fn get_ref(&self) -> &SocketAddr {
        &self.0
    }
}
