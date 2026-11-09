//! The terminal's smarts, kept free of UI so they can be unit tested:
//!
//! - `autocorrect`: `pip i` → `pip install`, typos in command names (`pyhton`),
//!   and file names that don't exist but have one close match
//!   (`-r reqirmemts.txt` → `-r requirements.txt`, `py app` → `py app.py`).
//! - `classify`: how one output line should look (error, warning, notice,
//!   traceback frame, ...), so stderr is no longer painted red wholesale.
//! - `parse_python_error`: the last Python traceback in a run, reduced to what
//!   the error card shows: error type, message, file:line, the code line, a
//!   plain-language hint and, when there is an obvious fix, a command to run it.

use std::path::{Path, PathBuf};

/// How a line of terminal output is drawn. Every kind maps to one theme token
/// in `panel.rs`, so the same meaning always gets the same color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineKind {
    Normal,
    /// A real error: pip's `ERROR:`, `Traceback`, the final `XxxError: ...` line.
    Err,
    /// The echoed command (prompt + what was typed).
    Cmd,
    /// Quiet output: pip's `[notice]`, `Requirement already satisfied`, `Using cached`.
    Dim,
    Warn,
    Success,
    /// `  File "app.py", line 3, in main`: clickable.
    Frame,
    /// The source line Python prints under a frame.
    Code,
    /// `    ^^^^` under a code line.
    Caret,
    /// "auto-corrected: pip i → pip install".
    Fixed,
    /// The end of a command: exit status and time.
    Status,
    /// `@@ -5,3 +5,4 @@` in a diff.
    Hunk,
    /// Error card rows (see `PyError`).
    CardHead,
    CardMsg,
    CardCode,
    CardHint,
    CardAction,
    Blank,
}

// ------------------------------------------------------------ edit distance

/// Optimal string alignment distance (Levenshtein + adjacent swaps), on chars.
pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=m {
        d[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut v = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = v;
        }
    }
    d[n][m]
}

/// How many edits a word of this length may need and still count as a typo.
fn budget(len: usize) -> usize {
    match len {
        0..=2 => 0,
        3..=4 => 1,
        5..=9 => 2,
        _ => 3,
    }
}

/// The single closest candidate within the typo budget (ties mean "not sure": None).
fn closest<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let w = word.to_lowercase();
    let max = budget(w.chars().count());
    let mut best: Option<(&str, usize)> = None;
    let mut tie = false;
    for c in candidates {
        let dist = distance(&w, &c.to_lowercase());
        if dist == 0 || dist > max {
            continue;
        }
        match best {
            Some((_, b)) if dist == b => tie = true,
            Some((_, b)) if dist > b => {}
            _ => {
                best = Some((c, dist));
                tie = false;
            }
        }
    }
    if tie { None } else { best.map(|b| b.0) }
}

// ------------------------------------------------------------ tokens

/// Byte spans of the words in a command line; quoted words keep their quotes.
fn tokens(line: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut quote = false;
    for (i, ch) in line.char_indices() {
        if ch == '"' {
            quote = !quote;
        }
        if ch.is_whitespace() && !quote {
            if let Some(s) = start.take() {
                out.push((s, i));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        out.push((s, line.len()));
    }
    out
}

fn unquote(s: &str) -> &str {
    s.trim_matches('"')
}

fn program_name(token: &str) -> String {
    let t = unquote(token);
    let name = Path::new(t).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| t.to_string());
    let lower = name.to_ascii_lowercase();
    lower.strip_suffix(".exe").map(str::to_string).unwrap_or(lower)
}

pub fn is_python(token: &str) -> bool {
    matches!(program_name(token).as_str(), "py" | "python" | "python3" | "pythonw" | "pyw")
}

/// Commands the auto-correct knows. A typo is only fixed toward one of these,
/// and only when the typed name is not a real program on PATH.
const KNOWN: &[&str] = &[
    "python", "python3", "py", "pip", "pip3", "pipx", "uv", "node", "npm", "npx", "pnpm", "yarn", "bun",
    "deno", "git", "cargo", "rustc", "code", "cd", "dir", "cls", "clear", "echo", "type", "where", "mkdir",
    "rmdir", "copy", "move", "ren", "tree", "start", "exit", "pytest", "flask", "streamlit", "jupyter",
    "black", "ruff", "mypy", "django-admin", "uvicorn", "gunicorn", "pyinstaller",
];

/// cmd.exe built-ins: never "corrected", they are not on PATH.
const BUILTINS: &[&str] = &[
    "set", "if", "for", "call", "vol", "ver", "title", "color", "date", "time", "pushd", "popd", "del",
    "erase", "rd", "md", "assoc", "ftype", "path", "prompt", "pause", "rem", "mklink", "goto", "shift",
    "setlocal", "endlocal", "chdir", "rename", "verify", "break", "pwd",
];

const PIP_COMMANDS: &[&str] = &[
    "install", "download", "uninstall", "freeze", "inspect", "list", "show", "check", "config", "search",
    "cache", "index", "wheel", "hash", "completion", "debug", "help",
];

/// Short forms people type for pip (npm habits included).
fn pip_alias(word: &str) -> Option<&'static str> {
    Some(match word.to_ascii_lowercase().as_str() {
        "i" | "in" | "ins" | "add" => "install",
        "un" | "uni" | "rm" | "remove" | "r" => "uninstall",
        "ls" | "l" | "ll" => "list",
        "s" | "info" => "show",
        "f" => "freeze",
        "up" | "upgrade" | "update" => "install --upgrade",
        _ => return None,
    })
}

