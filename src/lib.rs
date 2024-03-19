use std::path::PathBuf;

pub mod kanata;
pub mod oskbd;
pub mod tcp_server;

pub use kanata::Kanata;
pub use tcp_server::TcpServer;

type CfgPath = PathBuf;

pub struct ValidatedArgs {
    pub paths: Vec<CfgPath>,
    #[cfg(feature = "tcp_server")]
    pub port: Option<i32>,
    #[cfg(target_os = "linux")]
    pub symlink_path: Option<String>,
    pub nodelay: bool,
    #[cfg(feature = "simulated_output")]
    pub sim_paths: Vec<PathBuf>,
    #[cfg(feature = "simulated_output")]
    pub sim_appendix: Option<String>,
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
