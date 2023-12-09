use clap::Parser;
use serde::{Deserialize, Serialize};
use simplelog::*;

use std::io::{stdin, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::str::FromStr;
use std::time::Duration;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Port that kanata's TCP server is listening on
    #[clap(short, long)]
    port: u16,

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
    log::info!("attempting to connect to kanata");
    let kanata_conn = TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], args.port)),
        Duration::from_secs(5),
    )
    .expect("connect to kanata");
    log::info!("successfully connected");
    let writer_stream = kanata_conn.try_clone().expect("clone writer");
    let reader_stream = kanata_conn;
    std::thread::spawn(move || write_to_kanata(writer_stream));
    read_from_kanata(reader_stream);
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

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    LayerChange { new: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    ChangeLayer { new: String },
}

impl FromStr for ServerMessage {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

fn write_to_kanata(mut s: TcpStream) {
    log::info!("writer starting");
    log::info!("writer: type layer name then press enter to send a change layer request to kanata");
    let mut layer = String::new();
    loop {
        stdin().read_line(&mut layer).expect("stdin is readable");
        let new = layer.trim_end().to_owned();
        log::info!("writer: telling kanata to change layer to \"{new}\"");
        let msg =
            serde_json::to_string(&ClientMessage::ChangeLayer { new }).expect("deserializable");
        let expected_wsz = msg.len();
        let wsz = s.write(msg.as_bytes()).expect("stream writable");
        if wsz != expected_wsz {
            panic!("failed to write entire message {wsz} {expected_wsz}");
        }
        layer.clear();
    }
}

fn read_from_kanata(mut s: TcpStream) {
    log::info!("reader starting");
    let mut buf = vec![0; 256];
    loop {
        let sz = s.read(&mut buf).expect("stream readable");
        let msg = String::from_utf8_lossy(&buf[..sz]);
        let parsed_msg = ServerMessage::from_str(&msg).expect("kanata sends valid message");
        match parsed_msg {
            ServerMessage::LayerChange { new } => {
                log::info!("reader: kanata changed layers to \"{new}\"");
            }
        }
    }
}