/// Is `name` a program on PATH (any PATHEXT on Windows)?
pub fn on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| e.to_ascii_lowercase())
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&path).any(|dir| {
        exts.iter().any(|e| dir.join(format!("{name}{e}")).is_file()) || (!cfg!(windows) && dir.join(name).is_file())
    })
}

/// Names in a folder (files and folders), for file-name corrections and Tab.
fn entries(dir: &Path) -> Vec<(String, bool)> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| (e.file_name().to_string_lossy().into_owned(), e.file_type().map(|t| t.is_dir()).unwrap_or(false)))
                .collect()
        })
        .unwrap_or_default()
}

/// A file argument that doesn't exist: the one existing file it most likely means.
/// `want_ext` lists extensions to try when the name has none (`py app` → `app.py`).
fn fix_file(arg: &str, cwd: &Path, want_ext: &[&str]) -> Option<String> {
    let raw = unquote(arg);
    if raw.is_empty() || raw.starts_with('-') || raw.contains(['*', '?', '<', '>', '|']) || cwd.join(raw).exists() {
        return None;
    }
    let (dir_part, name) = match raw.rfind(['/', '\\']) {
        Some(i) => (&raw[..=i], &raw[i + 1..]),
        None => ("", raw),
    };
    let dir = cwd.join(dir_part);
    let files: Vec<String> = entries(&dir).into_iter().filter(|(_, d)| !d).map(|(n, _)| n).collect();
    if !name.contains('.') {
        for ext in want_ext {
            let with = format!("{name}.{ext}");
            if let Some(f) = files.iter().find(|f| f.eq_ignore_ascii_case(&with)) {
                return Some(format!("{dir_part}{f}"));
            }
        }
    }
    // A script name cut short or slightly off (`py ap`, `py aap`): match the
    // stems of files with the expected extension.
    let stem_hit = (!want_ext.is_empty() && !name.contains('.') && name.chars().count() >= 2)
        .then(|| {
            let lower = name.to_lowercase();
            let scripts: Vec<&String> = files
                .iter()
                .filter(|f| want_ext.iter().any(|e| f.to_lowercase().ends_with(&format!(".{e}"))))
                .collect();
            let starts: Vec<&&String> = scripts.iter().filter(|f| f.to_lowercase().starts_with(&lower)).collect();
            if starts.len() == 1 {
                return Some(starts[0].to_string());
            }
            let stems: Vec<(String, &String)> = scripts.iter().map(|f| (f.rsplit_once('.').map_or(f.to_string(), |s| s.0.to_string()), *f)).collect();
            let max = budget(lower.chars().count()).max(1);
            let mut best: Vec<(usize, &String)> = stems.iter().map(|(s, f)| (distance(&lower, &s.to_lowercase()), *f)).filter(|(d, _)| *d <= max).collect();
            best.sort_by_key(|b| b.0);
            match best.as_slice() {
                [one] => Some(one.1.clone()),
                [a, b, ..] if a.0 < b.0 => Some(a.1.clone()),
                _ => None,
            }
        })
        .flatten();
    let hit = match stem_hit {
        Some(h) => h,
        None => closest(name, files.iter().map(String::as_str))?.to_string(),
    };
    let fixed = format!("{dir_part}{hit}");
    Some(if fixed.contains(' ') { format!("\"{fixed}\"") } else { fixed })
}

pub struct Correction {
    pub line: String,
    /// (what was typed, what runs instead), for the "auto-corrected" note.
    pub fixes: Vec<(String, String)>,
}

