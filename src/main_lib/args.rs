use clap::Parser;
#[cfg(feature = "tcp_server")]
use kanata_state_machine::SocketAddrWrapper;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, verbatim_doc_comment)]
/// kanata: an advanced software key remapper
///
/// kanata remaps key presses to other keys or complex actions depending on the
/// configuration for that key. You can find the guide for creating a config
/// file here: https://github.com/jtroo/kanata/blob/main/docs/config.adoc
///
/// If you need help, please feel welcome to create an issue or discussion in
/// the kanata repository: https://github.com/jtroo/kanata
pub struct Args {
    // Display different platform specific paths based on the target OS
    #[cfg_attr(
        target_os = "windows",
        doc = r"Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'C:\Users\user\AppData\Roaming\kanata\kanata.kbd'."
    )]
    #[cfg_attr(
        target_os = "macos",
        doc = "Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'$HOME/Library/Application Support/kanata/kanata.kbd'."
    )]
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows")),
        doc = "Configuration file(s) to use with kanata. If not specified, defaults to
kanata.kbd in the current working directory and
'$XDG_CONFIG_HOME/kanata/kanata.kbd'."
    )]
    #[arg(short, long, verbatim_doc_comment)]
    pub cfg: Option<Vec<PathBuf>>,

    /// Read configuration from stdin instead of a file.
    #[arg(long, verbatim_doc_comment)]
    pub cfg_stdin: bool,

    /// Port or full address (IP:PORT) to run the optional TCP server on. If blank,
    /// no TCP port will be listened on.
    #[cfg(feature = "tcp_server")]
    #[arg(
        short = 'p',
        long = "port",
        value_name = "PORT or IP:PORT",
        verbatim_doc_comment
    )]
    pub tcp_server_address: Option<SocketAddrWrapper>,

    /// Path for the symlink pointing to the newly-created device. If blank, no
    /// symlink will be created.
    #[cfg(target_os = "linux")]
    #[arg(short, long, verbatim_doc_comment)]
    pub symlink_path: Option<String>,

    /// List the keyboards available for grabbing and exit.
    #[cfg(any(
        target_os = "macos",
        target_os = "linux",
        all(
            target_os = "windows",
            feature = "interception_driver",
            not(feature = "gui")
        )
    ))]
    #[arg(short, long)]
    pub list: bool,

    /// Disable logging, except for errors. Takes precedent over debug and trace.
    #[arg(short, long)]
    pub quiet: bool,

    /// Enable debug logging.
    #[arg(short, long)]
    pub debug: bool,

    /// Enable trace logging; implies --debug as well.
    #[arg(short, long)]
    pub trace: bool,

    /// Remove the startup delay.
    /// In some cases, removing the delay may cause keyboard issues on startup.
    #[arg(short, long, verbatim_doc_comment)]
    pub nodelay: bool,

    /// Milliseconds to wait before attempting to register a newly connected
    /// device. The default is 200.
    ///
    /// You may wish to increase this if you have a device that is failing
    /// to register - the device may be taking too long to become ready.
    #[cfg(target_os = "linux")]
    #[arg(short, long, verbatim_doc_comment)]
    pub wait_device_ms: Option<u64>,

    /// Validate configuration file and exit
    #[arg(long, verbatim_doc_comment)]
    pub check: bool,

    /// Log layer changes even if the configuration file has set the defcfg
    /// option to false. Useful if you are experimenting with a new
    /// configuration but want to default to no logging.
    #[arg(long, verbatim_doc_comment)]
    pub log_layer_changes: bool,

    /// Start up the process in GUI mode and does not run the Kanata processing.
    /// You likely don't want to be using this;
    /// it is typically used internally by the main Kanata process
    /// to spawn the child GUI process.
    #[cfg(feature = "iced_gui")]
    #[arg(long)]
    pub run_gui: bool,
}
