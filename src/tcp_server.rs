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

                        let addr = match stream.peer_addr() {
                            Ok(addr) => addr.to_string(),
                            Err(e) => {
                                log::warn!("failed to get peer address, using fallback: {e:?}");
                                format!("unknown_{}", std::ptr::addr_of!(stream) as usize)
                            }
                        };

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
                                                    }
                                                    Err(e) => {
                                                        log::error!(
                                                            "Failed to set mouse position: {}",
                                                            e
                                                        );
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
                                            // New command: Hello - capability detection
                                            ClientMessage::Hello {} => {
                                                let version = env!("CARGO_PKG_VERSION").to_string();
                                                let capabilities = vec![
                                                    "reload".to_string(),
                                                    "layer-names".to_string(),
                                                    "layer-change".to_string(),
                                                    "current-layer-name".to_string(),
                                                    "current-layer-info".to_string(),
                                                    "fake-key".to_string(),
                                                    "set-mouse".to_string(),
                                                ];
                                                let msg = ServerMessage::HelloOk {
                                                    version,
                                                    protocol: 1,
                                                    capabilities,
                                                };
                                                match stream.write_all(&msg.as_bytes()) {
                                                    Ok(_) => {
                                                        let _ = stream.flush();
                                                    }
                                                    Err(err) => {
                                                        log::error!(
                                                            "Error writing HelloOk response: {err}"
                                                        );
                                                        connections.lock().remove(&addr);
                                                        break;
                                                    }
                                                }
                                            }
                                            // Enhanced reload commands with wait/timeout
                                            ref reload_cmd @ (ClientMessage::Reload { .. }
                                            | ClientMessage::ReloadNext {
                                                ..
                                            }
                                            | ClientMessage::ReloadPrev {
                                                ..
                                            }
                                            | ClientMessage::ReloadNum {
                                                ..
                                            }
                                            | ClientMessage::ReloadFile {
                                                ..
                                            }) => {
                                                // Extract wait and timeout from command
                                                let (wait_flag, timeout) = match reload_cmd {
                                                    ClientMessage::Reload { wait, timeout_ms } => {
                                                        (*wait, *timeout_ms)
                                                    }
                                                    ClientMessage::ReloadNext {
                                                        wait,
                                                        timeout_ms,
                                                    } => (*wait, *timeout_ms),
                                                    ClientMessage::ReloadPrev {
                                                        wait,
                                                        timeout_ms,
                                                    } => (*wait, *timeout_ms),
                                                    ClientMessage::ReloadNum {
                                                        wait,
                                                        timeout_ms,
                                                        ..
                                                    } => (*wait, *timeout_ms),
                                                    ClientMessage::ReloadFile {
                                                        wait,
                                                        timeout_ms,
                                                        ..
                                                    } => (*wait, *timeout_ms),
                                                    _ => (None, None),
                                                };

                                                // Log specific action type
                                                match reload_cmd {
                                                    ClientMessage::Reload { .. } => {
                                                        log::info!("tcp server Reload action")
                                                    }
                                                    ClientMessage::ReloadNext { .. } => {
                                                        log::info!("tcp server ReloadNext action")
                                                    }
                                                    ClientMessage::ReloadPrev { .. } => {
                                                        log::info!("tcp server ReloadPrev action")
                                                    }
                                                    ClientMessage::ReloadNum { index, .. } => {
                                                        log::info!(
                                                            "tcp server ReloadNum action: index {index}"
                                                        )
                                                    }
                                                    ClientMessage::ReloadFile { path, .. } => {
                                                        log::info!(
                                                            "tcp server ReloadFile action: path {path}"
                                                        )
                                                    }
                                                    _ => unreachable!(),
                                                }

                                                let (response, reload_ok) = match kanata
                                                    .lock()
                                                    .handle_client_command(reload_cmd.clone())
                                                {
                                                    Ok(_) => (ServerResponse::Ok, true),
                                                    Err(e) => (
                                                        ServerResponse::Error {
                                                            msg: format!("{e}"),
                                                        },
                                                        false,
                                                    ),
                                                };
                                                if !send_response(
                                                    &mut stream,
                                                    response,
                                                    &connections,
                                                    &addr,
                                                ) {
                                                    break;
                                                }

                                                // If wait flag is set and reload started successfully,
                                                // poll for completion (success or failure)
                                                if reload_ok && wait_flag.unwrap_or(false) {
                                                    let timeout_ms = timeout.unwrap_or(5000);
                                                    let poll_interval =
                                                        std::time::Duration::from_millis(50);
                                                    let start = std::time::Instant::now();
                                                    let timeout_duration =
                                                        std::time::Duration::from_millis(
                                                            timeout_ms,
                                                        );

                                                    // Wait for reload to complete (success or failure)
                                                    let mut timed_out = false;
                                                    loop {
                                                        if start.elapsed() >= timeout_duration {
                                                            timed_out = true;
                                                            break;
                                                        }
                                                        if kanata.lock().is_reload_complete() {
                                                            break;
                                                        }
                                                        std::thread::sleep(poll_interval);
                                                    }

                                                    // Check final state: ok means success,
                                                    // complete but not ok means failure
                                                    let ok = kanata.lock().last_reload_succeeded();
                                                    let msg = ServerMessage::ReloadResult {
                                                        ok,
                                                        timeout_ms: if timed_out {
                                                            Some(timeout_ms)
                                                        } else {
                                                            None
                                                        },
                                                    };
                                                    match stream.write_all(&msg.as_bytes()) {
                                                        Ok(_) => {
                                                            let _ = stream.flush();
                                                        }
                                                        Err(err) => {
                                                            log::error!(
                                                                "Error writing ReloadResult: {err}"
                                                            );
                                                            connections.lock().remove(&addr);
                                                            break;
                                                        }
                                                    }
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
