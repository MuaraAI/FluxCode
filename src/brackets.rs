
const MAX_BYTES: usize = 2 * 1024 * 1024;

struct Syntax {
    line: &'static [&'static str],
    block: bool,
    quotes: &'static [u8],
    raw: bool,
}

const C_LIKE: Syntax = Syntax { line: &["//"], block: true, quotes: b"\"", raw: false };

fn syntax(lang: &str) -> Option<Syntax> {
    Some(match lang {
        "python" => Syntax { line: &["#"], block: false, quotes: b"\"'", raw: false },
        "javascript" | "typescript" | "tsx" => Syntax { quotes: b"\"'`", ..C_LIKE },
        "go" => Syntax { quotes: b"\"`", ..C_LIKE },
        "dart" | "json" => Syntax { quotes: b"\"'", ..C_LIKE },
        "php" => Syntax { line: &["//", "#"], block: true, quotes: b"\"'", raw: false },
        "css" | "scss" | "less" => Syntax { line: &[], block: true, quotes: b"\"'", raw: false },
        "rust" => Syntax { raw: true, ..C_LIKE },
        "c" | "cpp" | "java" | "cs" | "kt" | "scala" | "swift" => C_LIKE,
        _ => return None,
    })
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
    row: u32,
    col: u32,
}

impl Cursor<'_> {
    fn step(&mut self) {
        match self.b[self.i] {
            b'\n' => (self.row, self.col) = (self.row + 1, 0),
            c if c & 0xC0 != 0x80 => self.col += 1,
            _ => {}
        }
        self.i += 1;
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

    fn at(&self, s: &str) -> bool {
        self.b[self.i..].starts_with(s.as_bytes())
    }

    fn skip_past(&mut self, end: &str) {
        while self.i < self.b.len() && !self.at(end) {
            self.step();
        }
        for _ in 0..end.len().min(self.b.len() - self.i) {
            self.step();
        }
    }

    fn skip_string(&mut self) {
        let q = self.b[self.i];
        self.step();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.step();
            if c == b'\\' && self.i < self.b.len() {
                self.step();
            } else if c == q {
                return;
            }
        }
    }
}

pub(crate) fn first_problem(text: &str, lang: &str) -> Option<(u32, u32, String)> {
    let syn = syntax(lang)?;
    if text.len() > MAX_BYTES {
        return None;
    }
    let mut cur = Cursor { b: text.as_bytes(), i: 0, row: 0, col: 0 };
    let mut open: Vec<(u8, u32, u32)> = Vec::new();
    while cur.i < cur.b.len() {
        let c = cur.b[cur.i];
        if syn.line.iter().any(|s| cur.at(s)) {
            cur.skip_past("\n");
            continue;
        }
        if syn.block && cur.at("/*") {
            cur.skip_past("*/");
            continue;
        }
        if syn.raw && c == b'r' && !cur.after_ident() {
            let hashes = cur.b[cur.i + 1..].iter().take_while(|&&h| h == b'#').count();
            if cur.b.get(cur.i + 1 + hashes) == Some(&b'"') {
                (0..hashes + 2).for_each(|_| cur.step());
                cur.skip_past(&format!("\"{}", "#".repeat(hashes)));
                continue;
            }
        }
        if syn.quotes.contains(&c) {
            cur.skip_string();
            continue;
        }
        if c == b'\'' {
            match (cur.b.get(cur.i + 1), cur.b.get(cur.i + 2)) {
                (Some(b'\\'), _) => {
                    cur.step();
                    cur.step();
                    cur.skip_past("'");
                }
                (Some(_), Some(b'\'')) => (0..3).for_each(|_| cur.step()),
                _ => cur.step(),
            }
            continue;
        }
        if let b')' | b']' | b'}' = c {
            let want = match c {
                b')' => b'(',
                b']' => b'[',
                _ => b'{',
            };
            let close = c as char;
            match open.pop() {
                None => return Some((cur.row, cur.col, crate::trf!("Unmatched {b}"; b => close))),
                Some((o, ..)) if o != want => {
                    let msg = crate::trf!("{b} does not close {open}"; b => close, open => o as char);
                    return Some((cur.row, cur.col, msg));
                }
                Some(_) => {}
            }
        } else if let b'(' | b'[' | b'{' = c {
            open.push((c, cur.row, cur.col));
        }
        cur.step();
    }
    open.pop().map(|(o, row, col)| (row, col, crate::trf!("Unclosed {b}"; b => o as char)))
}

#[cfg(test)]
mod tests {
    use super::first_problem;

    fn at(text: &str, lang: &str) -> Option<(u32, u32)> {
        first_problem(text, lang).map(|(r, c, _)| (r, c))
    }

    #[test]
    fn balanced_code_has_no_problem() {
        assert_eq!(at("fn main() { let v = [1, (2)]; }\n", "rust"), None);
        assert_eq!(at("def f(x):\n    return {'a': [x]}\n", "python"), None);
    }

    #[test]
    fn extra_closing_brace_is_found() {
        let (row, col, msg) = first_problem("fn a() {\n    x();\n}}\n", "rust").unwrap();
        assert_eq!((row, col), (2, 1));
        assert_eq!(msg, "Unmatched }");
    }

    #[test]
    fn mismatch_and_unclosed_are_found() {
        assert_eq!(first_problem("f(a]", "javascript").unwrap().2, "] does not close (");
        assert_eq!(first_problem("if (x) {\n  y();\n", "c").unwrap(), (0, 7, "Unclosed {".to_string()));
    }

    #[test]
    fn brackets_in_strings_comments_and_chars_are_ignored() {
        assert_eq!(at("let s = \"{\\\"(\"; // )\n/* ] */ let c = '{'; fn f<'a>() {}\n", "rust"), None);
        assert_eq!(at("let j = r#\"{\"a\":\"]\"}\"#; let p = r\"C:\\\"; f(br\"[\");\n", "rust"), None);
        assert_eq!(at("x = \"\"\"\n(\n\"\"\"  # ]\ny = '['\n", "python"), None);
        assert_eq!(at("a { background: url(http://x/y.png); }\n", "css"), None);
        assert_eq!(at("const t = `${a} }`;\n", "typescript"), None);
    }

    #[test]
    fn fluxcode_own_sources_show_no_false_positive() {
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
            assert_eq!(first_problem(&text, "rust"), None, "{}", f.display());
        }
    }

    #[test]
    fn columns_count_characters_and_unknown_languages_are_skipped() {
        assert_eq!(at("s = 'é'\n)", "python"), Some((1, 0)));
        assert_eq!(at("é)", "rust"), Some((0, 1)));
        assert_eq!(at(":) }", "markdown"), None);
    }
}