/// Fix the obvious slips in a command line before it runs. Only safe rewrites:
/// pip sub-command aliases and typos, a misspelled command name, and file
/// arguments that don't exist but have exactly one close match.
pub fn autocorrect(line: &str, cwd: &Path) -> Option<Correction> {
    let spans = tokens(line);
    if spans.is_empty() {
        return None;
    }
    let words: Vec<&str> = spans.iter().map(|&(s, e)| &line[s..e]).collect();
    // (span index, replacement)
    let mut edits: Vec<(usize, String)> = Vec::new();

    // 1. The command name.
    let first = words[0];
    let first_lower = first.to_ascii_lowercase();
    let mut program = program_name(first);
    if !first.contains(['\\', '/', '.', '"', ':'])
        && !KNOWN.contains(&first_lower.as_str())
        && !BUILTINS.contains(&first_lower.as_str())
        && !cwd.join(first).exists()
        && !on_path(&first_lower)
    {
        let unix = if cfg!(windows) {
            match first_lower.as_str() {
                "ls" | "ll" | "la" => Some("dir"),
                "cat" => Some("type"),
                "which" => Some("where"),
                _ => None,
            }
        } else {
            None
        };
        if let Some(to) = unix.or_else(|| closest(first, KNOWN.iter().copied())) {
            edits.push((0, to.to_string()));
            program = to.to_string();
        }
    }

    // `ls -la` → `dir`: Windows' dir doesn't take Unix flags.
    let drop_flags = program == "dir" && first_lower != "dir";

    // 2. pip: the sub-command and `-r file`.
    let pip_at = if matches!(program.as_str(), "pip" | "pip3") {
        Some(1)
    } else if is_python(first) || program == "python" || program == "py" {
        words.iter().position(|w| *w == "-m").filter(|&i| words.get(i + 1).is_some_and(|w| w.eq_ignore_ascii_case("pip"))).map(|i| i + 2)
    } else {
        None
    };
    let mut python_script_from = None;
    if let Some(start) = pip_at {
        if let Some(sub) = (start..words.len()).find(|&i| !words[i].starts_with('-')) {
            let w = words[sub];
            if !PIP_COMMANDS.contains(&w.to_ascii_lowercase().as_str()) {
                if let Some(to) = pip_alias(w).or_else(|| closest(w, PIP_COMMANDS.iter().copied())) {
                    edits.push((sub, to.to_string()));
                }
            }
        }
        for i in start..words.len() {
            if matches!(words[i], "-r" | "--requirement" | "-c" | "--constraint") {
                if let Some(fixed) = words.get(i + 1).and_then(|f| fix_file(f, cwd, &["txt"])) {
                    edits.push((i + 1, fixed));
                }
            }
        }
    } else if is_python(first) || program == "python" || program == "py" {
        python_script_from = Some(1);
    }

    // 3. The script for python / node, and the file for type.
    let script = |from: usize, exts: &[&str], edits: &mut Vec<(usize, String)>| {
        for i in from..words.len() {
            let w = words[i];
            if matches!(w, "-m" | "-c") {
                return; // a module or inline code, not a file
            }
            if w.starts_with('-') {
                continue;
            }
            if let Some(fixed) = fix_file(w, cwd, exts) {
                edits.push((i, fixed));
            }
            return;
        }
    };
    if let Some(from) = python_script_from {
        script(from, &["py", "pyw"], &mut edits);
    } else if program == "node" {
        script(1, &["js", "mjs", "cjs"], &mut edits);
    } else if program == "type" && words.len() == 2 {
        script(1, &[], &mut edits);
    }

    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|e| e.0);
    let mut out = String::new();
    let mut fixes = Vec::new();
    let mut last = 0;
    for (ix, (s, e)) in spans.iter().enumerate() {
        out.push_str(&line[last..*s]);
        last = *e;
        if drop_flags && ix > 0 && words[ix].starts_with('-') {
            fixes.push((words[ix].to_string(), String::new()));
            // swallow the flag and the space before it
            while out.ends_with(' ') {
                out.pop();
            }
            continue;
        }
        match edits.iter().find(|e| e.0 == ix) {
            Some((_, to)) => {
                fixes.push((words[ix].to_string(), to.clone()));
                out.push_str(to);
            }
            None => out.push_str(words[ix]),
        }
    }
    out.push_str(&line[last..]);
    fixes.retain(|(_, to)| !to.is_empty());
    Some(Correction { line: out, fixes })
}

