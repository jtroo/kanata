use crate::Kanata;
use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::net::{TcpListener, TcpStream};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub enum EventNotification {
    LayerChange { new: String },
}

impl EventNotification {
    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_string(self)?.as_bytes().to_vec())
    }
}

impl FromStr for EventNotification {
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

        let cl = self.connections.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let addr = stream
                            .peer_addr()
                            .expect("could not find peer address")
                            .to_string();

                        {
                            cl.lock().insert(addr.clone(), stream);
                        }

                        if let Some(stream) = cl.lock().get(&addr) {
                            let stream = stream
                                .try_clone()
                                .expect("could not clone tcpstream to read incoming messages");

                            let k_cl = kanata.clone();
                            std::thread::spawn(move || {
                                log::info!("listening for incoming messages {}", &addr);
                                loop {
                                    let mut buffer: [u8; 1024] = [0; 1024];
                                    let mut reader = BufReader::new(&stream);
                                    if let Ok(size) = reader.read(&mut buffer) {
                                        if let Ok(event) = EventNotification::from_str(
                                            &String::from_utf8_lossy(&buffer[..size]),
                                        ) {
                                            match event {
                                                EventNotification::LayerChange { new } => {
                                                    k_cl.lock().change_layer(new);
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                        };
                    }
                    Err(_) => log::error!("not able to accept client connection"),
                }
            }
        });
    }
}
