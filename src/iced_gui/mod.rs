//! iced_gui code for the GUI --run-gui process option,
//! which is typically expected to be a child process.
//!
//! Connect to Kanata on its TCP port.
//! Subscribe to UI updates.
//! Handle TCP messages from Kanata main process to update the UI.

use iced::widget::{Column, column, text};

pub(crate) struct KanataGui {
    layer_name: String,
    layer_content: String,
    active_vkeys: String,
    chv2_state: String,
    zch_state: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Message {
    Update,
}

impl KanataGui {
    pub(crate) fn start(_addr: std::net::SocketAddr) -> iced::Result {
        iced::application("Kanata", Self::update, Self::view)
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
        kg.update(Message::Update);
        kg
    }

    pub(crate) fn view(&self) -> Column<'_, Message> {
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

    pub(crate) fn update(&mut self, _: Message) {}
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
