use clap::Parser;
use kanata_tcp_protocol::*;
use simplelog::*;
use std::io::{BufRead, BufReader, Write, stdin};
use std::net::{SocketAddr, TcpStream};
use std::process::exit;
use std::time::Duration;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Port that kanata's TCP server is listening on
    #[clap(short, long)]
    port: Option<u16>,

    /// Enable debug logging
    #[clap(short, long)]
    debug: bool,

    /// Enable trace logging (implies --debug as well)
    #[clap(short, long)]
    trace: bool,
}

fn main() {
    let args = Args::parse();
    init_logger(&args);
    print_usage();

    let port = match args.port {
        Some(p) => p,
        None => {
            log::error!("no port provided via the -p|--port flag; exiting");
            exit(1);
        }
    };
    log::info!("attempting to connect to kanata");
    let kanata_conn = TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(5),
    )
    .expect("connect to kanata");
    log::info!("successfully connected");
    let mut writer_stream = kanata_conn.try_clone().expect("clone writer");
    let reader_stream = kanata_conn;

    // Send Hello command to detect capabilities
    let hello_msg = serde_json::to_string(&ClientMessage::Hello { session_id: None })
        .expect("Hello message should serialize");
    writer_stream
        .write_all(hello_msg.as_bytes())
        .expect("write Hello");
    writer_stream.write_all(b"\n").expect("write newline");
    log::info!("sent Hello command");

    std::thread::spawn(move || write_to_kanata(writer_stream));
    read_from_kanata(reader_stream);
}

fn print_usage() {
    log::info!(
        "\n\
    You can also use any other software to connect to kanata over TCP.\n\
    The protocol is plaintext JSON with newline terminated messages.
\n\
    Layer change notifications from kanata look like:\n\
    {}
\n\
    Requests to change kanata's layer look like:\n\
    {}
\n\
    Configuration reload commands:\n\
    - reload: {}\n\
    - reload next: {}\n\
    - reload previous: {}\n\
    - reload specific index: {}\n\
    - reload specific file: {}
\n\
    Server responses for commands look like:\n\
    - Success: {}\n\
    - Error: {}
    ",
        serde_json::to_string(&ServerMessage::LayerChange {
            new: "newly-changed-to-layer".into()
        })
        .expect("deserializable"),
        serde_json::to_string(&ClientMessage::ChangeLayer {
            new: "requested-layer".into(),
            session_id: None,
        })
        .expect("deserializable"),
        serde_json::to_string(&ClientMessage::Reload {
            session_id: None,
            wait: None,
            timeout_ms: None
        })
        .expect("deserializable"),
        serde_json::to_string(&ClientMessage::ReloadNext {
            session_id: None,
            wait: None,
            timeout_ms: None
        })
        .expect("deserializable"),
        serde_json::to_string(&ClientMessage::ReloadPrev {
            session_id: None,
            wait: None,
            timeout_ms: None
        })
        .expect("deserializable"),
        serde_json::to_string(&ClientMessage::ReloadNum {
            index: 1,
            session_id: None,
            wait: None,
            timeout_ms: None
        })
        .expect("deserializable"),
        serde_json::to_string(&ClientMessage::ReloadFile {
            path: "/path/to/config.kbd".to_string(),
            session_id: None,
            wait: None,
            timeout_ms: None
        })
        .expect("deserializable"),
        serde_json::to_string(&ServerResponse::Ok).expect("deserializable"),
        serde_json::to_string(&ServerResponse::Error {
            msg: "Invalid config index: 5. Only 2 configs are available (0-1).".to_string()
        })
        .expect("deserializable"),
    )
}

fn init_logger(args: &Args) {
    let log_lvl = match (args.debug, args.trace) {
        (_, true) => LevelFilter::Trace,
        (true, false) => LevelFilter::Debug,
        (false, false) => LevelFilter::Info,
    };
    let mut log_cfg = ConfigBuilder::new();
    if let Err(e) = log_cfg.set_time_offset_to_local() {
        eprintln!("WARNING: could not set log TZ to local: {e:?}");
    };
    CombinedLogger::init(vec![TermLogger::new(
        log_lvl,
        log_cfg.build(),
        TerminalMode::Mixed,
        ColorChoice::AlwaysAnsi,
    )])
    .expect("init logger");
    log::info!(
        "kanata_example_tcp_client v{} starting",
        env!("CARGO_PKG_VERSION")
    );
}

