use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::{Kanata, TcpServer};

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
