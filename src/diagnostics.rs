use std::collections::HashSet;

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_DIAGS: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeDiag {
    pub row: u32,
    pub col: u32,
    pub end_col: u32,
    pub severity: Severity,
    pub message: String,
    pub source: &'static str,
}

impl CodeDiag {
    fn error(row: u32, col: u32, end_col: u32, message: String, source: &'static str) -> Self {
        Self { row, col, end_col: end_col.max(col + 1), severity: Severity::Error, message, source }
    }

    fn warn(row: u32, col: u32, end_col: u32, message: String, source: &'static str) -> Self {
        Self { row, col, end_col: end_col.max(col + 1), severity: Severity::Warning, message, source }
    }
}

pub fn analyze(text: &str, lang: &str) -> Vec<CodeDiag> {
    if text.len() > MAX_BYTES || text.is_empty() {
        return Vec::new();
    }
    let mut out = match lang {
        "json" => check_json(text),
        "javascript" | "typescript" | "tsx" => check_js_ts(text, lang == "tsx"),
        "python" => check_python(text),
        "html" | "xml" => check_markup(text, lang == "xml"),
        "css" | "scss" | "less" => check_general(text, lang),
        "toml" => check_toml(text),
        "rust" | "go" | "c" | "cpp" | "java" | "cs" | "kt" | "scala" | "swift" | "php" | "dart" | "zig" => check_general(text, lang),
        _ => Vec::new(),
    };
    out.truncate(MAX_DIAGS);
    out
}

