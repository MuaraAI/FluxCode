//! The framed protocol between the editor and its helper inside a distro.
//!
//! One request is in flight at a time, so the wire is simple: a frame is
//! `kind: u8`, `len: u32` (little endian), then `len` bytes. Kind 0 is JSON,
//! kind 1 is raw bytes. File contents travel as raw bytes rather than base64:
//! a 10 MB file would otherwise become 13 MB of JSON, and the whole point of
//! the helper is not moving that much data through the 9p share.

use serde::{Deserialize, Serialize};

/// Bumped when a request or a response changes shape. The editor replaces a
/// helper whose version differs, so the two never have to understand each
/// other across versions.
pub const PROTOCOL_VERSION: u32 = 2;

pub const FRAME_JSON: u8 = 0;
pub const FRAME_BYTES: u8 = 1;

/// What the editor asks the helper to do.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "m", rename_all = "snake_case")]
pub enum Request {
    /// Version and platform, sent once before anything else.
    Hello,
    /// One level of a folder.
    List { path: String },
    /// Every file under `root`, skipping the folders search skips. The answer
    /// is the file list behind Ctrl+P, so it never walks the 9p share.
    Scan { root: String, max: usize },
    /// A file's bytes, up to `limit`. The answer is a byte frame.
    Read { path: String, limit: u64 },
    /// A file's bytes, sent as a byte frame after this request.
    Write { path: String, crlf: bool },
    /// Whether a path exists and what it is.
    Stat { path: String },
    /// The folder's entries, matching `core::search`'s rules.
    Search { root: String, query: String, match_case: bool, whole_word: bool },
    /// `git` in the distro, so it sees Linux paths and line endings.
    Git { cwd: String, args: Vec<String> },
    /// Keeps a long-lived connection from being reaped.
    Ping,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Response {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json: Option<serde_json::Value>,
}

impl Response {
    pub fn error(message: impl Into<String>) -> Self {
        Self { ok: false, error: Some(message.into()), json: None }
    }

    pub fn json(value: impl Serialize) -> Self {
        match serde_json::to_value(value) {
            Ok(json) => Self { ok: true, error: None, json: Some(json) },
            Err(e) => Self::error(e.to_string()),
        }
    }

    /// A byte answer (a file's contents). `ok` lives in the JSON frame that
    /// follows, so a failed read still says why.
    pub fn bytes() -> Self {
        Self { ok: true, error: None, json: None }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub len: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Stat {
    pub exists: bool,
    pub is_dir: bool,
    pub is_file: bool,
    pub len: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Hello {
    pub protocol: u32,
    pub build: String,
    pub platform: String,
    pub home: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct HitWire {
    pub line: u32,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct FileHitsWire {
    pub path: String,
    pub hits: Vec<HitWire>,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct SearchWire {
    pub files: Vec<FileHitsWire>,
    pub total: usize,
    pub truncated: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct GitWire {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Write one frame.
pub fn write_frame(out: &mut impl std::io::Write, kind: u8, payload: &[u8]) -> std::io::Result<()> {
    out.write_all(&[kind])?;
    out.write_all(&(payload.len() as u32).to_le_bytes())?;
    out.write_all(payload)?;
    out.flush()
}

/// Read one frame. `None` means the other end closed cleanly.
pub fn read_frame(input: &mut impl std::io::Read) -> std::io::Result<Option<(u8, Vec<u8>)>> {
    let mut head = [0u8; 5];
    match input.read_exact(&mut head) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let kind = head[0];
    let len = u32::from_le_bytes([head[1], head[2], head[3], head[4]]) as usize;
    // A frame longer than this is not something this protocol sends; refuse it
    // rather than allocate whatever a broken length field asks for.
    if len > 512 * 1024 * 1024 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too long"));
    }
    let mut payload = vec![0u8; len];
    input.read_exact(&mut payload)?;
    Ok(Some((kind, payload)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, FRAME_JSON, br#"{"m":"ping"}"#).unwrap();
        write_frame(&mut buf, FRAME_BYTES, &[0, 1, 2, 255]).unwrap();
        let mut cursor = std::io::Cursor::new(buf);
        let (k, p) = read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!((k, p.as_slice()), (FRAME_JSON, br#"{"m":"ping"}"#.as_slice()));
        let (k, p) = read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!((k, p), (FRAME_BYTES, vec![0, 1, 2, 255]));
        assert!(read_frame(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn requests_use_a_tag_and_snake_case() {
        let json = serde_json::to_string(&Request::Read { path: "/a".into(), limit: 9 }).unwrap();
        assert_eq!(json, r#"{"m":"read","path":"/a","limit":9}"#);
        let back: Request = serde_json::from_str(r#"{"m":"ping"}"#).unwrap();
        assert_eq!(back, Request::Ping);
        let scan: Request = serde_json::from_str(r#"{"m":"scan","root":"/a","max":10}"#).unwrap();
        assert_eq!(scan, Request::Scan { root: "/a".into(), max: 10 });
    }

    #[test]
    fn a_server_error_keeps_its_message() {
        let r = Response::error("no such file");
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, r#"{"ok":false,"error":"no such file"}"#);
    }
}