fn write_to_kanata(mut s: TcpStream) {
    log::info!("writer starting");
    log::info!("writer: enter commands to send to kanata:");
    log::info!("  - layer name: change to that layer");
    log::info!("  - fk:KEYNAME: tap fake key");
    log::info!("  - reload: reload current config");
    log::info!("  - reload-next: reload next config");
    log::info!("  - reload-prev: reload previous config");
    log::info!("  - reload-num:N: reload config at index N");
    log::info!("  - reload-file:PATH: reload config file at PATH");
    log::info!("  - hello: send Hello command (capability detection)");
    log::info!("  - status: get engine status");
    log::info!("  - reload-wait: reload with readiness wait");
    log::info!("  - validate: send a small validation request");
    log::info!("  - subscribe: subscribe to ready/config_error events");
    let mut input = String::new();
    loop {
        stdin().read_line(&mut input).expect("stdin is readable");
        let command = input.trim_end().to_owned();

        let msg = if command.starts_with("fk:") {
            let fkname = command.trim_start_matches("fk:").into();
            log::info!("writer: telling kanata to tap fake key \"{fkname}\"");
            serde_json::to_string(&ClientMessage::ActOnFakeKey {
                name: fkname,
                action: FakeKeyActionMessage::Tap,
                session_id: None,
            })
            .expect("deserializable")
        } else if command == "reload" {
            log::info!("writer: telling kanata to reload current config");
            serde_json::to_string(&ClientMessage::Reload {
                session_id: None,
                wait: None,
                timeout_ms: None,
            })
            .expect("deserializable")
        } else if command == "reload-next" {
            log::info!("writer: telling kanata to reload next config");
            serde_json::to_string(&ClientMessage::ReloadNext {
                session_id: None,
                wait: None,
                timeout_ms: None,
            })
            .expect("deserializable")
        } else if command == "reload-prev" {
            log::info!("writer: telling kanata to reload previous config");
            serde_json::to_string(&ClientMessage::ReloadPrev {
                session_id: None,
                wait: None,
                timeout_ms: None,
            })
            .expect("deserializable")
        } else if command.starts_with("reload-num:") {
            let index_str = command.trim_start_matches("reload-num:");
            match index_str.parse::<usize>() {
                Ok(index) => {
                    log::info!("writer: telling kanata to reload config at index {index}");
                    serde_json::to_string(&ClientMessage::ReloadNum {
                        index,
                        session_id: None,
                        wait: None,
                        timeout_ms: None,
                    })
                    .expect("deserializable")
                }
                Err(_) => {
                    log::error!("Invalid number format for reload-num: {index_str}");
                    input.clear();
                    continue;
                }
            }
        } else if command.starts_with("reload-file:") {
            let path = command.trim_start_matches("reload-file:").to_string();
            log::info!("writer: telling kanata to reload config file \"{path}\"");
            serde_json::to_string(&ClientMessage::ReloadFile {
                path,
                session_id: None,
                wait: None,
                timeout_ms: None,
            })
            .expect("deserializable")
        } else if command == "hello" {
            log::info!("writer: sending Hello command");
            serde_json::to_string(&ClientMessage::Hello { session_id: None })
                .expect("deserializable")
        } else if command == "status" {
            log::info!("writer: requesting status");
            serde_json::to_string(&ClientMessage::Status { session_id: None })
                .expect("deserializable")
        } else if command == "validate" {
            // Minimal demo validation payload
            let cfg = "(defsrc)\n(deflayer base)\n".to_string();
            log::info!("writer: requesting validation");
            serde_json::to_string(&ClientMessage::Validate {
                config: cfg,
                mode: Some("strict".into()),
                session_id: None,
            })
            .expect("deserializable")
        } else if command == "subscribe" {
            log::info!("writer: subscribing to events");
            serde_json::to_string(&ClientMessage::Subscribe {
                events: vec!["ready".into(), "config_error".into()],
                session_id: None,
            })
            .expect("deserializable")
        } else if command == "reload-wait" {
            log::info!("writer: telling kanata to reload current config with wait");
            serde_json::to_string(&ClientMessage::Reload {
                session_id: None,
                wait: Some(true),
                timeout_ms: Some(2000),
            })
            .expect("deserializable")
        } else {
            log::info!("writer: telling kanata to change layer to \"{command}\"");
            serde_json::to_string(&ClientMessage::ChangeLayer {
                new: command,
                session_id: None,
            })
            .expect("deserializable")
        };

        s.write_all(msg.as_bytes()).expect("stream writable");
        input.clear();
    }
}