struct Scanner<'a> {
    b: &'a [u8],
    i: usize,
    row: u32,
    col: u32,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Self {
        Self { b: text.as_bytes(), i: 0, row: 0, col: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn peek_at(&self, off: usize) -> Option<u8> {
        self.b.get(self.i + off).copied()
    }

    fn at(&self, s: &str) -> bool {
        self.b[self.i..].starts_with(s.as_bytes())
    }

    fn step(&mut self) -> Option<u8> {
        let c = *self.b.get(self.i)?;
        match c {
            b'\n' => {
                self.row += 1;
                self.col = 0;
            }
            c if c & 0xC0 != 0x80 => self.col += 1,
            _ => {}
        }
        self.i += 1;
        Some(c)
    }

    fn skip_line(&mut self) {
        while let Some(c) = self.peek() {
            self.step();
            if c == b'\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self, nested: bool) -> Option<(u32, u32)> {
        let start = (self.row, self.col);
        self.step();
        self.step();
        let mut depth = 1usize;
        while self.i < self.b.len() {
            if nested && self.at("/*") {
                self.step();
                self.step();
                depth += 1;
            } else if self.at("*/") {
                self.step();
                self.step();
                depth -= 1;
                if depth == 0 {
                    return None;
                }
            } else {
                self.step();
            }
        }
        Some(start)
    }

    fn after_ident(&self) -> bool {
        let ident = |j: usize| self.b[j].is_ascii_alphanumeric() || self.b[j] == b'_';
        match self.i {
            0 => false,
            1 => self.b[0] != b'b' && ident(0),
            i if self.b[i - 1] == b'b' => ident(i - 2),
            i => ident(i - 1),
        }
    }
}

fn push_bracket_close(
    open: &mut Vec<(u8, u32, u32)>,
    close: u8,
    row: u32,
    col: u32,
    source: &'static str,
    diags: &mut Vec<CodeDiag>,
) {
    let want = match close {
        b')' => b'(',
        b']' => b'[',
        _ => b'{',
    };
    if let Some(&(o, _, _)) = open.last() {
        if o == want {
            open.pop();
            return;
        }
        if let Some(pos) = open.iter().rposition(|&(ch, _, _)| ch == want) {
            for (unclosed, r, c) in open.drain(pos + 1..) {
                diags.push(CodeDiag::error(
                    r,
                    c,
                    c + 1,
                    crate::trf!("Unclosed {b}"; b => unclosed as char),
                    source,
                ));
            }
            open.pop();
            return;
        }
        diags.push(CodeDiag::error(
            row,
            col,
            col + 1,
            crate::trf!("{b} does not close {open}"; b => close as char, open => o as char),
            source,
        ));
    } else {
        diags.push(CodeDiag::error(
            row,
            col,
            col + 1,
            crate::trf!("Unmatched {b}"; b => close as char),
            source,
        ));
    }
}

fn finish_open_brackets(open: Vec<(u8, u32, u32)>, source: &'static str, diags: &mut Vec<CodeDiag>) {
    for (o, r, c) in open {
        diags.push(CodeDiag::error(
            r,
            c,
            c + 1,
            crate::trf!("Unclosed {b}"; b => o as char),
            source,
        ));
    }
}

struct JsonParser<'a> {
    sc: Scanner<'a>,
    diags: Vec<CodeDiag>,
}

impl<'a> JsonParser<'a> {
    fn new(text: &'a str) -> Self {
        Self { sc: Scanner::new(text), diags: Vec::new() }
    }

    fn err(&mut self, row: u32, col: u32, end_col: u32, msg: String) {
        if self.diags.len() < MAX_DIAGS {
            self.diags.push(CodeDiag::error(row, col, end_col, msg, "JSON"));
        }
    }

    fn skip_ws_comments(&mut self) {
        while let Some(c) = self.sc.peek() {
            if matches!(c, b' ' | b'\t' | b'\r' | b'\n') {
                self.sc.step();
            } else if self.sc.at("//") {
                self.sc.skip_line();
            } else if self.sc.at("/*") {
                if let Some((r, col)) = self.sc.skip_block_comment(false) {
                    self.err(r, col, col + 2, crate::i18n::tr("Unterminated block comment").to_string());
                }
            } else {
                break;
            }
        }
    }

    fn parse_string(&mut self) -> Option<String> {
        let (r0, c0) = (self.sc.row, self.sc.col);
        let q = self.sc.peek()?;
        if q == b'\'' {
            self.sc.step();
            while let Some(c) = self.sc.peek() {
                if c == b'\n' {
                    break;
                }
                self.sc.step();
                if c == b'\\' {
                    self.sc.step();
                } else if c == b'\'' {
                    break;
                }
            }
            self.err(r0, c0, self.sc.col.max(c0 + 1), crate::i18n::tr("JSON strings must use double quotes").to_string());
            return Some(String::new());
        }
        if q != b'"' {
            return None;
        }
        self.sc.step();
        let mut val = String::new();
        while let Some(c) = self.sc.peek() {
            if c == b'"' {
                self.sc.step();
                return Some(val);
            }
            if c == b'\n' || c == b'\r' {
                self.err(r0, c0, self.sc.col.max(c0 + 1), crate::i18n::tr("Unterminated string").to_string());
                return Some(val);
            }
            if c < 0x20 {
                let (r, col) = (self.sc.row, self.sc.col);
                self.sc.step();
                self.err(r, col, col + 1, crate::i18n::tr("Unescaped control character in string").to_string());
                continue;
            }
            if c == b'\\' {
                let (er, ec) = (self.sc.row, self.sc.col);
                self.sc.step();
                match self.sc.peek() {
                    Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                        let ch = self.sc.step().unwrap();
                        val.push(ch as char);
                    }
                    Some(b'u') => {
                        self.sc.step();
                        let mut ok = true;
                        for _ in 0..4 {
                            if self.sc.peek().is_some_and(|h| h.is_ascii_hexdigit()) {
                                self.sc.step();
                            } else {
                                ok = false;
                                break;
                            }
                        }
                        if !ok {
                            self.err(er, ec, self.sc.col.max(ec + 2), crate::i18n::tr("Invalid Unicode escape sequence").to_string());
                        }
                    }
                    Some(_) => {
                        self.sc.step();
                        self.err(er, ec, ec + 2, crate::i18n::tr("Invalid escape sequence in string").to_string());
                    }
                    None => {
                        self.err(r0, c0, ec + 1, crate::i18n::tr("Unterminated string").to_string());
                        return Some(val);
                    }
                }
            } else {
                let start = self.sc.i;
                self.sc.step();
                while self.sc.i < self.sc.b.len() && (self.sc.b[self.sc.i] & 0xC0) == 0x80 {
                    self.sc.step();
                }
                if let Ok(s) = std::str::from_utf8(&self.sc.b[start..self.sc.i]) {
                    val.push_str(s);
                }
            }
        }
        self.err(r0, c0, self.sc.col.max(c0 + 1), crate::i18n::tr("Unterminated string").to_string());
        Some(val)
    }

    fn parse_number(&mut self) {
        let (r0, c0) = (self.sc.row, self.sc.col);
        if self.sc.peek() == Some(b'-') {
            self.sc.step();
        }
        match self.sc.peek() {
            Some(b'0') => {
                self.sc.step();
                if self.sc.peek().is_some_and(|c| c.is_ascii_digit()) {
                    while self.sc.peek().is_some_and(|c| c.is_ascii_digit()) {
                        self.sc.step();
                    }
                    self.err(r0, c0, self.sc.col, crate::i18n::tr("Leading zeros are not allowed in JSON numbers").to_string());
                }
            }
            Some(b'1'..=b'9') => {
                while self.sc.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.sc.step();
                }
            }
            _ => {
                self.err(r0, c0, self.sc.col.max(c0 + 1), crate::i18n::tr("Invalid number").to_string());
                return;
            }
        }
        if self.sc.peek() == Some(b'.') {
            self.sc.step();
            if !self.sc.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.err(r0, c0, self.sc.col, crate::i18n::tr("Expected digit after decimal point").to_string());
            } else {
                while self.sc.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.sc.step();
                }
            }
        }
        if matches!(self.sc.peek(), Some(b'e' | b'E')) {
            self.sc.step();
            if matches!(self.sc.peek(), Some(b'+' | b'-')) {
                self.sc.step();
            }
            if !self.sc.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.err(r0, c0, self.sc.col, crate::i18n::tr("Expected digit in exponent").to_string());
            } else {
                while self.sc.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.sc.step();
                }
            }
        }
    }

    fn parse_value(&mut self, depth: usize) -> bool {
        if depth > 128 || self.diags.len() >= MAX_DIAGS {
            return false;
        }
        self.skip_ws_comments();
        let Some(c) = self.sc.peek() else {
            return false;
        };
        match c {
            b'{' => self.parse_object(depth + 1),
            b'[' => self.parse_array(depth + 1),
            b'"' | b'\'' => {
                self.parse_string();
                true
            }
            b'-' | b'0'..=b'9' => {
                self.parse_number();
                true
            }
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let (r0, c0) = (self.sc.row, self.sc.col);
                let start = self.sc.i;
                while self.sc.peek().is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == b'_') {
                    self.sc.step();
                }
                let word = std::str::from_utf8(&self.sc.b[start..self.sc.i]).unwrap_or("");
                if !matches!(word, "true" | "false" | "null") {
                    self.err(r0, c0, self.sc.col, crate::trf!("Unexpected token '{tok}' in JSON"; tok => word));
                }
                true
            }
            _ => {
                let (r0, c0) = (self.sc.row, self.sc.col);
                let ch = c as char;
                self.sc.step();
                self.err(r0, c0, c0 + 1, crate::trf!("Unexpected character '{ch}' in JSON"; ch => ch));
                false
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> bool {
        let (r0, c0) = (self.sc.row, self.sc.col);
        self.sc.step();
        self.skip_ws_comments();
        if self.sc.peek() == Some(b'}') {
            self.sc.step();
            return true;
        }
        let mut keys = HashSet::new();
        loop {
            if self.diags.len() >= MAX_DIAGS {
                return true;
            }
            self.skip_ws_comments();
            let Some(c) = self.sc.peek() else {
                self.err(r0, c0, c0 + 1, crate::trf!("Unclosed {b}"; b => '{'));
                return true;
            };
            if c == b'}' {
                self.sc.step();
                return true;
            }
            let (kr, kc) = (self.sc.row, self.sc.col);
            let key = if c == b'"' || c == b'\'' {
                self.parse_string()
            } else if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
                let start = self.sc.i;
                while self.sc.peek().is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'$' || ch == b'-') {
                    self.sc.step();
                }
                let word = std::str::from_utf8(&self.sc.b[start..self.sc.i]).unwrap_or("").to_string();
                self.err(kr, kc, self.sc.col, crate::i18n::tr("Object keys in JSON must be double-quoted").to_string());
                Some(word)
            } else {
                self.err(kr, kc, kc + 1, crate::i18n::tr("Expected property name or '}'").to_string());
                self.sc.step();
                continue;
            };
            if let Some(k) = key {
                if !k.is_empty() && !keys.insert(k.clone()) && self.diags.len() < MAX_DIAGS {
                    self.diags.push(CodeDiag::warn(kr, kc, self.sc.col.max(kc + 1), crate::trf!("Duplicate key \"{k}\""; k => k), "JSON"));
                }
            }
            self.skip_ws_comments();
            if self.sc.peek() == Some(b':') {
                self.sc.step();
            } else {
                let (r, col) = (self.sc.row, self.sc.col);
                self.err(r, col, col + 1, crate::i18n::tr("Expected ':' after property name").to_string());
                if matches!(self.sc.peek(), Some(b',' | b'}')) {
                    if self.sc.peek() == Some(b',') {
                        self.sc.step();
                    }
                    continue;
                }
            }
            self.skip_ws_comments();
            if matches!(self.sc.peek(), Some(b',' | b'}' | b']') | None) {
                let (r, col) = (self.sc.row, self.sc.col);
                self.err(r, col, col + 1, crate::i18n::tr("Expected value after ':'").to_string());
            } else {
                self.parse_value(depth);
            }
            self.skip_ws_comments();
            match self.sc.peek() {
                Some(b',') => {
                    let (cr, cc) = (self.sc.row, self.sc.col);
                    self.sc.step();
                    self.skip_ws_comments();
                    if self.sc.peek() == Some(b'}') {
                        self.err(cr, cc, cc + 1, crate::i18n::tr("Trailing comma is not allowed in JSON").to_string());
                        self.sc.step();
                        return true;
                    }
                }
                Some(b'}') => {
                    self.sc.step();
                    return true;
                }
                None => {
                    self.err(r0, c0, c0 + 1, crate::trf!("Unclosed {b}"; b => '{'));
                    return true;
                }
                Some(_) => {
                    let (r, col) = (self.sc.row, self.sc.col);
                    self.err(r, col, col + 1, crate::i18n::tr("Expected ',' or '}'").to_string());
                    if self.sc.peek() == Some(b']') {
                        self.sc.step();
                        return true;
                    }
                }
            }
        }
    }

    fn parse_array(&mut self, depth: usize) -> bool {
        let (r0, c0) = (self.sc.row, self.sc.col);
        self.sc.step();
        self.skip_ws_comments();
        if self.sc.peek() == Some(b']') {
            self.sc.step();
            return true;
        }
        loop {
            if self.diags.len() >= MAX_DIAGS {
                return true;
            }
            self.skip_ws_comments();
            match self.sc.peek() {
                None => {
                    self.err(r0, c0, c0 + 1, crate::trf!("Unclosed {b}"; b => '['));
                    return true;
                }
                Some(b']') => {
                    self.sc.step();
                    return true;
                }
                _ => {}
            }
            if !self.parse_value(depth) && self.sc.peek().is_none() {
                self.err(r0, c0, c0 + 1, crate::trf!("Unclosed {b}"; b => '['));
                return true;
            }
            self.skip_ws_comments();
            match self.sc.peek() {
                Some(b',') => {
                    let (cr, cc) = (self.sc.row, self.sc.col);
                    self.sc.step();
                    self.skip_ws_comments();
                    if self.sc.peek() == Some(b']') {
                        self.err(cr, cc, cc + 1, crate::i18n::tr("Trailing comma is not allowed in JSON").to_string());
                        self.sc.step();
                        return true;
                    }
                }
                Some(b']') => {
                    self.sc.step();
                    return true;
                }
                None => {
                    self.err(r0, c0, c0 + 1, crate::trf!("Unclosed {b}"; b => '['));
                    return true;
                }
                Some(_) => {
                    let (r, col) = (self.sc.row, self.sc.col);
                    self.err(r, col, col + 1, crate::i18n::tr("Expected ',' or ']'").to_string());
                    if self.sc.peek() == Some(b'}') {
                        self.sc.step();
                        return true;
                    }
                }
            }
        }
    }
}

