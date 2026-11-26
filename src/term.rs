
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineKind {
    Normal,
    Err,
    Cmd,
    Dim,
    Warn,
    Success,
    Frame,
    Code,
    Caret,
    Fixed,
    Status,
    Result,
    Hunk,
    CardHead,
    CardMsg,
    CardCode,
    CardHint,
    CardAction,
    Blank,
}


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

fn budget(len: usize) -> usize {
    match len {
        0..=2 => 0,
        3..=4 => 1,
        5..=9 => 2,
        _ => 3,
    }
}

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

const KNOWN: &[&str] = &[
    "python", "python3", "py", "pip", "pip3", "pipx", "uv", "node", "npm", "npx", "pnpm", "yarn", "bun",
    "deno", "tsc", "tsx", "git", "cargo", "rustc", "rustup", "go", "java", "javac", "mvn", "gradle",
    "dotnet", "gcc", "g++", "clang", "clang++", "make", "cmake", "zig", "php", "composer", "ruby",
    "bundle", "lua", "dart", "flutter", "swift", "kotlinc", "perl", "elixir", "mix", "docker", "kubectl",
    "curl", "ssh", "code", "cd", "dir", "cls", "clear", "echo", "type", "where", "mkdir", "rmdir",
    "copy", "move", "ren", "tree", "start", "exit", "pytest", "flask", "streamlit", "jupyter",
    "black", "ruff", "mypy", "django-admin", "uvicorn", "gunicorn", "pyinstaller",
];

const BUILTINS: &[&str] = &[
    "set", "if", "for", "call", "vol", "ver", "title", "color", "date", "time", "pushd", "popd", "del",
    "erase", "rd", "md", "assoc", "ftype", "path", "prompt", "pause", "rem", "mklink", "goto", "shift",
    "setlocal", "endlocal", "chdir", "rename", "verify", "break", "pwd",
];

const PIP_COMMANDS: &[&str] = &[
    "install", "download", "uninstall", "freeze", "inspect", "list", "show", "check", "config", "search",
    "cache", "index", "wheel", "hash", "completion", "debug", "help",
];

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

fn entries(dir: &Path) -> Vec<(String, bool)> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| (e.file_name().to_string_lossy().into_owned(), e.file_type().map(|t| t.is_dir()).unwrap_or(false)))
                .collect()
        })
        .unwrap_or_default()
}

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
    pub fixes: Vec<(String, String)>,
}

pub fn autocorrect(line: &str, cwd: &Path) -> Option<Correction> {
    let spans = tokens(line);
    if spans.is_empty() {
        return None;
    }
    let words: Vec<&str> = spans.iter().map(|&(s, e)| &line[s..e]).collect();
    let mut edits: Vec<(usize, String)> = Vec::new();

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

    let drop_flags = program == "dir" && first_lower != "dir";

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
        script(1, &["js", "mjs", "cjs", "jsx"], &mut edits);
    } else if program == "tsx" {
        script(1, &["ts", "tsx", "mts", "cts"], &mut edits);
    } else if program == "php" {
        script(1, &["php"], &mut edits);
    } else if program == "ruby" {
        script(1, &["rb"], &mut edits);
    } else if program == "lua" {
        script(1, &["lua"], &mut edits);
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

struct CompletionSet {
    start: usize,
    dir_part: String,
    names: Vec<(String, bool)>,
}

fn completion_entries(line: &str, cwd: &Path) -> Option<CompletionSet> {
    if line.trim().is_empty() {
        return None;
    }
    let start = line.rfind(' ').map_or(0, |i| i + 1);
    let word = line[start..].trim_start_matches('"');
    let (dir_part, prefix) = match word.rfind(['/', '\\']) {
        Some(i) => (&word[..=i], &word[i + 1..]),
        None => ("", word),
    };
    let first_cmd = line[..start].split_whitespace().next().map(program_name).unwrap_or_default();
    let dirs_only = start > 0 && matches!(first_cmd.as_str(), "cd" | "chdir");
    let lower = prefix.to_lowercase();
    let mut names: Vec<(String, bool)> = entries(&cwd.join(dir_part))
        .into_iter()
        .filter(|(n, is_dir)| {
            (!dirs_only || *is_dir)
                && n.to_lowercase().starts_with(&lower)
                && (lower.starts_with('.') || !n.starts_with('.'))
        })
        .collect();
    if start == 0 && dir_part.is_empty() && !prefix.is_empty() {
        names.extend(KNOWN.iter().filter(|k| k.starts_with(&lower)).map(|k| (k.to_string(), false)));
    }
    names.sort_by_key(|a| a.0.to_lowercase());
    names.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
    Some(CompletionSet { start, dir_part: dir_part.to_string(), names })
}

fn format_completion(dir_part: &str, name: &str, dir: bool, done: bool) -> String {
    let sep = if cfg!(windows) { "\\" } else { "/" };
    let mut s = format!("{dir_part}{name}{}", if dir && done { sep } else { "" });
    if s.contains(' ') {
        s = format!("\"{s}{}", if done && !dir { "\"" } else { "" });
    }
    s
}

/// Full replacement lines for inline Tab cycling; empty when nothing should complete.
pub fn complete_candidates(line: &str, cwd: &Path) -> Vec<String> {
    let Some(set) = completion_entries(line, cwd) else {
        return Vec::new();
    };
    let single = set.names.len() == 1;
    set.names
        .into_iter()
        .map(|(n, d)| {
            let tail = if single && !d { " " } else { "" };
            format!("{}{}{}", &line[..set.start], format_completion(&set.dir_part, &n, d, true), tail)
        })
        .collect()
}


fn is_ident_path(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

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

const CODE_EXTS: &[&str] = &[
    ".py", ".pyw", ".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".mts", ".cts",
    ".rs", ".go", ".java", ".kt", ".kts", ".scala", ".groovy", ".gradle",
    ".c", ".h", ".cc", ".cpp", ".cxx", ".hpp", ".hh", ".m", ".mm",
    ".cs", ".fs", ".vb", ".zig", ".php", ".rb", ".lua", ".dart", ".swift",
    ".pl", ".pm", ".ex", ".exs", ".erl", ".hs", ".ml", ".nim", ".r",
    ".sh", ".bash", ".zsh", ".ps1", ".psm1", ".bat", ".cmd",
    ".vue", ".svelte", ".astro", ".html", ".htm", ".css", ".scss", ".less",
    ".json", ".jsonc", ".toml", ".yml", ".yaml", ".xml", ".sql", ".graphql",
    ".mcfunction", ".molang", ".glsl", ".wgsl", ".hlsl", ".proto", ".nix",
];

pub fn lang_from_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "py" | "pyw" => "python",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "rs" => "rust",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" => "c/c++",
        "cs" | "fs" | "vb" => "csharp",
        "zig" => "zig",
        "php" => "php",
        "rb" => "ruby",
        "lua" => "lua",
        "dart" => "dart",
        "swift" => "swift",
        "pl" | "pm" => "perl",
        "ex" | "exs" => "elixir",
        "sh" | "bash" | "zsh" => "shell",
        "ps1" | "psm1" => "powershell",
        "vue" => "vue",
        "svelte" => "svelte",
        "json" | "jsonc" => "json",
        "toml" => "toml",
        "yml" | "yaml" => "yaml",
        "sql" => "sql",
        "mcfunction" => "bedrock",
        _ => "runtime",
    }
}

