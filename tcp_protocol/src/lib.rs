use serde::{Deserialize, Serialize};
use std::str::FromStr;

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
    // UDP Authentication messages
    AuthResult {
        success: bool,
        session_id: Option<String>,
        expires_in_seconds: Option<u64>,
    },
    AuthRequired,
    SessionExpired,
    // Protocol expansion messages
    HelloOk {
        version: String,
        protocol: u8,
        capabilities: Vec<String>,
    },
    StatusInfo {
        engine_version: String,
        uptime_s: u64,
        ready: bool,
        last_reload: LastReloadInfo,
    },
    ReloadResult {
        ready: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    // New in PR2: validation and events
    ValidationResult {
        #[serde(default)]
        warnings: Vec<ValidationItem>,
        #[serde(default)]
        errors: Vec<ValidationItem>,
    },
    // Optional structured error detail that may follow a status Error line
    ErrorDetail {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        column: Option<u32>,
    },
    // Event messages for subscription
    Ready {
        at: String,
    },
    ConfigError {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        column: Option<u32>,
        at: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LastReloadInfo {
    pub ok: bool,
    /// Timestamp as epoch seconds (Unix timestamp). ISO8601/RFC3339 format can be added in future if needed.
    pub at: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    // UDP Authentication message
    Authenticate {
        token: String,
        client_name: Option<String>,
    },
    // Existing messages with optional session_id for UDP auth
    ChangeLayer {
        new: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    RequestLayerNames {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    RequestCurrentLayerInfo {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    RequestCurrentLayerName {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    ActOnFakeKey {
        name: String,
        action: FakeKeyActionMessage,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    SetMouse {
        x: u16,
        y: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    Reload {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadNext {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadPrev {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadNum {
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadFile {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    Hello {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    Status {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    // New in PR2
    Validate {
        config: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mode: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    Subscribe {
        events: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationItem {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
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
        // Test that our API contract matches expected JSON structure
        assert_eq!(
            serde_json::to_string(&ServerResponse::Ok).unwrap(),
            r#"{"status":"Ok"}"#
        );
        assert_eq!(
            serde_json::to_string(&ServerResponse::Error {
                msg: "test".to_string()
            })
            .unwrap(),
            r#"{"status":"Error","msg":"test"}"#
        );
    }

    #[test]
    fn test_as_bytes_includes_newline() {
        // Test our specific logic that adds newline termination
        let response = ServerResponse::Ok;
        let bytes = response.as_bytes();
        assert!(bytes.ends_with(b"\n"), "Response should end with newline");

        let error_response = ServerResponse::Error {
            msg: "test".to_string(),
        };
        let error_bytes = error_response.as_bytes();
        assert!(
            error_bytes.ends_with(b"\n"),
            "Error response should end with newline"
        );
    }

    #[test]
    fn test_hello_ok_json_format() {
        let msg = ServerMessage::HelloOk {
            version: "1.10.x".to_string(),
            protocol: 1,
            capabilities: vec!["reload".into(), "status".into(), "ready".into()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("HelloOk"));
        assert!(json.contains("\"protocol\":1"));
        assert!(json.contains("\"capabilities\""));
    }

    #[test]
    fn test_status_info_json_format() {
        let msg = ServerMessage::StatusInfo {
            engine_version: "1.10.x".into(),
            uptime_s: 12,
            ready: true,
            last_reload: LastReloadInfo {
                ok: true,
                at: "1730619223".into(),
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("StatusInfo"));
        assert!(json.contains("\"uptime_s\":12"));
        assert!(json.contains("\"ready\":true"));
        assert!(json.contains("\"last_reload\""));
    }

    #[test]
    fn test_reload_result_json_format() {
        let ready_msg = ServerMessage::ReloadResult {
            ready: true,
            timeout_ms: None,
        };
        let not_ready_msg = ServerMessage::ReloadResult {
            ready: false,
            timeout_ms: Some(2000),
        };
        let ready_json = serde_json::to_string(&ready_msg).unwrap();
        let not_ready_json = serde_json::to_string(&not_ready_msg).unwrap();
        assert!(ready_json.contains("ReloadResult"));
        assert!(ready_json.contains("\"ready\":true"));
        assert!(not_ready_json.contains("\"ready\":false"));
        assert!(not_ready_json.contains("\"timeout_ms\":2000"));
    }

    #[test]
    fn test_validation_result_json_format() {
        let msg = ServerMessage::ValidationResult {
            warnings: vec![ValidationItem {
                message: "w".into(),
                line: Some(1),
                column: None,
                code: None,
            }],
            errors: vec![],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("ValidationResult"));
        assert!(json.contains("warnings"));
    }
}