fn check_json(text: &str) -> Vec<CodeDiag> {
    let mut p = JsonParser::new(text);
    p.skip_ws_comments();
    if p.sc.peek().is_none() {
        return p.diags;
    }
    p.parse_value(0);
    p.skip_ws_comments();
    if p.sc.peek().is_some() && p.diags.is_empty() {
        let (r, col) = (p.sc.row, p.sc.col);
        p.err(r, col, col + 1, crate::i18n::tr("Unexpected trailing content after JSON value").to_string());
    }
    p.diags
}

const JS_TYPOS: &[(&str, &str)] = &[
    ("funtion", "function"),
    ("functoin", "function"),
    ("fnction", "function"),
    ("cosnt", "const"),
    ("conts", "const"),
    ("constt", "const"),
    ("retrun", "return"),
    ("reutrn", "return"),
    ("improt", "import"),
    ("imoprt", "import"),
    ("exprot", "export"),
    ("exoprt", "export"),
    ("awiat", "await"),
    ("asycn", "async"),
    ("calss", "class"),
    ("elese", "else"),
    ("whlie", "while"),
    ("swicth", "switch"),
];

fn js_regex_allowed_after(last_word: &str, last_punct: u8) -> bool {
    if matches!(
        last_word,
        "return" | "case" | "throw" | "in" | "of" | "typeof" | "void" | "delete" | "new" | "yield" | "await" | "else" | "do"
    ) {
        return true;
    }
    matches!(
        last_punct,
        0 | b'(' | b'[' | b'{' | b',' | b';' | b':' | b'=' | b'!' | b'?' | b'&' | b'|' | b'^' | b'~' | b'+' | b'-' | b'*' | b'%' | b'<' | b'>' | b'\n'
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JsTok {
    None,
    Ident,
    String,
    Number,
    CloseExpr,
    Op,
    BinOp,
}

fn js_allows_adjacent_value_after(word: &str) -> bool {
    matches!(
        word,
        "return"
            | "throw"
            | "case"
            | "typeof"
            | "void"
            | "delete"
            | "await"
            | "yield"
            | "new"
            | "in"
            | "instanceof"
            | "of"
            | "from"
            | "import"
            | "export"
            | "default"
            | "as"
            | "extends"
            | "implements"
            | "else"
            | "do"
            | "assert"
            | "with"
            | "type"
            | "interface"
            | "enum"
            | "declare"
            | "namespace"
            | "module"
            | "satisfies"
            | "is"
            | "keyof"
            | "readonly"
            | "abstract"
            | "async"
            | "static"
            | "get"
            | "set"
            | "public"
            | "private"
            | "protected"
            | "override"
            | "const"
            | "let"
            | "var"
            | "function"
            | "class"
            | "using"
            | "accessor"
            | "break"
            | "continue"
            | "for"
            | "asserts"
            | "infer"
            | "unique"
            | "out"
            | "global"
    )
}

fn js_allows_ident_after_value(word: &str) -> bool {
    matches!(
        word,
        "in" | "of"
            | "instanceof"
            | "as"
            | "is"
            | "from"
            | "satisfies"
            | "extends"
            | "implements"
            | "else"
            | "catch"
            | "finally"
            | "while"
            | "assert"
            | "with"
    )
}

fn check_js_ts(text: &str, tsx: bool) -> Vec<CodeDiag> {
    let mut sc = Scanner::new(text);
    let mut diags = Vec::new();
    let mut open: Vec<(u8, u32, u32)> = Vec::new();
    let mut cond_parens: Vec<usize> = Vec::new();
    let mut tpl_braces: Vec<usize> = Vec::new();
    let mut line_start = true;
    let mut last_word = String::new();
    let mut last_punct: u8 = 0;
    let mut prev_tok = JsTok::None;
    let mut prev_row: u32 = u32::MAX;
    let mut prev_op: &'static str = "";

    while let Some(c) = sc.peek() {
        if diags.len() >= MAX_DIAGS {
            break;
        }
        if c == b'\n' {
            sc.step();
            line_start = true;
            if last_punct == 0 {
                last_punct = b'\n';
            }
            continue;
        }
        if c == b' ' || c == b'\t' || c == b'\r' {
            sc.step();
            continue;
        }
        if sc.at("//") {
            sc.skip_line();
            line_start = true;
            continue;
        }
        if sc.at("/*") {
            if let Some((r, col)) = sc.skip_block_comment(false) {
                diags.push(CodeDiag::error(r, col, col + 2, crate::i18n::tr("Unterminated block comment").to_string(), "JS"));
            }
            continue;
        }
        let same_line = prev_row == sc.row;
        if c == b'"' || c == b'\'' {
            let (r0, c0) = (sc.row, sc.col);
            let bad_adj = same_line
                && match prev_tok {
                    JsTok::String | JsTok::Number | JsTok::CloseExpr => true,
                    JsTok::Ident => !js_allows_adjacent_value_after(&last_word),
                    _ => false,
                };
            sc.step();
            let mut closed = false;
            while let Some(ch) = sc.peek() {
                if ch == b'\\' {
                    sc.step();
                    if sc.peek() == Some(b'\r') {
                        sc.step();
                    }
                    sc.step();
                    continue;
                }
                if ch == b'\n' {
                    break;
                }
                sc.step();
                if ch == c {
                    closed = true;
                    break;
                }
            }
            let end_col = if sc.row == r0 { sc.col.max(c0 + 1) } else { c0 + 1 };
            if !closed {
                diags.push(CodeDiag::error(r0, c0, end_col, crate::i18n::tr("Unterminated string").to_string(), "JS"));
            } else if bad_adj {
                diags.push(CodeDiag::error(
                    r0,
                    c0,
                    end_col,
                    crate::i18n::tr("Unexpected string: missing operator or comma").to_string(),
                    "JS",
                ));
            }
            line_start = false;
            last_word.clear();
            last_punct = b'"';
            prev_tok = JsTok::String;
            prev_row = sc.row;
            prev_op = "";
            continue;
        }
        if c == b'`' {
            let entered_expr = scan_js_template(&mut sc, &mut open, &mut tpl_braces, &mut diags);
            line_start = false;
            last_word.clear();
            last_punct = if entered_expr { b'{' } else { b'`' };
            prev_tok = if entered_expr { JsTok::None } else { JsTok::String };
            prev_row = sc.row;
            prev_op = "";
            continue;
        }
        if c == b'/' && js_regex_allowed_after(&last_word, last_punct) && sc.peek_at(1).is_some_and(|n| n != b'/' && n != b'*' && n != b'=') {
            let save = (sc.i, sc.row, sc.col);
            sc.step();
            let mut in_class = false;
            let mut closed = false;
            while let Some(ch) = sc.peek() {
                if ch == b'\n' {
                    break;
                }
                sc.step();
                if ch == b'\\' {
                    if sc.peek().is_some_and(|n| n != b'\n') {
                        sc.step();
                    }
                } else if ch == b'[' {
                    in_class = true;
                } else if ch == b']' {
                    in_class = false;
                } else if ch == b'/' && !in_class {
                    closed = true;
                    break;
                }
            }
            if closed {
                while sc.peek().is_some_and(|f| f.is_ascii_alphabetic()) {
                    sc.step();
                }
                line_start = false;
                last_word.clear();
                last_punct = b'r';
                prev_tok = JsTok::String;
                prev_row = sc.row;
                prev_op = "";
                continue;
            } else {
                (sc.i, sc.row, sc.col) = save;
            }
        }
        if c.is_ascii_digit() {
            let (r0, c0) = (sc.row, sc.col);
            let bad_adj = same_line
                && match prev_tok {
                    JsTok::String | JsTok::Number | JsTok::CloseExpr => true,
                    JsTok::Ident => !js_allows_adjacent_value_after(&last_word),
                    _ => false,
                };
            let mut bad_num = false;
            if c == b'0' && matches!(sc.peek_at(1), Some(b'x' | b'X' | b'b' | b'B' | b'o' | b'O')) {
                let radix = sc.peek_at(1).unwrap().to_ascii_lowercase();
                sc.step();
                sc.step();
                let mut digits = 0;
                while let Some(ch) = sc.peek() {
                    let ok = match radix {
                        b'x' => ch.is_ascii_hexdigit() || ch == b'_',
                        b'b' => matches!(ch, b'0' | b'1' | b'_'),
                        _ => matches!(ch, b'0'..=b'7' | b'_'),
                    };
                    if ok {
                        if ch != b'_' {
                            digits += 1;
                        }
                        sc.step();
                    } else {
                        break;
                    }
                }
                if digits == 0 {
                    bad_num = true;
                }
                if sc.peek() == Some(b'n') {
                    sc.step();
                }
            } else {
                while sc.peek().is_some_and(|ch| ch.is_ascii_digit() || ch == b'_') {
                    sc.step();
                }
                if sc.peek() == Some(b'.') && sc.peek_at(1) != Some(b'.') {
                    sc.step();
                    while sc.peek().is_some_and(|ch| ch.is_ascii_digit() || ch == b'_') {
                        sc.step();
                    }
                }
                if matches!(sc.peek(), Some(b'e' | b'E')) {
                    sc.step();
                    if matches!(sc.peek(), Some(b'+' | b'-')) {
                        sc.step();
                    }
                    let mut exp_digits = 0;
                    while sc.peek().is_some_and(|ch| ch.is_ascii_digit() || ch == b'_') {
                        if sc.peek() != Some(b'_') {
                            exp_digits += 1;
                        }
                        sc.step();
                    }
                    if exp_digits == 0 {
                        bad_num = true;
                    }
                } else if sc.peek() == Some(b'n') {
                    sc.step();
                }
            }
            if sc.peek().is_some_and(|ch| ch.is_ascii_alphabetic() || ch == b'_' || ch == b'$') {
                bad_num = true;
                while sc.peek().is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'$') {
                    sc.step();
                }
            }
            if bad_num {
                diags.push(CodeDiag::error(r0, c0, sc.col.max(c0 + 1), crate::i18n::tr("Invalid number").to_string(), "JS"));
            } else if bad_adj {
                diags.push(CodeDiag::error(
                    r0,
                    c0,
                    sc.col.max(c0 + 1),
                    crate::i18n::tr("Unexpected number: missing operator or comma").to_string(),
                    "JS",
                ));
            }
            line_start = false;
            last_word.clear();
            last_punct = b'0';
            prev_tok = JsTok::Number;
            prev_row = sc.row;
            prev_op = "";
            continue;
        }
        if c == b'=' {
            let (r0, c0) = (sc.row, sc.col);
            let mut count = 0;
            while sc.peek() == Some(b'=') {
                sc.step();
                count += 1;
            }
            let is_arrow = count == 1 && sc.peek() == Some(b'>');
            if is_arrow {
                sc.step();
                line_start = false;
                last_word.clear();
                last_punct = b'>';
                prev_tok = JsTok::Op;
                prev_row = sc.row;
                prev_op = "=>";
                continue;
            }
            if count >= 4 {
                diags.push(CodeDiag::error(r0, c0, sc.col, crate::i18n::tr("Invalid operator '===='").to_string(), "JS"));
            } else if count == 1 {
                let in_cond = !open.is_empty() && cond_parens.last() == Some(&(open.len() - 1));
                let compound = matches!(last_punct, b'!' | b'<' | b'>' | b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^' | b'?');
                if !compound && prev_tok == JsTok::BinOp && matches!(prev_op, "==" | "===" | "!=" | "!==" | "&&" | "||" | "??") {
                    diags.push(CodeDiag::error(r0, c0, sc.col, crate::trf!("Unexpected operator '{op}'"; op => "="), "JS"));
                } else if !compound && in_cond {
                    if sc.peek() == Some(b'-')
                        && sc.peek_at(1).is_some_and(|n| n.is_ascii_alphabetic() || n == b'_' || n == b'$' || n == b'"' || n == b'\'')
                    {
                        diags.push(CodeDiag::error(
                            r0,
                            c0,
                            c0 + 2,
                            crate::i18n::tr("Unexpected '=-': did you mean '==='?").to_string(),
                            "JS",
                        ));
                    } else {
                        diags.push(CodeDiag::warn(
                            r0,
                            c0,
                            sc.col,
                            crate::i18n::tr("Assignment '=' in condition: did you mean '==='?").to_string(),
                            "JS",
                        ));
                    }
                }
                let mut look = sc.i;
                while look < sc.b.len() && matches!(sc.b[look], b' ' | b'\t') {
                    look += 1;
                }
                if !compound && matches!(sc.b.get(look), Some(b';' | b')' | b']' | b',')) {
                    diags.push(CodeDiag::error(r0, c0, sc.col, crate::i18n::tr("Expected expression after '='").to_string(), "JS"));
                }
            }
            line_start = false;
            last_word.clear();
            last_punct = b'=';
            prev_tok = JsTok::BinOp;
            prev_row = sc.row;
            prev_op = match count {
                1 => "=",
                2 => "==",
                _ => "===",
            };
            continue;
        }
        if (c == b'&' && sc.peek_at(1) == Some(b'&'))
            || (c == b'|' && sc.peek_at(1) == Some(b'|'))
            || (c == b'?' && sc.peek_at(1) == Some(b'?'))
        {
            let (r0, c0) = (sc.row, sc.col);
            let op = match c {
                b'&' => "&&",
                b'|' => "||",
                _ => "??",
            };
            sc.step();
            sc.step();
            if sc.peek() == Some(b'=') {
                sc.step();
            }
            if same_line && prev_tok == JsTok::BinOp {
                diags.push(CodeDiag::error(r0, c0, sc.col, crate::trf!("Unexpected operator '{op}'"; op => op), "JS"));
            }
            line_start = false;
            last_word.clear();
            last_punct = c;
            prev_tok = JsTok::BinOp;
            prev_row = sc.row;
            prev_op = op;
            continue;
        }
        if matches!(c, b'*' | b'/' | b'%') && sc.peek_at(1) != Some(b'=') {
            let (r0, c0) = (sc.row, sc.col);
            let is_pow = c == b'*' && sc.peek_at(1) == Some(b'*');
            let is_export_star = c == b'*' && matches!(last_word.as_str(), "export" | "import" | "function" | "yield");
            if is_pow {
                sc.step();
            }
            sc.step();
            let op = match (c, is_pow) {
                (b'*', true) => "**",
                (b'*', false) => "*",
                (b'/', _) => "/",
                _ => "%",
            };
            if same_line && !is_export_star && prev_tok == JsTok::BinOp {
                diags.push(CodeDiag::error(r0, c0, sc.col, crate::trf!("Unexpected operator '{op}'"; op => op), "JS"));
            }
            line_start = false;
            last_word.clear();
            last_punct = c;
            prev_tok = if is_export_star { JsTok::Op } else { JsTok::BinOp };
            prev_row = sc.row;
            prev_op = op;
            continue;
        }
        if c == b'\\' {
            let (r0, c0) = (sc.row, sc.col);
            sc.step();
            diags.push(CodeDiag::error(r0, c0, c0 + 1, crate::i18n::tr("Unexpected character '\\'").to_string(), "JS"));
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let (r0, c0) = (sc.row, sc.col);
            let at_stmt_start = line_start || matches!(last_punct, b';' | b'{' | b'}');
            let start = sc.i;
            while sc.peek().is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'$') {
                sc.step();
            }
            let word = std::str::from_utf8(&sc.b[start..sc.i]).unwrap_or("");
            if same_line && matches!(prev_tok, JsTok::String | JsTok::Number) && !js_allows_ident_after_value(word) {
                let jsx_attr = tsx && sc.peek() == Some(b'=');
                if !jsx_attr {
                    diags.push(CodeDiag::error(
                        r0,
                        c0,
                        sc.col,
                        crate::i18n::tr("Unexpected identifier: missing operator or comma").to_string(),
                        "JS",
                    ));
                }
            } else if !tsx
                && same_line
                && prev_tok == JsTok::Ident
                && !js_allows_adjacent_value_after(&last_word)
                && !js_allows_ident_after_value(word)
            {
                diags.push(CodeDiag::error(
                    r0,
                    c0,
                    sc.col,
                    crate::i18n::tr("Unexpected identifier: missing operator or comma").to_string(),
                    "JS",
                ));
            }
            if at_stmt_start {
                if let Some(&(_, fix)) = JS_TYPOS.iter().find(|&&(bad, _)| bad == word) {
                    let mut look = sc.i;
                    while look < sc.b.len() && matches!(sc.b[look], b' ' | b'\t') {
                        look += 1;
                    }
                    let next = sc.b.get(look).copied().unwrap_or(0);
                    let looks_like_kw = next.is_ascii_alphabetic()
                        || next == b'_'
                        || next == b'$'
                        || (matches!(fix, "else") && next == b'{')
                        || (matches!(fix, "while" | "switch") && next == b'(')
                        || (matches!(fix, "return") && matches!(next, b'"' | b'\'' | b'`' | b'(' | b'[' | b'{' | b'0'..=b'9' | b';'));
                    if looks_like_kw {
                        diags.push(CodeDiag::error(
                            r0,
                            c0,
                            sc.col,
                            crate::trf!("Unknown keyword '{bad}': did you mean '{fix}'?"; bad => word, fix => fix),
                            "JS",
                        ));
                    }
                }
                if matches!(word, "if" | "while" | "switch") {
                    let mut look = sc.i;
                    while look < sc.b.len() && matches!(sc.b[look], b' ' | b'\t') {
                        look += 1;
                    }
                    if sc.b.get(look) == Some(&b'{') {
                        diags.push(CodeDiag::error(
                            r0,
                            c0,
                            sc.col,
                            crate::trf!("Expected '(' after '{kw}'"; kw => word),
                            "JS",
                        ));
                    }
                }
            }
            line_start = false;
            last_word.clear();
            last_word.push_str(word);
            last_punct = b'a';
            prev_tok = JsTok::Ident;
            prev_row = sc.row;
            prev_op = "";
            continue;
        }
        if matches!(c, b'(' | b'[' | b'{') {
            if c == b'(' && matches!(last_word.as_str(), "if" | "while") {
                cond_parens.push(open.len());
            }
            open.push((c, sc.row, sc.col));
            sc.step();
            line_start = c == b'{';
            last_word.clear();
            last_punct = c;
            prev_tok = JsTok::None;
            prev_row = sc.row;
            prev_op = "";
            continue;
        }
        if matches!(c, b')' | b']' | b'}') {
            if c == b'}' && tpl_braces.last().copied() == Some(open.len().saturating_sub(1)) && open.last().is_some_and(|x| x.0 == b'{') {
                open.pop();
                tpl_braces.pop();
                sc.step();
                let pos = (sc.row, sc.col);
                let entered_expr = scan_js_template_body(&mut sc, &mut open, &mut tpl_braces, &mut diags, pos);
                line_start = false;
                last_word.clear();
                last_punct = if entered_expr { b'{' } else { b'`' };
                prev_tok = if entered_expr { JsTok::None } else { JsTok::String };
                prev_row = sc.row;
                prev_op = "";
                continue;
            }
            let (r, col) = (sc.row, sc.col);
            if same_line && prev_tok == JsTok::BinOp && matches!(c, b')' | b']') {
                diags.push(CodeDiag::error(
                    r,
                    col,
                    col + 1,
                    crate::trf!("Expected expression after '{op}'"; op => prev_op),
                    "JS",
                ));
            }
            let closed_cond = c == b')' && !open.is_empty() && cond_parens.last() == Some(&(open.len() - 1));
            if closed_cond {
                cond_parens.pop();
            }
            sc.step();
            push_bracket_close(&mut open, c, r, col, "JS", &mut diags);
            line_start = c == b'}';
            last_word.clear();
            last_punct = c;
            prev_tok = if matches!(c, b')' | b']') && !closed_cond { JsTok::CloseExpr } else { JsTok::None };
            prev_row = sc.row;
            prev_op = "";
            continue;
        }
        if same_line && prev_tok == JsTok::BinOp && matches!(c, b';' | b',') {
            diags.push(CodeDiag::error(
                sc.row,
                sc.col,
                sc.col + 1,
                crate::trf!("Expected expression after '{op}'"; op => prev_op),
                "JS",
            ));
        }
        sc.step();
        line_start = c == b';';
        last_word.clear();
        last_punct = c;
        prev_tok = match c {
            b';' | b',' | b':' | b'?' | b'.' => JsTok::None,
            _ => JsTok::Op,
        };
        prev_row = sc.row;
        prev_op = "";
    }

    finish_open_brackets(open, "JS", &mut diags);
    diags
}

fn scan_js_template(sc: &mut Scanner<'_>, open: &mut Vec<(u8, u32, u32)>, tpl_braces: &mut Vec<usize>, diags: &mut Vec<CodeDiag>) -> bool {
    let start = (sc.row, sc.col);
    sc.step();
    scan_js_template_body(sc, open, tpl_braces, diags, start)
}

fn scan_js_template_body(
    sc: &mut Scanner<'_>,
    open: &mut Vec<(u8, u32, u32)>,
    tpl_braces: &mut Vec<usize>,
    diags: &mut Vec<CodeDiag>,
    start: (u32, u32),
) -> bool {
    while let Some(c) = sc.peek() {
        if c == b'\\' {
            sc.step();
            sc.step();
            continue;
        }
        if c == b'`' {
            sc.step();
            return false;
        }
        if c == b'$' && sc.peek_at(1) == Some(b'{') {
            sc.step();
            let (r, col) = (sc.row, sc.col);
            tpl_braces.push(open.len());
            open.push((b'{', r, col));
            sc.step();
            return true;
        }
        sc.step();
    }
    diags.push(CodeDiag::error(start.0, start.1, start.1 + 1, crate::i18n::tr("Unterminated template literal").to_string(), "JS"));
    false
}

const PY_TYPOS: &[(&str, &str)] = &[
    ("dfe", "def"),
    ("improt", "import"),
    ("retrun", "return"),
    ("calss", "class"),
    ("elisf", "elif"),
    ("whlie", "while"),
];

fn check_python(text: &str) -> Vec<CodeDiag> {
    let mut sc = Scanner::new(text);
    let mut diags = Vec::new();
    let mut open: Vec<(u8, u32, u32)> = Vec::new();
    let mut depth_at_line_start: Vec<usize> = Vec::new();
    let mut in_triple_at_line_start: Vec<bool> = Vec::new();
    let mut cur_row: u32 = u32::MAX;

    while let Some(c) = sc.peek() {
        if diags.len() >= MAX_DIAGS {
            break;
        }
        if sc.row != cur_row {
            while depth_at_line_start.len() <= sc.row as usize {
                depth_at_line_start.push(open.len());
                in_triple_at_line_start.push(false);
            }
            cur_row = sc.row;
        }
        if c == b'#' {
            sc.skip_line();
            continue;
        }
        if c == b'"' || c == b'\'' {
            let (r0, c0) = (sc.row, sc.col);
            let triple = sc.peek_at(1) == Some(c) && sc.peek_at(2) == Some(c);
            if triple {
                sc.step();
                sc.step();
                sc.step();
                let mut closed = false;
                while let Some(ch) = sc.peek() {
                    if sc.row != cur_row {
                        while depth_at_line_start.len() <= sc.row as usize {
                            depth_at_line_start.push(open.len());
                            in_triple_at_line_start.push(true);
                        }
                        cur_row = sc.row;
                    }
                    if ch == b'\\' {
                        sc.step();
                        sc.step();
                        continue;
                    }
                    if ch == c && sc.peek_at(1) == Some(c) && sc.peek_at(2) == Some(c) {
                        sc.step();
                        sc.step();
                        sc.step();
                        closed = true;
                        break;
                    }
                    sc.step();
                }
                if !closed {
                    diags.push(CodeDiag::error(r0, c0, c0 + 3, crate::i18n::tr("Unterminated multiline string").to_string(), "Python"));
                }
            } else {
                sc.step();
                let mut closed = false;
                while let Some(ch) = sc.peek() {
                    if ch == b'\\' {
                        sc.step();
                        sc.step();
                        continue;
                    }
                    if ch == b'\n' {
                        break;
                    }
                    sc.step();
                    if ch == c {
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    diags.push(CodeDiag::error(r0, c0, sc.col.max(c0 + 1), crate::i18n::tr("Unterminated string").to_string(), "Python"));
                }
            }
            continue;
        }
        if matches!(c, b'(' | b'[' | b'{') {
            open.push((c, sc.row, sc.col));
            sc.step();
            continue;
        }
        if matches!(c, b')' | b']' | b'}') {
            let (r, col) = (sc.row, sc.col);
            sc.step();
            push_bracket_close(&mut open, c, r, col, "Python", &mut diags);
            continue;
        }
        sc.step();
    }
    finish_open_brackets(open, "Python", &mut diags);

    for (line_ix, raw) in text.lines().enumerate() {
        if diags.len() >= MAX_DIAGS {
            break;
        }
        if in_triple_at_line_start.get(line_ix).copied().unwrap_or(false)
            || depth_at_line_start.get(line_ix).copied().unwrap_or(0) > 0
        {
            continue;
        }
        let row = line_ix as u32;
        let lead: &str = raw.get(..raw.len() - raw.trim_start_matches([' ', '\t']).len()).unwrap_or("");
        if lead.contains('\t') && lead.contains(' ') {
            diags.push(CodeDiag::error(row, 0, lead.len() as u32, crate::i18n::tr("Mixed tabs and spaces in indentation").to_string(), "Python"));
        }
        let code = strip_py_comment(raw).trim_end();
        let trimmed = code.trim_start();
        if trimmed.is_empty() || trimmed.ends_with('\\') {
            continue;
        }
        let indent_cols = raw[..raw.len() - raw.trim_start().len()].chars().count() as u32;
        let first_word = trimmed.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').next().unwrap_or("");
        if let Some(&(_, fix)) = PY_TYPOS.iter().find(|&&(bad, _)| bad == first_word) {
            let after = trimmed[first_word.len()..].trim_start();
            if after.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') {
                diags.push(CodeDiag::error(
                    row,
                    indent_cols,
                    indent_cols + first_word.len() as u32,
                    crate::trf!("Unknown keyword '{bad}': did you mean '{fix}'?"; bad => first_word, fix => fix),
                    "Python",
                ));
                continue;
            }
        }
        let needs_colon = matches!(first_word, "def" | "class" | "if" | "elif" | "for" | "while" | "with" | "try" | "except" | "finally" | "else")
            || (first_word == "async"
                && trimmed["async".len()..]
                    .split_whitespace()
                    .next()
                    .is_some_and(|w| matches!(w, "def" | "for" | "with")));
        if needs_colon && py_line_brackets_balanced(trimmed) {
            let valid_header = match first_word {
                "else" | "try" | "finally" => trimmed == first_word || trimmed.starts_with(&format!("{first_word}:")),
                _ => trimmed.len() > first_word.len() && trimmed.as_bytes()[first_word.len()].is_ascii_whitespace(),
            };
            let has_inline_stmt = trimmed.contains(':');
            if valid_header && !trimmed.ends_with(':') && !has_inline_stmt {
                let end = indent_cols + trimmed.chars().count() as u32;
                diags.push(CodeDiag::error(
                    row,
                    end.saturating_sub(1),
                    end,
                    crate::i18n::tr("Expected ':' at end of statement").to_string(),
                    "Python",
                ));
            }
            if matches!(first_word, "if" | "elif" | "while") && trimmed.ends_with(':') {
                let cond = &trimmed[first_word.len()..trimmed.len() - 1];
                if let Some(off) = find_bare_assign(cond) {
                    let col = indent_cols + first_word.len() as u32 + off as u32;
                    diags.push(CodeDiag::error(
                        row,
                        col,
                        col + 1,
                        crate::i18n::tr("Use '==' to compare values instead of '='").to_string(),
                        "Python",
                    ));
                }
            }
        }
    }
    diags
}

fn strip_py_comment(line: &str) -> &str {
    let b = line.as_bytes();
    let mut i = 0;
    let mut q: u8 = 0;
    while i < b.len() {
        let c = b[i];
        if q != 0 {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                q = 0;
            }
        } else if c == b'"' || c == b'\'' {
            q = c;
        } else if c == b'#' {
            return &line[..i];
        }
        i += 1;
    }
    line
}