fn read_from_kanata(s: TcpStream) {
    log::info!("reader starting");
    let mut reader = BufReader::new(s);
    let mut msg = String::new();
    loop {
        msg.clear();
        reader.read_line(&mut msg).expect("stream readable");

        // Try to parse as ServerResponse first (for command responses)
        if let Ok(response) = serde_json::from_str::<ServerResponse>(&msg) {
            match response {
                ServerResponse::Ok => {
                    log::info!("✓ Command executed successfully");
                }
                ServerResponse::Error { msg } => {
                    log::error!("✗ Command failed: {}", msg);
                }
            }
            // After receiving a ServerResponse, try to read an optional second line
            // with detailed information (HelloOk, StatusInfo, ReloadResult, etc.)
            msg.clear();
            // Try to read next line (non-blocking check)
            // Note: This is a simple approach - in production you might want to use
            // a timeout or better buffering strategy
            match reader.read_line(&mut msg) {
                Ok(0) => {
                    // No more data available
                }
                Ok(_) => {
                    if !msg.trim().is_empty() {
                        // Try to parse as ServerMessage (for detailed responses)
                        if let Ok(detail_msg) = serde_json::from_str::<ServerMessage>(&msg) {
                            match detail_msg {
                                ServerMessage::HelloOk {
                                    version,
                                    protocol,
                                    capabilities,
                                } => {
                                    log::info!(
                                        "HelloOk: version={}, protocol={}, capabilities={:?}",
                                        version,
                                        protocol,
                                        capabilities
                                    );
                                }
                                ServerMessage::StatusInfo {
                                    engine_version,
                                    uptime_s,
                                    ready,
                                    last_reload,
                                } => {
                                    log::info!(
                                        "StatusInfo: version={}, uptime={}s, ready={}, last_reload={{ok={}, at={}}}",
                                        engine_version,
                                        uptime_s,
                                        ready,
                                        last_reload.ok,
                                        last_reload.at
                                    );
                                }
                                ServerMessage::ReloadResult { ready, timeout_ms } => {
                                    if ready {
                                        log::info!("ReloadResult: ready=true");
                                    } else {
                                        log::warn!(
                                            "ReloadResult: ready=false, timeout_ms={:?}",
                                            timeout_ms
                                        );
                                    }
                                }
                                _ => {
                                    log::info!("Got detail message: {:?}", detail_msg);
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    // Error reading - likely no more data or connection closed
                }
            }
            continue;
        }

        // Fall back to parsing as ServerMessage (for notifications)
        let parsed_msg: ServerMessage = match serde_json::from_str(&msg) {
            Ok(msg) => msg,
            Err(e) => {
                log::warn!("could not parse server message {msg}: {e:?}");
                std::process::exit(1);
            }
        };
        match parsed_msg {
            ServerMessage::LayerChange { new } => {
                log::info!("reader: kanata changed layers to \"{new}\"");
            }
            ServerMessage::HelloOk {
                version,
                protocol,
                capabilities,
            } => {
                log::info!(
                    "reader: HelloOk - version={}, protocol={}, capabilities={:?}",
                    version,
                    protocol,
                    capabilities
                );
            }
            ServerMessage::StatusInfo {
                engine_version,
                uptime_s,
                ready,
                last_reload,
            } => {
                log::info!(
                    "reader: StatusInfo - version={}, uptime={}s, ready={}, last_reload={{ok={}, at={}}}",
                    engine_version,
                    uptime_s,
                    ready,
                    last_reload.ok,
                    last_reload.at
                );
            }
            ServerMessage::ReloadResult { ready, timeout_ms } => {
                if ready {
                    log::info!("reader: ReloadResult - ready=true");
                } else {
                    log::warn!(
                        "reader: ReloadResult - ready=false, timeout_ms={:?}",
                        timeout_ms
                    );
                }
            }
            msg => {
                log::info!("got msg: {msg:?}");
            }
        }
    }
}
