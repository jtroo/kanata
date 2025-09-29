//! iced_gui code for the GUI --run-gui process option,
//! which is typically expected to be a child process.
//!
//! Connect to Kanata on its TCP port.
//! Subscribe to UI updates.
//! Handle TCP messages from Kanata main process to update the UI.

use async_net::TcpStream;
use futures::io::BufReader;
use futures::prelude::*;
use iced::Element;
use iced::widget::{column, container, pane_grid, text};
use kanata_tcp_protocol::*;

#[derive(Debug)]
pub(crate) enum Message {
    ServerMessage(ServerMessage),
}

#[derive(Debug)]
pub(crate) struct KanataGui {
    panes: pane_grid::State<Pane>,
}

#[derive(Debug, Clone)]
enum Pane {
    LayerPane(String),
    VkeysPane(String),
    ZippyPane(String),
}
use Pane::*;

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
                        sender.try_send(Message::ServerMessage(msg)).unwrap();
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
                            if let Err(e) = sender.try_send(Message::ServerMessage(msg)) {
                                log::error!("write to iced subscribe channel error: {e:?}");
                            }
                        }
                    }),
                )
            })
            .run_with(|| {
                let (mut panes, first_pane) = pane_grid::State::new(LayerPane("".into()));
                let _ = panes.split(pane_grid::Axis::Vertical, first_pane, ZippyPane("".into()));
                let _ = panes.split(pane_grid::Axis::Horizontal, first_pane, VkeysPane("".into()));
                (Self { panes }, iced::Task::none())
            })
    }

    pub(crate) fn view(&self) -> Element<'_, Message> {
        use iced::advanced::text::*;
        pane_grid(&self.panes, |_pane, pane_type, _is_maximized| {
            pane_grid::Content::new(match pane_type {
                LayerPane(l) => container(column![
                    text("Active Layer:")
                        .size(18)
                        .line_height(LineHeight::Absolute(32f32.into())),
                    text(l)
                        .font(iced::Font::MONOSPACE)
                        .shaping(Shaping::Advanced),
                ]),

                VkeysPane(v) => container(column![
                    text("Active VKeys:")
                        .size(18)
                        .line_height(LineHeight::Absolute(32f32.into())),
                    text(match v.is_empty() {
                        false => v,
                        true => "No active virtual keys",
                    })
                    .font(iced::Font::MONOSPACE)
                    .shaping(Shaping::Advanced),
                ]),

                ZippyPane(z) => container(column![
                    text("Zippychord State:")
                        .size(18)
                        .line_height(LineHeight::Absolute(32f32.into())),
                    text(z)
                        .font(iced::Font::MONOSPACE)
                        .shaping(Shaping::Advanced),
                ]),
            })
        }).into()
    }

    pub(crate) fn update(&mut self, msg: Message) {
        match msg {
            Message::ServerMessage(smsg) => match smsg {
                ServerMessage::DetailedInfo(info) => {
                    log::debug!("got info!");
                    for pane in self.panes.panes.values_mut() {
                        match pane {
                            LayerPane(l) => l.replace_range(.., &info.layer_config),
                            VkeysPane(v) => v.replace_range(.., &info.active_vkey_names),
                            ZippyPane(z) => z.replace_range(.., &info.zippychord_state),
                        }
                    };
                }
                ServerMessage::LayerChange { .. }
                | ServerMessage::LayerNames { .. }
                | ServerMessage::CurrentLayerInfo { .. }
                | ServerMessage::ConfigFileReload { .. }
                | ServerMessage::CurrentLayerName { .. }
                | ServerMessage::MessagePush { .. }
                | ServerMessage::Error { .. } => {},
            },
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
