use clap::Parser;
#[cfg(feature = "tcp_server")]
use kanata_state_machine::SocketAddrWrapper;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, verbatim_doc_comment)]
#[command(args_conflicts_with_subcommands = true)]
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
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[arg(short, long, verbatim_doc_comment)]
    pub symlink_path: Option<String>,

    /// List the keyboards available for grabbing and exit.
    #[cfg(any(
        target_os = "macos",
        any(target_os = "linux", target_os = "android"),
        all(target_os = "windows", feature = "interception_driver")
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
    #[cfg(any(target_os = "linux", target_os = "android"))]
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

    /// Skip the "Press enter to exit" prompt and exit immediately.
    /// Useful for running kanata as a background service (e.g., systemd)
    /// where automatic restart on failure is desired.
    #[arg(long, verbatim_doc_comment)]
    pub no_wait: bool,

    /// Exit code to use when emergency exit is triggered (LCtrl+Space+Escape).
    /// Default is 0 (success). Set to non-zero if your service manager should
    /// treat emergency exit as a failure and restart.
    #[arg(long, default_value = "0", verbatim_doc_comment)]
    pub emergency_exit_code: i32,

    /// Release the keyboard grab while the screen is locked or another user
    /// holds the console (fast user switching), and re-grab once kanata's
    /// session is back. Useful on shared Macs so the lock screen and other
    /// users get an unmodified keyboard. Off by default to preserve the
    /// historical always-grab behavior.
    #[cfg(target_os = "macos")]
    #[arg(long, verbatim_doc_comment)]
    pub release_grab_on_lock: bool,

    /// Request/register macOS Accessibility permission and exit.
    /// This does not read a config file, open keyboard devices, or start remapping.
    #[cfg(target_os = "macos")]
    #[arg(long, verbatim_doc_comment)]
    pub macos_request_permissions: bool,

    #[cfg(all(feature = "tcp_server", not(feature = "gui")))]
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[cfg(all(feature = "tcp_server", not(feature = "gui")))]
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Send a message to the TCP server of a running kanata and exit.
    ///
    /// Waits until kanata has processed the message. The exit code is
    /// non-zero if anything failed, e.g. an unknown layer or virtual key.
    Msg(super::msg::MsgArgs),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_wait_flag_default_false() {
        let args = Args::try_parse_from(["kanata"]).unwrap();
        assert!(!args.no_wait);
    }

    #[test]
    fn no_wait_flag_enabled() {
        let args = Args::try_parse_from(["kanata", "--no-wait"]).unwrap();
        assert!(args.no_wait);
    }

    #[test]
    fn no_wait_with_other_flags() {
        let args =
            Args::try_parse_from(["kanata", "--no-wait", "--nodelay", "-c", "test.kbd"]).unwrap();
        assert!(args.no_wait);
        assert!(args.nodelay);
    }

    #[test]
    fn emergency_exit_code_default() {
        let args = Args::try_parse_from(["kanata"]).unwrap();
        assert_eq!(args.emergency_exit_code, 0);
    }

    #[test]
    fn emergency_exit_code_custom() {
        let args = Args::try_parse_from(["kanata", "--emergency-exit-code", "42"]).unwrap();
        assert_eq!(args.emergency_exit_code, 42);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn release_grab_on_lock_default_false() {
        let args = Args::try_parse_from(["kanata"]).unwrap();
        assert!(!args.release_grab_on_lock);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn release_grab_on_lock_enabled() {
        let args = Args::try_parse_from(["kanata", "--release-grab-on-lock"]).unwrap();
        assert!(args.release_grab_on_lock);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_request_permissions_enabled() {
        let args = Args::try_parse_from(["kanata", "--macos-request-permissions"]).unwrap();
        assert!(args.macos_request_permissions);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn help_mentions_macos_request_permissions() {
        use clap::CommandFactory;

        let help = Args::command().render_long_help().to_string();
        assert!(help.contains("--macos-request-permissions"));
    }

    #[cfg(all(feature = "tcp_server", not(feature = "gui")))]
    #[test]
    fn msg_subcommand() {
        let args =
            Args::try_parse_from(["kanata", "msg", "-p", "9999", "act-on-fake-key", "fk"]).unwrap();
        assert!(matches!(args.command, Some(Command::Msg(_))));
    }

    #[cfg(all(feature = "tcp_server", not(feature = "gui")))]
    #[test]
    fn no_subcommand_by_default() {
        let args = Args::try_parse_from(["kanata", "-c", "test.kbd"]).unwrap();
        assert!(args.command.is_none());
    }

    #[cfg(all(feature = "tcp_server", not(feature = "gui")))]
    #[test]
    fn msg_conflicts_with_daemon_args() {
        let res = Args::try_parse_from(["kanata", "-c", "test.kbd", "msg", "-p", "1", "hello"]);
        assert!(res.is_err());
    }

    #[test]
    fn emergency_exit_code_with_other_flags() {
        let args = Args::try_parse_from([
            "kanata",
            "--emergency-exit-code",
            "1",
            "--no-wait",
            "-c",
            "test.kbd",
        ])
        .unwrap();
        assert_eq!(args.emergency_exit_code, 1);
        assert!(args.no_wait);
    }
}
