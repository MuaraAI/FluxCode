//! A real helper, over a real pipe: the client's framing against the server's
//! answers, without a distro. `FLUXCODE_HELPER_BIN` names the binary to run.

use std::io::BufReader;
use std::process::{Command, Stdio};

use crate::remote::proto::{read_frame, write_frame, Request, Response, FRAME_BYTES, FRAME_JSON, PROTOCOL_VERSION};

struct Helper {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Helper {
    fn start() -> Option<Self> {
        let bin = std::env::var("FLUXCODE_HELPER_BIN").ok()?;
        let mut child = Command::new(bin)
            .arg("--remote-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let stdin = child.stdin.take()?;
        let stdout = BufReader::new(child.stdout.take()?);
        Some(Self { child, stdin, stdout })
    }

    fn ask(&mut self, req: &Request) -> Response {
        let body = serde_json::to_vec(req).unwrap();
        write_frame(&mut self.stdin, FRAME_JSON, &body).unwrap();
        let (kind, payload) = read_frame(&mut self.stdout).unwrap().unwrap();
        assert_eq!(kind, FRAME_JSON);
        serde_json::from_slice(&payload).unwrap()
    }

    fn read(&mut self, path: &str, limit: u64) -> Vec<u8> {
        let body = serde_json::to_vec(&Request::Read { path: path.into(), limit }).unwrap();
        write_frame(&mut self.stdin, FRAME_JSON, &body).unwrap();
        let (kind, _) = read_frame(&mut self.stdout).unwrap().unwrap();
        assert_eq!(kind, FRAME_JSON);
        let (kind, bytes) = read_frame(&mut self.stdout).unwrap().unwrap();
        assert_eq!(kind, FRAME_BYTES);
        bytes
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "needs FLUXCODE_HELPER_BIN pointing at a built fluxcode: FLUXCODE_HELPER_BIN=target/debug/fluxcode cargo test --bin fluxcode -- --ignored"]
fn a_helper_answers_over_a_pipe() {
    let Some(mut h) = Helper::start() else {
        eprintln!("FLUXCODE_HELPER_BIN not set; skipping");
        return;
    };
    // A temp folder to list, read and write in.
    let dir = std::env::temp_dir().join(format!("fluxcode-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("a.txt"), b"needle in a haystack\n").unwrap();
    std::fs::write(dir.join("sub").join("deep.txt"), b"nested\n").unwrap();
    let root = dir.to_str().unwrap().to_string();

    let hello = h.ask(&Request::Hello);
    assert!(hello.ok, "{hello:?}");
    let hello: crate::remote::proto::Hello = serde_json::from_value(hello.json.unwrap()).unwrap();
    assert_eq!(hello.protocol, PROTOCOL_VERSION);

    let list = h.ask(&Request::List { path: root.clone() });
    assert!(list.ok, "{list:?}");
    let entries: Vec<crate::remote::proto::DirEntry> = serde_json::from_value(list.json.unwrap()).unwrap();
    assert!(entries[0].is_dir, "folders come first: {entries:?}");
    assert!(entries.iter().any(|e| e.name == "a.txt"));

    let search = h.ask(&Request::Search { root: root.clone(), query: "needle".into(), match_case: false, whole_word: false });
    assert!(search.ok, "{search:?}");
    let hits: crate::remote::proto::SearchWire = serde_json::from_value(search.json.unwrap()).unwrap();
    assert_eq!(hits.total, 1);
    assert_eq!(hits.files[0].hits[0].line, 1);

    // The Ctrl+P list comes from the same helper.
    let scan = h.ask(&Request::Scan { root: root.clone(), max: 100 });
    assert!(scan.ok, "{scan:?}");
    let files: Vec<String> = serde_json::from_value(scan.json.unwrap()).unwrap();
    assert!(files.iter().any(|p| p.ends_with("a.txt")), "{files:?}");
    assert!(files.iter().any(|p| p.ends_with("sub/deep.txt")), "{files:?}");

    assert_eq!(h.read(&format!("{root}/a.txt"), 64), b"needle in a haystack\n");

    let stat = h.ask(&Request::Stat { path: format!("{root}/sub") });
    let stat: crate::remote::proto::Stat = serde_json::from_value(stat.json.unwrap()).unwrap();
    assert!(stat.exists && stat.is_dir && !stat.is_file);

    // A write travels as a second frame after the request.
    let body = serde_json::to_vec(&Request::Write { path: format!("{root}/b.txt"), crlf: false }).unwrap();
    write_frame(&mut h.stdin, FRAME_JSON, &body).unwrap();
    write_frame(&mut h.stdin, FRAME_BYTES, b"written by the editor\n").unwrap();
    let (kind, payload) = read_frame(&mut h.stdout).unwrap().unwrap();
    assert_eq!(kind, FRAME_JSON);
    let r: Response = serde_json::from_slice(&payload).unwrap();
    assert!(r.ok, "{r:?}");
    assert_eq!(std::fs::read_to_string(dir.join("b.txt")).unwrap(), "written by the editor\n");

    // A bad path says why instead of hanging up.
    let bad = h.ask(&Request::List { path: "/nope/not/here".into() });
    assert!(!bad.ok);
    assert!(bad.error.is_some());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "needs FLUXCODE_HELPER_BIN pointing at a built fluxcode: FLUXCODE_HELPER_BIN=target/debug/fluxcode cargo test --bin fluxcode -- --ignored"]
fn a_commit_message_reaches_git_through_the_helper() {
    let Some(mut h) = Helper::start() else {
        eprintln!("FLUXCODE_HELPER_BIN not set; skipping");
        return;
    };
    let dir = std::env::temp_dir().join(format!("fluxcode-e2e-git-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let root = dir.to_str().unwrap().to_string();
    let git = |h: &mut Helper, args: &[&str], input: Option<&str>| {
        let req = Request::Git {
            cwd: root.clone(),
            args: args.iter().map(|s| s.to_string()).collect(),
            input: input.map(str::to_string),
        };
        h.ask(&req)
    };

    for args in [vec!["init"], vec!["config", "user.email", "t@example.com"], vec!["config", "user.name", "Test"]] {
        let r = git(&mut h, &args, None);
        assert!(r.ok, "{args:?}: {r:?}");
    }
    std::fs::write(dir.join("a.txt"), b"one\n").unwrap();
    let r = git(&mut h, &["add", "-A"], None);
    assert!(r.ok, "{r:?}");
    let r = git(&mut h, &["-c", "commit.gpgsign=false", "commit", "--quiet", "--file", "-"], Some("msg via stdin\n"));
    assert!(r.ok, "{r:?}");
    let r = git(&mut h, &["log", "-1", "--format=%s"], None);
    assert!(r.ok, "{r:?}");
    let subject: crate::remote::proto::GitWire = serde_json::from_value(r.json.unwrap()).unwrap();
    assert!(subject.success, "{subject:?}");
    assert_eq!(subject.stdout.trim(), "msg via stdin");

    // Amend with no message: no input, the pipe is closed, and git keeps the subject.
    std::fs::write(dir.join("a.txt"), b"two\n").unwrap();
    let r = git(&mut h, &["add", "-A"], None);
    assert!(r.ok, "{r:?}");
    let r = git(&mut h, &["-c", "commit.gpgsign=false", "commit", "--amend", "--quiet", "--no-edit"], None);
    assert!(r.ok, "{r:?}");
    let r = git(&mut h, &["log", "-1", "--format=%s"], None);
    assert!(r.ok, "{r:?}");
    let subject: crate::remote::proto::GitWire = serde_json::from_value(r.json.unwrap()).unwrap();
    assert!(subject.success, "{subject:?}");
    assert_eq!(subject.stdout.trim(), "msg via stdin");

    let _ = std::fs::remove_dir_all(&dir);
}
