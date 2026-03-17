use crate::oskbd::{KeyEvent, KeyValue};
use crate::{Kanata, ValidatedArgs, str_to_oscode};
use std::path::PathBuf;
use std::sync::mpsc;

fn passthru_args() -> ValidatedArgs {
    ValidatedArgs {
        paths: vec![PathBuf::from("./cfg_samples/minimal.kbd")],
        #[cfg(feature = "tcp_server")]
        tcp_server_address: None,
        nodelay: true,
    }
}

#[test]
fn passthru_runtime_output_channel_is_ready_and_emits_events() {
    let args = passthru_args();
    let (tx, rx) = mpsc::channel();
    let runtime = Kanata::new_with_output_channel(&args, Some(tx)).expect("passthru runtime");

    {
        let mut runtime = runtime.lock();
        assert!(runtime.kbd_out.output_ready());

        let key = str_to_oscode("a").expect("key code");
        runtime
            .kbd_out
            .write_key(key, KeyValue::Press)
            .expect("write key through passthru output");
    }

    let event = rx.try_recv().expect("passthru output event");
    let key_event = KeyEvent::try_from(event).expect("output event should decode");
    assert_eq!(key_event.code, str_to_oscode("a").expect("key code"));
    assert_eq!(key_event.value, KeyValue::Press);
}
