use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender};
use std::thread::sleep;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use kanata_tcp_protocol::{
    ClientMessage, HOLD_ACTIVATED, LAYER_CHANGE, ServerMessage, TAP_ACTIVATED,
};

use crate::{Kanata, TcpServer};

fn notification_test_client() -> (
    TcpStream,
    BufReader<TcpStream>,
    SyncSender<ServerMessage>,
    Receiver<crate::oskbd::KeyEvent>,
) {
    notification_test_client_with_cfg(
        "(defcfg danger-tcp-allow-keylogging yes)
         (defsrc a)
         (deflayer base a)",
    )
}

fn notification_test_client_with_cfg(
    cfg: &str,
) -> (
    TcpStream,
    BufReader<TcpStream>,
    SyncSender<ServerMessage>,
    Receiver<crate::oskbd::KeyEvent>,
) {
    let k = Kanata::new_from_str(cfg, Default::default()).expect("failed to parse cfg");
    let address = TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("free port");
    let (wakeup_tx, wakeup_rx) = std::sync::mpsc::sync_channel(100);
    let mut server = TcpServer::new(address, wakeup_tx);
    server.start(Arc::new(Mutex::new(k)));

    let (notification_tx, notification_rx) = std::sync::mpsc::sync_channel(100);
    Kanata::start_notification_loop(notification_rx, server.connections.clone());

    let stream = TcpStream::connect(address).expect("server accepts");
    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut initial = String::new();
    reader.read_line(&mut initial).expect("initial message");
    assert!(
        initial.contains("LayerChange"),
        "unexpected message: {initial}"
    );
    (stream, reader, notification_tx, wakeup_rx)
}

fn sync_connection(stream: &mut TcpStream, reader: &mut BufReader<TcpStream>) {
    stream
        .write_all(
            &serde_json::to_vec(&ClientMessage::RequestCurrentLayerName {})
                .expect("serialize current layer request"),
        )
        .expect("send current layer request");
    stream
        .write_all(b"\n")
        .expect("terminate current layer request");

    let mut line = String::new();
    reader.read_line(&mut line).expect("current layer response");
    assert!(
        line.contains("CurrentLayerName"),
        "unexpected response: {line}"
    );
}

fn set_subscriptions(
    stream: &mut TcpStream,
    reader: &mut BufReader<TcpStream>,
    options: Vec<&str>,
) {
    let subscribe = ClientMessage::Subscribe(options.into_iter().map(str::to_owned).collect());
    stream
        .write_all(&serde_json::to_vec(&subscribe).expect("serialize subscribe"))
        .expect("send subscribe");
    stream.write_all(b"\n").expect("terminate subscribe");
    sync_connection(stream, reader);
}

fn assert_no_notification(reader: &mut BufReader<TcpStream>) {
    reader
        .get_ref()
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("set read timeout");
    let mut line = String::new();
    let err = reader
        .read_line(&mut line)
        .expect_err("unexpected notification");
    assert!(
        matches!(
            err.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ),
        "unexpected read error: {err}"
    );
    reader
        .get_ref()
        .set_read_timeout(None)
        .expect("clear read timeout");
}

#[test]
fn disconnected_clients_are_removed_from_connections() {
    let k = Kanata::new_from_str("(defsrc a)(deflayer base a)", Default::default())
        .expect("failed to parse cfg");
    let address = TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("free port");
    let (tx, _rx) = std::sync::mpsc::sync_channel(100);
    let mut server = TcpServer::new(address, tx);
    server.start(Arc::new(Mutex::new(k)));

    for _ in 0..20 {
        let stream = TcpStream::connect(address).expect("server accepts");
        // The server writes the initial LayerChange before registering the client.
        let mut line = String::new();
        BufReader::new(&stream)
            .read_line(&mut line)
            .expect("initial message");
        assert!(line.contains("LayerChange"), "unexpected message: {line}");
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    while !server.connections.lock().is_empty() {
        assert!(
            Instant::now() < deadline,
            "{} closed clients still registered",
            server.connections.lock().len()
        );
        sleep(Duration::from_millis(10));
    }
}

#[test]
fn tcp_notifications_default_to_layer_change_only() {
    let (mut stream, mut reader, notification_tx, _wakeup_rx) = notification_test_client();
    sync_connection(&mut stream, &mut reader);
    notification_tx
        .send(ServerMessage::LayerChange { new: "base".into() })
        .expect("send layer change");
    notification_tx
        .send(ServerMessage::HoldActivated { key: "a".into() })
        .expect("send hold activation");
    notification_tx
        .send(ServerMessage::TapActivated { key: "a".into() })
        .expect("send tap activation");

    let mut line = String::new();
    reader.read_line(&mut line).expect("layer notification");
    assert!(
        line.contains("LayerChange"),
        "unexpected notification: {line}"
    );
    assert_no_notification(&mut reader);
}

#[test]
fn tcp_notifications_can_be_disabled() {
    let (mut stream, mut reader, notification_tx, _wakeup_rx) = notification_test_client();
    set_subscriptions(&mut stream, &mut reader, vec![]);
    notification_tx
        .send(ServerMessage::LayerChange { new: "base".into() })
        .expect("send layer change");
    notification_tx
        .send(ServerMessage::HoldActivated { key: "a".into() })
        .expect("send hold activation");
    notification_tx
        .send(ServerMessage::TapActivated { key: "a".into() })
        .expect("send tap activation");

    assert_no_notification(&mut reader);
}

#[test]
fn tcp_notifications_can_all_be_enabled() {
    let (mut stream, mut reader, notification_tx, _wakeup_rx) = notification_test_client();
    set_subscriptions(
        &mut stream,
        &mut reader,
        vec![LAYER_CHANGE, HOLD_ACTIVATED, TAP_ACTIVATED],
    );
    notification_tx
        .send(ServerMessage::LayerChange { new: "base".into() })
        .expect("send layer change");
    notification_tx
        .send(ServerMessage::HoldActivated { key: "a".into() })
        .expect("send hold activation");
    notification_tx
        .send(ServerMessage::TapActivated { key: "a".into() })
        .expect("send tap activation");

    for notification in ["LayerChange", "HoldActivated", "TapActivated"] {
        let mut line = String::new();
        reader.read_line(&mut line).expect("notification");
        assert!(
            line.contains(notification),
            "expected {notification}, got {line}"
        );
    }
}

#[test]
fn tcp_notifications_denied_with_keylogging_disabled() {
    let (mut stream, mut reader, _notification_tx, _wakeup_rx) = notification_test_client_with_cfg(
        "(defsrc a)
          (deflayer base a)",
    );
    let keylogging_opts = &[HOLD_ACTIVATED, TAP_ACTIVATED];
    for opt in keylogging_opts {
        let subscribe = ClientMessage::Subscribe(vec![opt.to_string()]);
        stream
            .write_all(&serde_json::to_vec(&subscribe).expect("serialize subscribe"))
            .expect("send subscribe");
        stream.write_all(b"\n").expect("terminate subscribe");
        let mut line = String::new();
        reader.read_line(&mut line).expect("current layer response");
        assert!(
            line.contains(&format!(
                "Subscribed with options [\\\"{}\\\"] while danger-tcp-allow-keylogging is false",
                opt
            )),
            "unexpected response: {line}"
        );
        line.clear();
    }
}