fn parse_u32_prefix(s: &str) -> Option<(u32, &str)> {
    let n = s.bytes().take_while(|b| b.is_ascii_digit()).count();
    if n == 0 {
        return None;
    }
    let val = s[..n].parse::<u32>().ok()?;
    (val > 0).then_some((val, &s[n..]))
}

fn parse_line_col_after(after: &str, delim: u8) -> Option<(u32, u32)> {
    match delim {
        b':' => {
            if let Some(rest) = after.strip_prefix("line ") {
                let (line, _) = parse_u32_prefix(rest)?;
                return Some((line, 0));
            }
            if let Some(rest) = after.strip_prefix(" line ") {
                let (line, _) = parse_u32_prefix(rest)?;
                return Some((line, 0));
            }
            if let Some(rest) = after.strip_prefix('[') {
                let (line, rest) = parse_u32_prefix(rest)?;
                let col = rest.strip_prefix(',').and_then(parse_u32_prefix).map_or(0, |(c, _)| c);
                return Some((line, col));
            }
            let (line, rest) = parse_u32_prefix(after)?;
            if let Some(after_col) = rest.strip_prefix(':') {
                if let Some((col, _)) = parse_u32_prefix(after_col) {
                    return Some((line, col));
                }
            }
            if let Some(after_char) = rest.strip_prefix(" char:") {
                if let Some((col, _)) = parse_u32_prefix(after_char) {
                    return Some((line, col));
                }
            }
            Some((line, 0))
        }
        b'(' => {
            let (line, rest) = parse_u32_prefix(after)?;
            let col = rest.strip_prefix(',').and_then(parse_u32_prefix).map_or(0, |(c, _)| c);
            Some((line, col))
        }
        b' ' => {
            let rest = after.strip_prefix("on line ")?;
            let (line, _) = parse_u32_prefix(rest)?;
            Some((line, 0))
        }
        _ => None,
    }
}

fn clean_extracted_path(raw: &str) -> Option<String> {
    let mut s = raw.trim().trim_matches(|c| matches!(c, '"' | '\'' | '(' | '[' | '<'));
    if let Some(rest) = s.strip_prefix("file:///") {
        s = rest;
    } else if let Some(rest) = s.strip_prefix("file://") {
        if !rest.starts_with('/') {
            return Some(format!("//{rest}"));
        }
        s = rest;
    }
    let lower = s.to_ascii_lowercase();
    if s.is_empty()
        || s.starts_with('<')
        || lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("ws://")
        || lower.starts_with("wss://")
        || lower.starts_with("ftp://")
        || lower.starts_with("node:")
    {
        return None;
    }
    if s.len() > 3 && s.starts_with('/') && s.as_bytes()[2] == b':' {
        s = &s[1..];
    }
    Some(s.to_string())
}

fn extract_path_before(t: &str, path_end: usize) -> Option<String> {
    let prefix = &t[..path_end];
    if let Some(i) = prefix.rfind("file://") {
        return clean_extracted_path(&prefix[i..]);
    }
    // Check if enclosed in quotes or parens right before the path (e.g. `at fn (C:\a b\x.js:1:2)` or `(App.java:19)`).
    if let Some(i) = prefix.rfind(['(', '"', '\'']) {
        let opener = prefix.as_bytes()[i];
        let prev_ok = opener == b'(' || i == 0 || !prefix.as_bytes()[i - 1].is_ascii_alphanumeric();
        let cand = &prefix[i + 1..];
        if prev_ok && !cand.is_empty() && !cand.starts_with(char::is_whitespace) && !cand.contains([')', ']', '"', '\'']) {
            return clean_extracted_path(cand);
        }
    }
    // Windows drive letter (`C:\` or `C:/`) preserves paths with spaces.
    let bytes = prefix.as_bytes();
    for i in (0..bytes.len().saturating_sub(2)).rev() {
        if bytes[i].is_ascii_alphabetic()
            && bytes[i + 1] == b':'
            && matches!(bytes[i + 2], b'\\' | b'/')
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
        {
            if i >= 1 && bytes[i - 1] == b'/' && (i == 1 || bytes[i - 2].is_ascii_whitespace()) {
                return clean_extracted_path(&prefix[i - 1..]);
            }
            return clean_extracted_path(&prefix[i..]);
        }
    }
    // Known labels that directly precede a relative or Unix path.
    for marker in ["panicked at ", " in ", " at ", "At ", "from ", "--> ", "e: ", "w: ", "[ERROR] ", "[WARNING] ", "lua: ", "luajit: "] {
        if let Some(i) = prefix.rfind(marker) {
            let cand = prefix[i + marker.len()..].trim_start();
            if !cand.is_empty() && !cand.contains("://") {
                return clean_extracted_path(cand);
            }
        }
    }
    // Walk back to last whitespace or bracket delimiter.
    let start = prefix.rfind(|c: char| c.is_whitespace() || matches!(c, '(' | '[' | '<' | '"' | '\'' | ',' | ';')).map_or(0, |i| i + 1);
    let cand = &prefix[start..];
    if cand.contains("://") {
        return None;
    }
    clean_extracted_path(cand)
}

/// Extract `(path, line, col)` from any language's error or stack-trace line (`col` is 1-based, 0 if unknown).
pub fn parse_path_loc(text: &str) -> Option<(String, u32, u32)> {
    if let Some((p, l, _)) = parse_frame(text) {
        if !p.starts_with('<') {
            return Some((p, l, 0));
        }
    }
    let mut t = text.trim();
    for prefix in ["--> ", "e: ", "w: ", "[ERROR] ", "[WARNING] "] {
        t = t.strip_prefix(prefix).unwrap_or(t).trim_start();
    }
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    let mut best: Option<(usize, usize, u32, u32)> = None;
    for ext in CODE_EXTS {
        let mut from = 0;
        while let Some(k) = lower[from..].find(ext) {
            let i = from + k;
            let end = i + ext.len();
            if let Some(&delim) = t.as_bytes().get(end) {
                if matches!(delim, b':' | b'(' | b' ') {
                    if let Some((line, col)) = parse_line_col_after(&t[end + 1..], delim) {
                        if best.is_none_or(|(b, _, _, _)| i < b) {
                            best = Some((i, end, line, col));
                        }
                        break;
                    }
                }
            }
            from = i + 1;
        }
    }
    let (_, path_end, line, col) = best?;
    let path = extract_path_before(t, path_end)?;
    Some((path, line, col))
}

