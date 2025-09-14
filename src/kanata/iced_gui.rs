use crate::kanata::Kanata;

use std::sync::Arc;

use iced::widget::{Column, column, text};
use parking_lot::Mutex;

pub(crate) struct KanataGuiState {
    gui_update_tx: Option<smol::channel::Sender<Message>>,
    ticks_since_last_update: u16,
}
impl KanataGuiState {
    pub(crate) fn new() -> Self {
        Self {
            gui_update_tx: None,
            ticks_since_last_update: 0,
        }
    }
}

pub(crate) struct KanataGui {
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


use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

#[derive(Default, Debug)]
struct App {
    window: Option<Box<Window>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {}
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        println!("{event:?}");
        match event {
            WindowEvent::CloseRequested => {
                println!("Close was requested; stopping");
                event_loop.exit();
            },
            WindowEvent::RedrawRequested => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                let window = self.window.as_ref().expect("redraw request without a window");

                // Notify that you're about to draw.
                window.pre_present_notify();

                // For contiguous redraw loop you can request a redraw from here.
                // window.request_redraw();
            },
            _ => (),
        }
    }
}


impl KanataGui {
    pub(crate) fn start(k: Arc<Mutex<Kanata>>) -> iced::Result {
        let (tx, rx) = smol::channel::bounded::<Message>(10);
        k.lock().iced_gui_state.gui_update_tx = Some(tx);
        use winit::*;
        log::info!("running iced app");
            let event_loop = event_loop::EventLoop::new().unwrap();

    // For alternative loop run options see `pump_events` and `run_on_demand` examples.
        log::info!("actually winit not iced");
    event_loop.run_app(&mut App::default()).unwrap();

        /*
        iced::application("Kanata", Self::update, Self::view)
            .subscription(move |_| iced::Subscription::run_with_id(0u8, rx.clone()))
            .run_with(|| (Self::from_kanata(k), iced::Task::none()))
            */
        Ok(())
    }

    fn from_kanata(k: Arc<Mutex<Kanata>>) -> Self {
        let mut kg = Self {
            k,
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
        let klk = self.k.lock();
        let current_layer_index = klk.layout.b().current_layer();
        let layer_info = &klk.layer_info[current_layer_index];
        self.layer_name.clear();
        self.layer_name.push_str(&layer_info.name);
        self.layer_content.clear();
        self.layer_content.push_str(&layer_info.cfg_text);
        drop(klk);
    }
}

impl Kanata {
    pub(crate) fn refresh_iced_gui(&mut self) {
        let Some(ref tx) = self.iced_gui_state.gui_update_tx else {
            return;
        };
        self.iced_gui_state.ticks_since_last_update = 0;
        if let Err(e) = tx.try_send(Message::Update) {
            log::warn!("Failed to send to iced gui {e:?}. Aborting gui updates.");
            self.iced_gui_state.gui_update_tx = None;
        }
    }
    pub(crate) fn tick_iced_gui(&mut self, ticks: u16) {
        let Some(ref tx) = self.iced_gui_state.gui_update_tx else {
            return;
        };
        self.iced_gui_state.ticks_since_last_update += ticks;
        // refresh at 30Hz
        if self.iced_gui_state.ticks_since_last_update > 33 {
            if let Err(e) = tx.try_send(Message::Update) {
                log::warn!("Failed to send to iced gui {e:?}. Aborting gui updates.");
                self.iced_gui_state.gui_update_tx = None;
            }
            self.iced_gui_state.ticks_since_last_update = 0;
        }
    }
}
