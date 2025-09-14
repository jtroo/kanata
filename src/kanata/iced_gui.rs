use crate::kanata::Kanata;
use parking_lot::Mutex;
use std::sync::Arc;

pub(crate) struct KanataGuiState {
    k: Arc<Mutex<Kanata>>,
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

/// - layer
/// - zipchord state
/// - chordsv2 state
/// - active input vkeys
/// - live reloaded
use iced::widget::{Column, column, text};

impl KanataGuiState {
    pub(crate) fn start(k: Arc<Mutex<Kanata>>) -> iced::Result {
        let (_tx, rx) = smol::channel::bounded::<Message>(10);
        iced::application("Kanata", Self::update, Self::view)
            .subscription(move |_| iced::Subscription::run_with_id(0u8, rx.clone()))
            .run_with(|| (Self::from_kanata(k), iced::Task::none()))
    }

    fn from_kanata(k: Arc<Mutex<Kanata>>) -> Self {
        Self {
            k,
            layer_name: todo!(),
            layer_content: todo!(),
            active_vkeys: todo!(),
            chv2_state: todo!(),
            zch_state: todo!(),
        }
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

    pub(crate) fn update(&mut self, message: Message) {
        use Message::*;
        match message {
            Update => todo!(),
        }
    }
}
