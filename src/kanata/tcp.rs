use super::*;

impl Kanata {
    #[cfg(feature = "tcp_server")]
    pub fn start_notification_loop(
        rx: Receiver<ServerMessage>,
        clients: crate::tcp_server::Connections,
        #[cfg(feature = "iced_gui")]
        subscribed_to_detailed_info: crate::tcp_server::SubscribedToDetailedInfo,
    ) {
        use std::io::Write;
        info!("listening for event notifications to relay to connected clients");
        std::thread::spawn(move || {
            loop {
                match rx.recv() {
                    Err(_) => {
                        panic!("channel disconnected")
                    }
                    Ok(event) => {
                        let mut stale_clients = vec![];
                        let mut clients = clients.lock();
                        if matches!(event, ServerMessage::DetailedInfo { .. }) {
                            #[cfg(feature = "iced_gui")]
                            {
                                let notification = event.as_bytes();
                                let all_subscribed = subscribed_to_detailed_info.lock();
                                for subscribed in all_subscribed.iter() {
                                    match clients.get(subscribed.as_str()) {
                                        Some(mut c) => match c.write_all(&notification) {
                                            Ok(_) => {
                                                log::debug!(
                                                    "tcp detailed info sent to {subscribed}"
                                                );
                                            }
                                            Err(e) => {
                                                log::warn!(
                                                    "removing tcp client where write failed: {subscribed}, {e:?}"
                                                );
                                                // the client is no longer connected, let's remove them
                                                stale_clients.push(subscribed.as_str().to_owned());
                                            }
                                        },
                                        None => {
                                            // client was disconnected, do the unsubscribe.
                                            stale_clients.push(subscribed.as_str().to_owned());
                                        }
                                    }
                                }
                            }
                        } else {
                            let notification = event.as_bytes();
                            for (id, client) in &mut *clients {
                                match client.write_all(&notification) {
                                    Ok(_) => {
                                        log::debug!("tcp message sent to {id}");
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "removing tcp client where write failed: {id}, {e:?}"
                                        );
                                        // the client is no longer connected, let's remove them
                                        stale_clients.push(id.clone());
                                    }
                                }
                            }
                        }

                        for id in &stale_clients {
                            log::warn!("removing disconnected tcp client: {id}");
                            clients.remove(id);
                            subscribed_to_detailed_info.lock().remove(id);
                        }
                    }
                }
            }
        });
    }

    #[cfg(not(feature = "tcp_server"))]
    pub fn start_notification_loop(
        _rx: Receiver<ServerMessage>,
        _clients: crate::tcp_server::Connections,
    ) {
    }
}
