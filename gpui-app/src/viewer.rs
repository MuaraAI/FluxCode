//! Read-only views of files a text editor can't show: Office documents and PDFs
//! as their text, archives as a file list, SQLite databases as their tables, and
//! any other binary file as a hex dump. Everything is read natively with the
//! ZIP reader in `docs.rs` and `miniz_oxide` (already in the build).

use std::path::Path;

/// How much of an unknown binary file the hex view shows.
const HEX_BYTES: usize = 64 * 1024;

pub struct View {
    /// What the editor shows (read only).
    pub text: String,
    /// Highlighter token for that text ("text", "csv", "sql", ...).
    pub lang: &'static str,
    /// Status-bar name of the view ("Hex", "Word document", ...).
    pub label: &'static str,
    /// One line above the view: what this is and what is shown.
    pub banner: String,
}

pub fn human_size(n: u64) -> String {
    match n {
        0..=1023 => format!("{n} B"),
        1024..=1_048_575 => format!("{:.1} KB", n as f64 / 1024.),
        _ => format!("{:.1} MB", n as f64 / 1_048_576.),
    }
}

fn ext_of(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// True when the bytes look like text (no NUL near the start, mostly valid UTF-8).
pub fn looks_text(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(8192)];
    if head.contains(&0) {
        // UTF-16 with a BOM is text too; it is decoded by `decode_utf16`.
        return head.starts_with(&[0xff, 0xfe]) || head.starts_with(&[0xfe, 0xff]);
    }
    true
}

/// UTF-16 (with BOM) to a String, for files Windows tools write that way.
pub fn decode_utf16(bytes: &[u8]) -> Option<String> {
    let le = bytes.starts_with(&[0xff, 0xfe]);
    let be = bytes.starts_with(&[0xfe, 0xff]);
    if !le && !be {
        return None;
    }
    let units: Vec<u16> = bytes[2..]
        .chunks_exact(2)
        .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
        .collect();
    Some(String::from_utf16_lossy(&units))
}

/// A view for a file that isn't plain text, or None when the editor can show it.
pub fn view(path: &Path, bytes: &[u8]) -> Option<View> {
    let ext = ext_of(path);
    let size = human_size(bytes.len() as u64);
    if matches!(ext.as_str(), "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp") {
        let (label, what) = match ext.as_str() {
            "docx" | "odt" => (crate::i18n::tr("Word document"), crate::i18n::tr("the document's text")),
            "xlsx" | "ods" => (crate::i18n::tr("Spreadsheet"), crate::i18n::tr("every sheet as tab-separated rows")),
            _ => (crate::i18n::tr("Slides"), crate::i18n::tr("the text of each slide")),
        };
        return Some(match crate::docs::office_text(path) {
            Ok(text) => View {
                lang: if ext == "xlsx" || ext == "ods" { "tsv" } else { "markdown" },
                text,
                label,
                banner: crate::trf!("{label} · {size} · showing {what}, read only"; label => label, size => size, what => what),
            },
            Err(e) => hex_view(bytes, &crate::trf!("{label} · {size} · couldn't read it ({e}), showing the bytes"; label => label, size => size, e => e)),
        });
    }
    if bytes.starts_with(b"%PDF") {
        return Some(pdf_view(bytes, &size));
    }
    if bytes.starts_with(b"SQLite format 3\0") {
        return Some(sqlite_view(bytes, &size));
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        if let Some(v) = zip_view(bytes, &size) {
            return Some(v);
        }
    }
    if looks_text(bytes) {
        return None;
    }
    let what = kind_name(&ext, bytes);
    let more = if bytes.len() > HEX_BYTES { crate::trf!(", first {0} shown"; 0 => human_size(HEX_BYTES as u64)) } else { String::new() };
    Some(hex_view(bytes, &crate::trf!("{what} · {size}{more} · hex view, read only"; what => what, size => size, more => more)))
}