fn py_line_brackets_balanced(line: &str) -> bool {
    let b = line.as_bytes();
    let mut i = 0;
    let mut q: u8 = 0;
    let mut depth = 0isize;
    while i < b.len() {
        let c = b[i];
        if q != 0 {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                q = 0;
            }
        } else if c == b'"' || c == b'\'' {
            q = c;
        } else if matches!(c, b'(' | b'[' | b'{') {
            depth += 1;
        } else if matches!(c, b')' | b']' | b'}') {
            depth -= 1;
            if depth < 0 {
                return false;
            }
        }
        i += 1;
    }
    q == 0 && depth == 0
}

fn find_bare_assign(cond: &str) -> Option<usize> {
    let b = cond.as_bytes();
    let mut i = 0;
    let mut q: u8 = 0;
    let mut depth = 0usize;
    while i < b.len() {
        let c = b[i];
        if q != 0 {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                q = 0;
            }
        } else if c == b'"' || c == b'\'' {
            q = c;
        } else if matches!(c, b'(' | b'[' | b'{') {
            depth += 1;
        } else if matches!(c, b')' | b']' | b'}') {
            depth = depth.saturating_sub(1);
        } else if c == b'=' && depth == 0 {
            let prev = i.checked_sub(1).map(|p| b[p]).unwrap_or(0);
            let next = b.get(i + 1).copied().unwrap_or(0);
            if !matches!(prev, b'=' | b'!' | b'<' | b'>' | b':') && next != b'=' {
                return Some(cond[..i].chars().count());
            }
        }
        i += 1;
    }
    None
}

