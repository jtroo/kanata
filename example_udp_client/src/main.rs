use kanata_tcp_protocol::*;
use std::io::{self, Write};
use std::net::UdpSocket;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Kanata UDP Client Example");
    println!("========================");

    // Connect to local UDP server
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    let server_addr = "127.0.0.1:37001";

    println!("Connecting to UDP server at {}", server_addr);

    // Get auth token from user
    print!("Enter authentication token: ");
    io::stdout().flush()?;
    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let token = token.trim().to_string();

    // Authenticate
    let auth_msg = ClientMessage::Authenticate {
        token,
        client_name: Some("Example Client".to_string()),
    };

    let auth_data = serde_json::to_vec(&auth_msg)?;
    socket.send_to(&auth_data, server_addr)?;

    // Receive authentication response
    let mut buf = [0u8; 1024];
    let (size, _) = socket.recv_from(&mut buf)?;
    let response: ServerMessage = serde_json::from_slice(&buf[..size])?;

    let session_id = match response {
        ServerMessage::AuthResult {
            success,
            session_id,
            expires_in_seconds,
        } => {
            if success {
                println!("✅ Authentication successful!");
                if let Some(expires) = expires_in_seconds {
                    println!("   Session expires in {} seconds", expires);
                }
                session_id
            } else {
                println!("❌ Authentication failed!");
                return Ok(());
            }
        }
        ServerMessage::Error { msg } => {
            println!("❌ Authentication error: {}", msg);
            return Ok(());
        }
        _ => {
            println!("❌ Unexpected response during authentication");
            return Ok(());
        }
    };

    println!("\nAvailable commands:");
    println!("  layers    - Get layer names");
    println!("  current   - Get current layer name");
    println!("  info      - Get current layer info");
    println!("  change    - Change to a specific layer");
    println!("  reload    - Reload configuration");
    println!("  quit      - Exit");

    // Interactive command loop
    loop {
        print!("\n> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input == "quit" {
            break;
        }

        let message = match input {
            "layers" => ClientMessage::RequestLayerNames {
                session_id: session_id.clone(),
            },
            "current" => ClientMessage::RequestCurrentLayerName {
                session_id: session_id.clone(),
            },
            "info" => ClientMessage::RequestCurrentLayerInfo {
                session_id: session_id.clone(),
            },
            "reload" => ClientMessage::Reload {
                session_id: session_id.clone(),
                wait: None,
                timeout_ms: None,
            },
            input if input.starts_with("change ") => {
                let layer_name = input.strip_prefix("change ").unwrap().to_string();
                ClientMessage::ChangeLayer {
                    new: layer_name,
                    session_id: session_id.clone(),
                }
            }
            _ => {
                println!(
                    "Unknown command. Available: layers, current, info, change <name>, reload, quit"
                );
                continue;
            }
        };

        // Send message
        let data = serde_json::to_vec(&message)?;
        socket.send_to(&data, server_addr)?;

        // For commands that expect responses
        match &message {
            ClientMessage::RequestLayerNames { .. }
            | ClientMessage::RequestCurrentLayerName { .. }
            | ClientMessage::RequestCurrentLayerInfo { .. }
            | ClientMessage::Reload { .. } => {
                let mut buf = [0u8; 4096];
                match socket.recv_from(&mut buf) {
                    Ok((size, _)) => {
                        if let Ok(response) = serde_json::from_slice::<ServerMessage>(&buf[..size])
                        {
                            match response {
                                ServerMessage::LayerNames { names } => {
                                    println!("Available layers: {:?}", names);
                                }
                                ServerMessage::CurrentLayerName { name } => {
                                    println!("Current layer: {}", name);
                                }
                                ServerMessage::CurrentLayerInfo { name, cfg_text } => {
                                    println!("Current layer: {}", name);
                                    println!(
                                        "Config snippet: {}",
                                        cfg_text.chars().take(100).collect::<String>()
                                    );
                                }
                                ServerMessage::Error { msg } => {
                                    println!("❌ Error: {}", msg);
                                }
                                other => {
                                    println!("Response: {:?}", other);
                                }
                            }
                        } else if let Ok(response) =
                            serde_json::from_slice::<ServerResponse>(&buf[..size])
                        {
                            match response {
                                ServerResponse::Ok => {
                                    println!("✅ Command completed successfully");
                                }
                                ServerResponse::Error { msg } => {
                                    println!("❌ Error: {}", msg);
                                }
                            }
                        } else {
                            println!("❌ Failed to parse response");
                        }
                    }
                    Err(e) => {
                        println!("❌ No response received: {}", e);
                    }
                }
            }
            ClientMessage::ChangeLayer { .. } => {
                println!("✅ Layer change command sent");
            }
            _ => {}
        }
    }

    println!("Goodbye!");
    Ok(())
}
