#[derive(Default)]
pub(crate) struct Counter {
    value: i64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Message {
    Increment,
    Decrement,
}

/// - layer
/// - zipchord state
/// - chordsv2 state
/// - active input vkeys
/// - active output keys
/// - live reloaded

use iced::widget::{Column, button, column, text};

impl Counter {
    pub(crate) fn view(&self) -> Column<Message> {
        column![
            text("Active Layer Name:"),
            // TODO: name
            text("Active Layer Content:"),
            // TODO: content
            text("Active VKeys:"),
            text("Zippychord State:"),
            text("ChordsV2 State:"),
            button("+").on_press(Message::Increment),
            text(self.value),
            button("-").on_press(Message::Decrement),
        ]
    }
}

impl Counter {
    pub(crate) fn update(&mut self, message: Message) {
        match message {
            Message::Increment => {
                self.value += 1;
            }
            Message::Decrement => {
                self.value -= 1;
            }
        }
    }
}
