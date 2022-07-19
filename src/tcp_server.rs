use anyhow::Result;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub enum EventNotification {
    LayerChange { new: String },
}

impl EventNotification {
    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_string(self)?.as_bytes().to_vec())
    }
}

pub struct NotificationServer {
    pub port: i32,
    pub connections: Arc<Mutex<HashMap<String, TcpStream>>>,
}

impl NotificationServer {
    pub fn new(port: i32) -> Self {
        Self {
            port,
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start(&mut self) {
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

                        cl.lock().insert(addr, stream);
                    }
                    Err(_) => log::error!("not able to accept client connection"),
                }
            }
        });
    }
}
