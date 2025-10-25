//! iced_gui code for the GUI --run-gui process option,
//! which is typically expected to be a child process.
//!
//! Connect to Kanata on its TCP port.
//! Subscribe to UI updates.
//! Handle TCP messages from Kanata main process to update the UI.

use async_net::TcpStream;
use futures::io::BufReader;
use futures::prelude::*;
use iced::widget::pane_grid::PaneGrid;
use iced::widget::{button, column, container, pane_grid, responsive, row, scrollable, text};
use iced::{Element, Fill};

use kanata_tcp_protocol::*;

#[derive(Clone, Debug)]
pub(crate) enum Message {
    ServerMessage(ServerMessage),
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    Maximize(pane_grid::Pane),
    Restore,
    Close(pane_grid::Pane),
}

pub(crate) struct KanataGui {
    panes: pane_grid::State<Pane>,
}

#[derive(Debug, Clone)]
enum PaneContent {
    Layer(String),
    Vkeys(String),
    Zippy(String),
}
use PaneContent::*;

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
                let (mut panes, first_pane) = pane_grid::State::new(Pane::new(
                    "Active Layer Configuration",
                    Layer("".into()),
                ));
                let _ = panes.split(
                    pane_grid::Axis::Vertical,
                    first_pane,
                    Pane::new("Zippychord", Zippy("".into())),
                );
                let _ = panes.split(
                    pane_grid::Axis::Horizontal,
                    first_pane,
                    Pane::new("Virtual Keys Active", Vkeys("".into())),
                );
                (Self { panes }, iced::Task::none())
            })
    }

    pub(crate) fn view(&self) -> Element<'_, Message> {
        let total_panes = self.panes.len();

        let pane_grid = PaneGrid::new(&self.panes, |id, pane, is_maximized| {
            let title = row![text(&pane.name),].spacing(5);

            let title_bar = pane_grid::TitleBar::new(title)
                .controls(pane_grid::Controls::dynamic(
                    view_controls(id, total_panes, is_maximized),
                    button(text("X").size(14))
                        .style(button::danger)
                        .padding(3)
                        .on_press(Message::Close(id)),
                ))
                .padding(10)
                .style(style::title_bar_active);

            pane_grid::Content::new(responsive(move |_size| view_content(&pane.pane_content)))
                .title_bar(title_bar)
                .style(style::pane_active)
        })
        .width(Fill)
        .height(Fill)
        .spacing(10)
        .on_drag(Message::Dragged)
        .on_resize(10, Message::Resized);

        container(pane_grid)
            .width(Fill)
            .height(Fill)
            .padding(10)
            .into()
    }

    pub(crate) fn update(&mut self, msg: Message) {
        match msg {
            Message::ServerMessage(smsg) => match smsg {
                ServerMessage::DetailedInfo(info) => {
                    log::debug!("got info!");
                    for pane in self.panes.panes.values_mut() {
                        match &mut pane.pane_content {
                            Layer(l) => l.replace_range(.., &info.layer_config),
                            Vkeys(v) => v.replace_range(.., &info.active_vkey_names),
                            Zippy(z) => z.replace_range(.., &info.zippychord_state),
                        }
                    }
                }
                ServerMessage::LayerChange { .. }
                | ServerMessage::LayerNames { .. }
                | ServerMessage::CurrentLayerInfo { .. }
                | ServerMessage::ConfigFileReload { .. }
                | ServerMessage::CurrentLayerName { .. }
                | ServerMessage::MessagePush { .. }
                | ServerMessage::Error { .. } => {}
            },
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
            }
            Message::Dragged(pane_grid::DragEvent::Dropped { pane, target }) => {
                self.panes.drop(pane, target);
            }
            Message::Dragged(_) => {}
            Message::Maximize(pane) => self.panes.maximize(pane),
            Message::Restore => {
                self.panes.restore();
            }
            Message::Close(pane) => {
                self.panes.close(pane);
            }
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

#[derive(Clone)]
struct Pane {
    name: String,
    pane_content: PaneContent,
}

impl Pane {
    fn new(name: &str, pane_content: PaneContent) -> Self {
        Self {
            name: name.into(),
            pane_content,
        }
    }
}

fn view_content<'a>(pane_content: &'a PaneContent) -> Element<'a, Message> {
    let content = match pane_content {
        Layer(l) => column![
            text(l)
                .font(iced::Font::MONOSPACE)
                .shaping(text::Shaping::Advanced),
        ],

        Vkeys(v) => column![
            text(match v.is_empty() {
                false => v,
                true => "No active virtual keys",
            })
            .font(iced::Font::MONOSPACE)
            .shaping(text::Shaping::Advanced),
        ],

        Zippy(z) => column![
            text(z)
                .font(iced::Font::MONOSPACE)
                .shaping(text::Shaping::Advanced),
        ],
    };

    container(scrollable(content)).padding(5).into()
}

fn view_controls<'a>(
    pane: pane_grid::Pane,
    total_panes: usize,
    is_maximized: bool,
) -> Element<'a, Message> {
    let row = row![].spacing(5).push_maybe(if total_panes > 1 {
        let (content, message) = if is_maximized {
            ("Restore", Message::Restore)
        } else {
            ("Maximize", Message::Maximize(pane))
        };

        Some(
            button(text(content).size(14))
                .style(button::secondary)
                .padding(3)
                .on_press(message),
        )
    } else {
        None
    });

    let close = button(text("Close").size(14))
        .style(button::danger)
        .padding(3)
        .on_press(Message::Close(pane));

    row.push(close).into()
}

mod style {
    use iced::widget::container;
    use iced::{Border, Theme};

    pub fn title_bar_active(theme: &Theme) -> container::Style {
        let palette = theme.extended_palette();

        container::Style {
            background: Some(palette.background.weak.color.into()),
            ..Default::default()
        }
    }

    pub fn pane_active(theme: &Theme) -> container::Style {
        let palette = theme.extended_palette();

        container::Style {
            border: Border {
                width: 2.0,
                color: palette.background.strong.color,
                ..Border::default()
            },
            ..Default::default()
        }
    }
}