/// A short name for a binary file, from its magic bytes or extension.
fn kind_name(ext: &str, b: &[u8]) -> String {
    let magic: &[(&[u8], &str)] = &[
        (b"MZ", "Windows program"),
        (b"\x7fELF", "Linux program"),
        (b"\xca\xfe\xba\xbe", "Java class"),
        (b"\0asm", "WebAssembly module"),
        (b"\x1f\x8b", "Gzip archive"),
        (b"7z\xbc\xaf\x27\x1c", "7-Zip archive"),
        (b"Rar!", "RAR archive"),
        (b"ID3", "MP3 audio"),
        (b"OggS", "Ogg media"),
        (b"fLaC", "FLAC audio"),
        (b"RIFF", "RIFF media (WAV/AVI)"),
        (b"wOFF", "Web font"),
        (b"wOF2", "Web font"),
        (b"\0\x01\0\0", "TrueType font"),
        (b"OTTO", "OpenType font"),
    ];
    if let Some((_, n)) = magic.iter().find(|(m, _)| b.starts_with(m)) {
        return (*n).to_string();
    }
    if b.len() > 8 && &b[4..8] == b"ftyp" {
        return "MP4 media".into();
    }
    match ext {
        "pyc" => "Python bytecode".into(),
        "dll" | "exe" | "so" | "dylib" => "Program library".into(),
        "db" | "sqlite" | "sqlite3" => "Database".into(),
        "" => "Binary file".into(),
        e => crate::trf!(".{e} binary file"; e => e),
    }
}

/// Classic hex dump: offset, 16 bytes in two groups, the printable characters.
pub fn hex_dump(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().min(HEX_BYTES) / 16 * 78);
    for (i, row) in bytes[..bytes.len().min(HEX_BYTES)].chunks(16).enumerate() {
        out.push_str(&format!("{:08x}  ", i * 16));
        for j in 0..16 {
            match row.get(j) {
                Some(b) => out.push_str(&format!("{b:02x} ")),
                None => out.push_str("   "),
            }
            if j == 7 {
                out.push(' ');
            }
        }
        out.push_str(" |");
        out.extend(row.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }));
        out.push_str("|\n");
    }
    out
}

fn hex_view(bytes: &[u8], banner: &str) -> View {
    View { text: hex_dump(bytes), lang: "text", label: crate::i18n::tr("Hex"), banner: banner.to_string() }
}

// ---------------------------------------------------------------- archives

fn zip_view(bytes: &[u8], size: &str) -> Option<View> {
    let entries = crate::docs::zip_entries(bytes)?;
    let total: usize = entries.iter().map(|e| e.4).sum();
    let mut text = String::new();
    for (name, _, csize, _, usize_) in &entries {
        if name.ends_with('/') {
            continue;
        }
        let ratio = if *usize_ > 0 { 100 - (*csize * 100 / usize_).min(100) } else { 0 };
        text.push_str(&format!("{:>10}  {:>3}%  {name}\n", human_size(*usize_ as u64), ratio));
    }
    let files = entries.iter().filter(|e| !e.0.ends_with('/')).count();
    Some(View {
        text,
        lang: "text",
        label: crate::i18n::tr("Archive"),
        banner: crate::trf!("ZIP archive · {size} · {files} files, {0} unpacked · size, space saved, path"; size => size, files => files, 0 => human_size(total as u64)),
    })
}

// ---------------------------------------------------------------- PDF

/// Text of a PDF: inflate its content streams and collect the strings drawn
/// with Tj / TJ / ' / ". Works for most PDFs made from documents; scanned
/// pages and fonts with custom encodings have no extractable text.
pub fn pdf_text(b: &[u8]) -> String {
    let mut out = String::new();
    let mut at = 0;
    while let Some(i) = find(b, b"stream", at) {
        let mut s = i + 6;
        if b.get(s) == Some(&b'\r') {
            s += 1;
        }
        if b.get(s) == Some(&b'\n') {
            s += 1;
        }
        let Some(e) = find(b, b"endstream", s) else { break };
        // The stream's dictionary sits just before "stream".
        let dict_start = i.saturating_sub(400);
        let dict = &b[dict_start..i];
        let raw = &b[s..e];
        let data = if find(dict, b"FlateDecode", 0).is_some() {
            miniz_oxide::inflate::decompress_to_vec_zlib(raw).ok()
        } else if find(dict, b"/Filter", 0).is_none() {
            Some(raw.to_vec())
        } else {
            None
        };
        if let Some(d) = data {
            if find(&d, b"BT", 0).is_some() {
                content_text(&d, &mut out);
            }
        }
        at = e + 9;
    }
    out
}