const VOID_HTML_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr",
];

fn check_markup(text: &str, xml: bool) -> Vec<CodeDiag> {
    let mut sc = Scanner::new(text);
    let mut diags = Vec::new();
    let mut stack: Vec<(String, u32, u32)> = Vec::new();
    let source = if xml { "XML" } else { "HTML" };

    while let Some(c) = sc.peek() {
        if diags.len() >= MAX_DIAGS {
            break;
        }
        if c != b'<' {
            sc.step();
            continue;
        }
        if sc.at("<!--") {
            let (r0, c0) = (sc.row, sc.col);
            for _ in 0..4 {
                sc.step();
            }
            let mut closed = false;
            while sc.i < sc.b.len() {
                if sc.at("-->") {
                    for _ in 0..3 {
                        sc.step();
                    }
                    closed = true;
                    break;
                }
                sc.step();
            }
            if !closed {
                diags.push(CodeDiag::error(r0, c0, c0 + 4, crate::i18n::tr("Unterminated HTML comment").to_string(), source));
            }
            continue;
        }
        if sc.at("<![CDATA[") {
            while sc.i < sc.b.len() && !sc.at("]]>") {
                sc.step();
            }
            for _ in 0..3 {
                sc.step();
            }
            continue;
        }
        if sc.at("<!") || sc.at("<?") {
            while let Some(ch) = sc.peek() {
                sc.step();
                if ch == b'>' {
                    break;
                }
            }
            continue;
        }
        let (r0, c0) = (sc.row, sc.col);
        sc.step();
        let closing = sc.peek() == Some(b'/');
        if closing {
            sc.step();
        }
        if !sc.peek().is_some_and(|ch| ch.is_ascii_alphabetic()) {
            continue;
        }
        let start = sc.i;
        while sc.peek().is_some_and(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_' | b':' | b'.')) {
            sc.step();
        }
        let tag_raw = std::str::from_utf8(&sc.b[start..sc.i]).unwrap_or("");
        let tag = if xml { tag_raw.to_string() } else { tag_raw.to_ascii_lowercase() };
        let mut self_closing = false;
        let mut q: u8 = 0;
        let mut closed_angle = false;
        while let Some(ch) = sc.peek() {
            if q != 0 {
                sc.step();
                if ch == q {
                    q = 0;
                }
            } else if ch == b'"' || ch == b'\'' {
                q = ch;
                sc.step();
            } else if ch == b'/' && sc.peek_at(1) == Some(b'>') {
                self_closing = true;
                sc.step();
                sc.step();
                closed_angle = true;
                break;
            } else if ch == b'>' {
                sc.step();
                closed_angle = true;
                break;
            } else {
                sc.step();
            }
        }
        if !closed_angle {
            diags.push(CodeDiag::error(
                r0,
                c0,
                c0 + tag_raw.len() as u32 + 1,
                crate::trf!("Unclosed tag <{tag}>"; tag => tag_raw),
                source,
            ));
            break;
        }
        if closing {
            if let Some(pos) = stack.iter().rposition(|(t, _, _)| *t == tag) {
                for (unclosed, r, col) in stack.drain(pos + 1..) {
                    diags.push(CodeDiag::error(
                        r,
                        col,
                        col + unclosed.len() as u32 + 2,
                        crate::trf!("Unclosed tag <{tag}>"; tag => unclosed),
                        source,
                    ));
                }
                stack.pop();
            } else {
                diags.push(CodeDiag::error(
                    r0,
                    c0,
                    c0 + tag_raw.len() as u32 + 3,
                    crate::trf!("Unexpected closing tag </{tag}>"; tag => tag_raw),
                    source,
                ));
            }
        } else if !self_closing && (xml || !VOID_HTML_TAGS.contains(&tag.as_str())) {
            if !xml && matches!(tag.as_str(), "script" | "style") {
                let end_needle = format!("</{tag}");
                while sc.i < sc.b.len() {
                    if sc.b[sc.i..].len() >= end_needle.len()
                        && sc.b[sc.i..sc.i + end_needle.len()].eq_ignore_ascii_case(end_needle.as_bytes())
                    {
                        while let Some(ch) = sc.peek() {
                            sc.step();
                            if ch == b'>' {
                                break;
                            }
                        }
                        break;
                    }
                    sc.step();
                }
            } else {
                stack.push((tag, r0, c0));
            }
        }
    }
    for (unclosed, r, col) in stack {
        diags.push(CodeDiag::error(
            r,
            col,
            col + unclosed.len() as u32 + 2,
            crate::trf!("Unclosed tag <{tag}>"; tag => unclosed),
            source,
        ));
    }
    diags
}

