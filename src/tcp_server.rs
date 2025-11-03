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
        // Minimal subscription registry keyed by addr -> events
        let subscriptions: Arc<Mutex<HashMap<String, Vec<String>>>> =
            Arc::new(Mutex::new(HashMap::default()));
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
                        let subscriptions = subscriptions.clone();
                        let kanata = kanata.clone();
                        let wakeup_channel = wakeup_channel.clone();
                        std::thread::spawn(move || {
                            for v in reader {
                                match v {
                                    Ok(event) => {
                                        log::debug!("tcp server received command: {:?}", event);
                                        match event {
                                            // TCP server ignores authentication messages since TCP doesn't use auth
                                            ClientMessage::Authenticate { .. } => {
                                                log::debug!(
                                                    "TCP server ignoring authentication message (not needed for TCP)"
                                                );
                                                continue;
                                            }
                                            ClientMessage::ChangeLayer { new, .. } => {
                                                kanata.lock().change_layer(new);
                                            }
                                            ClientMessage::RequestLayerNames { .. } => {
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
                                            ClientMessage::ActOnFakeKey {
                                                name, action, ..
                                            } => {
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
                                            ClientMessage::SetMouse { x, y, .. } => {
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
                                            ClientMessage::RequestCurrentLayerInfo { .. } => {
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
                                            ClientMessage::RequestCurrentLayerName { .. } => {
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
                                            ClientMessage::Hello { .. } => {
                                                let version = env!("CARGO_PKG_VERSION").to_string();
                                                let capabilities = vec![
                                                    "reload".to_string(),
                                                    "status".to_string(),
                                                    "ready".to_string(),
                                                ];
                                                let msg = ServerMessage::HelloOk {
                                                    version,
                                                    protocol: 1,
                                                    capabilities,
                                                };
                                                // Send status response first
                                                if !send_response(
                                                    &mut stream,
                                                    ServerResponse::Ok,
                                                    &connections,
                                                    &addr,
                                                ) {
                                                    break;
                                                }
                                                // Send HelloOk details on second line
                                                match stream.write_all(&msg.as_bytes()) {
                                                    Ok(_) => {
                                                        // Flush to ensure immediate delivery
                                                        let _ = stream.flush();
                                                    }
                                                    Err(err) => {
                                                        log::error!(
                                                            "Error writing HelloOk response: {err}"
                                                        );
                                                        // Don't break connection - first line already sent successfully
                                                    }
                                                }
                                            }
                                            ClientMessage::Status { .. } => {
                                                let k = kanata.lock();
                                                let engine_version =
                                                    env!("CARGO_PKG_VERSION").to_string();
                                                let uptime_s = k.get_uptime_s();
                                                let ready = k.is_ready();
                                                let last_reload = k.get_last_reload_info();
                                                drop(k);

                                                let msg = ServerMessage::StatusInfo {
                                                    engine_version,
                                                    uptime_s,
                                                    ready,
                                                    last_reload,
                                                };
                                                // Send status response first
                                                if !send_response(
                                                    &mut stream,
                                                    ServerResponse::Ok,
                                                    &connections,
                                                    &addr,
                                                ) {
                                                    break;
                                                }
                                                // Send StatusInfo details on second line
                                                match stream.write_all(&msg.as_bytes()) {
                                                    Ok(_) => {
                                                        // Flush to ensure immediate delivery
                                                        let _ = stream.flush();
                                                    }
                                                    Err(err) => {
                                                        log::error!(
                                                            "Error writing StatusInfo response: {err}"
                                                        );
                                                        // Don't break connection - first line already sent successfully
                                                    }
                                                }
                                            }

                                            // Validate config (preflight)
                                            ClientMessage::Validate { config, .. } => {
                                                // Default: strict mode behavior; for now unused
                                                // Try parsing using kanata_parser
                                                let (warnings, errors) =
                                                    match kanata_parser::cfg::new_from_str(
                                                        &config,
                                                        HashMap::default(),
                                                    ) {
                                                        Ok(_) => (Vec::new(), Vec::new()),
                                                        Err(e) => {
                                                            let item = ValidationItem {
                                                                message: format!("{e}"),
                                                                line: None,
                                                                column: None,
                                                                code: Some(
                                                                    "CONFIG_PARSE".to_string(),
                                                                ),
                                                            };
                                                            (Vec::new(), vec![item])
                                                        }
                                                    };

                                                // Send status then details
                                                if !send_response(
                                                    &mut stream,
                                                    ServerResponse::Ok,
                                                    &connections,
                                                    &addr,
                                                ) {
                                                    break;
                                                }
                                                let msg = ServerMessage::ValidationResult {
                                                    warnings,
                                                    errors,
                                                };
                                                let _ = stream.write_all(&msg.as_bytes());
                                            }

                                            // Subscribe to events (stubbed)
                                            ClientMessage::Subscribe { events, .. } => {
                                                subscriptions.lock().insert(addr.clone(), events);
                                                let _ = send_response(
                                                    &mut stream,
                                                    ServerResponse::Ok,
                                                    &connections,
                                                    &addr,
                                                );
                                            }

                                            // Handle reload commands with unified response protocol
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
                                                // Extract wait and timeout_ms from the command
                                                let (wait_flag, timeout) = match reload_cmd {
                                                    ClientMessage::Reload {
                                                        wait,
                                                        timeout_ms,
                                                        ..
                                                    } => (*wait, *timeout_ms),
                                                    ClientMessage::ReloadNext {
                                                        wait,
                                                        timeout_ms,
                                                        ..
                                                    } => (*wait, *timeout_ms),
                                                    ClientMessage::ReloadPrev {
                                                        wait,
                                                        timeout_ms,
                                                        ..
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
                                                match &reload_cmd {
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

                                                let response = match kanata
                                                    .lock()
                                                    .handle_client_command(reload_cmd.clone())
                                                {
                                                    Ok(_) => ServerResponse::Ok,
                                                    Err(e) => ServerResponse::Error {
                                                        msg: format!("{e}"),
                                                    },
                                                };
                                                let was_ok = matches!(response, ServerResponse::Ok);

                                                // Send initial status response
                                                if !send_response(
                                                    &mut stream,
                                                    response,
                                                    &connections,
                                                    &addr,
                                                ) {
                                                    break;
                                                }

                                                // If there was an immediate error, optionally send structured detail
                                                if !was_ok {
                                                    let detail = ServerMessage::ErrorDetail {
                                                        code: "RELOAD_FAILED".to_string(),
                                                        message: "Reload request failed"
                                                            .to_string(),
                                                        line: None,
                                                        column: None,
                                                    };
                                                    let _ = stream.write_all(&detail.as_bytes());
                                                }

                                                // If wait is requested, check readiness and send ReloadResult
                                                if was_ok && wait_flag == Some(true) {
                                                    let timeout_val = timeout.unwrap_or(2000);
                                                    let start = std::time::Instant::now();
                                                    let mut ready = false;

                                                    // Poll for readiness with timeout
                                                    // Note: This blocks the TCP handler thread, but timeout is bounded (default 2s)
                                                    // and readiness should be reached quickly after reload completes
                                                    while start.elapsed().as_millis()
                                                        < timeout_val as u128
                                                    {
                                                        let k = kanata.lock();
                                                        ready = k.is_ready();
                                                        drop(k);

                                                        if ready {
                                                            break;
                                                        }

                                                        // Small sleep to avoid busy-waiting
                                                        std::thread::sleep(
                                                            std::time::Duration::from_millis(50),
                                                        );
                                                    }

                                                    let result_msg = if ready {
                                                        ServerMessage::ReloadResult {
                                                            ready: true,
                                                            timeout_ms: None,
                                                        }
                                                    } else {
                                                        ServerMessage::ReloadResult {
                                                            ready: false,
                                                            timeout_ms: Some(timeout_val),
                                                        }
                                                    };

                                                    // Send ReloadResult details on second line
                                                    match stream.write_all(&result_msg.as_bytes()) {
                                                        Ok(_) => {
                                                            // Flush to ensure immediate delivery
                                                            let _ = stream.flush();
                                                        }
                                                        Err(err) => {
                                                            log::error!(
                                                                "Error writing ReloadResult response: {err}"
                                                            );
                                                            // Don't break connection - first line already sent successfully
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