/// Tab in the terminal: complete the last word as a path (or a command name
/// when it is the only word). Returns the new line and, when several names
/// match, the list to show.
pub fn complete(line: &str, cwd: &Path) -> (Option<String>, Vec<String>) {
    let start = line.rfind(' ').map_or(0, |i| i + 1);
    let word = line[start..].trim_start_matches('"');
    let (dir_part, prefix) = match word.rfind(['/', '\\']) {
        Some(i) => (&word[..=i], &word[i + 1..]),
        None => ("", word),
    };
    let lower = prefix.to_lowercase();
    let mut names: Vec<(String, bool)> = entries(&cwd.join(dir_part))
        .into_iter()
        .filter(|(n, _)| n.to_lowercase().starts_with(&lower) && (lower.starts_with('.') || !n.starts_with('.')))
        .collect();
    if start == 0 && dir_part.is_empty() && !prefix.is_empty() {
        names.extend(KNOWN.iter().filter(|k| k.starts_with(&lower)).map(|k| (k.to_string(), false)));
    }
    names.sort_by_key(|a| a.0.to_lowercase());
    names.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
    let sep = if cfg!(windows) { "\\" } else { "/" };
    let render = |name: &str, dir: bool, done: bool| {
        let mut s = format!("{dir_part}{name}{}", if dir && done { sep } else { "" });
        if s.contains(' ') {
            s = format!("\"{s}{}", if done && !dir { "\"" } else { "" });
        }
        s
    };
    match names.len() {
        0 => (None, Vec::new()),
        1 => {
            let (n, d) = &names[0];
            let tail = if *d { "" } else { " " };
            (Some(format!("{}{}{}", &line[..start], render(n, *d, true), tail)), Vec::new())
        }
        _ => {
            // Extend to the longest common prefix, then list the choices.
            let first = names[0].0.to_lowercase();
            let mut common = first.len();
            for (n, _) in &names[1..] {
                let n = n.to_lowercase();
                common = common.min(first.chars().zip(n.chars()).take_while(|(a, b)| a == b).map(|(a, _)| a.len_utf8()).sum());
            }
            let ext: String = names[0].0.chars().take(first[..common].chars().count()).collect();
            let list = names.iter().map(|(n, d)| if *d { format!("{n}{sep}") } else { n.clone() }).collect();
            let new = (ext.len() > prefix.len()).then(|| format!("{}{}", &line[..start], render(&ext, false, false)));
            (new, list)
        }
    }
}

// ------------------------------------------------------------ classification

