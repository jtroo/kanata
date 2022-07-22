use crate::Kanata;
use anyhow::Result;
use net2::TcpStreamExt;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    LayerChange { new: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    ChangeLayer { new: String },
}

impl ServerMessage {
    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_string(self)?.as_bytes().to_vec())
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
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start(&mut self, kanata: Arc<Mutex<Kanata>>) {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .expect("could not start the tcp server");

        let connections = self.connections.clone();

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        stream
                            .set_keepalive(Some(Duration::from_secs(30)))
                            .expect("could not set tcp connection keepalive");

                        let addr = stream
                            .peer_addr()
                            .expect("could not find peer address")
                            .to_string();

                        connections.lock().insert(
                            addr.clone(),
                            stream.try_clone().expect("could not clone stream"),
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
