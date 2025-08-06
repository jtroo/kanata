use crate::Kanata;
use crate::oskbd::*;

#[cfg(feature = "tcp_server")]
use kanata_tcp_protocol::*;
use parking_lot::Mutex;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc::SyncSender as Sender;

#[cfg(feature = "tcp_server")]
type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;
#[cfg(feature = "tcp_server")]
use kanata_parser::cfg::SimpleSExpr;
#[cfg(feature = "tcp_server")]
use std::io::Write;
#[cfg(feature = "tcp_server")]
use std::net::{TcpListener, TcpStream};

#[cfg(feature = "tcp_server")]
pub type Connections = Arc<Mutex<HashMap<String, TcpStream>>>;

#[cfg(not(feature = "tcp_server"))]
pub type Connections = ();

#[cfg(feature = "tcp_server")]
use kanata_parser::custom_action::FakeKeyAction;

#[cfg(feature = "tcp_server")]
fn send_response(
    stream: &mut TcpStream,
    response: ServerResponse,
    connections: &Connections,
    addr: &str,
) -> bool {
    if let Err(write_err) = stream.write_all(&response.as_bytes()) {
        log::error!("stream write error: {write_err}");
        connections.lock().remove(addr);
        return false;
    }
    true
}

#[cfg(feature = "tcp_server")]
fn to_action(val: FakeKeyActionMessage) -> FakeKeyAction {
    match val {
        FakeKeyActionMessage::Press => FakeKeyAction::Press,
        FakeKeyActionMessage::Release => FakeKeyAction::Release,
        FakeKeyActionMessage::Tap => FakeKeyAction::Tap,
        FakeKeyActionMessage::Toggle => FakeKeyAction::Toggle,
    }
}

#[cfg(feature = "tcp_server")]
pub struct TcpServer {
    pub address: SocketAddr,
    pub connections: Connections,
    pub wakeup_channel: Sender<KeyEvent>,
}

#[cfg(not(feature = "tcp_server"))]
pub struct TcpServer {
    pub connections: Connections,
}

impl TcpServer {
    #[cfg(feature = "tcp_server")]
    pub fn new(address: SocketAddr, wakeup_channel: Sender<KeyEvent>) -> Self {
        Self {
            address,
            connections: Arc::new(Mutex::new(HashMap::default())),
            wakeup_channel,
        }
    }

    #[cfg(not(feature = "tcp_server"))]
    pub fn new(_address: SocketAddr, _wakeup_channel: Sender<KeyEvent>) -> Self {
        Self { connections: () }
    }