fn check_toml(text: &str) -> Vec<CodeDiag> {
    let mut sc = Scanner::new(text);
    let mut diags = Vec::new();
    let mut open: Vec<(u8, u32, u32)> = Vec::new();
    let mut depth_per_line: Vec<usize> = Vec::new();
    let mut in_ml_per_line: Vec<bool> = Vec::new();
    let mut cur_row: u32 = u32::MAX;

    while let Some(c) = sc.peek() {
        if diags.len() >= MAX_DIAGS {
            break;
        }
        if sc.row != cur_row {
            while depth_per_line.len() <= sc.row as usize {
                depth_per_line.push(open.len());
                in_ml_per_line.push(false);
            }
            cur_row = sc.row;
        }
        if c == b'#' {
            sc.skip_line();
            continue;
        }
        if c == b'"' || c == b'\'' {
            let (r0, c0) = (sc.row, sc.col);
            let triple = sc.peek_at(1) == Some(c) && sc.peek_at(2) == Some(c);
            if triple {
                sc.step();
                sc.step();
                sc.step();
                let mut closed = false;
                while let Some(ch) = sc.peek() {
                    if sc.row != cur_row {
                        while depth_per_line.len() <= sc.row as usize {
                            depth_per_line.push(open.len());
                            in_ml_per_line.push(true);
                        }
                        cur_row = sc.row;
                    }
                    if c == b'"' && ch == b'\\' {
                        sc.step();
                        sc.step();
                        continue;
                    }
                    if ch == c && sc.peek_at(1) == Some(c) && sc.peek_at(2) == Some(c) {
                        sc.step();
                        sc.step();
                        sc.step();
                        closed = true;
                        break;
                    }
                    sc.step();
                }
                if !closed {
                    diags.push(CodeDiag::error(r0, c0, c0 + 3, crate::i18n::tr("Unterminated multiline string").to_string(), "TOML"));
                }
            } else {
                sc.step();
                let mut closed = false;
                while let Some(ch) = sc.peek() {
                    if c == b'"' && ch == b'\\' {
                        sc.step();
                        sc.step();
                        continue;
                    }
                    if ch == b'\n' {
                        break;
                    }
                    sc.step();
                    if ch == c {
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    diags.push(CodeDiag::error(r0, c0, sc.col.max(c0 + 1), crate::i18n::tr("Unterminated string").to_string(), "TOML"));
                }
            }
            continue;
        }
        if matches!(c, b'[' | b'{') {
            open.push((c, sc.row, sc.col));
            sc.step();
            continue;
        }
        if matches!(c, b']' | b'}') {
            let (r, col) = (sc.row, sc.col);
            sc.step();
            push_bracket_close(&mut open, c, r, col, "TOML", &mut diags);
            continue;
        }
        sc.step();
    }
    finish_open_brackets(open, "TOML", &mut diags);

    for (line_ix, raw) in text.lines().enumerate() {
        if diags.len() >= MAX_DIAGS {
            break;
        }
        if in_ml_per_line.get(line_ix).copied().unwrap_or(false) || depth_per_line.get(line_ix).copied().unwrap_or(0) > 0 {
            continue;
        }
        let code = strip_py_comment(raw).trim();
        if code.is_empty() || code.starts_with('[') {
            continue;
        }
        if !code.contains('=') {
            let col = raw[..raw.len() - raw.trim_start().len()].chars().count() as u32;
            diags.push(CodeDiag::error(
                line_ix as u32,
                col,
                col + code.chars().count() as u32,
                crate::i18n::tr("Expected '=' in TOML key-value assignment").to_string(),
                "TOML",
            ));
        }
    }
    diags
}

struct GenSyntax {
    line: &'static [&'static str],
    block: bool,
    nested_block: bool,
    quotes: &'static [u8],
    multiline_quotes: &'static [u8],
    raw_rust: bool,
    source: &'static str,
}

