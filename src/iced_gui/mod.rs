//! iced_gui code for the GUI --run-gui process option,
//! which is typically expected to be a child process.
//!
//! Connect to Kanata on its TCP port.
//! Subscribe to UI updates.
//! Handle TCP messages from Kanata main process to update the UI.

use async_net::TcpStream;
use futures::io::BufReader;
use futures::prelude::*;
use iced::widget::{Column, column, Rule, Space, text};
use kanata_tcp_protocol::*;

#[derive(Clone, Debug, Default)]
pub(crate) struct KanataGui {
    layer_content: String,
    active_vkeys: String,
    zch_state: String,
}

impl KanataGui {
    pub(crate) fn start(addr: std::net::SocketAddr) -> iced::Result {
        iced::application("Kanata", Self::update, Self::view)
            .subscription(move |_| {
                iced::Subscription::run_with_id(
                    0,
                    iced::stream::channel(10, async move |mut sender| {
                        log::info!("Attempting connection to Kanata");
                        let mut buf = String::new();
                        let mut stream = BufReader::new(
                            TcpStream::connect(addr)
                                .await
                                .expect("connect to kanata main proc"),
                        );
                        stream.read_line(&mut buf).await.expect("read LayerChange");
                        let msg = ServerMessage::deserialize_json(&buf)
                            .expect("kanata sends LayerChange to client on connect");
                        buf.clear();
                        sender.try_send(msg).unwrap();
                        stream
                            .write_all(&ClientMessage::SubscribeToDetailedInfo.as_bytes())
                            .await
                            .expect("write to kanata succeeds");
                        stream.read_line(&mut buf).await.expect("read Ok");
                        match ServerResponse::deserialize_json(&buf) {
                            Ok(ServerResponse::Ok) => {}
                            Ok(ServerResponse::Error { msg }) => {
                                panic!("kanata rejected subscribe with error: {msg}")
                            }
                            Err(e) => panic!("error: {e:?}"),
                        };
                        buf.clear();

                        log::info!("Connected to kanata successfully. Waiting for info updates.");
                        loop {
                            log::debug!("trying to read a line");
                            if let Err(e) = stream.read_line(&mut buf).await {
                                log::error!("read from kanata sock error: {e:?}");
                                std::thread::sleep(std::time::Duration::from_secs(1));
                                continue;
                            }
                            let msg = match ServerMessage::deserialize_json(&buf) {
                                Ok(v) => v,
                                Err(e) => {
                                    log::error!(
                                        "deserialize server message error {e:?}. msg: {buf}"
                                    );
                                    continue;
                                }
                            };
                            buf.clear();
                            if let Err(e) = sender.try_send(msg) {
                                log::error!("write to iced subscribe channel error: {e:?}");
                            }
                        }
                    }),
                )
            })
            .run()
    }

    pub(crate) fn view(&self) -> Column<'_, ServerMessage> {
        use iced::advanced::text::*;
        match self.zch_state.is_empty() {
            false => {
                // Zippychord is enabled
                column![
                    text("Active Layer:")
                        .size(32)
                        .line_height(LineHeight::Absolute(60f32.into())),
                    text(&self.layer_content)
                        .font(iced::Font::MONOSPACE)
                        .shaping(Shaping::Advanced),
                    Space::new(0, 30),

                    // Container boundary
                    Rule::horizontal(0),
                    Space::new(0, 10),
                    text("Active VKeys:")
                        .size(32)
                        .line_height(LineHeight::Absolute(60f32.into())),
                    text(match self.active_vkeys.is_empty() {
                        false => &self.active_vkeys,
                        true => "No active virtual keys",
                    })
                        .font(iced::Font::MONOSPACE)
                        .shaping(Shaping::Advanced),
                    Space::new(0, 30),

                    // Container boundary
                    Rule::horizontal(0),
                    Space::new(0, 10),
                    text("Zippychord State:")
                        .size(32)
                        .line_height(LineHeight::Absolute(60f32.into())),
                    text(&self.zch_state)
                        .font(iced::Font::MONOSPACE)
                        .shaping(Shaping::Advanced),
                ]
            }
            true => {
                // Zippychord is disabled
                column![
                    text("Active Layer:")
                        .size(32)
                        .line_height(LineHeight::Absolute(60f32.into())),
                    text(&self.layer_content)
                        .font(iced::Font::MONOSPACE)
                        .shaping(Shaping::Advanced),
                    Space::new(0, 30),

                    // Container boundary
                    Rule::horizontal(0),
                    Space::new(0, 10),
                    text("Active VKeys:")
                        .size(32)
                        .line_height(LineHeight::Absolute(60f32.into())),
                    text(&self.active_vkeys)
                        .font(iced::Font::MONOSPACE)
                        .shaping(Shaping::Advanced),
                ]
            }
        }
    }

    pub(crate) fn update(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::DetailedInfo(info) => {
                log::debug!("got info!");
                self.layer_content = info.layer_config;
                self.active_vkeys = info.active_vkey_names;
                self.zch_state = info.zippychord_state;
            }
            ServerMessage::LayerChange { .. }
            | ServerMessage::LayerNames { .. }
            | ServerMessage::CurrentLayerInfo { .. }
            | ServerMessage::ConfigFileReload { .. }
            | ServerMessage::CurrentLayerName { .. }
            | ServerMessage::MessagePush { .. }
            | ServerMessage::Error { .. } => {}
        }
    }
}

/// Start up the same Kanata binary as a child process,
/// expecting that the convention is followed that `argv[0]`
/// is the executable path of Kanata itself.
/// Passes in only the `--run-gui` and `-p` flags to the child,
/// which will start up the GUI process, connecting on the specified port.
pub(crate) fn spawn_child_gui_process(p_flag: &str) {
    if let Err(e) = std::process::Command::new(std::env::args().next().unwrap())
        .arg("--run-gui")
        .arg("-p")
        .arg(p_flag)
        .spawn()
    {
        log::error!("failed to spawn GUI: {e}");
    }
}