fn find(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
    if from >= h.len() || n.is_empty() {
        return None;
    }
    h[from..].windows(n.len()).position(|w| w == n).map(|p| p + from)
}

/// Strings shown by one content stream, a new line per text line.
fn content_text(d: &[u8], out: &mut String) {
    let mut i = 0;
    let mut line = String::new();
    let flush = |line: &mut String, out: &mut String| {
        let t = line.trim_end();
        if !t.is_empty() {
            out.push_str(t);
            out.push('\n');
        }
        line.clear();
    };
    while i < d.len() {
        match d[i] {
            b'(' => {
                let (s, next) = pdf_string(d, i + 1);
                line.push_str(&s);
                i = next;
            }
            // A big kerning gap inside TJ is a space.
            b'-' if d.get(i + 1).is_some_and(|c| c.is_ascii_digit()) => {
                let mut j = i + 1;
                while j < d.len() && (d[j].is_ascii_digit() || d[j] == b'.') {
                    j += 1;
                }
                if std::str::from_utf8(&d[i + 1..j]).ok().and_then(|n| n.parse::<f32>().ok()).is_some_and(|n| n > 180.) && !line.ends_with(' ') {
                    line.push(' ');
                }
                i = j;
            }
            b'T' if matches!(d.get(i + 1), Some(b'*' | b'd' | b'D')) => {
                flush(&mut line, out);
                i += 2;
            }
            b'E' if d.get(i + 1) == Some(&b'T') => {
                flush(&mut line, out);
                i += 2;
            }
            b'\'' | b'"' => {
                flush(&mut line, out);
                i += 1;
            }
            _ => i += 1,
        }
    }
    flush(&mut line, out);
}

/// A literal string `( ... )` starting after the parenthesis; returns it and the index after `)`.
fn pdf_string(d: &[u8], mut i: usize) -> (String, usize) {
    let mut s = Vec::new();
    let mut depth = 0;
    while i < d.len() {
        match d[i] {
            b'\\' if i + 1 < d.len() => {
                i += 1;
                match d[i] {
                    b'n' => s.push(b'\n'),
                    b'r' => {}
                    b't' => s.push(b'\t'),
                    b'0'..=b'7' => {
                        let mut v = 0u32;
                        let mut k = 0;
                        while k < 3 && i < d.len() && (b'0'..=b'7').contains(&d[i]) {
                            v = v * 8 + (d[i] - b'0') as u32;
                            i += 1;
                            k += 1;
                        }
                        s.push(v as u8);
                        continue;
                    }
                    c => s.push(c),
                }
            }
            b'(' => {
                depth += 1;
                s.push(b'(');
            }
            b')' if depth == 0 => return (latin1(&s), i + 1),
            b')' => {
                depth -= 1;
                s.push(b')');
            }
            c => s.push(c),
        }
        i += 1;
    }
    (latin1(&s), i)
}

fn latin1(b: &[u8]) -> String {
    b.iter().filter(|&&c| c >= 0x20 || c == b'\t').map(|&c| c as char).collect()
}

fn pdf_view(bytes: &[u8], size: &str) -> View {
    let pages = count(bytes, b"/Type /Page") + count(bytes, b"/Type/Page") - count(bytes, b"/Type /Pages") - count(bytes, b"/Type/Pages");
    let text = pdf_text(bytes);
    let letters = text.chars().filter(|c| c.is_alphanumeric()).count();
    let pages_s = match pages {
        0 => String::new(),
        1 => crate::i18n::tr("1 page · ").to_string(),
        n => crate::trf!("{pages} pages · "; pages => n),
    };
    if letters >= 20 && letters * 2 >= text.chars().filter(|c| !c.is_whitespace()).count() {
        View { text, lang: "text", label: "PDF", banner: crate::trf!("PDF · {pages_s}{size} · showing its text, read only"; pages_s => pages_s, size => size) }
    } else {
        let mut v = hex_view(bytes, &crate::trf!("PDF · {pages_s}{size} · no readable text (scanned or custom fonts), showing the bytes"; pages_s => pages_s, size => size));
        v.label = "PDF";
        v
    }
}