pub fn parse_path_line(text: &str) -> Option<(String, u32)> {
    parse_path_loc(text).map(|(p, l, _)| (p, l))
}

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
    if indented
        && (t.starts_with("at ") || t.starts_with("from ") || t.contains(".go:"))
        && parse_path_loc(t).is_some()
    {
        return LineKind::Frame;
    }
    if python_error_line(text).is_some() {
        return LineKind::Err;
    }
    let lower = t.to_ascii_lowercase();
    if t.starts_with("> Task ") || (t.starts_with("[INFO] ") && !t.contains("BUILD")) {
        return LineKind::Dim;
    }
    const GRADLE_NOISE: &[&str] = &[
        "Consider enabling configuration cache",
        "Deprecated Gradle features were used",
        "You can use '--warning-mode all'",
        "For more on this, please refer to",
        "Starting a Gradle Daemon",
        "To honour the JVM settings",
        "Reusing configuration cache",
        "Configuration cache entry",
        "Calculating task graph",
        "* Try:",
        "> Run with --",
        "> Get more help at",
    ];
    if GRADLE_NOISE.iter().any(|p| t.starts_with(p)) || (t.contains(" actionable task") && t.contains(": ")) {
        return LineKind::Dim;
    }
    if t.starts_with("--> ") && parse_path_loc(t).is_some() {
        return if prev == Some(LineKind::Warn) { LineKind::Warn } else { LineKind::Frame };
    }
    if t.starts_with("BUILD SUCCESSFUL")
        || t.starts_with("[INFO] BUILD SUCCESS")
        || t.starts_with("Build succeeded")
        || t.starts_with("Finished `")
        || t.starts_with("test result: ok.")
    {
        return LineKind::Success;
    }
    let has_loc = parse_path_loc(t).is_some();
    if t.starts_with("FAILURE:")
        || t.starts_with("BUILD FAILED")
        || t.starts_with("[ERROR]")
        || t.starts_with("[INFO] BUILD FAILURE")
        || t.starts_with("e: ")
        || t.starts_with("Uncaught ")
        || t.starts_with("Exception in thread ")
        || t.starts_with("Caused by: ")
        || t.starts_with("Unhandled exception")
        || t.starts_with("panic:")
        || t.starts_with("PHP Fatal error:")
        || t.starts_with("PHP Parse error:")
        || (t.starts_with("thread '") && t.contains("panicked at"))
        || lower.starts_with("error[e")
        || (has_loc
            && (lower.contains(": error")
                || lower.contains(": fatal error")
                || lower.contains("): error ")
                || lower.contains(" - error ts")
                || lower.starts_with("lua:")
                || lower.starts_with("luajit:")
                || (!indented && (t.contains(":in `") || t.contains(":in '") || t.contains(": line ")))))
    {
        return LineKind::Err;
    }
    if t.starts_with("[WARNING]")
        || t.starts_with("w: ")
        || t.starts_with("PHP Warning:")
        || lower.starts_with("warning[")
        || (has_loc && (lower.contains(": warning") || lower.contains("): warning ")))
    {
        return LineKind::Warn;
    }
    if lower.starts_with("error:")
        || lower.starts_with("fatal:")
        || lower.starts_with("fatal error:")
        || lower.starts_with("npm err!")
        || lower.contains("is not recognized as an internal or external command")
        || lower.contains("is not recognized as the name of a cmdlet")
        || lower.ends_with(": command not found")
        || (prev == Some(LineKind::Err) && lower.starts_with("operable program or batch file"))
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


#[derive(Debug, Clone, PartialEq)]
pub struct PyError {
    pub kind: String,
    pub message: String,
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
    pub col: Option<u32>,
    pub source: &'static str,
    pub caret: Option<(usize, usize)>,
    pub code: Option<String>,
    pub hint: Option<String>,
    pub fix: Option<String>,
}

fn is_user_frame(path: &str) -> bool {
    let p = path.to_ascii_lowercase().replace('/', "\\");
    let stdlib = p.contains("\\lib\\") && p.split('\\').any(|c| c.starts_with("python"));
    !(p.starts_with('<')
        || p.starts_with("node:")
        || p.contains("site-packages")
        || p.contains("dist-packages")
        || p.contains("\\node_modules\\")
        || p.contains("\\.cargo\\registry\\")
        || p.contains("\\rustc\\")
        || p.contains("\\go\\src\\")
        || stdlib)
}

fn resolve_err_path(raw: &str, cwd: &Path) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() { p } else { cwd.join(p) }
}

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

pub fn is_shell_not_found_line(text: &str) -> bool {
    let l = text.trim().to_ascii_lowercase();
    l.contains("is not recognized as an internal or external command")
        || l.contains("is not recognized as the name of a cmdlet")
        || l.ends_with(": command not found")
        || l.starts_with("operable program or batch file")
}

fn advise(kind: &str, msg: &str, python: &str, cwd: &Path) -> (Option<String>, Option<String>) {
    let short = kind.rsplit('.').next().unwrap_or(kind);
    let between = |s: &str| s.split('\'').nth(1).map(str::to_string);
    match short {
        "ModuleNotFoundError" => {
            let Some(module) = between(msg) else { return (None, None) };
            if module == "tkinter" || module == "_tkinter" {
                return (Some("Reinstall Python with tcl/tk enabled.".into()), None);
            }
            let root = module.split('.').next().unwrap_or(&module).to_string();
            let pkg = pip_package(&root).map(str::to_string).unwrap_or(root.clone());
            (
                Some(format!("Missing package '{root}'.")),
                Some(format!("{} -m pip install {pkg}", quoted(python))),
            )
        }
        "ImportError" if msg.contains("cannot import name") => {
            (Some("Symbol not found in module.".into()), None)
        }
        "NameError" => (
            Some(if msg.contains("Did you mean") {
                "Undefined name; use the suggested spelling.".into()
            } else {
                "Undefined name; check spelling or imports.".into()
            }),
            None,
        ),
        "SyntaxError" => {
            let m = msg.to_ascii_lowercase();
            let h = if m.contains("was never closed") {
                "Unclosed bracket or quote."
            } else if m.contains("unterminated string") || m.contains("eol while scanning") {
                "Unterminated string."
            } else if m.contains("expected ':'") {
                "Missing ':' at end of header."
            } else if m.contains("forgot a comma") {
                "Missing comma between items."
            } else if m.contains("invalid character") {
                "Invalid character (check pasted quotes or dashes)."
            } else if m.contains("'(' was never closed") || m.contains("unmatched") {
                "Mismatched brackets."
            } else {
                "Invalid syntax near '^'."
            };
            (Some(h.into()), None)
        }
        "IndentationError" | "TabError" => (
            Some("Inconsistent indentation; use 4 spaces and don't mix tabs.".into()),
            None,
        ),
        "TypeError" => {
            let h = if msg.contains("can only concatenate str") || msg.contains("must be str, not") {
                "Can't join str with non-str; use str(x) or an f-string."
            } else if msg.contains("unsupported operand") && msg.contains("'str'") {
                "String used in numeric op; convert with int(x) or float(x)."
            } else if msg.contains("required positional argument") {
                "Missing required argument."
            } else if msg.contains("takes") && msg.contains("positional argument") {
                "Too many arguments (methods need self)."
            } else if msg.contains("'NoneType'") {
                "Value is None."
            } else if msg.contains("not callable") {
                "Object is not callable."
            } else {
                "Wrong type for this operation."
            };
            (Some(h.into()), None)
        }
        "AttributeError" => (
            Some(if msg.contains("'NoneType'") {
                "Value is None.".into()
            } else {
                "Attribute not found; check spelling.".into()
            }),
            None,
        ),
        "KeyError" => (Some(format!("Missing key {msg} (or use .get()).")), None),
        "IndexError" => (Some("Index out of range (valid: 0..len - 1).".into()), None),
        "ValueError" if msg.contains("invalid literal for int()") || msg.contains("could not convert string to float") => {
            (Some("Not a valid number.".into()), None)
        }
        "ValueError" => (Some("Value rejected by function.".into()), None),
        "ZeroDivisionError" => (Some("Division by zero.".into()), None),
        "FileNotFoundError" => {
            let near = between(msg).and_then(|f| fix_file(&f, cwd, &[]));
            let h = match near {
                Some(n) => format!("Did you mean '{}'?", unquote(&n)),
                None => format!("Not found in {}", cwd.display()),
            };
            (Some(h), None)
        }
        "RecursionError" => (Some("Infinite recursion; check the base case.".into()), None),
        "UnicodeDecodeError" => (Some("Pass encoding=\"utf-8\" when opening.".into()), None),
        "PermissionError" => (Some("File locked by another process or read-only.".into()), None),
        "TclError" => (Some("Invalid Tk widget option or asset.".into()), None),
        _ => (None, None),
    }
}

