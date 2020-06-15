use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use ron::de;
use serde::Deserialize;

use log::{debug, info};

use crate::effects::perform_effect;
use crate::effects::EffectValue;
use crate::ktrl::Ktrl;

#[derive(Debug, Clone, Deserialize)]
pub enum KtrlIpcReq {
    IpcDoEffect(EffectValue),
}

#[derive(Debug, Clone, Deserialize)]
pub enum KtrlIpcResp {
    Ok,
    Error(String),
}

impl KtrlIpcResp {
    fn to_str(self) -> String {
        match self {
            Self::Ok => String::from("OK"),
            Self::Error(err) => err,
        }
    }
}

pub struct KtrlIpc {
    _ctx: zmq::Context,
    socket: zmq::Socket,
    ktrl: Arc<Mutex<Ktrl>>,
}

impl KtrlIpc {
    pub fn new(ktrl: Arc<Mutex<Ktrl>>, port: usize) -> Result<Self, std::io::Error> {
        let ctx = zmq::Context::new();
        let socket = ctx.socket(zmq::REP)?;
        let endpoint = format!("tcp://127.0.0.1:{}", port);
        socket.bind(&endpoint)?;
        info!("Listening for IPC requests on {}", endpoint);
        Ok(Self {
            _ctx: ctx,
            socket,
            ktrl,
        })
    }

    fn handle_ipc_req(&self, req: &zmq::Message) -> KtrlIpcResp {
        let mut ktrl = self.ktrl.lock().expect("Failed to lock ktrl (poisoned)");

        let req_str = match req.as_str() {
            Some(req_str) => req_str,
            _ => return KtrlIpcResp::Error("Request has an invalid string".to_string()),
        };

        debug!("Recived an IPC req: '{}'", req_str);

        let req: KtrlIpcReq = match de::from_str(req_str) {
            Ok(req) => req,
            Err(err) => return KtrlIpcResp::Error(err.to_string()),
        };

        let KtrlIpcReq::IpcDoEffect(fx_val) = req;
        match perform_effect(&mut ktrl, fx_val) {
            Ok(_) => KtrlIpcResp::Ok,
            Err(err) => return KtrlIpcResp::Error(err.to_string()),
        }
    }

    fn ipc_loop(&self) -> Result<(), std::io::Error> {
        let mut msg = zmq::Message::new();

        loop {
            self.socket.recv(&mut msg, 0)?;

            let resp = self.handle_ipc_req(&msg);

            self.socket
                .send(&resp.to_str(), 0)
                .expect("Failed to send a reply");
        }
    }

    pub fn spawn_ipc_thread(self) {
        thread::spawn(move || {
            self.ipc_loop().unwrap();
        });
    }

    pub fn send_ipc_req(port: usize, req: String) -> Result<(), std::io::Error> {
        let ctx = zmq::Context::new();
        let socket = ctx.socket(zmq::REQ)?;
        let endpoint = format!("tcp://127.0.0.1:{}", port);

        info!("Sending an ipc msg to ktrl: {}", endpoint);
        socket.connect(&endpoint)?;
        socket.send(&req, 0)?;

        let mut msg = zmq::Message::new();
        socket.recv(&mut msg, 0)?;
        info!(
            "Received: {}",
            msg.as_str()
                .expect("Couldn't parse the ipc reply as a string")
        );

        Ok(())
    }
}