fn is_ident_path(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

/// `NameError: x` / `requests.exceptions.HTTPError: ...` / `KeyboardInterrupt`.
pub fn python_error_line(text: &str) -> Option<(&str, &str)> {
    if text.starts_with(char::is_whitespace) {
        return None;
    }
    let (name, msg) = match text.find(':') {
        Some(i) => (&text[..i], text[i + 1..].trim()),
        None => (text.trim_end(), ""),
    };
    let last = name.rsplit('.').next().unwrap_or(name);
    let looks = ["Error", "Exception", "Interrupt", "Exit", "Iteration"].iter().any(|s| last.ends_with(s))
        && last.chars().next().is_some_and(|c| c.is_ascii_uppercase());
    (looks && is_ident_path(name)).then_some((name, msg))
}

/// `  File "C:\x\app.py", line 12, in main` → (path, line, function).
pub fn parse_frame(text: &str) -> Option<(String, u32, Option<String>)> {
    let t = text.trim_start();
    let rest = t.strip_prefix("File \"")?;
    let q = rest.find('"')?;
    let path = rest[..q].to_string();
    let after = rest[q + 1..].strip_prefix(", line ")?;
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    let line = digits.parse().ok()?;
    let func = after[digits.len()..].strip_prefix(", in ").map(|f| f.trim().to_string());
    Some((path, line, func))
}

/// `C:\x\app.py:12: UserWarning: ...` → (path, line).
pub fn parse_path_line(text: &str) -> Option<(String, u32)> {
    let t = text.trim_start();
    for ext in [".py:", ".pyw:"] {
        if let Some(i) = t.to_ascii_lowercase().find(ext) {
            let path = &t[..i + ext.len() - 1];
            let digits: String = t[i + ext.len()..].chars().take_while(char::is_ascii_digit).collect();
            if let Ok(line) = digits.parse() {
                return Some((path.to_string(), line));
            }
        }
    }
    None
}

/// The kind of one finished (or growing) output line. `prev` is the kind of the
/// line above it, so a traceback's code and caret lines can be recognized.
pub fn classify(text: &str, stderr: bool, prev: Option<LineKind>) -> LineKind {
    let t = text.trim_start();
    let indented = text.len() != t.len();
    if t.is_empty() {
        return LineKind::Normal;
    }
    if t.starts_with("[notice]") {
        return LineKind::Dim;
    }
    if t.starts_with("Traceback (most recent call last)") {
        return LineKind::Err;
    }
    if indented && parse_frame(text).is_some() {
        return LineKind::Frame;
    }
    if indented && matches!(prev, Some(LineKind::Frame | LineKind::Code)) {
        if t.chars().all(|c| matches!(c, '^' | '~' | ' ')) {
            return LineKind::Caret;
        }
        if prev == Some(LineKind::Frame) {
            return LineKind::Code;
        }
    }
    if python_error_line(text).is_some() {
        return LineKind::Err;
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("error:")
        || lower.starts_with("fatal:")
        || lower.starts_with("npm err!")
        || lower.contains("is not recognized as an internal or external command")
        || lower.starts_with("the system cannot find")
        || lower.contains(": can't open file '")
    {
        return LineKind::Err;
    }
    if lower.starts_with("warning:") || lower.starts_with("deprecation:") || lower.starts_with("npm warn") || t.contains("Warning: ") {
        return LineKind::Warn;
    }
    if t.starts_with("Successfully installed") || t.starts_with("Successfully uninstalled") || t.starts_with("Successfully built") {
        return LineKind::Success;
    }
    if t.starts_with("Requirement already satisfied")
        || t.starts_with("Using cached")
        || t.starts_with("Downloading ")
        || t.starts_with("Installing collected packages")
        || t.starts_with("Found existing installation")
        || t.starts_with("Uninstalling ")
        || (indented && t.starts_with("Successfully uninstalled"))
    {
        return LineKind::Dim;
    }
    let _ = stderr; // stderr alone is not an error: pip, git and npm print progress there
    LineKind::Normal
}

// ------------------------------------------------------------ Python errors

#[derive(Debug, Clone, PartialEq)]
pub struct PyError {
    pub kind: String,
    pub message: String,
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
    /// 0-based column of the first `^`, relative to the stripped code line.
    pub caret: Option<(usize, usize)>,
    pub code: Option<String>,
    pub hint: Option<String>,
    /// A command that fixes it (`py -m pip install customtkinter`).
    pub fix: Option<String>,
}

/// Code the user wrote, as opposed to the standard library or installed packages.
fn is_user_frame(path: &str) -> bool {
    let p = path.to_ascii_lowercase().replace('/', "\\");
    let stdlib = p.contains("\\lib\\") && p.split('\\').any(|c| c.starts_with("python"));
    !(p.starts_with('<') || p.contains("site-packages") || p.contains("dist-packages") || stdlib)
}

/// Package name on PyPI for an import name that differs from it.
pub fn pip_package(module: &str) -> Option<&'static str> {
    let root = module.split('.').next().unwrap_or(module);
    Some(match root {
        "cv2" => "opencv-python",
        "PIL" => "pillow",
        "sklearn" => "scikit-learn",
        "skimage" => "scikit-image",
        "yaml" => "pyyaml",
        "bs4" => "beautifulsoup4",
        "dotenv" => "python-dotenv",
        "docx" => "python-docx",
        "pptx" => "python-pptx",
        "serial" => "pyserial",
        "usb" => "pyusb",
        "Crypto" => "pycryptodome",
        "jwt" => "PyJWT",
        "dateutil" => "python-dateutil",
        "fitz" => "pymupdf",
        "win32api" | "win32con" | "win32gui" | "win32com" | "pywintypes" => "pywin32",
        "googleapiclient" => "google-api-python-client",
        "telegram" => "python-telegram-bot",
        "discord" => "discord.py",
        "attr" => "attrs",
        "mpl_toolkits" => "matplotlib",
        "OpenGL" => "PyOpenGL",
        "gi" => "PyGObject",
        "wx" => "wxPython",
        "magic" => "python-magic",
        "Levenshtein" => "python-Levenshtein",
        "MySQLdb" => "mysqlclient",
        "psycopg2" => "psycopg2-binary",
        _ => return None,
    })
}

fn quoted(s: &str) -> String {
    if s.contains(' ') { format!("\"{s}\"") } else { s.to_string() }
}

/// Plain-language advice for common Python errors, and a fix command if one is obvious.
fn advise(kind: &str, msg: &str, python: &str, cwd: &Path) -> (Option<String>, Option<String>) {
    let short = kind.rsplit('.').next().unwrap_or(kind);
    let between = |s: &str| s.split('\'').nth(1).map(str::to_string);
    match short {
        "ModuleNotFoundError" => {
            let Some(module) = between(msg) else { return (None, None) };
            if module == "tkinter" || module == "_tkinter" {
                return (Some("tkinter comes with Python itself. Reinstall Python with \"tcl/tk and IDLE\" ticked.".into()), None);
            }
            let root = module.split('.').next().unwrap_or(&module).to_string();
            let pkg = pip_package(&root).map(str::to_string).unwrap_or(root.clone());
            (
                Some(format!("'{root}' isn't installed for this Python. Install it, then run again.")),
                Some(format!("{} -m pip install {pkg}", quoted(python))),
            )
        }
        "ImportError" if msg.contains("cannot import name") => {
            (Some("That name doesn't exist in the module: check the spelling, or the module's version.".into()), None)
        }
        "NameError" => (
            Some(if msg.contains("Did you mean") {
                "Python doesn't know this name. Use the suggested name, or define it before this line.".into()
            } else {
                "Python doesn't know this name here. Check the spelling, or define it (or import it) before this line.".into()
            }),
            None,
        ),
        "SyntaxError" => {
            let m = msg.to_ascii_lowercase();
            let h = if m.contains("was never closed") {
                "A bracket or quote opened here is never closed. Add the closing one."
            } else if m.contains("unterminated string") || m.contains("eol while scanning") {
                "This text has no closing quote. Add the matching \" or '."
            } else if m.contains("expected ':'") {
                "Lines with if, elif, else, for, while, def and class end with a colon ':'."
            } else if m.contains("forgot a comma") {
                "Put a comma between the items."
            } else if m.contains("invalid character") {
                "There is a character Python can't read, often a curly quote or a dash pasted from a document. Retype it."
            } else if m.contains("'(' was never closed") || m.contains("unmatched") {
                "Brackets don't pair up. Check every ( [ { has its closing ) ] }."
            } else {
                "Python couldn't read this line. Look for a missing bracket, quote, colon or comma at the ^ mark."
            };
            (Some(h.into()), None)
        }
        "IndentationError" | "TabError" => (
            Some("Indent each block the same way: 4 spaces under if/for/def, and don't mix tabs with spaces.".into()),
            None,
        ),
        "TypeError" => {
            let h = if msg.contains("can only concatenate str") || msg.contains("must be str, not") {
                "Text and numbers can't be joined with +. Use str(x), or an f-string: f\"Total: {x}\"."
            } else if msg.contains("unsupported operand") && msg.contains("'str'") {
                "One of the values is text. Convert it with int(x) or float(x) first."
            } else if msg.contains("required positional argument") {
                "The function needs more arguments than it was given."
            } else if msg.contains("takes") && msg.contains("positional argument") {
                "The function was given more arguments than it accepts. Inside a class, the first parameter must be self."
            } else if msg.contains("'NoneType'") {
                "The value is None here: a function probably returned nothing."
            } else if msg.contains("not callable") {
                "Something that isn't a function is being called with (). A variable may hide a function of the same name."
            } else {
                "A value has the wrong type for this operation."
            };
            (Some(h.into()), None)
        }
        "AttributeError" => (
            Some(if msg.contains("'NoneType'") {
                "The value is None: something before this line returned nothing.".into()
            } else {
                "That object has no such attribute or method. Check the spelling.".into()
            }),
            None,
        ),
        "KeyError" => (Some(format!("The dictionary has no key {msg}. Check the key, or use .get({msg}).")), None),
        "IndexError" => (Some("The index is past the end. Valid positions run from 0 to len(...) - 1.".into()), None),
        "ValueError" if msg.contains("invalid literal for int()") || msg.contains("could not convert string to float") => {
            (Some("The text isn't a number. Check the input, or catch it with try/except ValueError.".into()), None)
        }
        "ValueError" => (Some("The value has the right type but content the function can't use.".into()), None),
        "ZeroDivisionError" => (Some("Division by zero. Check the divisor before dividing.".into()), None),
        "FileNotFoundError" => {
            let near = between(msg).and_then(|f| fix_file(&f, cwd, &[]));
            let h = match near {
                Some(n) => format!("No such file. Did you mean '{}'? Relative paths start in {}", unquote(&n), cwd.display()),
                None => format!("No such file. Relative paths start in the folder the program runs from: {}", cwd.display()),
            };
            (Some(h), None)
        }
        "RecursionError" => (Some("A function keeps calling itself. Check its stop condition.".into()), None),
        "UnicodeDecodeError" => (Some("Open the file with encoding=\"utf-8\".".into()), None),
        "PermissionError" => (Some("The file is open in another program, or the folder is read-only.".into()), None),
        "TclError" => (Some("Tk rejected a widget option, image or font. Check the value at this line.".into()), None),
        _ => (None, None),
    }
}

/// The last Python error in a block of output lines (`(text, kind)` as classified).
/// `python` is the interpreter used, for the fix command.
pub fn parse_python_error(lines: &[(&str, LineKind)], python: &str, cwd: &Path) -> Option<PyError> {
    let err_ix = lines.iter().rposition(|(t, k)| *k == LineKind::Err && python_error_line(t).is_some())?;
    let (kind, message) = python_error_line(lines[err_ix].0)?;
    if kind == "KeyboardInterrupt" || kind == "SystemExit" {
        return None;
    }
    // Frames above the error line, up to the traceback header.
    let mut frames: Vec<(String, u32, Option<String>, Option<(usize, usize)>)> = Vec::new();
    let mut i = err_ix;
    while i > 0 {
        i -= 1;
        let (t, k) = lines[i];
        match k {
            LineKind::Frame => {
                if let Some((p, l, _)) = parse_frame(t) {
                    let code = lines.get(i + 1).filter(|(_, k)| *k == LineKind::Code).map(|(c, _)| c.trim().to_string());
                    let caret = lines
                        .get(i + 2)
                        .filter(|(_, k)| *k == LineKind::Caret)
                        .zip(lines.get(i + 1))
                        .and_then(|((c, _), (code_line, _))| {
                            let indent = code_line.len() - code_line.trim_start().len();
                            let start = c.find(['^', '~'])?;
                            let end = c.rfind(['^', '~'])? + 1;
                            Some((start.saturating_sub(indent), end.saturating_sub(indent)))
                        });
                    frames.push((p, l, code, caret));
                }
            }
            LineKind::Code | LineKind::Caret => {}
            _ => break,
        }
    }
    frames.reverse();
    let pick = frames.iter().rev().find(|f| is_user_frame(&f.0)).or(frames.last());
    let (hint, fix) = advise(kind, message, python, cwd);
    Some(PyError {
        kind: kind.to_string(),
        message: message.to_string(),
        file: pick.map(|f| {
            let p = PathBuf::from(&f.0);
            if p.is_absolute() { p } else { cwd.join(p) }
        }),
        line: pick.map(|f| f.1),
        caret: pick.and_then(|f| f.3),
        code: pick.and_then(|f| f.2.clone()),
        hint,
        fix,
    })
}

/// What went wrong in a finished command, for the card: a Python error, a script
/// Python can't find, or a command Windows doesn't know.
pub fn parse_problem(lines: &[(&str, LineKind)], command: &str, python: &str, cwd: &Path) -> Option<PyError> {
    if let Some(e) = parse_python_error(lines, python, cwd) {
        return Some(e);
    }
    let card = |kind: &str, message: String, hint: Option<String>, fix: Option<String>| PyError {
        kind: kind.into(),
        message,
        file: None,
        line: None,
        caret: None,
        code: None,
        hint,
        fix,
    };
    // python.exe: can't open file 'C:\\proj\\ap': [Errno 2] No such file or directory
    if let Some((t, _)) = lines.iter().find(|(t, _)| t.contains(": can't open file '")) {
        let quoted_path = t.split('\'').nth(1).unwrap_or("").replace("\\\\", "\\");
        let name = Path::new(&quoted_path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(quoted_path.clone());
        let near = fix_file(&name, cwd, &["py", "pyw"]);
        let fix = near.as_ref().map(|n| command.replacen(&name, unquote(n), 1)).filter(|f| f != command);
        let hint = match &near {
            Some(n) => format!("Did you mean {}?", unquote(n)),
            None => "Check the name with  dir *.py  or open the folder that has the script.".into(),
        };
        return Some(card("File not found", format!("Python can't find '{name}' in {}", cwd.display()), Some(hint), fix));
    }
    // 'pyhton' is not recognized as an internal or external command,
    if let Some((t, _)) = lines.iter().find(|(t, _)| t.contains("is not recognized as an internal or external command")) {
        let name = t.split('\'').nth(1).unwrap_or("").to_string();
        let near = closest(&name, KNOWN.iter().copied()).filter(|k| on_path(k) || matches!(*k, "cd" | "dir" | "cls" | "type" | "echo"));
        let fix = near.map(|n| command.replacen(&name, n, 1));
        let hint = match near {
            Some(n) => format!("Did you mean {n}?"),
            None if matches!(name.as_str(), "python" | "python3" | "pip" | "pip3") => {
                "Python isn't on PATH. Try py (the Windows launcher), e.g.  py -m pip install ...".into()
            }
            None => "Check the spelling, or install the program and open a new terminal.".into(),
        };
        return Some(card("Unknown command", format!("'{name}' isn't a program or command here"), Some(hint), fix));
    }
    None
}

/// Break a long message into rows for the card (the panel list has fixed-height rows).
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
            rows.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() || rows.is_empty() {
        rows.push(cur);
    }
    rows
}

/// "3.2s", "120ms", "1m 05s".
pub fn elapsed(d: std::time::Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.)
    } else {
        format!("{}m {:02}s", ms / 60_000, (ms / 1000) % 60)
    }
}

