//! Agent memory: a native port of AoC Memory
//! (github.com/MyKineID/aoc-memory-for-ai-agent, `ant_memory.py`).
//!
//! Memories behave like ant trails. Each has a strength `tau` that evaporates
//! with idle time (`tau *= 0.99^idle_days`), grows when it proves useful
//! (`+0.1`), shrinks when it misleads (`*0.9`), and is archived below 0.05.
//! Recall ranks by `P = tau * eta^2`, where `eta` is the token overlap with the
//! query scaled by the trail's success record. No embeddings, no database.
//!
//! The store is the same file as the Python tool (`~/.ant-memory/trails.json`,
//! or `ANT_MEMORY_DIR`), with the same fields and ids, so both can share it.
//! Unknown fields written by other tools are kept.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

const RHO: f64 = 0.01;
const K: f64 = 0.1;
const TAU_MIN: f64 = 0.05;
const STOP: &[&str] = &[
    "yang", "dan", "di", "ke", "dari", "untuk", "pada", "dengan", "adalah", "itu", "ini", "the", "a", "an", "of", "to",
    "in", "on", "for", "is", "are", "be", "as", "at", "by", "or", "if", "with", "this", "that", "not", "no",
];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Trail {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub tau: f64,
    #[serde(default)]
    pub success: u32,
    #[serde(default)]
    pub fail: u32,
    pub created: String,
    pub last_access: String,
    /// Fields other tools add (superseded_by, ...), written back unchanged.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

pub fn dir() -> PathBuf {
    if let Some(d) = std::env::var_os("ANT_MEMORY_DIR") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    home.join(".ant-memory")
}

fn now() -> DateTime<Utc> {
    Utc::now()
}

fn iso(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Micros, false)
}

pub fn tokens(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit()))
        .filter(|t| t.len() > 1 && !STOP.contains(t))
        .map(str::to_string)
        .collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    let union = a.union(b).count();
    if union == 0 { 0.0 } else { a.intersection(b).count() as f64 / union as f64 }
}

fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// Apply evaporation for the idle time since last access.
fn materialize(t: &mut Trail, touch: bool) {
    if let Ok(last) = DateTime::parse_from_rfc3339(&t.last_access) {
        let idle = (now() - last.with_timezone(&Utc)).num_milliseconds().max(0) as f64 / 86_400_000.0;
        t.tau = round6(t.tau * (1.0 - RHO).powf(idle));
    }
    if touch {
        t.last_access = iso(now());
    }
}

fn eta(t: &Trail, q: &HashSet<String>) -> f64 {
    let hay = tokens(&format!("{} {}", t.content, t.tags.join(" ")));
    let loc = if q.is_empty() { 0.0 } else { q.intersection(&hay).count() as f64 / q.len() as f64 };
    let (s, f) = (t.success as f64, t.fail as f64);
    let prior = if s + f == 0.0 { 0.5 } else { s / (s + f) };
    round6(loc * (0.5 + 0.5 * prior))
}

fn sha8(s: &str) -> String {
    sha1_smol::Sha1::from(s).digest().to_string()[..8].to_string()
}

/// The store on disk. Every call reads the file fresh, so the Python tool and
/// other agents writing the same store stay in sync.
pub struct Store {
    dir: PathBuf,
}

impl Default for Store {
    fn default() -> Self {
        Self { dir: dir() }
    }
}

impl Store {
    #[cfg(test)]
    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn trails_path(&self) -> PathBuf {
        self.dir.join("trails.json")
    }