fn count(h: &[u8], n: &[u8]) -> usize {
    h.windows(n.len()).filter(|w| *w == n).count()
}

// ---------------------------------------------------------------- SQLite

/// Tables, indexes and views of an SQLite database with their SQL and row
/// counts, read straight from the file's b-tree pages (no SQLite library).
fn sqlite_view(b: &[u8], size: &str) -> View {
    let page = match u16::from_be_bytes([b[16], b[17]]) {
        1 => 65536,
        n => n as usize,
    };
    let mut rows = Vec::new();
    let mut budget = 20_000;
    table_rows(b, page, 1, &mut rows, &mut budget);
    let mut text = String::new();
    let mut tables = 0;
    for r in &rows {
        // sqlite_schema columns: type, name, tbl_name, rootpage, sql
        let get = |i: usize| match r.get(i) {
            Some(Val::Text(s)) => s.clone(),
            Some(Val::Int(n)) => n.to_string(),
            _ => String::new(),
        };
        let kind = get(0);
        let name = get(1);
        let sql = get(4);
        let root = match r.get(3) {
            Some(Val::Int(n)) => *n as usize,
            _ => 0,
        };
        if kind == "table" {
            tables += 1;
            let mut budget = 50_000;
            let n = if root > 0 { count_rows(b, page, root, &mut budget) } else { 0 };
            let approx = if budget == 0 { "+" } else { "" };
            text.push_str(&crate::trf!("-- table {name}: {n}{approx} rows\n"; name => name, n => n, approx => approx));
        } else {
            text.push_str(&format!("-- {kind} {name}\n"));
        }
        if !sql.is_empty() {
            text.push_str(&sql);
            text.push_str(";\n\n");
        } else {
            text.push('\n');
        }
    }
    if text.is_empty() {
        text = "-- No tables.\n".into();
    }
    View { text, lang: "sql", label: "SQLite", banner: crate::trf!("SQLite database · {size} · {tables} tables with their SQL and row counts, read only"; size => size, tables => tables) }
}

#[derive(Debug)]
enum Val {
    Null,
    Int(i64),
    Text(String),
    Other,
}

fn varint(b: &[u8], at: usize) -> (u64, usize) {
    let mut v: u64 = 0;
    for i in 0..9 {
        let Some(&c) = b.get(at + i) else { return (v, i.max(1)) };
        if i == 8 {
            return ((v << 8) | c as u64, 9);
        }
        v = (v << 7) | (c & 0x7f) as u64;
        if c & 0x80 == 0 {
            return (v, i + 1);
        }
    }
    (v, 9)
}

fn page_at(b: &[u8], page: usize, n: usize) -> Option<(&[u8], usize)> {
    let start = (n - 1) * page;
    let p = b.get(start..start + page)?;
    // Page 1 starts with the 100-byte file header.
    Some((p, if n == 1 { 100 } else { 0 }))
}

/// Every row (local payload only) of the table b-tree rooted at page `n`.
fn table_rows(b: &[u8], page: usize, n: usize, out: &mut Vec<Vec<Val>>, budget: &mut usize) {
    if *budget == 0 {
        return;
    }
    *budget -= 1;
    let Some((p, h)) = page_at(b, page, n) else { return };
    let kind = p[h];
    let cells = u16::from_be_bytes([p[h + 3], p[h + 4]]) as usize;
    let hdr = if kind == 0x05 { 12 } else { 8 };
    for i in 0..cells {
        let at = h + hdr + 2 * i;
        let Some(off) = p.get(at..at + 2).map(|x| u16::from_be_bytes([x[0], x[1]]) as usize) else { return };
        match kind {
            0x05 => {
                if let Some(c) = p.get(off..off + 4) {
                    table_rows(b, page, u32::from_be_bytes([c[0], c[1], c[2], c[3]]) as usize, out, budget);
                }
            }
            0x0d => {
                let (len, a) = varint(p, off);
                let (_, r) = varint(p, off + a);
                let start = off + a + r;
                let local = (len as usize).min(page.saturating_sub(35)).min(p.len().saturating_sub(start));
                out.push(record(&p[start..start + local]));
            }
            _ => return,
        }
    }
    if kind == 0x05 {
        let right = u32::from_be_bytes([p[h + 8], p[h + 9], p[h + 10], p[h + 11]]) as usize;
        table_rows(b, page, right, out, budget);
    }
}

