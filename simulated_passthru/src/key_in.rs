use kanata_state_machine::oskbd::*;
use log::*;

use winapi::ctypes::*;
use winapi::shared::minwindef::*;

use crate::oskbd::HOOK_CB;

/// Exported function: receives key input and uses event_loop's input event handler
/// callback (which will in turn communicate via the internal kanata's channels to
/// keyberon state machine etc.)
#[no_mangle]
pub extern "win64" fn input_ev_listener(vk: c_uint, sc: c_uint, up: c_int) -> LRESULT {
    #[cfg(feature = "perf_logging")]
    let start = std::time::Instant::now();
    let key_event = InputEvent::from_vk_sc(vk, sc, up); //{code:KEY_0,value:Press}
    let mut h_cbl = HOOK_CB.lock(); // to access the closure we move its box out of the mutex
    // and put it back after it returned
    if let Some(mut fnhook) = h_cbl.take() {
        // move our opt+boxed closure, replacing it with None, can't just .unwrap since Copy
        // trait not implemented for dyn fnMut
        let handled = fnhook(key_event); // box(closure)() = closure()
        *h_cbl = Some(fnhook); // put our closure back
        if handled {
            // now try to get the out key events that another thread should've sent via
            #[cfg(feature = "perf_logging")]
            debug!(
                " 🕐{}μs   →→→✓ {key_event} from {vk} sc={sc} up={up}",
                (start.elapsed()).as_micros()
            );
            #[cfg(not(feature = "perf_logging"))]
            debug!("   →→→✓ {key_event} from {vk} sc={sc} up={up}");
            1
        } else {
            0
        }
    } else {
        error!(
            "fnHook processing key events isn't available yet {key_event} from {vk} sc={sc} up={up}"
        );
        0
    }
}
