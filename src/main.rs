use std::env;
use std::path::Path;

mod setup;
mod process;

fn main() -> Result<(), std::io::Error> {
    let kbd_path_str = env::args().nth(1).expect("Missing keyboard path");
    let kbd_path = Path::new(&kbd_path_str);
    let (kbd_in, kbd_out) = setup::ktrl_setup(kbd_path)?;

    println!("ktrl: Setup Complete");
    process::ktrl_process(kbd_in, kbd_out)?;

    Ok(())
}
