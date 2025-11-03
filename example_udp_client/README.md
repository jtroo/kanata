# Kanata UDP Client Example

This is an example client that demonstrates how to communicate with Kanata's UDP server using authenticated sessions.

## Features Demonstrated

- **Authentication**: Secure token-based authentication with session management
- **Layer Management**: Get current layer info, list available layers, change layers
- **Configuration Reloading**: Trigger live configuration reloads
- **Interactive CLI**: User-friendly command-line interface

## Usage

1. Start Kanata with UDP server enabled:
   ```bash
   kanata --cfg your-config.kbd --udp-port 37001
   ```

2. Note the authentication token from Kanata's output:
   ```
   [INFO] UDP server started on 127.0.0.1:37001  
   [INFO] UDP auth token: a8f3d2e1b4c7f9a2 (save this for clients)
   ```

3. Run the UDP client:
   ```bash
   cargo run -p kanata_example_udp_client
   ```

4. Enter the authentication token when prompted

5. Use interactive commands:
   - `layers` - List all available layers
   - `current` - Show current active layer name  
   - `info` - Show current layer info with config snippet
   - `change <layer_name>` - Switch to specified layer
   - `reload` - Reload kanata configuration
   - `quit` - Exit the client

## Authentication Flow

1. **Connect**: Client binds to a UDP socket and connects to Kanata server
2. **Authenticate**: Client sends `Authenticate` message with token
3. **Session**: Server responds with session ID and expiration time
4. **Commands**: Client includes session ID in subsequent requests
5. **Auto-expire**: Sessions automatically expire after timeout (default 30 minutes)

## Example Session

```
Kanata UDP Client Example
========================
Connecting to UDP server at 127.0.0.1:37001
Enter authentication token: a8f3d2e1b4c7f9a2
✅ Authentication successful!
   Session expires in 1800 seconds

Available commands:
  layers    - Get layer names
  current   - Get current layer name
  info      - Get current layer info  
  change    - Change to a specific layer
  reload    - Reload configuration
  quit      - Exit

> layers
Available layers: ["base", "numbers", "symbols"]

> current  
Current layer: base

> change numbers
✅ Layer change command sent

> current
Current layer: numbers

> quit
Goodbye!
```

## Protocol Details

All messages use JSON serialization. The client sends `ClientMessage` variants and receives `ServerMessage` or `ServerResponse` variants.

### Authentication Messages
- `ClientMessage::Authenticate { token, client_name }`
- `ServerMessage::AuthResult { success, session_id, expires_in_seconds }`

### Layer Management Messages
- `ClientMessage::RequestLayerNames { session_id }`
- `ClientMessage::RequestCurrentLayerName { session_id }`  
- `ClientMessage::ChangeLayer { new, session_id }`

### Configuration Messages
- `ClientMessage::Reload { session_id }`
- `ServerResponse::Ok` or `ServerResponse::Error { msg }`

## Security Notes

- Authentication tokens are randomly generated on each Kanata startup
- Sessions have configurable timeout (default 30 minutes)
- Server binds to localhost only by default for security
- Use `--udp-no-auth` flag only for testing on trusted networks

## Building

```bash
# Build just the UDP client
cargo build -p kanata_example_udp_client

# Build with the main Kanata workspace  
cargo build
```