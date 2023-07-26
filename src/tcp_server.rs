suse crate::{Kanata, oskbd};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::str::FromStr;
use std::sync::Arc;
use kanata_parser::keys::OsCode;

type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;

#[derive(Debug, Serialize, Deserialize)]
pub enum KeyAction {
    Press,
    Release,
    Repeat
}

#[derive(Debug, Serialize, Deserialize)]
pub enum KeyEventKind {
    Input,
    Output
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyEvent {
    key: String,
    action: KeyAction,
    kind: KeyEventKind
}

impl KeyEvent {
    pub fn from_input(event: &oskbd::KeyEvent) -> Self {
        Self {
            key: format!("{:?}", event.code),
            action: match event.value {
                oskbd::KeyValue::Press => KeyAction::Press,
                oskbd::KeyValue::Release => KeyAction::Release,
                oskbd::KeyValue::Repeat => KeyAction::Repeat
            },
            kind: KeyEventKind::Input
        }
    }

    pub fn from_output(code: &OsCode, action: KeyAction) -> Self {
        Self {
            key: format!("{:?}", code),
            action,
            kind: KeyEventKind::Output
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    LayerChange { new: String, text: String },
    KeyEvent(KeyEvent)
}

#[test]
fn layer_change_serializes() {
    serde_json::to_string(&ServerMessage::LayerChange {
        new: "hello".into(),
        text: "world".into(),
    })
    .expect("ServerMessage serializes");
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    ChangeLayer { new: String },
}

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

pub struct TcpServer {
    pub port: i32,
    pub connections: Arc<Mutex<HashMap<String, TcpStream>>>,
}

impl TcpServer {
    pub fn new(port: i32) -> Self {
        Self {
            port,
            connections: Arc::new(Mutex::new(HashMap::default())),
        }
    }

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
                            let layer_info = &k.layer_info[k.layout.b().current_layer()];
                            if let Err(e) = stream.write(
                                &ServerMessage::LayerChange {
                                    new: layer_info.name.clone(),
                                    text: layer_info.cfg_text.clone(),
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
}