pub fn parse_python_error(lines: &[(&str, LineKind)], python: &str, cwd: &Path) -> Option<PyError> {
    let err_ix = lines.iter().rposition(|(t, k)| *k == LineKind::Err && python_error_line(t).is_some())?;
    let (kind, message) = python_error_line(lines[err_ix].0)?;
    if kind == "KeyboardInterrupt" || kind == "SystemExit" {
        return None;
    }
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
        file: pick.map(|f| resolve_err_path(&f.0, cwd)),
        line: pick.map(|f| f.1),
        col: None,
        source: "python",
        caret: pick.and_then(|f| f.3),
        code: pick.and_then(|f| f.2.clone()),
        hint,
        fix,
    })
}

fn parse_js_error(lines: &[(&str, LineKind)], cwd: &Path) -> Option<PyError> {
    let err_ix = lines.iter().position(|(t, _)| {
        let s = t.trim_start();
        let s = s.strip_prefix("Uncaught ").unwrap_or(s);
        python_error_line(s).is_some()
    })?;
    let raw_err = lines[err_ix].0.trim_start();
    let stripped = raw_err.strip_prefix("Uncaught ").unwrap_or(raw_err);
    let (kind, message) = python_error_line(stripped)?;

    let mut frames: Vec<(String, u32, u32)> = Vec::new();
    for (t, _) in &lines[err_ix + 1..] {
        let s = t.trim_start();
        if !s.starts_with("at ") {
            if !s.is_empty() && !frames.is_empty() {
                break;
            }
            continue;
        }
        if let Some(loc) = parse_path_loc(s) {
            frames.push(loc);
        }
    }
    let mut header_loc: Option<(String, u32, u32)> = None;
    let mut code: Option<String> = None;
    let mut caret: Option<(usize, usize)> = None;
    for i in (0..err_ix).rev() {
        let t = lines[i].0;
        if let Some((p, l, c)) = parse_path_loc(t) {
            let lang = lang_from_path(Path::new(&p));
            if matches!(lang, "javascript" | "typescript") {
                header_loc = Some((p, l, c));
                if let Some((code_line, _)) = lines.get(i + 1) {
                    if !code_line.trim().is_empty() && !code_line.trim_start().starts_with("at ") && i + 1 < err_ix {
                        code = Some(code_line.trim().to_string());
                        if let Some((mark, _)) = lines.get(i + 2) {
                            let m = mark.trim();
                            if !m.is_empty() && m.chars().all(|ch| matches!(ch, '^' | '~' | ' ')) {
                                let indent = code_line.len() - code_line.trim_start().len();
                                if let (Some(a), Some(b)) = (mark.find(['^', '~']), mark.rfind(['^', '~'])) {
                                    caret = Some((a.saturating_sub(indent), (b + 1).saturating_sub(indent)));
                                }
                            }
                        }
                    }
                }
                break;
            }
        }
    }
    let has_js_evidence = header_loc.is_some()
        || frames.iter().any(|(p, _, _)| matches!(lang_from_path(Path::new(p)), "javascript" | "typescript"))
        || lines.iter().any(|(t, _)| t.contains("node:internal/") || t.contains("Cannot find module '") || t.contains("Cannot find package '"));
    if !has_js_evidence {
        return None;
    }

    let pick = frames.iter().find(|(p, _, _)| is_user_frame(p)).cloned().or(header_loc).or_else(|| frames.first().cloned());
    let file = pick.as_ref().map(|(p, _, _)| resolve_err_path(p, cwd));
    let source = file.as_deref().map(lang_from_path).filter(|&s| s == "typescript").unwrap_or("javascript");
    let (hint, fix) = js_advise(kind, message);
    Some(PyError {
        kind: kind.to_string(),
        message: message.to_string(),
        file,
        line: pick.as_ref().map(|(_, l, _)| *l),
        col: pick.as_ref().and_then(|(_, _, c)| (*c > 0).then_some(*c)),
        source,
        caret,
        code,
        hint,
        fix,
    })
}

fn js_advise(kind: &str, msg: &str) -> (Option<String>, Option<String>) {
    if msg.contains("Cannot find module '") || msg.contains("Cannot find package '") {
        if let Some(pkg) = msg.split('\'').nth(1) {
            if !pkg.starts_with('.') && !pkg.starts_with('/') && !pkg.contains('\\') {
                let root = if pkg.starts_with('@') {
                    pkg.splitn(3, '/').take(2).collect::<Vec<_>>().join("/")
                } else {
                    pkg.split('/').next().unwrap_or(pkg).to_string()
                };
                return (
                    Some(format!("Missing package '{root}'.")),
                    Some(format!("npm install {root}")),
                );
            }
            return (Some("Check the import path or extension.".into()), None);
        }
    }
    let hint = match kind {
        "ReferenceError" => Some("Undefined identifier; check spelling or imports.".into()),
        "TypeError" if msg.contains("Cannot read properties of undefined") || msg.contains("Cannot read properties of null") => {
            Some("Value is null or undefined; guard it or use '?.'.".into())
        }
        "TypeError" if msg.contains("is not a function") => {
            Some("Target is not callable; check the import or type.".into())
        }
        "SyntaxError" => Some("Invalid syntax (brackets, commas, or ESM vs CJS).".into()),
        _ => None,
    };
    (hint, None)
}