/// `C:\Users\me\Documents\proj\src` → `~\Documents\proj\src`; inside the open
/// folder, from the folder's own name.
pub fn short_cwd(cwd: &Path, root: Option<&Path>) -> String {
    if let Some(root) = root {
        if let Ok(rel) = cwd.strip_prefix(root) {
            let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return if rel.as_os_str().is_empty() { name } else { Path::new(&name).join(rel).display().to_string() };
        }
    }
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        if let Ok(rel) = cwd.strip_prefix(&home) {
            return Path::new("~").join(rel).display().to_string();
        }
    }
    cwd.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("fluxterm-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        for f in ["requirements.txt", "app.py", "data.json"] {
            let _ = std::fs::write(d.join(f), "");
        }
        let _ = std::fs::create_dir_all(d.join("ikon"));
        d
    }

    #[test]
    fn pip_aliases() {
        let d = tmp();
        let c = autocorrect("py -m pip i -r reqirmemts.txt", &d).unwrap();
        assert_eq!(c.line, "py -m pip install -r requirements.txt");
        assert_eq!(c.fixes.len(), 2);
        assert_eq!(autocorrect("pip isntall flask", &d).unwrap().line, "pip install flask");
        assert_eq!(autocorrect("pip up pip", &d).unwrap().line, "pip install --upgrade pip");
        assert!(autocorrect("pip install flask", &d).is_none());
    }

    #[test]
    fn scripts_and_commands() {
        let d = tmp();
        assert_eq!(autocorrect("py app", &d).unwrap().line, "py app.py");
        assert_eq!(autocorrect("py ap.py", &d).unwrap().line, "py app.py");
        assert_eq!(autocorrect("py ap", &d).unwrap().line, "py app.py");
        assert_eq!(autocorrect("py aap", &d).unwrap().line, "py app.py");
        assert!(autocorrect("py -m http.server", &d).is_none());
        assert!(autocorrect("py -c \"print(1)\"", &d).is_none());
        assert_eq!(autocorrect("pyhton app.py", &d).unwrap().line, "python app.py");
        if cfg!(windows) && !on_path("ls") {
            assert_eq!(autocorrect("ls -la", &d).map(|c| c.line).unwrap_or_default().trim(), "dir");
        }
    }

    #[test]
    fn classifies_pip_output() {
        assert_eq!(classify("[notice] A new release of pip is available", true, None), LineKind::Dim);
        assert_eq!(classify("ERROR: Could not open requirements file", true, None), LineKind::Err);
        assert_eq!(classify("WARNING: Retrying", true, None), LineKind::Warn);
        assert_eq!(classify("Successfully installed pillow-13.0.0", false, None), LineKind::Success);
        assert_eq!(classify("Collecting customtkinter", false, None), LineKind::Normal);
        assert_eq!(classify("some progress on stderr", true, None), LineKind::Normal);
    }

    #[test]
    fn parses_traceback() {
        let d = tmp();
        let raw = [
            "Traceback (most recent call last):",
            "  File \"C:\\proj\\app.py\", line 17, in <module>",
            "    total = harga * qty",
            "            ^^^^^",
            "NameError: name 'harga' is not defined. Did you mean: 'harga_kopi'?",
        ];
        let mut lines = Vec::new();
        let mut prev = None;
        for t in raw {
            let k = classify(t, true, prev);
            lines.push((t, k));
            prev = Some(k);
        }
        assert_eq!(lines[1].1, LineKind::Frame);
        assert_eq!(lines[2].1, LineKind::Code);
        assert_eq!(lines[3].1, LineKind::Caret);
        let e = parse_python_error(&lines, "py", &d).unwrap();
        assert_eq!(e.kind, "NameError");
        assert_eq!(e.line, Some(17));
        assert_eq!(e.code.as_deref(), Some("total = harga * qty"));
        assert_eq!(e.caret, Some((8, 13)));
        assert!(e.hint.is_some());
    }

    #[test]
    fn missing_module_fix() {
        let d = tmp();
        let lines = [("ModuleNotFoundError: No module named 'cv2'", LineKind::Err)];
        let e = parse_python_error(&lines, "py", &d).unwrap();
        assert_eq!(e.fix.as_deref(), Some("py -m pip install opencv-python"));
    }

    #[test]
    fn syntax_error_without_traceback() {
        let d = tmp();
        let raw = ["  File \"C:\\p\\app.py\", line 3", "    print(\"hi\"", "         ^", "SyntaxError: '(' was never closed"];
        let mut lines = Vec::new();
        let mut prev = None;
        for t in raw {
            let k = classify(t, true, prev);
            lines.push((t, k));
            prev = Some(k);
        }
        let e = parse_python_error(&lines, "py", &d).unwrap();
        assert_eq!(e.line, Some(3));
        assert_eq!(e.caret, Some((5, 6)));
    }

    #[test]
    fn tab_completion() {
        let d = tmp();
        assert_eq!(complete("py ap", &d).0.as_deref(), Some("py app.py "));
        assert_eq!(complete("cd ik", &d).0.as_deref(), Some(format!("cd ikon{}", if cfg!(windows) { "\\" } else { "/" }).as_str()));
    }
}