    /// The trails, or an error when the file exists but can't be parsed (then
    /// nothing is written, so a store another tool is mid-writing is never lost).
    pub fn load(&self) -> Result<Vec<Trail>, String> {
        match std::fs::read_to_string(self.trails_path()) {
            Ok(s) => serde_json::from_str(s.trim_start_matches('\u{feff}')).map_err(|e| format!("memory store unreadable: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn save_file(&self, name: &str, data: &[Trail]) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let path = self.dir.join(name);
        let tmp = self.dir.join(format!("{name}.tmp"));
        let text = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    fn save(&self, trails: &[Trail]) -> Result<(), String> {
        self.save_file("trails.json", trails)
    }

    fn archive(&self, mut gone: Vec<Trail>, reason: &str) -> Result<(), String> {
        let path = self.dir.join("archive.json");
        let mut arch: Vec<Trail> = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        for t in &mut gone {
            t.extra.insert("archived_reason".into(), reason.into());
            t.extra.insert("archived_at".into(), iso(now()).into());
        }
        arch.extend(gone);
        self.save_file("archive.json", &arch)
    }

    /// Store a fact. Identical content re-learns (tau += 0.1); near-duplicates
    /// (Jaccard >= 0.8) merge into the existing trail. Returns the trail id.
    pub fn add(&self, content: &str, tags: &[String]) -> Result<String, String> {
        let content = content.trim();
        if content.is_empty() {
            return Err("nothing to remember".into());
        }
        let mut trails = self.load()?;
        let id = sha8(content);
        if let Some(t) = trails.iter_mut().find(|t| t.id == id) {
            materialize(t, true);
            t.tau = round6(t.tau + K);
            self.save(&trails)?;
            return Ok(id);
        }
        let q = tokens(content);
        if let Some(t) = trails.iter_mut().find(|t| jaccard(&q, &tokens(&t.content)) >= 0.8) {
            materialize(t, true);
            t.tau = round6(t.tau + K);
            let mut all: Vec<String> = t.tags.iter().chain(tags).cloned().collect();
            all.sort();
            all.dedup();
            t.tags = all;
            let id = t.id.clone();
            self.save(&trails)?;
            return Ok(id);
        }
        let ts = iso(now());
        trails.push(Trail {
            id: id.clone(),
            content: content.to_string(),
            tags: tags.to_vec(),
            tau: 1.0,
            success: 0,
            fail: 0,
            created: ts.clone(),
            last_access: ts,
            extra: Default::default(),
        });
        self.save(&trails)?;
        Ok(id)
    }

    /// Top `n` trails for `query`, ranked by P = tau * eta^2. `scope` keeps trails
    /// tagged for that project plus trails with no project tag. Touches what it returns.
    pub fn recall(&self, query: &str, n: usize, scope: Option<&str>) -> Vec<Trail> {
        let Ok(mut trails) = self.load() else { return Vec::new() };
        let q = tokens(query);
        let mut scored: Vec<(f64, usize)> = Vec::new();
        for (i, t) in trails.iter_mut().enumerate() {
            materialize(t, false);
            let in_scope = match scope {
                None => true,
                Some(tag) => t.tags.iter().any(|x| x == tag) || !t.tags.iter().any(|x| x.starts_with("project:")),
            };
            let p = t.tau * eta(t, &q).powi(2);
            if in_scope && p > 0.0 {
                scored.push((p, i));
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(n);
        let ts = iso(now());
        for (_, i) in &scored {
            trails[*i].last_access = ts.clone();
        }
        let out = scored.iter().map(|(_, i)| trails[*i].clone()).collect();
        let _ = self.save(&trails);
        out
    }

    fn update(&self, id: &str, f: impl FnOnce(&mut Trail)) -> Result<(), String> {
        let mut trails = self.load()?;
        let t = trails.iter_mut().find(|t| t.id.starts_with(id)).ok_or("memory not found")?;
        materialize(t, true);
        f(t);
        self.save(&trails)
    }

    /// The trail helped: tau += 0.1, success + 1.
    pub fn reinforce(&self, id: &str) -> Result<(), String> {
        self.update(id, |t| {
            t.tau = round6(t.tau + K);
            t.success += 1;
        })
    }

    /// The trail misled: tau *= 0.9, fail + 1.
    pub fn fail(&self, id: &str) -> Result<(), String> {
        self.update(id, |t| {
            t.fail += 1;
            t.tau = round6(t.tau * 0.9);
        })
    }

    pub fn forget(&self, id: &str) -> Result<(), String> {
        let mut trails = self.load()?;
        let ix = trails.iter().position(|t| t.id.starts_with(id)).ok_or("memory not found")?;
        let gone = trails.remove(ix);
        self.archive(vec![gone], "manual")?;
        self.save(&trails)
    }

    /// Evaporate everything and archive trails below tau 0.05.
    pub fn decay(&self) -> Result<(usize, usize), String> {
        // Never create a store just to evaporate an empty one.
        if !self.trails_path().is_file() {
            return Ok((0, 0));
        }
        let mut trails = self.load()?;
        for t in &mut trails {
            materialize(t, false);
        }
        let (keep, gone): (Vec<Trail>, Vec<Trail>) = trails.into_iter().partition(|t| t.tau >= TAU_MIN);
        let n = (keep.len(), gone.len());
        if !gone.is_empty() {
            self.archive(gone, "evaporated")?;
        }
        self.save(&keep)?;
        Ok(n)
    }

    /// All trails, strongest first (evaporation applied, not saved).
    pub fn list(&self) -> Vec<Trail> {
        let mut trails = self.load().unwrap_or_default();
        for t in &mut trails {
            materialize(t, false);
        }
        trails.sort_by(|a, b| b.tau.partial_cmp(&a.tau).unwrap_or(std::cmp::Ordering::Equal));
        trails
    }
}

/// Lines `REMEMBER: fact` at the end of an agent reply: (reply without them, facts).
pub fn take_remember_lines(text: &str) -> (String, Vec<String>) {
    let mut facts = Vec::new();
    let mut kept = Vec::new();
    for line in text.lines() {
        let t = line.trim().trim_start_matches(['*', '-', '_', ' ']);
        let lower = t.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("remember:") {
            let fact = t[t.len() - rest.len()..].trim_matches(|c: char| c == '*' || c == '_' || c.is_whitespace());
            if !fact.is_empty() {
                facts.push(fact.to_string());
                continue;
            }
        }
        kept.push(line);
    }
    (kept.join("\n").trim_end().to_string(), facts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        let d = std::env::temp_dir().join(format!("fluxmem-{}-{}", std::process::id(), now().timestamp_nanos_opt().unwrap_or(0)));
        Store::at(d)
    }

    #[test]
    fn add_recall_reinforce_forget() {
        let s = store();
        let id = s.add("deploy preview runs via GH Actions on every push", &["ci".into()]).unwrap();
        assert_eq!(id.len(), 8);
        // identical content re-learns, near-duplicate merges
        assert_eq!(s.add("deploy preview runs via GH Actions on every push", &[]).unwrap(), id);
        let other = s.add("python app runs with py app.py on Windows", &["project:kopiku".into()]).unwrap();
        let hits = s.recall("how does deploy work", 3, None);
        assert_eq!(hits[0].id, id);
        assert!((hits[0].tau - 1.1).abs() < 0.01);
        // scoping: a trail for another project is left out
        assert!(s.recall("python app", 3, Some("project:other")).iter().all(|t| t.id != other));
        assert!(s.recall("python app", 3, Some("project:kopiku")).iter().any(|t| t.id == other));
        s.reinforce(&id).unwrap();
        s.fail(&other).unwrap();
        let list = s.list();
        assert_eq!(list[0].success, 1);
        s.forget(&other[..4]).unwrap();
        assert_eq!(s.list().len(), 1);
        assert_eq!(s.decay().unwrap(), (1, 0));
    }

    #[test]
    fn python_compatible_tokens() {
        let t = tokens("Deploy di VPS-kiwora dengan GH Actions, a");
        assert!(t.contains("deploy") && t.contains("vps") && t.contains("kiwora") && t.contains("gh"));
        assert!(!t.contains("di") && !t.contains("dengan") && !t.contains("a"));
    }

    #[test]
    fn remember_lines() {
        let (text, facts) = take_remember_lines("Done.\n\nREMEMBER: run tests with `py -m pytest`\n**Remember:** venv is .venv");
        assert_eq!(text, "Done.");
        assert_eq!(facts, vec!["run tests with `py -m pytest`".to_string(), "venv is .venv".to_string()]);
    }
}