fn parse_rust_error(lines: &[(&str, LineKind)], cwd: &Path) -> Option<PyError> {
    for (i, (t, _)) in lines.iter().enumerate() {
        let s = t.trim_start();
        if s.starts_with("thread '") && s.contains("panicked at") {
            let loc = parse_path_loc(s);
            let msg = if let Some(after) = s.split("panicked at '").nth(1) {
                after.split('\'').next().unwrap_or("").to_string()
            } else {
                lines.get(i + 1).map(|(m, _)| m.trim().to_string()).unwrap_or_default()
            };
            let (file, line, col) = match loc {
                Some((p, l, c)) => (Some(resolve_err_path(&p, cwd)), Some(l), (c > 0).then_some(c)),
                None => (None, None, None),
            };
            return Some(PyError {
                kind: "Rust panic".into(),
                message: if msg.is_empty() { s.to_string() } else { msg },
                file,
                line,
                col,
                source: "rust",
                caret: None,
                code: None,
                hint: None,
                fix: None,
            });
        }
        let lower = s.to_ascii_lowercase();
        if lower.starts_with("error[e") || (lower.starts_with("error:") && lines.get(i + 1).is_some_and(|(n, _)| n.trim_start().starts_with("--> "))) {
            let (kind, msg) = match s.find(':') {
                Some(c) => {
                    let head = &s[..c];
                    let code_id = head.strip_prefix("error[").and_then(|r| r.strip_suffix(']')).unwrap_or("Rust error");
                    (code_id.to_string(), s[c + 1..].trim().to_string())
                }
                None => ("Rust error".into(), s.to_string()),
            };
            let mut loc = None;
            let mut code = None;
            for (next, _) in lines[i + 1..].iter().take(6) {
                let ns = next.trim_start();
                if loc.is_none() && ns.starts_with("--> ") {
                    loc = parse_path_loc(ns);
                } else if code.is_none() && ns.contains(" | ") {
                    if let Some((num, src)) = ns.split_once(" | ") {
                        if !num.trim().is_empty() && num.trim().chars().all(|c| c.is_ascii_digit()) {
                            code = Some(src.trim().to_string());
                        }
                    }
                }
            }
            if let Some((p, l, c)) = loc {
                return Some(PyError {
                    kind,
                    message: msg,
                    file: Some(resolve_err_path(&p, cwd)),
                    line: Some(l),
                    col: (c > 0).then_some(c),
                    source: "rust",
                    caret: None,
                    code,
                    hint: None,
                    fix: None,
                });
            }
        }
    }
    None
}

fn parse_go_error(lines: &[(&str, LineKind)], cwd: &Path) -> Option<PyError> {
    if let Some(i) = lines.iter().position(|(t, _)| t.trim_start().starts_with("panic:")) {
        let msg = lines[i].0.trim_start().strip_prefix("panic:").unwrap_or("").trim().to_string();
        let frame = lines[i + 1..]
            .iter()
            .filter_map(|(t, _)| parse_path_loc(t))
            .filter(|(p, _, _)| p.to_ascii_lowercase().ends_with(".go"))
            .find(|(p, _, _)| is_user_frame(p));
        let (file, line, col) = match frame {
            Some((p, l, c)) => (Some(resolve_err_path(&p, cwd)), Some(l), (c > 0).then_some(c)),
            None => (None, None, None),
        };
        return Some(PyError {
            kind: "Go panic".into(),
            message: msg,
            file,
            line,
            col,
            source: "go",
            caret: None,
            code: None,
            hint: None,
            fix: None,
        });
    }
    for (t, _) in lines {
        let s = t.trim_start();
        if s.len() != t.len() {
            continue;
        }
        if let Some((p, l, c)) = parse_path_loc(s) {
            if p.to_ascii_lowercase().ends_with(".go") {
                if let Some(idx) = s.find(&format!(":{l}")) {
                    let after = &s[idx + 1 + l.to_string().len()..];
                    let msg = after.trim_start_matches(|ch: char| ch == ':' || ch.is_ascii_digit()).trim();
                    if !msg.is_empty() && !msg.starts_with("+0x") {
                        return Some(PyError {
                            kind: "Go error".into(),
                            message: msg.to_string(),
                            file: Some(resolve_err_path(&p, cwd)),
                            line: Some(l),
                            col: (c > 0).then_some(c),
                            source: "go",
                            caret: None,
                            code: None,
                            hint: None,
                            fix: None,
                        });
                    }
                }
            }
        }
    }
    None
}

fn parse_jvm_error(lines: &[(&str, LineKind)], cwd: &Path) -> Option<PyError> {
    let err_ix = lines.iter().rposition(|(t, _)| {
        let s = t.trim_start();
        s.starts_with("Exception in thread ") || s.starts_with("Caused by: ")
    })?;
    let raw = lines[err_ix].0.trim_start();
    let rest = if let Some(r) = raw.strip_prefix("Caused by: ") {
        r
    } else if let Some(i) = raw.find("\" ") {
        &raw[i + 2..]
    } else {
        raw
    };
    let (kind_full, msg) = match rest.find(':') {
        Some(i) => (rest[..i].trim(), rest[i + 1..].trim()),
        None => (rest.trim(), ""),
    };
    let kind = kind_full.rsplit('.').next().unwrap_or(kind_full);
    let frames: Vec<(String, u32, u32)> = lines[err_ix + 1..]
        .iter()
        .take_while(|(t, _)| t.trim_start().starts_with("at "))
        .filter_map(|(t, _)| parse_path_loc(t))
        .collect();
    let pick = frames.iter().find(|(p, _, _)| is_user_frame(p)).or(frames.first());
    let file = pick.map(|(p, _, _)| resolve_err_path(p, cwd));
    let source = file.as_deref().map(lang_from_path).unwrap_or("java");
    Some(PyError {
        kind: kind.to_string(),
        message: msg.to_string(),
        file,
        line: pick.map(|(_, l, _)| *l),
        col: pick.and_then(|(_, _, c)| (*c > 0).then_some(*c)),
        source,
        caret: None,
        code: None,
        hint: None,
        fix: None,
    })
}

fn parse_dotnet_error(lines: &[(&str, LineKind)], cwd: &Path) -> Option<PyError> {
    let err_ix = lines.iter().position(|(t, _)| t.trim_start().starts_with("Unhandled exception"))?;
    let raw = lines[err_ix].0.trim_start();
    let rest = raw.strip_prefix("Unhandled exception.").or_else(|| raw.strip_prefix("Unhandled exception:")).unwrap_or(raw).trim();
    let (kind_full, msg) = match rest.find(':') {
        Some(i) => (rest[..i].trim(), rest[i + 1..].trim()),
        None => (rest, ""),
    };
    let kind = kind_full.rsplit('.').next().unwrap_or(kind_full);
    let pick = lines[err_ix + 1..].iter().filter_map(|(t, _)| parse_path_loc(t)).find(|(p, _, _)| is_user_frame(p));
    Some(PyError {
        kind: kind.to_string(),
        message: msg.to_string(),
        file: pick.as_ref().map(|(p, _, _)| resolve_err_path(p, cwd)),
        line: pick.as_ref().map(|(_, l, _)| *l),
        col: pick.as_ref().and_then(|(_, _, c)| (*c > 0).then_some(*c)),
        source: "csharp",
        caret: None,
        code: None,
        hint: None,
        fix: None,
    })
}

fn parse_generic_compiler_error(lines: &[(&str, LineKind)], cwd: &Path) -> Option<PyError> {
    // PowerShell: message line followed by `At path.ps1:N char:C`
    for (i, (t, _)) in lines.iter().enumerate() {
        let s = t.trim_start();
        if s.starts_with("At ") && s.contains(" char:") {
            if let Some((p, l, c)) = parse_path_loc(s) {
                let msg = lines[..i].iter().rev().map(|(x, _)| x.trim()).find(|x| !x.is_empty()).unwrap_or("").to_string();
                return Some(PyError {
                    kind: "PowerShell error".into(),
                    message: msg,
                    file: Some(resolve_err_path(&p, cwd)),
                    line: Some(l),
                    col: (c > 0).then_some(c),
                    source: "powershell",
                    caret: None,
                    code: None,
                    hint: None,
                    fix: None,
                });
            }
        }
    }
    // Single-line compiler or scripting errors across C/C++, C#, TS (tsc), Java, Kotlin, Zig, Dart, Swift, PHP, Ruby, Lua, Shell.
    for (t, k) in lines {
        let s = t.trim();
        let Some((p, l, c)) = parse_path_loc(s) else { continue };
        let lower = s.to_ascii_lowercase();
        let is_err = *k == LineKind::Err
            || lower.contains(": error")
            || lower.contains(": fatal error")
            || lower.contains("): error ")
            || lower.contains(" - error ts")
            || s.starts_with("PHP Fatal error:")
            || s.starts_with("PHP Parse error:")
            || s.starts_with("e: ")
            || s.starts_with("[ERROR]");
        if !is_err {
            continue;
        }
        let file = resolve_err_path(&p, cwd);
        let source = lang_from_path(&file);
        let (kind, msg) = extract_compiler_kind_msg(s, source);
        return Some(PyError {
            kind,
            message: msg,
            file: Some(file),
            line: Some(l),
            col: (c > 0).then_some(c),
            source,
            caret: None,
            code: None,
            hint: None,
            fix: None,
        });
    }
    None
}

