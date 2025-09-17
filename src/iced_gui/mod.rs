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
    layer_name: String,
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
                        let msg = ServerMessage::deserialize_json(&buf).unwrap();
                        buf.clear();
                        sender.try_send(msg).unwrap();
                        stream
                            .write_all(&ClientMessage::SubscribeToDetailedInfo.as_bytes())
                            .await
                            .expect("write to kanata");
                        stream.read_line(&mut buf).await.expect("read Ok");
                        match ServerResponse::deserialize_json(&buf) {
                            Ok(ServerResponse::Ok) => {}
                            Ok(ServerResponse::Error { msg }) => {
                                panic!("kanata rejected subscribe with error: {msg}")
                            }
                            Err(e) => panic!("error: {e:?}"),
                        };
                        loop {
                            stream.read_line(&mut buf).await.unwrap();
                            let msg = ServerMessage::deserialize_json(&buf).unwrap();
                            buf.clear();
                            sender.try_send(msg).unwrap();
                        }
                    }),
                )
            })
            .run_with(|| (Self::new(), iced::Task::none()))
    }

    fn new() -> Self {
        let mut kg = Self {
            layer_name: String::new(),
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
            text("Active Layer Name:"),
            text(&self.layer_name),
            text("Active Layer Content:"),
            text(&self.layer_content),
            text("Active VKeys:"),
            text(&self.active_vkeys),
            text("ChordsV2 State:"),
            text(&self.chv2_state),
            text("Zippychord State:"),
            text(&self.zch_state),
        ]
    }

    pub(crate) fn update(&mut self, _: ServerMessage) {}
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
