//! Files attached to an agent message: what kind each is and how it is sent.
//!
//! - Images go as ACP image blocks (base64) when the agent accepts images.
//! - Text files (code, txt, md, csv, json, ...) go inline as text resources.
//! - Word, Excel and PowerPoint files are ZIP archives of XML: their text is
//!   read here (a small ZIP reader + `miniz_oxide`, both already in the build)
//!   and sent as text, plus a link to the original.
//! - Anything else (PDF, archives, media) goes as a link the agent opens with
//!   its own tools.

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Image,
    Text,
    Office,
    Pdf,
    Other,
}

pub fn kind(path: &Path) -> Kind {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => Kind::Image,
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" => Kind::Office,
        "pdf" => Kind::Pdf,
        "txt" | "md" | "markdown" | "csv" | "tsv" | "json" | "jsonc" | "yaml" | "yml" | "toml" | "ini" | "cfg" | "log" | "xml"
        | "html" | "htm" | "css" | "scss" | "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx" | "py" | "pyw" | "rs" | "go" | "java"
        | "kt" | "c" | "h" | "cpp" | "hpp" | "cc" | "cs" | "php" | "rb" | "lua" | "sh" | "bat" | "cmd" | "ps1" | "sql" | "vue"
        | "svelte" | "svg" | "env" | "gitignore" | "dockerfile" | "r" | "dart" | "swift" => Kind::Text,
        // No extension: text if it decodes as UTF-8.
        "" => Kind::Text,
        _ => Kind::Other,
    }
}

pub fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "csv" => "text/csv",
        "json" => "application/json",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        _ => "text/plain",
    }
}

pub fn file_uri(path: &Path) -> String {
    format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
}

// ------------------------------------------------------------ ZIP (read only)

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}
fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The files in a ZIP archive: (name, compressed data offset, compressed size, method, size).
pub(crate) fn zip_entries(b: &[u8]) -> Option<Vec<(String, usize, usize, u16, usize)>> {
    // End of central directory: signature PK\5\6, searched from the end (comment ≤ 64 KiB).
    let start = b.len().saturating_sub(22 + 65_535);
    let eocd = (start..b.len().saturating_sub(21)).rev().find(|&i| b[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])?;
    let count = u16le(b, eocd + 10)? as usize;
    let mut at = u32le(b, eocd + 16)? as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if b.get(at..at + 4)? != [0x50, 0x4b, 0x01, 0x02] {
            return None;
        }
        let method = u16le(b, at + 10)?;
        let csize = u32le(b, at + 20)? as usize;
        let usize_ = u32le(b, at + 24)? as usize;
        let nlen = u16le(b, at + 28)? as usize;
        let elen = u16le(b, at + 30)? as usize;
        let clen = u16le(b, at + 32)? as usize;
        let local = u32le(b, at + 42)? as usize;
        let name = String::from_utf8_lossy(b.get(at + 46..at + 46 + nlen)?).into_owned();
        // Local header: fixed 30 bytes + its own name and extra field.
        let lnlen = u16le(b, local + 26)? as usize;
        let lelen = u16le(b, local + 28)? as usize;
        out.push((name, local + 30 + lnlen + lelen, csize, method, usize_));
        at += 46 + nlen + elen + clen;
    }
    Some(out)
}

/// One file from a ZIP archive, decompressed (stored or deflate). Capped at 20 MB.
fn zip_read(b: &[u8], entries: &[(String, usize, usize, u16, usize)], name: &str) -> Option<Vec<u8>> {
    let (_, off, csize, method, size) = entries.iter().find(|e| e.0 == name)?;
    if *size > 20 * 1024 * 1024 {
        return None;
    }
    let data = b.get(*off..*off + *csize)?;
    match method {
        0 => Some(data.to_vec()),
        8 => miniz_oxide::inflate::decompress_to_vec(data).ok(),
        _ => None,
    }
}

// ------------------------------------------------------------ XML text

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Text of every `<tag ...>text</tag>` in order (tag given without `<`, e.g. "w:t").
fn tag_texts<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        // `<w:t>` or `<w:t xml:space=..>`, but not `<w:tab/>` or `<w:tbl>`.
        if !after.starts_with('>') && !after.starts_with(' ') {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else { break };
        if after[..gt].ends_with('/') {
            rest = &after[gt..];
            continue;
        }
        let body = &after[gt + 1..];
        let Some(end) = body.find(&close) else { break };
        out.push(&body[..end]);
        rest = &body[end + close.len()..];
    }
    out
}

