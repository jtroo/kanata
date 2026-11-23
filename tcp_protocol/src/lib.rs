//! Kanata TCP Protocol
//!
//! This crate defines the JSON message format for communication between
//! TCP clients and the Kanata keyboard remapping daemon.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const LAYER_CHANGE: &str = "LayerChange";
pub const HOLD_ACTIVATED: &str = "HoldActivated";
pub const TAP_ACTIVATED: &str = "TapActivated";
pub const MESSAGE_PUSH: &str = "MessagePush";
pub const CONFIG_FILE_RELOAD: &str = "ConfigFileReload";
pub const SEQ_STATE: &str = "SeqState";
pub const VALID_SUBSCRIBE_OPTIONS: [&str; 6] = [
    LAYER_CHANGE,
    HOLD_ACTIVATED,
    TAP_ACTIVATED,
    MESSAGE_PUSH,
    CONFIG_FILE_RELOAD,
    SEQ_STATE,
];

/// What happened to a leader sequence (`sldr`/`defseq`) to cause a `SeqState` message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeqPhase {
    /// A sequence leader action armed (or re-armed) sequence mode.
    Started,
    /// A key was added to the sequence and more keys are needed to finish it.
    Advanced,
    /// The keys typed completed a `defseq` sequence.
    Completed,
    /// Sequence mode ended without a match: invalid key or a cancel action.
    Cancelled,
    /// Sequence mode ended because `sequence-timeout` ran out.
    TimedOut,
}

/// Messages sent from the server to connected clients.
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    LayerChange {
        new: String,
    },
    LayerNames {
        names: Vec<String>,
    },
    FakeKeyNames {
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
    /// Response to Reload commands when `wait: true` was specified.
    /// Introduced in protocol v1.11.
    ReloadResult {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    /// Sent when a tap-hold key transitions to hold state.
    /// The `key` field is the physical key name (e.g., `"caps"`, `"a"`).
    HoldActivated {
        key: String,
    },
    /// Sent when a tap-hold key triggers its tap action.
    /// The `key` field is the physical key name (e.g., `"caps"`, `"a"`).
    TapActivated {
        key: String,
    },
    /// Sent whenever leader sequence mode starts, advances or ends.
    /// `prefix` is the keys typed so far, space separated, e.g. `"g r"`.
    /// Keys typed while holding modifiers are written with prefixes, e.g. `"S-a"`.
    /// `candidates` lists the configured sequences that `prefix` can still complete
    /// (empty unless `phase` is `Started` or `Advanced`), in the same notation.
    SeqState {
        prefix: String,
        candidates: Vec<String>,
        phase: SeqPhase,
    },
}

/// Returns true if the subscribe option logs user typing over TCP
/// and returns false otherwise.
pub fn subscribe_option_is_keylogging(opt: &str) -> bool {
    matches!(opt, TAP_ACTIVATED | HOLD_ACTIVATED | SEQ_STATE)
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
    Subscribe(Vec<String>),
    ChangeLayer {
        new: String,
    },
    RequestLayerNames {},
    RequestFakeKeyNames {},
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

    #[test]
    fn test_request_fake_key_names() {
        let json = r#"{"RequestFakeKeyNames":{}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::RequestFakeKeyNames {}));
    }

    #[test]
    fn test_fake_key_names_response() {
        let msg = ServerMessage::FakeKeyNames {
            names: vec!["email-sig".to_string(), "nav-mode".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"FakeKeyNames":{"names":["email-sig","nav-mode"]}}"#
        );

        // Round-trip
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ServerMessage::FakeKeyNames { names } => {
                assert_eq!(names, vec!["email-sig", "nav-mode"]);
            }
            _ => panic!("Expected FakeKeyNames"),
        }
    }

    #[test]
    fn test_fake_key_names_empty() {
        let msg = ServerMessage::FakeKeyNames { names: vec![] };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"FakeKeyNames":{"names":[]}}"#);
    }

    #[test]
    fn test_hold_activated_json_format() {
        let msg = ServerMessage::HoldActivated {
            key: "caps".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"HoldActivated":{"key":"caps"}}"#);
    }

    #[test]
    fn test_tap_activated_json_format() {
        let msg = ServerMessage::TapActivated {
            key: "a".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"TapActivated":{"key":"a"}}"#);
    }

    #[test]
    fn test_seq_state_json_format() {
        let msg = ServerMessage::SeqState {
            prefix: "g".to_string(),
            candidates: vec!["g r".to_string(), "g t".to_string()],
            phase: SeqPhase::Advanced,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"SeqState":{"prefix":"g","candidates":["g r","g t"],"phase":"Advanced"}}"#
        );
        match serde_json::from_str(&json).unwrap() {
            ServerMessage::SeqState {
                prefix,
                candidates,
                phase,
            } => {
                assert_eq!(prefix, "g");
                assert_eq!(candidates, ["g r", "g t"]);
                assert_eq!(phase, SeqPhase::Advanced);
            }
            _ => panic!("Expected SeqState"),
        }
    }

    #[test]
    fn test_seq_state_is_a_valid_keylogging_option() {
        assert!(VALID_SUBSCRIBE_OPTIONS.contains(&SEQ_STATE));
        assert!(subscribe_option_is_keylogging(SEQ_STATE));
        assert!(!subscribe_option_is_keylogging(LAYER_CHANGE));
    }

    #[test]
    fn test_subscribe_json_format() {
        let msg =
            ClientMessage::Subscribe(vec![LAYER_CHANGE.to_string(), HOLD_ACTIVATED.to_string()]);
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"Subscribe":["LayerChange","HoldActivated"]}"#);
        match serde_json::from_str(&json).unwrap() {
            ClientMessage::Subscribe(options) => {
                assert_eq!(
                    options,
                    vec![LAYER_CHANGE.to_string(), HOLD_ACTIVATED.to_string()]
                );
            }
            _ => panic!("Expected Subscribe"),
        }
    }
}