fn extract_compiler_kind_msg(line: &str, source: &str) -> (String, String) {
    let lower = line.to_ascii_lowercase();
    for marker in ["): error ", " - error ", ": fatal error:", ": error:", ": error "] {
        if let Some(i) = lower.find(marker) {
            let after = line[i + marker.len()..].trim();
            // Extract compiler code like `TS2304:`, `CS1002:`, `C2065:` when present.
            if let Some((code, rest)) = after.split_once(':') {
                let code_t = code.trim();
                if !code_t.is_empty() && code_t.len() <= 12 && code_t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && code_t.chars().any(|c| c.is_ascii_digit()) {
                    return (code_t.to_string(), rest.trim().to_string());
                }
            }
            return (lang_error_label(source), after.to_string());
        }
    }
    if let Some(rest) = line.strip_prefix("PHP Fatal error:").or_else(|| line.strip_prefix("PHP Parse error:")) {
        let msg = rest.split(" in ").next().unwrap_or(rest).trim();
        return ("PHP error".into(), msg.to_string());
    }
    if let Some(i) = line.find(":in `").or_else(|| line.find(":in '")) {
        if let Some(after) = line[i..].split_once(": ") {
            return ("Ruby error".into(), after.1.trim().to_string());
        }
    }
    if let Some(i) = line.find(": line ") {
        if let Some(after) = line[i + 7..].split_once(": ") {
            return ("Shell error".into(), after.1.trim().to_string());
        }
    }
    (lang_error_label(source), line.to_string())
}

fn lang_error_label(source: &str) -> String {
    match source {
        "typescript" => "TypeScript error",
        "javascript" => "JavaScript error",
        "rust" => "Rust error",
        "go" => "Go error",
        "java" => "Java error",
        "kotlin" => "Kotlin error",
        "c/c++" => "C/C++ error",
        "csharp" => "C# error",
        "zig" => "Zig error",
        "php" => "PHP error",
        "ruby" => "Ruby error",
        "lua" => "Lua error",
        "dart" => "Dart error",
        "swift" => "Swift error",
        "shell" => "Shell error",
        "powershell" => "PowerShell error",
        _ => "Error",
    }
    .to_string()
}

pub fn parse_problem(lines: &[(&str, LineKind)], command: &str, python: &str, cwd: &Path) -> Option<PyError> {
    let has_py_frame = lines.iter().any(|(t, _)| parse_frame(t).is_some() || t.trim_start().starts_with("Traceback (most recent call last)"));
    let first_word = command.split_whitespace().next().unwrap_or("");
    if has_py_frame || is_python(first_word) {
        if let Some(e) = parse_python_error(lines, python, cwd) {
            return Some(e);
        }
    }
    if let Some(e) = parse_js_error(lines, cwd)
        .or_else(|| parse_rust_error(lines, cwd))
        .or_else(|| parse_jvm_error(lines, cwd))
        .or_else(|| parse_dotnet_error(lines, cwd))
        .or_else(|| parse_go_error(lines, cwd))
        .or_else(|| parse_generic_compiler_error(lines, cwd))
    {
        return Some(e);
    }
    if let Some(e) = parse_python_error(lines, python, cwd) {
        return Some(e);
    }
    let card = |kind: &str, message: String, hint: Option<String>, fix: Option<String>| PyError {
        kind: kind.into(),
        message,
        file: None,
        line: None,
        col: None,
        source: "runtime",
        caret: None,
        code: None,
        hint,
        fix,
    };
    if let Some((t, _)) = lines.iter().find(|(t, _)| t.contains(": can't open file '")) {
        let quoted_path = t.split('\'').nth(1).unwrap_or("").replace("\\\\", "\\");
        let name = Path::new(&quoted_path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(quoted_path);
        let near = fix_file(&name, cwd, &["py", "pyw"]);
        let fix = near.as_ref().map(|n| command.replacen(&name, unquote(n), 1)).filter(|f| f != command);
        let hint = near.as_ref().map(|n| format!("Did you mean '{}'?", unquote(n)));
        return Some(card("Not found", format!("'{name}' not found"), hint, fix));
    }
    if let Some((t, _)) = lines.iter().find(|(t, _)| {
        is_shell_not_found_line(t) && !t.trim().to_ascii_lowercase().starts_with("operable program or batch file")
    }) {
        let name = if let Some(n) = t.split('\'').nth(1).filter(|s| !s.is_empty()) {
            n.to_string()
        } else {
            t.trim().split(':').rev().nth(1).unwrap_or("").trim().to_string()
        };
        let near = closest(&name, KNOWN.iter().copied()).filter(|k| on_path(k) || matches!(*k, "cd" | "dir" | "cls" | "type" | "echo"));
        let fix = near.map(|n| command.replacen(&name, n, 1));
        let msg = if near.is_none() && matches!(name.as_str(), "python" | "python3" | "pip" | "pip3") {
            format!("'{name}' not found (try 'py')")
        } else {
            format!("'{name}' not found")
        };
        let hint = near.map(|n| format!("Did you mean '{n}'?"));
        return Some(card("Not found", msg, hint, fix));
    }
    None
}

pub fn failure_reason(lines: &[(&str, LineKind)]) -> Option<String> {
    let generic = |t: &str| {
        let t = t.trim_start();
        t.starts_with("BUILD FAILED") || t.starts_with("FAILURE:") || t.starts_with("[INFO] BUILD FAILURE") || t.starts_with("[ERROR] BUILD") || t.starts_with("Traceback")
    };
    let found = lines
        .iter()
        .position(|(t, k)| *k == LineKind::Err && parse_path_line(t).is_some())
        .or_else(|| {
            let i = lines.iter().position(|(t, _)| t.trim_start().starts_with("* What went wrong"))?;
            lines[i + 1..].iter().position(|(t, _)| !t.trim().is_empty()).map(|j| i + 1 + j)
        })
        .or_else(|| lines.iter().rposition(|(t, k)| *k == LineKind::Err && !t.trim().is_empty() && !generic(t)))?;
    let shown = lines.iter().rposition(|(t, k)| !t.trim().is_empty() && *k != LineKind::Blank).unwrap_or(0);
    (found + 3 <= shown).then(|| lines[found].0.trim().to_string())
}

fn home_lower() -> &'static str {
    static HOME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HOME.get_or_init(|| {
        std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map(|h| h.trim_end_matches(['\\', '/']).to_lowercase()).unwrap_or_default()
    })
}

