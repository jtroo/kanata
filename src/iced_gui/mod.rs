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
    pub(crate) fn start() -> iced::Result {
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

    pub(crate) fn update(&mut self, _: Message) {
    }
}
