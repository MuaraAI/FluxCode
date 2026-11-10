//! Facts about a model for the model menu: how many tokens it reads at once (its
//! context window), how many it can write in one answer, and whether it is free.
//!
//! They come from the catalog OpenCode keeps on this PC (models.dev's list, cached at
//! `~/.cache/opencode/models.json`). Nothing is fetched: without the file the menu shows
//! the names alone. The file is several MB, so it is read in the background, once per
//! start, and only the models an agent offers are kept.

use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meta {
    /// Tokens the model reads at once.
    pub context: u64,
    /// Tokens it can write in one answer.
    pub output: u64,
    /// Input and output both cost nothing.
    pub free: bool,
}

/// What has been read so far, by `provider/model` id; None until a read finished.
static KEPT: Mutex<Option<HashMap<String, Meta>>> = Mutex::new(None);

/// OpenCode's copy of the catalog.
pub fn file() -> Option<PathBuf> {
    if let Some(x) = std::env::var_os("XDG_CACHE_HOME").filter(|x| !x.is_empty()) {
        return Some(PathBuf::from(x).join("opencode").join("models.json"));
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".cache").join("opencode").join("models.json"))
}

/// The facts for these ids, from the catalog's JSON (`{provider: {models: {id: …}}}`).
pub fn parse(json: &str, ids: &[String]) -> HashMap<String, Meta> {
    let Ok(v) = serde_json::from_str::<Value>(json) else { return HashMap::new() };
    let mut out = HashMap::new();
    for id in ids {
        let Some((provider, model)) = id.split_once('/') else { continue };
        let m = &v[provider]["models"][model];
        let num = |x: &Value| x.as_u64().or_else(|| x.as_f64().map(|f| f as u64)).unwrap_or(0);
        let context = num(&m["limit"]["context"]);
        if context == 0 {
            continue;
        }
        let cost = |k: &str| m["cost"][k].as_f64();
        let free = matches!((cost("input"), cost("output")), (Some(i), Some(o)) if i == 0.0 && o == 0.0);
        out.insert(id.clone(), Meta { context, output: num(&m["limit"]["output"]), free });
    }
    out
}

/// The facts for one id, once they have been read.
pub fn get(id: &str) -> Option<Meta> {
    KEPT.lock().ok()?.as_ref()?.get(id).copied()
}

/// The ids not looked up yet (all of them before the first read).
pub fn missing(ids: &[String]) -> Vec<String> {
    let Ok(g) = KEPT.lock() else { return Vec::new() };
    match g.as_ref() {
        None => ids.to_vec(),
        Some(_) => Vec::new(),
    }
}

/// Read the catalog for these ids (off the UI thread) and keep what it says.
pub fn load(ids: &[String]) {
    let found = file().and_then(|f| std::fs::read_to_string(f).ok()).map(|j| parse(&j, ids)).unwrap_or_default();
    if let Ok(mut g) = KEPT.lock() {
        g.get_or_insert_with(HashMap::new).extend(found);
    }
}

/// The catalog id of a Claude model named like "Opus 5.5" or "Claude Sonnet 4.6".
pub fn claude_id(family: &str, version: f32) -> String {
    let v = format!("{version}").replace('.', "-");
    format!("anthropic/claude-{}-{v}", family.to_lowercase())
}

/// "1M", "1.5M", "262K", "8K".
pub fn tokens(n: u64) -> String {
    if n >= 950_000 {
        let m = (n as f64 / 1_000_000.0 * 10.0).round() / 10.0;
        if m.fract() == 0.0 { format!("{}M", m as u64) } else { format!("{m}M") }
    } else if n >= 1000 {
        format!("{}K", (n as f64 / 1000.0).round() as u64)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{claude_id, parse, tokens};

    #[test]
    fn reads_limits_and_free_models() {
        let json = r#"{"opencode":{"models":{
            "glm-5.3-flash-free":{"limit":{"context":200000,"output":32000},"cost":{"input":0,"output":0}},
            "glm-5.3":{"limit":{"context":1000000,"output":131072},"cost":{"input":1.4,"output":4}}}},
            "anthropic":{"models":{"claude-opus-5-5":{"limit":{"context":1000000,"output":128000},"cost":{"input":4,"output":20}}}}}"#;
        let ids: Vec<String> = ["opencode/glm-5.3-flash-free", "opencode/glm-5.3", "opencode/nope", "anthropic/claude-opus-5-5"].map(String::from).to_vec();
        let m = parse(json, &ids);
        assert!(m["opencode/glm-5.3-flash-free"].free);
        assert_eq!(m["opencode/glm-5.3-flash-free"].context, 200_000);
        assert!(!m["opencode/glm-5.3"].free);
        assert!(!m.contains_key("opencode/nope"));
        assert_eq!(m[&claude_id("Opus", 5.5)].output, 128_000);
        assert_eq!(claude_id("Sonnet", 5.0), "anthropic/claude-sonnet-5");
    }

    #[test]
    fn token_counts_read_short() {
        assert_eq!(tokens(1_048_576), "1M");
        assert_eq!(tokens(1_000_000), "1M");
        assert_eq!(tokens(262_144), "262K");
        assert_eq!(tokens(131_072), "131K");
        assert_eq!(tokens(1_500_000), "1.5M");
    }
}
