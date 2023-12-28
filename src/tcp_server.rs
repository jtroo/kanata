use crate::Kanata;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

#[cfg(feature = "tcp_server")]
type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;
#[cfg(feature = "tcp_server")]
use std::io::{Read, Write};
#[cfg(feature = "tcp_server")]
use std::net::{TcpListener, TcpStream};

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    LayerChange { new: String },
}

#[test]
fn layer_change_serializes() {
    serde_json::to_string(&ServerMessage::LayerChange {
        new: "hello".into(),
    })
    .expect("ServerMessage serializes");
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    ChangeLayer { new: String },
}

#[cfg(feature = "tcp_server")]
impl ServerMessage {
    pub fn as_bytes(&self) -> Vec<u8> {
        serde_json::to_string(self)
            .expect("ServerMessage should serialize")
            .as_bytes()
            .to_vec()
    }
}

impl FromStr for ClientMessage {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[cfg(feature = "tcp_server")]
pub type Connections = Arc<Mutex<HashMap<String, TcpStream>>>;

#[cfg(not(feature = "tcp_server"))]
pub type Connections = ();

#[cfg(feature = "tcp_server")]
pub struct TcpServer {
    pub port: i32,
    pub connections: Connections,
}

#[cfg(not(feature = "tcp_server"))]
pub struct TcpServer {
    pub connections: Connections,
}

impl TcpServer {
    #[cfg(feature = "tcp_server")]
    pub fn new(port: i32) -> Self {
        Self {
            port,
            connections: Arc::new(Mutex::new(HashMap::default())),
        }
    }

    #[cfg(not(feature = "tcp_server"))]
    pub fn new(_port: i32) -> Self {
        Self { connections: () }
    }

    #[cfg(feature = "tcp_server")]
    pub fn start(&mut self, kanata: Arc<Mutex<Kanata>>) {
        let listener =
            TcpListener::bind(format!("0.0.0.0:{}", self.port)).expect("TCP server starts");

        let connections = self.connections.clone();

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        {
                            let k = kanata.lock();
                            log::info!(
                                "new client connection, sending initial LayerChange event to inform them of current layer"
                            );
                            if let Err(e) = stream.write(
                                &ServerMessage::LayerChange {
                                    new: k.layer_info[k.layout.b().current_layer()].name.clone(),
                                }
                                .as_bytes(),
                            ) {
                                log::warn!("failed to write to stream, dropping it: {e:?}");
                                continue;
                            }
                        }

                        let addr = stream
                            .peer_addr()
                            .expect("incoming conn has known address")
                            .to_string();

                        connections.lock().insert(
                            addr.clone(),
                            stream.try_clone().expect("stream is clonable"),
                        );

                        log::info!("listening for incoming messages {}", &addr);

                        let connections = connections.clone();
                        let kanata = kanata.clone();
                        std::thread::spawn(move || loop {
                            let mut buf = vec![0; 1024];
                            match stream.read(&mut buf) {
                                Ok(size) => {
                                    if let Ok(event) = ClientMessage::from_str(
                                        &String::from_utf8_lossy(&buf[..size]),
                                    ) {
                                        match event {
                                            ClientMessage::ChangeLayer { new } => {
                                                kanata.lock().change_layer(new);
                                            }
                                        }
                                    } else {
                                        log::warn!(
                                            "client sent an invalid message of size {size}, disconnecting them"
                                        );
                                        // Ignore write result because we're about to disconnect
                                        // the client anyway.
                                        let _ = stream.write(
                                            "you sent an invalid message; disconnecting you"
                                                .as_bytes(),
                                        );
                                        connections.lock().remove(&addr);
                                        break;
                                    }
                                }
                                Err(_) => {
                                    log::warn!("removing disconnected tcp client: {addr}");
                                    connections.lock().remove(&addr);
                                    break;
                                }
                            }
                        });
                    }
                    Err(_) => log::error!("not able to accept client connection"),
                }
            }
        });
    }

    #[cfg(not(feature = "tcp_server"))]
    pub fn start(&mut self, _kanata: Arc<Mutex<Kanata>>) {}
}
