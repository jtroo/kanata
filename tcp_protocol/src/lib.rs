use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    LayerChange { new: String },
    LayerNames { names: Vec<String> },
    Error { msg: String },
}

impl ServerMessage {
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut msg = serde_json::to_vec(self).expect("ServerMessage should serialize");
        msg.push(b'\n');
        msg
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    ChangeLayer {
        new: String,
    },
    RequestLayerNames {},
    ActOnFakeKey {
        name: String,
        action: FakeKeyActionMessage,
    },
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

impl From<&str> for FakeKeyActionMessage {
    fn from(s: &str) -> FakeKeyActionMessage {
        match s {
            "Press" => FakeKeyActionMessage::Press,
            "Release" => FakeKeyActionMessage::Release,
            "Tap" => FakeKeyActionMessage::Tap,
            "Toggle" => FakeKeyActionMessage::Toggle,
            _ => FakeKeyActionMessage::Press,   // What's the best practice here?
        }
    }
}
