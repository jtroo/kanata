use clap::Parser;
use kanata_tcp_protocol::*;
use simplelog::*;
use std::io::{stdin, BufRead, BufReader, Write};
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
    let writer_stream = kanata_conn.try_clone().expect("clone writer");
    let reader_stream = kanata_conn;
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
    ",
        serde_json::to_string(&ServerMessage::LayerChange {
            new: "newly-changed-to-layer".into()
        })
        .expect("deserializable"),
        serde_json::to_string(&ClientMessage::ChangeLayer {
            new: "requested-layer".into()
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
        eprintln!("WARNING: could not set log TZ to local: {:?}", e);
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
    log::info!("writer: type layer name then press enter to send a change layer request to kanata");
    let mut layer = String::new();
    loop {
        stdin().read_line(&mut layer).expect("stdin is readable");
        let new = layer.trim_end().to_owned();
        if new.starts_with("fk:") {
            let fkname = new.trim_start_matches("fk:").into();
            log::info!("writer: telling kanata to tap fake key \"{fkname}\"");
            let msg = serde_json::to_string(&ClientMessage::ActOnFakeKey {
                name: fkname,
                action: FakeKeyActionMessage::Tap,
            })
            .expect("deserializable");
            s.write_all(msg.as_bytes()).expect("stream writable");
            layer.clear();
            continue;
        }
        log::info!("writer: telling kanata to change layer to \"{new}\"");
        let msg =
            serde_json::to_string(&ClientMessage::ChangeLayer { new }).expect("deserializable");
        s.write_all(msg.as_bytes()).expect("stream writable");
        layer.clear();
    }
}

fn read_from_kanata(s: TcpStream) {
    log::info!("reader starting");
    let mut reader = BufReader::new(s);
    let mut msg = String::new();
    loop {
        msg.clear();
        reader.read_line(&mut msg).expect("stream readable");
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
            msg => {
                log::info!("got msg: {msg:?}");
            }
        }
    }
}