fn gen_syntax(lang: &str) -> Option<GenSyntax> {
    Some(match lang {
        "rust" => GenSyntax { line: &["//"], block: true, nested_block: true, quotes: b"\"", multiline_quotes: b"\"", raw_rust: true, source: "Rust" },
        "go" => GenSyntax { line: &["//"], block: true, nested_block: false, quotes: b"\"`", multiline_quotes: b"`", raw_rust: false, source: "Go" },
        "php" => GenSyntax { line: &["//", "#"], block: true, nested_block: false, quotes: b"\"'", multiline_quotes: b"\"'", raw_rust: false, source: "PHP" },
        "css" | "scss" | "less" => GenSyntax {
            line: if lang == "css" { &[] } else { &["//"] },
            block: true,
            nested_block: false,
            quotes: b"\"'",
            multiline_quotes: b"",
            raw_rust: false,
            source: "CSS",
        },
        "dart" => GenSyntax { line: &["//"], block: true, nested_block: false, quotes: b"\"'", multiline_quotes: b"", raw_rust: false, source: "Dart" },
        "zig" => GenSyntax { line: &["//"], block: false, nested_block: false, quotes: b"\"", multiline_quotes: b"", raw_rust: false, source: "Zig" },
        "c" | "cpp" | "java" | "cs" | "kt" | "scala" | "swift" => {
            GenSyntax { line: &["//"], block: true, nested_block: matches!(lang, "kt" | "swift"), quotes: b"\"", multiline_quotes: b"", raw_rust: false, source: "Code" }
        }
        _ => return None,
    })
}