pub fn display_text(text: &str) -> std::borrow::Cow<'_, str> {
    if let Some(rest) = text.strip_prefix("Requirement already satisfied: ") {
        let pkg = rest.split(" in ").next().unwrap_or(rest).trim();
        let ver = text
            .trim_end()
            .strip_suffix(')')
            .and_then(|t| t.rsplit_once('('))
            .map(|(_, v)| v)
            .filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.'));
        return match ver {
            Some(v) => format!("{pkg} {v} is already installed").into(),
            None => format!("{pkg} is already installed").into(),
        };
    }
    let home = home_lower();
    if home.len() < 4 {
        return text.into();
    }
    let lower = text.to_lowercase();
    if lower.len() != text.len() || !lower.contains(home) {
        return text.into();
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (i, _) in lower.match_indices(home) {
        out.push_str(&text[last..i]);
        out.push('~');
        last = i + home.len();
    }
    out.push_str(&text[last..]);
    out.into()
}

pub fn wrap_ranges(s: &str, first: usize, hang: usize) -> Vec<(usize, usize)> {
    let first = first.max(8);
    let rest = first.saturating_sub(hang).max(8);
    let idx: Vec<usize> = s.char_indices().map(|(i, _)| i).chain(std::iter::once(s.len())).collect();
    let n = idx.len() - 1;
    if n <= first {
        return vec![(0, s.len())];
    }
    let mut out = Vec::new();
    let mut start = 0; // char index
    let mut cols = first;
    while n - start > cols {
        let hard = start + cols;
        let chars: Vec<char> = s[idx[start]..idx[hard]].chars().collect();
        let soft = chars.iter().rposition(|c| matches!(c, ' ' | '\\' | '/' | ',' | '-' | '_')).filter(|p| *p >= cols / 3).map(|p| start + p + 1);
        let end = soft.unwrap_or(hard);
        out.push((idx[start], idx[end]));
        start = end;
        cols = rest;
    }
    out.push((idx[start], s.len()));
    out
}

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

pub fn fit_path_left(path: &str, max: usize) -> String {
    let n = path.chars().count();
    if n <= max {
        return path.to_string();
    }
    let max = max.max(2);
    let tail_start = path.char_indices().map(|(i, _)| i).find(|&i| path[i..].chars().count() < max).unwrap_or(path.len());
    let tail = &path[tail_start..];
    match tail.find(['\\', '/']) {
        Some(k) if tail[k..].chars().count() > 1 => format!("…{}", &tail[k..]),
        _ => format!("…{tail}"),
    }
}

