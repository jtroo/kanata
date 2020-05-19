use std::env;
use std::path::Path;

mod kbd_in;
mod kbd_out;
mod ktrl;
mod layers;

use kbd_in::KbdIn;
use kbd_out::KbdOut;
use ktrl::Ktrl;

fn main() -> Result<(), std::io::Error> {
    let kbd_path_str = env::args().nth(1).expect("Missing keyboard path");

    let kbd_path = Path::new(&kbd_path_str);
    let kbd_in = KbdIn::new(kbd_path)?;

    let kbd_out = KbdOut::new()?;
    println!("ktrl: Setup Complete");

    let mut ktrl = Ktrl::new(kbd_in, kbd_out);
    ktrl.event_loop()?;

    Ok(())
}
