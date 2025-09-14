#[derive(Default)]
pub(crate) struct Counter {
    value: i64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Message {
    Increment,
    Decrement,
}

use iced::widget::{button, column, text, Column};

impl Counter {
    pub(crate) fn view(&self) -> Column<Message> {
        column![
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
