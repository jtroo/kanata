//! iced_gui code for the GUI --run-gui process option,
//! which is typically expected to be a child process.
//!
//! Connect to Kanata on its TCP port.
//! Subscribe to UI updates.
//! Handle TCP messages from Kanata main process to update the UI.

use async_net::TcpStream;
use futures::io::BufReader;
use futures::prelude::*;
use iced::widget::{Column, column, text};
use kanata_tcp_protocol::*;

pub(crate) struct KanataGui {
    layer_content: String,
    active_vkeys: String,
    chv2_state: String,
    zch_state: String,
}

impl KanataGui {
    pub(crate) fn start(addr: std::net::SocketAddr) -> iced::Result {
        iced::application("Kanata", Self::update, Self::view)
            .subscription(move |_| {
                iced::Subscription::run_with_id(
                    0,
                    iced::stream::channel(10, async move |mut sender| {
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
                        loop {
                            if let Err(e) = stream.read_line(&mut buf).await {
                                log::error!("read from kanata sock error: {e:?}");
                                continue;
                            }
                            let msg = match ServerMessage::deserialize_json(&buf) {
                                Ok(v) => v,
                                Err(e) => {
                                    log::error!("deserialize server message error {e:?}");
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
            .run_with(|| (Self::new(), iced::Task::none()))
    }

    fn new() -> Self {
        let mut kg = Self {
            layer_content: String::new(),
            active_vkeys: String::new(),
            chv2_state: String::new(),
            zch_state: String::new(),
        };
        kg.update(ServerMessage::ConfigFileReload { new: "".into() });
        kg
    }

    pub(crate) fn view(&self) -> Column<'_, ServerMessage> {
        column![
            text("Active Layer:"),
            text(&self.layer_content),
            text("Active VKeys:"),
            text(&self.active_vkeys),
            text("ChordsV2 State:"),
            text(&self.chv2_state),
            text("Zippychord State:"),
            text(&self.zch_state),
        ]
    }

    pub(crate) fn update(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::DetailedInfo(info) => {
                self.layer_content = info.layer_config;
                self.active_vkeys = info.active_vkey_names;
                self.chv2_state = info.chordsv2_state;
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

/// Start up the same Kanata binary using the typical argv[0] name as a child process,
/// but passes in only the `--run-gui` flag
/// which will start up the GUI process.
pub(crate) fn spawn_child_gui_process() {
    if let Err(e) = std::process::Command::new(std::env::args().next().unwrap())
        .arg("--run-gui")
        .spawn()
    {
        log::error!("failed to spawn GUI: {e}");
    }
}