fn check_general(text: &str, lang: &str) -> Vec<CodeDiag> {
    let Some(syn) = gen_syntax(lang) else { return Vec::new() };
    let mut sc = Scanner::new(text);
    let mut diags = Vec::new();
    let mut open: Vec<(u8, u32, u32)> = Vec::new();

    while let Some(c) = sc.peek() {
        if diags.len() >= MAX_DIAGS {
            break;
        }
        if syn.line.iter().any(|s| sc.at(s)) {
            sc.skip_line();
            continue;
        }
        if syn.block && sc.at("/*") {
            if let Some((r, col)) = sc.skip_block_comment(syn.nested_block) {
                diags.push(CodeDiag::error(r, col, col + 2, crate::i18n::tr("Unterminated block comment").to_string(), syn.source));
            }
            continue;
        }
        if syn.raw_rust && c == b'r' && !sc.after_ident() {
            let hashes = sc.b[sc.i + 1..].iter().take_while(|&&h| h == b'#').count();
            if sc.b.get(sc.i + 1 + hashes) == Some(&b'"') {
                let (r0, c0) = (sc.row, sc.col);
                for _ in 0..hashes + 2 {
                    sc.step();
                }
                let end = format!("\"{}", "#".repeat(hashes));
                let mut closed = false;
                while sc.i < sc.b.len() {
                    if sc.at(&end) {
                        for _ in 0..end.len() {
                            sc.step();
                        }
                        closed = true;
                        break;
                    }
                    sc.step();
                }
                if !closed {
                    diags.push(CodeDiag::error(r0, c0, c0 + hashes as u32 + 2, crate::i18n::tr("Unterminated raw string").to_string(), syn.source));
                }
                continue;
            }
        }
        if syn.quotes.contains(&c) {
            let (r0, c0) = (sc.row, sc.col);
            let multi = syn.multiline_quotes.contains(&c);
            sc.step();
            let mut closed = false;
            while let Some(ch) = sc.peek() {
                if ch == b'\\' && c != b'`' {
                    sc.step();
                    if sc.peek() == Some(b'\r') {
                        sc.step();
                    }
                    sc.step();
                    continue;
                }
                if ch == b'\n' && !multi {
                    break;
                }
                sc.step();
                if ch == c {
                    closed = true;
                    break;
                }
            }
            if !closed {
                diags.push(CodeDiag::error(r0, c0, sc.col.max(c0 + 1), crate::i18n::tr("Unterminated string").to_string(), syn.source));
            }
            continue;
        }
        if c == b'\'' {
            match (sc.peek_at(1), sc.peek_at(2)) {
                (Some(b'\\'), _) => {
                    sc.step();
                    sc.step();
                    while let Some(ch) = sc.peek() {
                        sc.step();
                        if ch == b'\'' || ch == b'\n' {
                            break;
                        }
                    }
                }
                (Some(_), Some(b'\'')) => {
                    sc.step();
                    sc.step();
                    sc.step();
                }
                _ => {
                    sc.step();
                }
            }
            continue;
        }
        if matches!(c, b'(' | b'[' | b'{') {
            open.push((c, sc.row, sc.col));
            sc.step();
            continue;
        }
        if matches!(c, b')' | b']' | b'}') {
            let (r, col) = (sc.row, sc.col);
            sc.step();
            push_bracket_close(&mut open, c, r, col, syn.source, &mut diags);
            continue;
        }
        sc.step();
    }
    finish_open_brackets(open, syn.source, &mut diags);
    diags
}

#[cfg(test)]
mod tests {
    use super::{analyze, Severity};

    #[test]
    fn json_catches_common_writing_mistakes_and_allows_bedrock_comments() {
        let ok_bedrock = "{\n  // Bedrock block definition\n  \"format_version\": \"1.20.0\",\n  \"minecraft:block\": { \"description\": { \"identifier\": \"k:cut\" } }\n}\n";
        assert!(analyze(ok_bedrock, "json").is_empty());

        let missing_comma = "{\n  \"a\": 1\n  \"b\": 2\n}\n";
        let d = analyze(missing_comma, "json");
        assert!(!d.is_empty() && d[0].severity == Severity::Error, "{d:?}");

        let trailing = "{\n  \"a\": 1,\n}\n";
        let d = analyze(trailing, "json");
        assert!(d.iter().any(|x| x.message.contains("Trailing comma")), "{d:?}");

        let unquoted = "{ foo: 1 }";
        let d = analyze(unquoted, "json");
        assert!(d.iter().any(|x| x.message.contains("double-quoted")), "{d:?}");

        let dup = "{ \"a\": 1, \"a\": 2 }";
        let d = analyze(dup, "json");
        assert!(d.iter().any(|x| x.severity == Severity::Warning && x.message.contains("Duplicate key")), "{d:?}");
    }

    #[test]
    fn js_handles_templates_regex_and_catches_syntax_typos() {
        let valid = "import { world } from '@minecraft/server';\nconst re = /[a-z\"'(]+/gi;\nconst s = `hi ${fn(1, { a: `nested ${x}` })}`;\n";
        assert!(analyze(valid, "javascript").is_empty(), "{:?}", analyze(valid, "javascript"));

        let bad = "funtion greet(name) {\n  const x = ;\n  const s = \"unclosed;\n  if (x ==== 1) {\n}\n";
        let d = analyze(bad, "javascript");
        assert!(d.iter().any(|x| x.message.contains("function")), "{d:?}");
        assert!(d.iter().any(|x| x.message.contains("Expected expression")), "{d:?}");
        assert!(d.iter().any(|x| x.message.contains("Unterminated string")), "{d:?}");
        assert!(d.iter().any(|x| x.message.contains("====")), "{d:?}");
        assert!(d.iter().any(|x| x.message.contains("Unclosed {")), "{d:?}");

        let bedrock_bad = r#"
system.afterEvents.scriptEventReceive.subscribe(({ id, sourceEntity }) => {
  if (id =-S "kiwha:calib" && sourceEntity instanceof Player) giveCalib(sourceEntity);
  if (id === "kiwha:menu" && sourceEntity instanceof Player) openMenu(sourceEntity);
});
"#;
        let bd = analyze(bedrock_bad, "javascript");
        assert!(bd.iter().any(|x| x.row == 2 && x.message.contains("=-")), "{bd:?}");
        assert!(bd.iter().any(|x| x.row == 2 && x.message.contains("Unexpected string")), "{bd:?}");

        let bedrock_ok = r#"
import { system, Player } from "@minecraft/server";
system.afterEvents.scriptEventReceive.subscribe(({ id, sourceEntity }) => {
  if (id === "kiwha:calib" && sourceEntity instanceof Player) giveCalib(sourceEntity);
  if (id === "kiwha:menu" && sourceEntity instanceof Player) openMenu(sourceEntity);
  if ((id = nextId())) return "ok";
  for (let i = 0; i < 10; i++) { const n = 1_000n; }
});
"#;
        let ok_d = analyze(bedrock_ok, "javascript");
        assert!(ok_d.is_empty(), "expected zero false positives: {ok_d:?}");

        let expr_bad = "const a = 123abc;\nif (x && ) {}\nconst y = foo bar;\n";
        let ed = analyze(expr_bad, "javascript");
        assert!(ed.iter().any(|x| x.row == 0 && x.message.contains("Invalid number")), "{ed:?}");
        assert!(ed.iter().any(|x| x.row == 1 && x.message.contains("Expected expression")), "{ed:?}");
        assert!(ed.iter().any(|x| x.row == 2 && x.message.contains("Unexpected identifier")), "{ed:?}");
    }

    #[test]
    fn python_catches_missing_colon_mixed_indent_and_assignment_in_if() {
        let ok = "def run(x=1):\n    if x == 1 and (y := f(a=2)):\n        return f\"ok {x}\"\n";
        assert!(analyze(ok, "python").is_empty(), "{:?}", analyze(ok, "python"));

        let bad = "def run(x)\n\t if x = 1:\n        retrun x\n";
        let d = analyze(bad, "python");
        assert!(d.iter().any(|x| x.message.contains("Expected ':'")), "{d:?}");
        assert!(d.iter().any(|x| x.message.contains("Mixed tabs")), "{d:?}");
        assert!(d.iter().any(|x| x.message.contains("'=='")), "{d:?}");
        assert!(d.iter().any(|x| x.message.contains("return")), "{d:?}");
    }

    #[test]
    fn html_and_toml_diagnostics() {
        let ok_html = "<!DOCTYPE html><html><body><img src=\"a.png\"><br><div><p>hi</p></div></body></html>";
        assert!(analyze(ok_html, "html").is_empty(), "{:?}", analyze(ok_html, "html"));

        let bad_html = "<div><span>hi</div>";
        let d = analyze(bad_html, "html");
        assert!(d.iter().any(|x| x.message.contains("Unclosed tag <span>")), "{d:?}");

        let bad_toml = "[package]\nname = \"flux\"\nbroken line\n";
        let d = analyze(bad_toml, "toml");
        assert!(d.iter().any(|x| x.message.contains("Expected '='")), "{d:?}");
    }

    #[test]
    fn fluxcode_own_rust_sources_have_zero_false_positives() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        assert!(files.len() > 50);
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            let d = analyze(&text, "rust");
            assert!(d.is_empty(), "{}: {d:?}", f.display());
        }
    }

    #[test]
    fn unicode_columns_raw_strings_css_and_unknown_langs() {
        let d = analyze("s = 'é'\n)", "python");
        assert_eq!(d.first().map(|x| (x.row, x.col)), Some((1, 0)));
        let d = analyze("é)", "rust");
        assert_eq!(d.first().map(|x| (x.row, x.col)), Some((0, 1)));
        assert!(analyze(":) }", "markdown").is_empty());
        assert!(analyze("let j = r#\"{\"a\":\"]\"}\"#; let p = r\"C:\\\"; f(br\"[\");\n", "rust").is_empty());
        assert!(analyze("a { background: url(http://x/y.png); }\n", "css").is_empty());
    }
}