fn docx_text(xml: &str) -> String {
    let mut out = String::new();
    for para in xml.split("</w:p>") {
        let line: String = tag_texts(para, "w:t").into_iter().map(unescape).collect();
        if !line.trim().is_empty() {
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    out
}

/// Column letters of a cell reference ("BC12" -> 54), 1-based.
fn col_index(r: &str) -> usize {
    r.chars().take_while(|c| c.is_ascii_alphabetic()).fold(0, |n, c| n * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1))
}

fn xlsx_sheet(xml: &str, shared: &[String], max_rows: usize) -> String {
    let mut out = String::new();
    for (n, row) in xml.split("</row>").enumerate() {
        if n >= max_rows {
            out.push_str("…\n");
            break;
        }
        let mut cells: Vec<(usize, String)> = Vec::new();
        for cell in row.split("<c ").skip(1) {
            let head = &cell[..cell.find('>').unwrap_or(0)];
            let attr = |k: &str| head.split(&format!("{k}=\"")).nth(1).and_then(|v| v.split('"').next());
            let col = attr("r").map(col_index).unwrap_or(cells.len() + 1);
            let t = attr("t").unwrap_or("");
            let v = tag_texts(cell, "v").first().map(|v| unescape(v)).unwrap_or_default();
            let text = match t {
                "s" => v.parse::<usize>().ok().and_then(|i| shared.get(i).cloned()).unwrap_or_default(),
                "inlineStr" => tag_texts(cell, "t").into_iter().map(unescape).collect(),
                _ => v,
            };
            cells.push((col, text));
        }
        if cells.iter().all(|c| c.1.is_empty()) {
            continue;
        }
        let width = cells.iter().map(|c| c.0).max().unwrap_or(0);
        let mut line = vec![String::new(); width];
        for (col, text) in cells {
            if col >= 1 {
                line[col - 1] = text.replace(['\t', '\n'], " ");
            }
        }
        out.push_str(&line.join("\t"));
        out.push('\n');
    }
    out
}

/// The readable text of a .docx / .xlsx / .pptx file (tab-separated rows for sheets).
pub fn office_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let entries = zip_entries(&bytes).ok_or("not a readable Office file")?;
    let read = |name: &str| zip_read(&bytes, &entries, name).map(|b| String::from_utf8_lossy(&b).into_owned());
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut out = String::new();
    match ext.as_str() {
        "docx" => out = docx_text(&read("word/document.xml").ok_or("no document text")?),
        "xlsx" => {
            let shared: Vec<String> = read("xl/sharedStrings.xml")
                .map(|x| x.split("</si>").filter(|s| s.contains("<si")).map(|si| tag_texts(si, "t").into_iter().map(unescape).collect()).collect())
                .unwrap_or_default();
            let mut sheets: Vec<&String> = entries.iter().map(|e| &e.0).filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml")).collect();
            sheets.sort_by_key(|n| n.trim_start_matches("xl/worksheets/sheet").trim_end_matches(".xml").parse::<usize>().unwrap_or(0));
            for (i, s) in sheets.iter().enumerate() {
                if let Some(x) = read(s) {
                    out.push_str(&format!("## Sheet {}\n", i + 1));
                    out.push_str(&xlsx_sheet(&x, &shared, 2000));
                }
            }
        }
        "pptx" => {
            let mut slides: Vec<&String> = entries.iter().map(|e| &e.0).filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml")).collect();
            slides.sort_by_key(|n| n.trim_start_matches("ppt/slides/slide").trim_end_matches(".xml").parse::<usize>().unwrap_or(0));
            for (i, s) in slides.iter().enumerate() {
                if let Some(x) = read(s) {
                    out.push_str(&format!("## Slide {}\n", i + 1));
                    for para in x.split("</a:p>") {
                        let line: String = tag_texts(para, "a:t").into_iter().map(unescape).collect();
                        if !line.trim().is_empty() {
                            out.push_str(&line);
                            out.push('\n');
                        }
                    }
                }
            }
        }
        "odt" | "ods" | "odp" => {
            let x = read("content.xml").ok_or("no content")?;
            for para in x.split("</text:p>") {
                let mut line = String::new();
                let mut in_tag = false;
                for ch in para.chars() {
                    match ch {
                        '<' => in_tag = true,
                        '>' => in_tag = false,
                        c if !in_tag => line.push(c),
                        _ => {}
                    }
                }
                let line = unescape(line.trim());
                if !line.is_empty() {
                    out.push_str(&line);
                    out.push('\n');
                }
            }
        }
        _ => return Err("not an Office file".into()),
    }
    if out.len() > 400_000 {
        out.truncate(400_000);
        out.push_str("\n… (cut at 400 KB)");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal ZIP with stored (uncompressed) files.
    fn zip(files: &[(&str, &str)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data) in files {
            let off = out.len() as u32;
            out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data.as_bytes());
            central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            central.extend_from_slice(&off.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_off = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    fn tmp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("fluxdocs-{}-{name}", std::process::id()));
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn reads_docx() {
        let xml = r#"<w:document><w:body><w:p><w:r><w:t>Halo</w:t></w:r><w:r><w:t xml:space="preserve"> dunia &amp; kopi</w:t></w:r></w:p><w:p><w:r><w:tab/><w:t>Baris 2</w:t></w:r></w:p></w:body></w:document>"#;
        let p = tmp("a.docx", &zip(&[("word/document.xml", xml)]));
        assert_eq!(office_text(&p).unwrap(), "Halo dunia & kopi\nBaris 2\n");
    }

    #[test]
    fn reads_xlsx() {
        let shared = r#"<sst><si><t>Menu</t></si><si><t>Harga</t></si><si><t>Kopi</t></si></sst>"#;
        let sheet = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row><row r="2"><c r="A2" t="s"><v>2</v></c><c r="C2"><v>15000</v></c></row></sheetData></worksheet>"#;
        let p = tmp("b.xlsx", &zip(&[("xl/sharedStrings.xml", shared), ("xl/worksheets/sheet1.xml", sheet)]));
        assert_eq!(office_text(&p).unwrap(), "## Sheet 1\nMenu\tHarga\nKopi\t\t15000\n");
    }

    #[test]
    fn kinds() {
        assert_eq!(kind(Path::new("a.PNG")), Kind::Image);
        assert_eq!(kind(Path::new("laporan.xlsx")), Kind::Office);
        assert_eq!(kind(Path::new("tugas.pdf")), Kind::Pdf);
        assert_eq!(kind(Path::new("app.py")), Kind::Text);
        assert_eq!(kind(Path::new("x.zip")), Kind::Other);
        assert_eq!(col_index("BC12"), 55);
    }
}
