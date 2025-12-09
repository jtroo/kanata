//! Kanata TCP Protocol
//!
//! This crate defines the JSON message format for communication between
//! TCP clients and the Kanata keyboard remapping daemon.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Messages sent from the server to connected clients.
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    LayerChange {
        new: String,
    },
    LayerNames {
        names: Vec<String>,
    },
    CurrentLayerInfo {
        name: String,
        cfg_text: String,
    },
    ConfigFileReload {
        new: String,
    },
    CurrentLayerName {
        name: String,
    },
    MessagePush {
        message: serde_json::Value,
    },
    Error {
        msg: String,
    },
    /// Response to `Hello` command with server capabilities.
    /// Introduced in protocol v1.11.
    HelloOk {
        version: String,
        protocol: u8,
        capabilities: Vec<String>,
    },
    /// Basic status info response.
    /// Introduced in protocol v1.11.
    StatusInfo {
        ready: bool,
    },
    /// Response to Reload commands when `wait: true` was specified.
    /// Introduced in protocol v1.11.
    ReloadResult {
        ready: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "status")]
pub enum ServerResponse {
    Ok,
    Error { msg: String },
}

impl ServerResponse {
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut msg = serde_json::to_vec(self).expect("ServerResponse should serialize");
        msg.push(b'\n');
        msg
    }
}

impl ServerMessage {
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut msg = serde_json::to_vec(self).expect("ServerMessage should serialize");
        msg.push(b'\n');
        msg
    }
}

/// Messages sent from clients to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    ChangeLayer {
        new: String,
    },
    RequestLayerNames {},
    RequestCurrentLayerInfo {},
    RequestCurrentLayerName {},
    ActOnFakeKey {
        name: String,
        action: FakeKeyActionMessage,
    },
    SetMouse {
        x: u16,
        y: u16,
    },

    /// Reload the current configuration file.
    Reload {
        /// If true, block until reload completes or times out.
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        /// Maximum time to wait for reload (milliseconds). Default: 5000.
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadNext {
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadPrev {
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadNum {
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadFile {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },

    /// Request server capabilities and version.
    /// Introduced in protocol v1.11.
    Hello {},
    /// Request basic status info (ready flag).
    /// Introduced in protocol v1.11.
    StatusInfo {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum FakeKeyActionMessage {
    Press,
    Release,
    Tap,
    Toggle,
}

impl FromStr for ClientMessage {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_response_json_format() {
        assert_eq!(
            serde_json::to_string(&ServerResponse::Ok).unwrap(),
            r#"{"status":"Ok"}"#
        );
    }

    #[test]
    fn test_as_bytes_includes_newline() {
        let response = ServerResponse::Ok;
        assert!(response.as_bytes().ends_with(b"\n"));
    }

    #[test]
    fn test_hello_ok_json_format() {
        let msg = ServerMessage::HelloOk {
            version: "1.10.0".to_string(),
            protocol: 1,
            capabilities: vec!["reload".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("HelloOk"));
        assert!(json.contains("\"version\":\"1.10.0\""));
    }

    #[test]
    fn test_reload_with_wait() {
        let msg = ClientMessage::Reload {
            wait: Some(true),
            timeout_ms: Some(5000),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("wait\":true"));
        assert!(json.contains("timeout_ms\":5000"));
    }

    #[test]
    fn test_reload_minimal() {
        // Backward compatible: no optional fields
        let json = r#"{"Reload":{}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Reload { wait, timeout_ms } => {
                assert!(wait.is_none());
                assert!(timeout_ms.is_none());
            }
            _ => panic!("Expected Reload"),
        }
    }

    #[test]
    fn test_existing_commands_unchanged() {
        // Verify existing commands still parse without any new fields
        let json = r#"{"ChangeLayer":{"new":"nav"}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::ChangeLayer { new } if new == "nav"));

        let json = r#"{"RequestLayerNames":{}}"#;
        let _msg: ClientMessage = serde_json::from_str(json).unwrap();

        let json = r#"{"ActOnFakeKey":{"name":"test","action":"Tap"}}"#;
        let _msg: ClientMessage = serde_json::from_str(json).unwrap();
    }
}
