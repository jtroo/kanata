use evdev_rs::enums::EventType;
use log::{error, info};

use std::convert::TryFrom;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time;

use std::sync::Arc;
use std::sync::Mutex;

use crate::keys::KeyEvent;
use crate::KbdIn;
use crate::KbdOut;

#[cfg(feature = "sound")]
use crate::effects::Dj;

pub struct KtrlArgs {
    pub kbd_path: PathBuf,
    pub config_path: PathBuf,
}

pub struct Ktrl {
    pub kbd_in_path: PathBuf,
    pub kbd_out: KbdOut,
}

impl Ktrl {
    pub fn new(args: KtrlArgs) -> Result<Self, std::io::Error> {
        let kbd_out = match KbdOut::new() {
            Ok(kbd_out) => kbd_out,
            Err(err) => {
                error!("Failed to open the output uinput device. Make sure you've added ktrl to the `uinput` group");
                return Err(err);
            }
        };

        Ok(Self {
            kbd_in_path: args.kbd_path,
            kbd_out,
            #[cfg(feature = "sound")]
            dj,
        })
    }

    pub fn new_arc(args: KtrlArgs) -> Result<Arc<Mutex<Self>>, std::io::Error> {
        Ok(Arc::new(Mutex::new(Self::new(args)?)))
    }

    fn handle_key_event(&mut self, _event: &KeyEvent) -> Result<(), String> {
        todo!()
    }

    fn handle_time_tick(&mut self) {
        todo!()
    }

    pub fn start_processing_loop(ktrl: Arc<Mutex<Self>>, rx: Receiver<KeyEvent>) {
        info!("Ktrl: entering the processing loop");
        std::thread::spawn(move || {
            info!("Starting processing loop");
            if let Ok(kev) = rx.try_recv() {
                if let Err(e) = ktrl.lock().unwrap().handle_key_event(&kev) {
                    error!("Failed to process key event {:?}", e);
                }
            } else {
                ktrl.lock().unwrap().handle_time_tick();
                // Sleep every 7 ms; process at 144 Hz.
                //
                // Since this is not a keyboard FW, no need to check any faster - the OS sends the
                // events to the event loop and they get buffered in the mpsc channel.
                std::thread::sleep(time::Duration::from_millis(7));
            }
        });
    }

    pub fn event_loop(ktrl: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<(), std::io::Error> {
        info!("Ktrl: entering the event loop");

        let kbd_in_path = {
            let ktrl = ktrl.lock().expect("Failed to lock ktrl (poisoned)");
            ktrl.kbd_in_path.clone()
        };

        let kbd_in = match KbdIn::new(&kbd_in_path) {
            Ok(kbd_in) => kbd_in,
            Err(err) => {
                error!("Failed to open the input keyboard device. Make sure you've added ktrl to the `input` group");
                return Err(err);
            }
        };

        loop {
            let in_event = kbd_in.read()?;

            // Filter uninteresting events
            if in_event.event_type == EventType::EV_SYN || in_event.event_type == EventType::EV_MSC
            {
                continue;
            }

            // Pass-through non-key events
            let key_event = match KeyEvent::try_from(in_event.clone()) {
                Ok(ev) => ev,
                _ => {
                    let mut ktrl = ktrl.lock().unwrap();
                    ktrl.kbd_out.write(in_event)?;
                    continue;
                }
            };

            // Send key events to the processing loop
            if let Err(e) = tx.send(key_event) {
                error!("Could not send on ch: {:?}", e);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "failed to send on mpsc",
                ));
            }
        }
    }
}
