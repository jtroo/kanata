use crate::Kanata;
use crate::oskbd::*;

#[cfg(feature = "udp_server")]
use kanata_tcp_protocol::*;
use parking_lot::Mutex;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc::SyncSender as Sender;

#[cfg(feature = "udp_server")]
use std::collections::HashMap;
#[cfg(feature = "udp_server")]
use std::net::UdpSocket;
use std::time::Duration;
#[cfg(feature = "udp_server")]
use std::time::SystemTime;
#[cfg(feature = "udp_server")]
use rand::{thread_rng, Rng};
#[cfg(feature = "udp_server")]
use rand::distributions::Alphanumeric;

#[cfg(feature = "udp_server")]
#[derive(Debug, PartialEq)]
enum AuthStatus {
    Authenticated,
    NotAuthenticated,
    SessionExpired,
}

#[cfg(feature = "udp_server")]
type Sessions = Arc<Mutex<HashMap<SocketAddr, SessionInfo>>>;

#[cfg(not(feature = "udp_server"))]
pub type Sessions = ();

#[cfg(feature = "udp_server")]
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub last_activity: SystemTime,
    pub expires_at: SystemTime,
    pub client_name: Option<String>,
}

#[cfg(feature = "udp_server")]
pub struct UdpServer {
    pub address: SocketAddr,
    pub auth_token: String,
    pub sessions: Sessions,
    pub wakeup_channel: Sender<KeyEvent>,
    pub session_timeout: Duration,
    pub auth_required: bool,
}

#[cfg(not(feature = "udp_server"))]
pub struct UdpServer {
    pub sessions: Sessions,
}

impl UdpServer {
    #[cfg(feature = "udp_server")]
    pub fn new(
        address: SocketAddr,
        wakeup_channel: Sender<KeyEvent>,
        auth_token: Option<String>,
        session_timeout: Duration,
        auth_required: bool,
    ) -> Self {
        let token = auth_token.unwrap_or_else(|| {
            thread_rng()
                .sample_iter(&Alphanumeric)
                .take(32)
                .map(char::from)
                .collect()
        });

        Self {
            address,
            auth_token: token,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            wakeup_channel,
            session_timeout,
            auth_required,
        }
    }

    #[cfg(not(feature = "udp_server"))]
    pub fn new(
        _address: SocketAddr,
        _wakeup_channel: Sender<KeyEvent>,
        _auth_token: Option<String>,
        _session_timeout: Duration,
        _auth_required: bool,
    ) -> Self {
        Self { sessions: () }
    }

    #[cfg(feature = "udp_server")]
    pub fn get_auth_token(&self) -> &str {
        &self.auth_token
    }

    #[cfg(not(feature = "udp_server"))]
    pub fn get_auth_token(&self) -> &str {
        ""
    }