    #[cfg(feature = "tcp_server")]
    pub fn start(&mut self, kanata: Arc<Mutex<Kanata>>) {
        use kanata_parser::cfg::FAKE_KEY_ROW;

        use crate::kanata::handle_fakekey_action;

        let listener = TcpListener::bind(self.address).expect("TCP server starts");

        let connections = self.connections.clone();
        let wakeup_channel = self.wakeup_channel.clone();

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
                        let reader = serde_json::Deserializer::from_reader(
                            stream.try_clone().expect("stream is clonable"),
                        )
                        .into_iter::<ClientMessage>();

                        log::info!("listening for incoming messages {addr}");

                        let connections = connections.clone();
                        let kanata = kanata.clone();
                        let wakeup_channel = wakeup_channel.clone();
                        std::thread::spawn(move || {
                            for v in reader {
                                match v {
                                    Ok(event) => {
                                        log::debug!("tcp server received command: {:?}", event);
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
                                                        .map(|info| info.name.clone())
                                                        .collect::<Vec<_>>(),
                                                };
                                                match stream.write_all(&msg.as_bytes()) {
                                                    Ok(_) => {}
                                                    Err(err) => log::error!(
                                                        "server could not send response: {err}"
                                                    ),
                                                }
                                            }
                                            ClientMessage::ActOnFakeKey { name, action } => {
                                                let mut k = kanata.lock();
                                                let index = match k.virtual_keys.get(&name) {
                                                    Some(index) => Some(*index as u16),
                                                    None => {
                                                        if let Err(e) = stream.write_all(
                                                            &ServerMessage::Error {
                                                                msg: format!(
                                                                "unknown virtual/fake key: {name}"
                                                            ),
                                                            }
                                                            .as_bytes(),
                                                        ) {
                                                            log::error!("stream write error: {e}");
                                                            connections.lock().remove(&addr);
                                                            break;
                                                        }
                                                        continue;
                                                    }
                                                };
                                                if let Some(index) = index {
                                                    log::info!(
                                                        "tcp server fake-key action: {name},{action:?}"
                                                    );
                                                    handle_fakekey_action(
                                                        to_action(action),
                                                        k.layout.bm(),
                                                        FAKE_KEY_ROW,
                                                        index,
                                                    );
                                                }
                                                drop(k);
                                            }
                                            ClientMessage::SetMouse { x, y } => {
                                                log::info!(
                                                    "tcp server SetMouse action: x {x} y {y}"
                                                );
                                                match kanata.lock().kbd_out.set_mouse(x, y) {
                                                    Ok(_) => {
                                                        log::info!(
                                                            "sucessfully did set mouse position to: x {x} y {y}"
                                                        );
                                                        // Optionally send a success message to the
                                                        // client
                                                    }
                                                    Err(e) => {
                                                        log::error!(
                                                            "Failed to set mouse position: {}",
                                                            e
                                                        );
                                                        // Implement any error handling logic here,
                                                        // such as sending an error response to
                                                        // the client
                                                    }
                                                }
                                            }
                                            ClientMessage::RequestCurrentLayerInfo {} => {
                                                let mut k = kanata.lock();
                                                let cur_layer = k.layout.bm().current_layer();
                                                let msg = ServerMessage::CurrentLayerInfo {
                                                    name: k.layer_info[cur_layer].name.clone(),
                                                    cfg_text: k.layer_info[cur_layer]
                                                        .cfg_text
                                                        .clone(),
                                                };
                                                drop(k);
                                                match stream.write_all(&msg.as_bytes()) {
                                                    Ok(_) => {}
                                                    Err(err) => log::error!(
                                                        "Error writing response to RequestCurrentLayerInfo: {err}"
                                                    ),
                                                }
                                            }
                                            ClientMessage::RequestCurrentLayerName {} => {
                                                let mut k = kanata.lock();
                                                let cur_layer = k.layout.bm().current_layer();
                                                let msg = ServerMessage::CurrentLayerName {
                                                    name: k.layer_info[cur_layer].name.clone(),
                                                };
                                                drop(k);
                                                match stream.write_all(&msg.as_bytes()) {
                                                    Ok(_) => {}
                                                    Err(err) => log::error!(
                                                        "Error writing response to RequestCurrentLayerName: {err}"
                                                    ),
                                                }
                                            }
                                            // Handle reload commands with unified response protocol
                                            reload_cmd @ (ClientMessage::Reload {}
                                            | ClientMessage::ReloadNext {}
                                            | ClientMessage::ReloadPrev {}
                                            | ClientMessage::ReloadNum { .. }
                                            | ClientMessage::ReloadFile { .. }) => {
                                                // Log specific action type
                                                match &reload_cmd {
                                                    ClientMessage::Reload {} => {
                                                        log::info!("tcp server Reload action")
                                                    }
                                                    ClientMessage::ReloadNext {} => {
                                                        log::info!("tcp server ReloadNext action")
                                                    }
                                                    ClientMessage::ReloadPrev {} => {
                                                        log::info!("tcp server ReloadPrev action")
                                                    }
                                                    ClientMessage::ReloadNum { index } => {
                                                        log::info!(
                                                            "tcp server ReloadNum action: index {index}"
                                                        )
                                                    }
                                                    ClientMessage::ReloadFile { path } => {
                                                        log::info!(
                                                            "tcp server ReloadFile action: path {path}"
                                                        )
                                                    }
                                                    _ => unreachable!(),
                                                }

                                                let response = match kanata
                                                    .lock()
                                                    .handle_client_command(reload_cmd)
                                                {
                                                    Ok(_) => ServerResponse::Ok,
                                                    Err(e) => ServerResponse::Error {
                                                        msg: format!("{e}"),
                                                    },
                                                };
                                                if !send_response(
                                                    &mut stream,
                                                    response,
                                                    &connections,
                                                    &addr,
                                                ) {
                                                    break;
                                                }
                                            }
                                        }
                                        use kanata_parser::keys::*;
                                        wakeup_channel
                                            .send(KeyEvent {
                                                code: OsCode::KEY_RESERVED,
                                                value: KeyValue::WakeUp,
                                            })
                                            .expect("write key event");
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "client sent an invalid message, disconnecting them. Err: {e:?}"
                                        );
                                        // Send proper error response for malformed JSON
                                        let response = ServerResponse::Error {
                                            msg: format!("Failed to deserialize command: {e}"),
                                        };
                                        let _ = stream.write_all(&response.as_bytes());
                                        connections.lock().remove(&addr);
                                        break;
                                    }
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

#[cfg(feature = "tcp_server")]
pub fn simple_sexpr_to_json_array(exprs: &[SimpleSExpr]) -> serde_json::Value {
    let mut result = Vec::new();

    for expr in exprs.iter() {
        match expr {
            SimpleSExpr::Atom(s) => result.push(serde_json::Value::String(s.clone())),
            SimpleSExpr::List(list) => result.push(simple_sexpr_to_json_array(list)),
        }
    }

    serde_json::Value::Array(result)
}
