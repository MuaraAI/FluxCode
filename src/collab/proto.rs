//! The messages host and guests exchange (one JSON object per encrypted frame).

use super::ot::Op;
use serde::{Deserialize, Serialize};

/// Who made an edit with an AI agent: shown next to the edit on every screen.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AiInfo {
    /// The agent ("Claude Agent", "OpenCode").
    pub agent: String,
    /// The model as the agent names it ("anthropic/claude-sonnet", "GLM-5.3-Flash").
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub effort: String,
    #[serde(default)]
    pub mode: String,
}

impl AiInfo {
    /// "Claude Agent · claude-sonnet · high effort"
    pub fn line(&self) -> String {
        let mut parts = vec![self.agent.clone()];
        if !self.model.is_empty() {
            parts.push(self.model.rsplit('/').next().unwrap_or(&self.model).to_string());
        }
        if !self.effort.is_empty() && self.effort != "default" {
            parts.push(format!("{} effort", self.effort));
        }
        if !self.mode.is_empty() {
            parts.push(format!("{} mode", self.mode));
        }
        parts.join(" · ")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Peer {
    pub id: u64,
    /// GitHub login when signed in, else a name the person typed.
    pub name: String,
    #[serde(default)]
    pub github: bool,
    #[serde(default)]
    pub host: bool,
    /// The file (path relative to the shared folder) and line they are on.
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub line: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Msg {
    // ---- guest -> host
    Hello { name: String, github: bool, build: String },
    Open { path: String },
    Edit { path: String, rev: u64, op: Op, #[serde(default)] ai: Option<AiInfo> },
    Cursor { path: String, line: u32 },
    // ---- host -> guest
    Welcome { you: u64, host: String, folder: String, files: Vec<String>, peers: Vec<Peer> },
    Peers { peers: Vec<Peer> },
    Snapshot { path: String, rev: u64, text: String },
    /// Someone's edit, as the host applied it (`author` 0 is the host).
    Applied { path: String, rev: u64, op: Op, author: u64, #[serde(default)] ai: Option<AiInfo> },
    /// The host took your edit; it is now revision `rev`.
    Ack { path: String, rev: u64 },
    Refused { why: String },
    // ---- both
    Bye { why: String },
}

#[cfg(test)]
mod tests {
    use super::{AiInfo, Msg};
    use crate::collab::ot::Op;

    #[test]
    fn messages_survive_json() {
        let m = Msg::Edit { path: "src/main.rs".into(), rev: 3, op: Op::diff("ab", "aXb"), ai: Some(AiInfo { agent: "Codex".into(), model: "openai/gpt-x".into(), effort: "high".into(), mode: String::new() }) };
        let back: Msg = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn ai_line_names_agent_model_and_effort() {
        let a = AiInfo { agent: "Claude Agent".into(), model: "anthropic/claude-sonnet".into(), effort: "high".into(), mode: "build".into() };
        assert_eq!(a.line(), "Claude Agent · claude-sonnet · high effort · build mode");
        assert_eq!(AiInfo { agent: "goose".into(), effort: "default".into(), ..Default::default() }.line(), "goose");
    }
}
