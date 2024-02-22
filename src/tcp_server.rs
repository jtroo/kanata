use crate::Kanata;
use kanata_parser::custom_action::FakeKeyAction;
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
    LayerNames { names: Vec<String> },
}

#[cfg(feature = "tcp_server")]
impl ServerMessage {
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut msg = serde_json::to_vec(self).expect("ServerMessage should serialize");
        msg.push(b'\n');
        msg
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    ChangeLayer {
        new: String,
    },
    RequestLayerNames {},
    ActOnFakeKey {
        name: String,
        action: FakeKeyActionMessage,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum FakeKeyActionMessage {
    Press,
    Release,
    Tap,
    Toggle,
}

impl Into<FakeKeyAction> for FakeKeyActionMessage {
    fn into(self) -> FakeKeyAction {
        match self {
            FakeKeyActionMessage::Press => FakeKeyAction::Press,
            FakeKeyActionMessage::Release => FakeKeyAction::Release,
            FakeKeyActionMessage::Tap => FakeKeyAction::Tap,
            FakeKeyActionMessage::Toggle => FakeKeyAction::Toggle,
        }
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
        use kanata_parser::cfg::FAKE_KEY_ROW;

        use crate::kanata::handle_fakekey_action;

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
                                            ClientMessage::RequestLayerNames {} => {
                                                let msg = ServerMessage::LayerNames {
                                                    names: kanata
                                                        .lock()
                                                        .layer_info
                                                        .iter()
                                                        .step_by(2) // skip every other name, which is a duplicate
                                                        .map(|info| info.name.clone())
                                                        .collect::<Vec<_>>(),
                                                };
                                                match stream.write(&msg.as_bytes()) {
                                                    Ok(_) => {}
                                                    Err(err) => log::error!(
                                                        "server could not send response: {}",
                                                        err
                                                    ),
                                                }
                                            }
                                            ClientMessage::ActOnFakeKey { name, action } => {
                                                let mut k = kanata.lock();
                                                let index = match k.fake_keys.get(&name) {
                                                    Some(index) => Some(*index as u16),
                                                    None => {
                                                        if let Err(e) = writeln!(
                                                            stream,
                                                            "unknown fake key {name}"
                                                        ) {
                                                            log::error!("stream write error: {e}")
                                                        }
                                                        None
                                                    }
                                                };
                                                if let Some(index) = index {
                                                    handle_fakekey_action(
                                                        action.into(),
                                                        k.layout.bm(),
                                                        FAKE_KEY_ROW,
                                                        index,
                                                    );
                                                }
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

#[test]
fn layer_change_serializes() {
    serde_json::to_string(&ServerMessage::LayerChange {
        new: "hello".into(),
    })
    .expect("ServerMessage serializes");
}