    #[cfg(feature = "udp_server")]
    pub fn start(&mut self, kanata: Arc<Mutex<Kanata>>) -> Result<(), Box<dyn std::error::Error>> {

        // Create and bind UDP socket
        let socket = UdpSocket::bind(self.address)?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        log::info!("UDP server started on {}", self.address);
        
        if self.auth_required {
            log::info!("UDP auth token: {} (save this for clients)", self.auth_token);
        } else {
            log::warn!("UDP server running without authentication - this is insecure!");
        }
        
        let sessions = self.sessions.clone();
        let wakeup_channel = self.wakeup_channel.clone();
        let auth_token = self.auth_token.clone();
        let session_timeout = self.session_timeout;
        let auth_required = self.auth_required;

        // Start session cleanup thread
        let cleanup_sessions = sessions.clone();
        std::thread::Builder::new()
            .name("udp-session-cleanup".to_string())
            .spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(60)); // Cleanup every minute
                    let now = SystemTime::now();
                    let mut sessions_guard = cleanup_sessions.lock();
                    sessions_guard.retain(|addr, session| {
                        let keep = now < session.expires_at;
                        if !keep {
                            log::info!("UDP session expired for {}", addr);
                        }
                        keep
                    });
                }
            })?;

        // Main UDP server loop
        std::thread::Builder::new()
            .name("udp-server".to_string())
            .spawn(move || {
                let mut buf = [0u8; 4096];
                
                loop {
                    match socket.recv_from(&mut buf) {
                        Ok((size, addr)) => {
                            let data = &buf[..size];
                            
                            // Parse the message
                            let message: ClientMessage = match serde_json::from_slice(data) {
                                Ok(msg) => msg,
                                Err(e) => {
                                    log::warn!("Failed to parse UDP message from {}: {}", addr, e);
                                    let error_response = ServerMessage::Error {
                                        msg: format!("Failed to parse message: {}", e),
                                    };
                                    let _ = socket.send_to(&error_response.as_bytes(), addr);
                                    continue;
                                }
                            };

                            log::debug!("UDP server received command from {}: {:?}", addr, message);

                            // Handle authentication
                            if let ClientMessage::Authenticate { token, client_name } = message {
                                Self::handle_authentication(
                                    &socket, addr, &token, client_name, &auth_token, 
                                    &sessions, session_timeout, auth_required
                                );
                                continue;
                            }

                            // Check authentication for other messages
                            if auth_required {
                                match Self::check_authentication(&message, addr, &sessions) {
                                    AuthStatus::Authenticated => {
                                        // Continue processing
                                    }
                                    AuthStatus::NotAuthenticated => {
                                        let response = ServerMessage::AuthRequired;
                                        let _ = socket.send_to(&response.as_bytes(), addr);
                                        continue;
                                    }
                                    AuthStatus::SessionExpired => {
                                        let response = ServerMessage::SessionExpired;
                                        let _ = socket.send_to(&response.as_bytes(), addr);
                                        continue;
                                    }
                                }
                            }

                            // Update session activity
                            if auth_required {
                                Self::update_session_activity(addr, &sessions);
                            }

                            // Handle the actual message
                            Self::handle_client_message(
                                message, &socket, addr, &kanata, &wakeup_channel
                            );
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // Timeout is expected, continue
                            continue;
                        }
                        Err(e) => {
                            log::error!("UDP server receive error: {}", e);
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            })?;

        Ok(())
    }

    #[cfg(feature = "udp_server")]
    fn handle_authentication(
        socket: &UdpSocket,
        addr: SocketAddr,
        provided_token: &str,
        client_name: Option<String>,
        expected_token: &str,
        sessions: &Sessions,
        session_timeout: Duration,
        auth_required: bool,
    ) {
        if !auth_required {
            // Authentication disabled, always succeed
            let response = ServerMessage::AuthResult {
                success: true,
                session_id: Some("no-auth".to_string()),
                expires_in_seconds: Some(u64::MAX),
            };
            let _ = socket.send_to(&response.as_bytes(), addr);
            return;
        }

        if provided_token == expected_token {
            // Generate session
            let session_id: String = thread_rng()
                .sample_iter(&Alphanumeric)
                .take(16)
                .map(char::from)
                .collect();
            
            let now = SystemTime::now();
            let expires_at = now + session_timeout;
            
            let session = SessionInfo {
                session_id: session_id.clone(),
                last_activity: now,
                expires_at,
                client_name: client_name.clone(),
            };

            sessions.lock().insert(addr, session);
            
            let client_info = client_name.as_deref().unwrap_or("unknown");
            log::info!("UDP client '{}' authenticated from {}", client_info, addr);

            let response = ServerMessage::AuthResult {
                success: true,
                session_id: Some(session_id),
                expires_in_seconds: Some(session_timeout.as_secs()),
            };
            let _ = socket.send_to(&response.as_bytes(), addr);
        } else {
            log::warn!("UDP authentication failed for {}", addr);
            let response = ServerMessage::AuthResult {
                success: false,
                session_id: None,
                expires_in_seconds: None,
            };
            let _ = socket.send_to(&response.as_bytes(), addr);
        }
    }

    #[cfg(feature = "udp_server")]
    fn check_authentication(message: &ClientMessage, addr: SocketAddr, sessions: &Sessions) -> AuthStatus {
        let mut sessions_guard = sessions.lock();
        let session = match sessions_guard.get(&addr) {
            Some(s) => s.clone(),
            None => return AuthStatus::NotAuthenticated,
        };

        // Check if session is expired
        if SystemTime::now() > session.expires_at {
            // Remove expired session immediately
            sessions_guard.remove(&addr);
            return AuthStatus::SessionExpired;
        }

        // Extract session_id from the message
        let message_session_id = match message {
            ClientMessage::ChangeLayer { session_id, .. } => session_id,
            ClientMessage::ActOnFakeKey { session_id, .. } => session_id,
            ClientMessage::RequestLayerNames { session_id, .. } => session_id,
            ClientMessage::RequestCurrentLayerName { session_id, .. } => session_id,
            ClientMessage::RequestCurrentLayerInfo { session_id, .. } => session_id,
            ClientMessage::Reload { session_id, .. } => session_id,
            ClientMessage::ReloadNext { session_id, .. } => session_id,
            ClientMessage::ReloadPrev { session_id, .. } => session_id,
            ClientMessage::ReloadNum { session_id, .. } => session_id,
            ClientMessage::ReloadFile { session_id, .. } => session_id,
            ClientMessage::SetMouse { session_id, .. } => session_id,
            ClientMessage::Authenticate { .. } => {
                // Authentication messages don't need session validation
                return AuthStatus::Authenticated;
            }
        };

        // Validate that the session_id in the message matches our stored session
        match message_session_id {
            Some(provided_id) => {
                if provided_id == &session.session_id {
                    AuthStatus::Authenticated
                } else {
                    log::warn!("Session ID mismatch from {}: provided '{}', expected '{}'", 
                              addr, provided_id, session.session_id);
                    AuthStatus::NotAuthenticated
                }
            }
            None => {
                log::warn!("Missing session_id in message from {}", addr);
                AuthStatus::NotAuthenticated
            }
        }
    }

    #[cfg(feature = "udp_server")]
    fn update_session_activity(addr: SocketAddr, sessions: &Sessions) {
        let mut sessions_guard = sessions.lock();
        if let Some(session) = sessions_guard.get_mut(&addr) {
            session.last_activity = SystemTime::now();
        }
    }

    #[cfg(feature = "udp_server")]
    fn handle_client_message(
        message: ClientMessage,
        socket: &UdpSocket,
        addr: SocketAddr,
        kanata: &Arc<Mutex<Kanata>>,
        wakeup_channel: &Sender<KeyEvent>,
    ) {

        match message {
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
                let _ = socket.send_to(&msg.as_bytes(), addr);
            }
            ClientMessage::ActOnFakeKey { name, action, .. } => {
                use kanata_parser::cfg::FAKE_KEY_ROW;
                use crate::kanata::handle_fakekey_action;
                
                let mut k = kanata.lock();
                let index = match k.virtual_keys.get(&name) {
                    Some(index) => Some(*index as u16),
                    None => {
                        let error_msg = ServerMessage::Error {
                            msg: format!("unknown virtual/fake key: {name}"),
                        };
                        let _ = socket.send_to(&error_msg.as_bytes(), addr);
                        return;
                    }
                };
                if let Some(index) = index {
                    log::debug!("UDP server fake-key action: {name},{action:?}");
                    handle_fakekey_action(
                        Self::to_action(action),
                        k.layout.bm(),
                        FAKE_KEY_ROW,
                        index,
                    );
                }
                drop(k);
            }
            ClientMessage::SetMouse { x, y, .. } => {
                log::debug!("UDP server SetMouse action: x {x} y {y}");
                match kanata.lock().kbd_out.set_mouse(x, y) {
                    Ok(_) => {
                        log::debug!("successfully set mouse position to: x {x} y {y}");
                    }
                    Err(e) => {
                        log::error!("Failed to set mouse position: {}", e);
                        let error_msg = ServerMessage::Error {
                            msg: format!("Failed to set mouse position: {}", e),
                        };
                        let _ = socket.send_to(&error_msg.as_bytes(), addr);
                    }
                }
            }
            ClientMessage::RequestCurrentLayerInfo { .. } => {
                let mut k = kanata.lock();
                let cur_layer = k.layout.bm().current_layer();
                let msg = ServerMessage::CurrentLayerInfo {
                    name: k.layer_info[cur_layer].name.clone(),
                    cfg_text: k.layer_info[cur_layer].cfg_text.clone(),
                };
                drop(k);
                let _ = socket.send_to(&msg.as_bytes(), addr);
            }
            ClientMessage::RequestCurrentLayerName { .. } => {
                let mut k = kanata.lock();
                let cur_layer = k.layout.bm().current_layer();
                let msg = ServerMessage::CurrentLayerName {
                    name: k.layer_info[cur_layer].name.clone(),
                };
                drop(k);
                let _ = socket.send_to(&msg.as_bytes(), addr);
            }
            // Handle reload commands
            reload_cmd @ (ClientMessage::Reload { .. }
            | ClientMessage::ReloadNext { .. }
            | ClientMessage::ReloadPrev { .. }
            | ClientMessage::ReloadNum { .. }
            | ClientMessage::ReloadFile { .. }) => {
                let response = match kanata.lock().handle_client_command(reload_cmd) {
                    Ok(_) => ServerResponse::Ok,
                    Err(e) => ServerResponse::Error {
                        msg: format!("{e}"),
                    },
                };
                let _ = socket.send_to(&response.as_bytes(), addr);
            }
            ClientMessage::Authenticate { .. } => {
                // This should have been handled earlier
                log::warn!("Received duplicate authentication message from {}", addr);
            }
        }

        // Send wakeup signal
        use kanata_parser::keys::*;
        if wakeup_channel.try_send(KeyEvent {
            code: OsCode::KEY_RESERVED,
            value: KeyValue::WakeUp,
        }).is_err() {
            log::warn!("Failed to send wakeup signal (channel full or receiver dropped)");
        }
    }

    #[cfg(feature = "udp_server")]
    fn to_action(val: FakeKeyActionMessage) -> kanata_parser::custom_action::FakeKeyAction {
        match val {
            FakeKeyActionMessage::Press => kanata_parser::custom_action::FakeKeyAction::Press,
            FakeKeyActionMessage::Release => kanata_parser::custom_action::FakeKeyAction::Release,
            FakeKeyActionMessage::Tap => kanata_parser::custom_action::FakeKeyAction::Tap,
            FakeKeyActionMessage::Toggle => kanata_parser::custom_action::FakeKeyAction::Toggle,
        }
    }

    #[cfg(not(feature = "udp_server"))]
    pub fn start(&mut self, _kanata: Arc<Mutex<Kanata>>) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}