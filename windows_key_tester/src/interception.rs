use anyhow::Result;
use interception as ic;

pub fn start() -> Result<()> {
    let intrcptn = ic::Interception::new().expect("interception driver should init: have you completed the interception driver installation?");
    intrcptn.set_filter(ic::is_keyboard, ic::Filter::KeyFilter(ic::KeyFilter::all()));
    let mut strokes = [ic::Stroke::Keyboard {
        code: ic::ScanCode::Esc,
        state: ic::KeyState::empty(),
        information: 0,
    }; 32];

    log::info!("interception attached, you can type now");
    loop {
        let dev = intrcptn.wait_with_timeout(std::time::Duration::from_millis(1));
        if dev > 0 {
            let num_strokes = intrcptn.receive(dev, &mut strokes);
            let num_strokes = num_strokes as usize;

            for i in 0..num_strokes {
                log::info!("got stroke {:?}", strokes[i]);
                intrcptn.send(dev, &strokes[i..i + 1]);
            }
        }
    }
}