fn count_rows(b: &[u8], page: usize, n: usize, budget: &mut usize) -> usize {
    if *budget == 0 {
        return 0;
    }
    *budget -= 1;
    let Some((p, h)) = page_at(b, page, n) else { return 0 };
    let cells = u16::from_be_bytes([p[h + 3], p[h + 4]]) as usize;
    match p[h] {
        0x0d => cells,
        0x05 => {
            let mut total = 0;
            for i in 0..cells {
                let at = h + 12 + 2 * i;
                let Some(off) = p.get(at..at + 2).map(|x| u16::from_be_bytes([x[0], x[1]]) as usize) else { break };
                if let Some(c) = p.get(off..off + 4) {
                    total += count_rows(b, page, u32::from_be_bytes([c[0], c[1], c[2], c[3]]) as usize, budget);
                }
            }
            let right = u32::from_be_bytes([p[h + 8], p[h + 9], p[h + 10], p[h + 11]]) as usize;
            total + count_rows(b, page, right, budget)
        }
        _ => 0,
    }
}

/// Decode an SQLite record (header of serial types, then the values).
fn record(r: &[u8]) -> Vec<Val> {
    let (hlen, mut at) = varint(r, 0);
    let mut types = Vec::new();
    while at < (hlen as usize).min(r.len()) {
        let (t, n) = varint(r, at);
        types.push(t);
        at += n;
    }
    let mut pos = hlen as usize;
    let mut out = Vec::new();
    for t in types {
        let (val, len) = match t {
            0 => (Val::Null, 0),
            1..=6 => {
                let len = [0, 1, 2, 3, 4, 6, 8][t as usize];
                let mut v: i64 = 0;
                for i in 0..len {
                    v = (v << 8) | *r.get(pos + i).unwrap_or(&0) as i64;
                }
                // Sign-extend.
                let shift = 64 - 8 * len as u32;
                ((Val::Int((v << shift) >> shift)), len)
            }
            7 => (Val::Other, 8),
            8 => (Val::Int(0), 0),
            9 => (Val::Int(1), 0),
            t if t >= 13 && t % 2 == 1 => {
                let len = ((t - 13) / 2) as usize;
                let end = (pos + len).min(r.len());
                (Val::Text(String::from_utf8_lossy(r.get(pos..end).unwrap_or(&[])).into_owned()), len)
            }
            t if t >= 12 => (Val::Other, ((t - 12) / 2) as usize),
            _ => (Val::Other, 0),
        };
        out.push(val);
        pos += len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{hex_dump, human_size, looks_text, pdf_text, view};
    use std::path::Path;

    #[test]
    fn hex() {
        let d = hex_dump(b"KIWDB\x00\x01\x02abcdefghijklmnop");
        let first = d.lines().next().unwrap();
        assert!(first.starts_with("00000000  4b 49 57 44 42 00 01 02  61"), "{first}");
        assert!(first.ends_with("|KIWDB...abcdefgh|"), "{first}");
        assert_eq!(d.lines().count(), 2);
        assert_eq!(human_size(2048), "2.0 KB");
    }

    #[test]
    fn binary_vs_text() {
        assert!(looks_text(b"hello\nworld"));
        assert!(looks_text(b""));
        assert!(!looks_text(b"ab\0cd"));
        assert!(view(Path::new("a.txt"), b"plain").is_none());
        let v = view(Path::new("seg-1.klog"), b"\x00\x01KLOG").unwrap();
        assert_eq!(v.label, "Hex");
        assert!(v.banner.starts_with(".klog binary file"), "{}", v.banner);
    }

    #[test]
    fn pdf() {
        let content = b"BT /F1 12 Tf 72 712 Td (Halo dunia) Tj T* [(Baris) -250 (dua)] TJ ET";
        let z = miniz_oxide::deflate::compress_to_vec_zlib(content, 6);
        let mut pdf = b"%PDF-1.4\n1 0 obj << /Length 10 /Filter /FlateDecode >>\nstream\n".to_vec();
        pdf.extend_from_slice(&z);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        assert_eq!(pdf_text(&pdf), "Halo dunia\nBaris dua\n");
    }
}