pub fn last_folder(path: &str) -> &str {
    path.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().filter(|s| !s.is_empty()).unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_shrink_from_the_left() {
        let p = "~\\Documents\\Project Pribadi\\app";
        assert_eq!(fit_path_left(p, 60), p);
        assert_eq!(fit_path_left(p, 22), "…\\Project Pribadi\\app");
        assert!(fit_path_left(p, 22).chars().count() <= 22);
        assert_eq!(fit_path_left(p, 8), "…\\app");
        assert_eq!(fit_path_left("C:\\Paper-Test-Server", 8).chars().count(), 8);
        assert_eq!(last_folder("~\\Documents\\Project Pribadi"), "Project Pribadi");
        assert_eq!(last_folder("C:\\"), "C:");
        assert_eq!(last_folder("proj"), "proj");
    }

    #[test]
    fn compiler_errors_link_to_the_file() {
        let p = |s: &str| parse_path_line(s);
        assert_eq!(
            p("C:\\x\\src\\Main.java:12: error: ';' expected"),
            Some(("C:\\x\\src\\Main.java".into(), 12))
        );
        assert_eq!(
            p("e: file:///C:/x/Main.kt:7:3 Unresolved reference"),
            Some(("C:/x/Main.kt".into(), 7))
        );
        assert_eq!(
            p("e: file://wsl.localhost/Ubuntu/home/me/Main.kt:7:3 Unresolved reference"),
            Some(("//wsl.localhost/Ubuntu/home/me/Main.kt".into(), 7))
        );
        assert_eq!(p("  --> src/main.rs:3:5"), Some(("src/main.rs".into(), 3)));
        assert_eq!(p("src/a.cpp:40:9: error: expected ';'"), Some(("src/a.cpp".into(), 40)));
        assert_eq!(p("Program.cs(12,5): error CS1002: ; expected"), Some(("Program.cs".into(), 12)));
        assert_eq!(p("[ERROR] /C:/x/App.java:[8,1] class, interface expected"), Some(("C:/x/App.java".into(), 8)));
        assert_eq!(p("Downloading https://repo.maven.apache.org/x.json"), None);
        assert_eq!(classify("C:\\x\\Main.java:12: error: ';' expected", true, None), LineKind::Err);
        assert_eq!(classify("BUILD SUCCESSFUL in 3s", false, None), LineKind::Success);
        assert_eq!(classify("FAILURE: Build failed with an exception.", true, None), LineKind::Err);
        assert_eq!(classify("> Task :compileJava", false, None), LineKind::Dim);
    }

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
    fn friendly_pip_lines() {
        let t = "Requirement already satisfied: customtkinter in c:\\users\\x\\python312\\lib\\site-packages (from -r requirements.txt (line 1)) (6.0.0)";
        assert_eq!(display_text(t), "customtkinter 6.0.0 is already installed");
        assert_eq!(display_text("Requirement already satisfied: pip in c:\\p"), "pip is already installed");
        assert_eq!(display_text("plain line"), "plain line");
    }

    #[test]
    fn wraps_at_folder_boundaries() {
        let s = "C:\\Users\\User\\AppData\\Local\\Programs\\Python\\Python312\\Lib\\site-packages";
        let rows = wrap_ranges(s, 30, 0);
        assert!(rows.len() > 1);
        assert!(rows.iter().all(|(a, b)| s[*a..*b].chars().count() <= 30));
        assert_eq!(rows.iter().map(|(a, b)| &s[*a..*b]).collect::<String>(), s);
        assert!(s[rows[0].0..rows[0].1].ends_with('\\'));
        assert_eq!(wrap_ranges("short", 30, 2), vec![(0, 5)]);
        let jar = "build\\main\\libs\\KiwEssentials-0.1.0-SNAPSHOT.jar";
        let rows = wrap_ranges(jar, 46, 2);
        assert!(rows.iter().all(|(a, b)| b - a > 3));
    }

    #[test]
    fn gradle_housekeeping_is_dim() {
        assert_eq!(classify("55 actionable tasks: 21 executed, 1 from cache, 33 up-to-date", false, None), LineKind::Dim);
        assert_eq!(classify("1 actionable task: 1 executed", false, None), LineKind::Dim);
        assert_eq!(classify("Consider enabling configuration cache to speed up this build: https://docs.gradle.org/x", false, None), LineKind::Dim);
        assert_eq!(classify("> Run with --stacktrace option to get the stack trace.", true, None), LineKind::Dim);
        assert_eq!(classify("> Compilation failed; see the compiler error output for details.", true, None), LineKind::Normal);
        assert_eq!(classify("BUILD SUCCESSFUL in 2s", false, None), LineKind::Success);
    }

    #[test]
    fn says_why_a_build_failed() {
        let k = |t: &'static str| (t, classify(t, true, None));
        let javac = [
            k("C:\\p\\src\\Main.java:12: error: ';' expected"),
            k("        foo()"),
            k("1 error"),
            k(""),
            k("FAILURE: Build failed with an exception."),
            k("* What went wrong:"),
            k("Execution failed for task ':compileJava'."),
            k("BUILD FAILED in 2s"),
        ];
        assert_eq!(failure_reason(&javac).as_deref(), Some("C:\\p\\src\\Main.java:12: error: ';' expected"));
        let gradle = [k("FAILURE: Build failed with an exception."), k(""), k("* What went wrong:"), k("Could not resolve all files."), k(""), k("* Try:"), k("> Run with --info"), k("BUILD FAILED in 1s")];
        assert_eq!(failure_reason(&gradle).as_deref(), Some("Could not resolve all files."));
        assert_eq!(failure_reason(&[k("'foo' is not recognized as an internal or external command,")]), None);
        assert_eq!(classify("operable program or batch file.", true, Some(LineKind::Err)), LineKind::Err);
        assert_eq!(classify("operable program or batch file.", true, Some(LineKind::Normal)), LineKind::Normal);
    }

    #[test]
    fn tab_completion() {
        let d = tmp();
        assert!(complete_candidates("", &d).is_empty());
        assert!(complete_candidates("   ", &d).is_empty());
        assert_eq!(complete_candidates("py ap", &d), vec!["py app.py "]);
        let sep = if cfg!(windows) { "\\" } else { "/" };
        assert_eq!(complete_candidates("cd ik", &d), vec![format!("cd ikon{sep}")]);
        assert_eq!(complete_candidates("cd ", &d), vec![format!("cd ikon{sep}")]);
    }

    #[test]
    fn multilang_path_loc_and_errors() {
        let d = tmp();
        assert_eq!(parse_path_loc("    at loadUser (C:\\proj\\src\\auth.ts:42:13)"), Some(("C:\\proj\\src\\auth.ts".into(), 42, 13)));
        assert_eq!(parse_path_loc("    at file:///C:/proj/index.mjs:18:7"), Some(("C:/proj/index.mjs".into(), 18, 7)));
        assert_eq!(parse_path_loc("    at com.example.App.main(App.java:19)"), Some(("App.java".into(), 19, 0)));
        assert_eq!(parse_path_loc("thread 'main' panicked at src/engine.rs:104:9:"), Some(("src/engine.rs".into(), 104, 9)));
        assert_eq!(parse_path_loc("PHP Parse error: syntax error in C:\\site\\app.php on line 22"), Some(("C:\\site\\app.php".into(), 22, 0)));
        assert_eq!(parse_path_loc("   at App.Program.Main() in C:\\app\\Program.cs:line 19"), Some(("C:\\app\\Program.cs".into(), 19, 0)));
        assert_eq!(parse_path_loc("./deploy.sh: line 14: foo: command not found"), Some(("./deploy.sh".into(), 14, 0)));
        assert_eq!(parse_path_loc("At C:\\scripts\\build.ps1:9 char:4"), Some(("C:\\scripts\\build.ps1".into(), 9, 4)));
        assert_eq!(parse_path_loc("https://example.com:8080/api.json:10"), None);
        assert_eq!(parse_path_loc("    at node:internal/modules/cjs/loader:1148:14"), None);

        let js = [
            ("C:\\proj\\app.js:12", LineKind::Normal),
            ("console.log(foo);", LineKind::Normal),
            ("            ^", LineKind::Normal),
            ("ReferenceError: foo is not defined", LineKind::Err),
            ("    at Object.<anonymous> (C:\\proj\\app.js:12:13)", LineKind::Frame),
        ];
        let e = parse_problem(&js, "node app.js", "py", &d).unwrap();
        assert_eq!(e.kind, "ReferenceError");
        assert_eq!(e.source, "javascript");
        assert_eq!(e.line, Some(12));
        assert_eq!(e.col, Some(13));
        assert_eq!(e.code.as_deref(), Some("console.log(foo);"));

        let js_mod = [
            ("Error: Cannot find module 'express'", LineKind::Err),
            ("    at Function.Module._resolveFilename (node:internal/modules/cjs/loader:1148:15)", LineKind::Frame),
            ("    at Object.<anonymous> (C:\\proj\\server.js:1:17)", LineKind::Frame),
        ];
        let e = parse_problem(&js_mod, "node server.js", "py", &d).unwrap();
        assert_eq!(e.fix.as_deref(), Some("npm install express"));
        assert_eq!(e.line, Some(1));

        let rust = [
            ("error[E0308]: mismatched types", LineKind::Err),
            (" --> src/main.rs:42:9", LineKind::Frame),
            ("42 |     let x: u32 = \"hi\";", LineKind::Normal),
        ];
        let e = parse_problem(&rust, "cargo build", "py", &d).unwrap();
        assert_eq!(e.kind, "E0308");
        assert_eq!(e.source, "rust");
        assert_eq!(e.line, Some(42));
        assert_eq!(e.col, Some(9));

        let go = [("./main.go:24:12: undefined: foo", LineKind::Err)];
        let e = parse_problem(&go, "go run main.go", "py", &d).unwrap();
        assert_eq!(e.source, "go");
        assert_eq!(e.line, Some(24));
        assert_eq!(e.col, Some(12));

        let tsc = [("src/app.ts(15,8): error TS2304: Cannot find name 'bar'.", LineKind::Err)];
        let e = parse_problem(&tsc, "npx tsc", "py", &d).unwrap();
        assert_eq!(e.kind, "TS2304");
        assert_eq!(e.source, "typescript");
        assert_eq!(e.line, Some(15));
        assert_eq!(e.col, Some(8));

        let java = [
            ("Exception in thread \"main\" java.lang.NullPointerException: boom", LineKind::Err),
            ("    at com.example.App.main(App.java:19)", LineKind::Frame),
        ];
        let e = parse_problem(&java, "java App", "py", &d).unwrap();
        assert_eq!(e.kind, "NullPointerException");
        assert_eq!(e.source, "java");
        assert_eq!(e.line, Some(19));

        let php = [("PHP Parse error: syntax error in C:\\site\\app.php on line 22", LineKind::Err)];
        let e = parse_problem(&php, "php app.php", "py", &d).unwrap();
        assert_eq!(e.source, "php");
        assert_eq!(e.line, Some(22));

        let zig = [("src/main.zig:14:9: error: expected type 'u32'", LineKind::Err)];
        let e = parse_problem(&zig, "zig build", "py", &d).unwrap();
        assert_eq!(e.source, "zig");
        assert_eq!(e.line, Some(14));
        assert_eq!(e.col, Some(9));
    }

    #[test]
    fn unknown_command_error_is_concise() {
        let d = tmp();
        let raw = [
            ("'tste' is not recognized as an internal or external command,", LineKind::Err),
            ("operable program or batch file.", LineKind::Err),
        ];
        let e = parse_problem(&raw, "tste", "py", &d).unwrap();
        assert_eq!(e.kind, "Not found");
        assert_eq!(e.message, "'tste' not found");
        assert_eq!(e.hint, None);
        assert_eq!(e.fix, None);
    }
}
