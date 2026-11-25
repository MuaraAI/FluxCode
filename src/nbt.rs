use crate::diagnostics::{CodeDiag, Severity};
use crate::viewer::{human_size, View};
use std::fmt::Write;
use std::path::Path;

const MAX_DEPTH: usize = 64;
const MAX_ARRAY_LEN: usize = 4 * 1024 * 1024;
const MAX_LEVELDB_RECORDS: usize = 160;
const MAX_LEVELDB_CHARS: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbtFormat {
    BedrockHeadered { storage_version: u32 },
    BedrockRaw,
    JavaGzip,
    JavaRaw,
}

impl NbtFormat {
    pub fn is_le(self) -> bool {
        matches!(self, Self::BedrockHeadered { .. } | Self::BedrockRaw)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::BedrockHeadered { .. } | Self::BedrockRaw => "Bedrock NBT",
            Self::JavaGzip | Self::JavaRaw => "Java NBT",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List { elem_type: u8, items: Vec<Tag> },
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    pub fn type_id(&self) -> u8 {
        match self {
            Self::Byte(_) => 1,
            Self::Short(_) => 2,
            Self::Int(_) => 3,
            Self::Long(_) => 4,
            Self::Float(_) => 5,
            Self::Double(_) => 6,
            Self::ByteArray(_) => 7,
            Self::String(_) => 8,
            Self::List { .. } => 9,
            Self::Compound(_) => 10,
            Self::IntArray(_) => 11,
            Self::LongArray(_) => 12,
        }
    }

    fn as_compound(&self) -> Option<&[(String, Tag)]> {
        match self {
            Self::Compound(entries) => Some(entries),
            _ => None,
        }
    }

    fn get(&self, key: &str) -> Option<&Tag> {
        self.as_compound()?.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Byte(v) => Some(i64::from(*v)),
            Self::Short(v) => Some(i64::from(*v)),
            Self::Int(v) => Some(i64::from(*v)),
            Self::Long(v) => Some(*v),
            _ => None,
        }
    }
}

pub struct DecodedNbt {
    pub format: NbtFormat,
    pub snbt: String,
    pub label: &'static str,
    pub banner: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnbtError {
    pub row: u32,
    pub col: u32,
    pub end_col: u32,
    pub message: String,
}

impl std::fmt::Display for SnbtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}, col {}: {}", self.row + 1, self.col + 1, self.message)
    }
}

struct BinReader<'a> {
    b: &'a [u8],
    pos: usize,
    le: bool,
}

impl<'a> BinReader<'a> {
    fn new(b: &'a [u8], le: bool) -> Self {
        Self { b, pos: 0, le }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).ok_or_else(|| "offset overflow".to_string())?;
        if end > self.b.len() {
            return Err("unexpected end of NBT data".into());
        }
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn i8(&mut self) -> Result<i8, String> {
        Ok(self.u8()? as i8)
    }

    fn u16(&mut self) -> Result<u16, String> {
        let s = self.take(2)?;
        let a = [s[0], s[1]];
        Ok(if self.le { u16::from_le_bytes(a) } else { u16::from_be_bytes(a) })
    }

    fn i16(&mut self) -> Result<i16, String> {
        Ok(self.u16()? as i16)
    }

    fn i32(&mut self) -> Result<i32, String> {
        let s = self.take(4)?;
        let a = [s[0], s[1], s[2], s[3]];
        Ok(if self.le { i32::from_le_bytes(a) } else { i32::from_be_bytes(a) })
    }

    fn i64(&mut self) -> Result<i64, String> {
        let s = self.take(8)?;
        let a = [s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]];
        Ok(if self.le { i64::from_le_bytes(a) } else { i64::from_be_bytes(a) })
    }

    fn f32(&mut self) -> Result<f32, String> {
        let s = self.take(4)?;
        let a = [s[0], s[1], s[2], s[3]];
        Ok(if self.le { f32::from_le_bytes(a) } else { f32::from_be_bytes(a) })
    }

    fn f64(&mut self) -> Result<f64, String> {
        let s = self.take(8)?;
        let a = [s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]];
        Ok(if self.le { f64::from_le_bytes(a) } else { f64::from_be_bytes(a) })
    }

    fn string(&mut self) -> Result<String, String> {
        let len = self.u16()? as usize;
        let bytes = self.take(len)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    fn read_root(&mut self) -> Result<(String, Tag), String> {
        let kind = self.u8()?;
        if kind != 10 {
            return Err(format!("NBT root must be compound (10), got {kind}"));
        }
        let name = self.string()?;
        let tag = self.read_payload(10, 0)?;
        Ok((name, tag))
    }

    fn read_payload(&mut self, kind: u8, depth: usize) -> Result<Tag, String> {
        if depth > MAX_DEPTH {
            return Err("NBT nesting too deep".into());
        }
        match kind {
            1 => Ok(Tag::Byte(self.i8()?)),
            2 => Ok(Tag::Short(self.i16()?)),
            3 => Ok(Tag::Int(self.i32()?)),
            4 => Ok(Tag::Long(self.i64()?)),
            5 => Ok(Tag::Float(self.f32()?)),
            6 => Ok(Tag::Double(self.f64()?)),
            7 => {
                let len = self.i32()?;
                if len < 0 || (len as usize) > MAX_ARRAY_LEN {
                    return Err("invalid NBT byte array length".into());
                }
                let raw = self.take(len as usize)?;
                Ok(Tag::ByteArray(raw.iter().map(|&b| b as i8).collect()))
            }
            8 => Ok(Tag::String(self.string()?)),
            9 => {
                let elem_type = self.u8()?;
                let len = self.i32()?;
                if len < 0 || (len as usize) > MAX_ARRAY_LEN {
                    return Err("invalid NBT list length".into());
                }
                let len = len as usize;
                if elem_type == 0 && len > 0 {
                    return Err("non-empty TAG_End list".into());
                }
                if elem_type > 12 {
                    return Err(format!("unknown NBT list element type {elem_type}"));
                }
                let mut items = Vec::with_capacity(len.min(1024));
                for _ in 0..len {
                    items.push(self.read_payload(elem_type, depth + 1)?);
                }
                Ok(Tag::List { elem_type, items })
            }
            10 => {
                let mut entries = Vec::new();
                loop {
                    let t = self.u8()?;
                    if t == 0 {
                        break;
                    }
                    if t > 12 {
                        return Err(format!("unknown NBT tag type {t}"));
                    }
                    let key = self.string()?;
                    let val = self.read_payload(t, depth + 1)?;
                    entries.push((key, val));
                }
                Ok(Tag::Compound(entries))
            }
            11 => {
                let len = self.i32()?;
                if len < 0 || (len as usize) > MAX_ARRAY_LEN / 4 {
                    return Err("invalid NBT int array length".into());
                }
                let mut out = Vec::with_capacity((len as usize).min(1024));
                for _ in 0..len {
                    out.push(self.i32()?);
                }
                Ok(Tag::IntArray(out))
            }
            12 => {
                let len = self.i32()?;
                if len < 0 || (len as usize) > MAX_ARRAY_LEN / 8 {
                    return Err("invalid NBT long array length".into());
                }
                let mut out = Vec::with_capacity((len as usize).min(1024));
                for _ in 0..len {
                    out.push(self.i64()?);
                }
                Ok(Tag::LongArray(out))
            }
            _ => Err(format!("unknown NBT tag type {kind}")),
        }
    }
}

struct BinWriter {
    buf: Vec<u8>,
    le: bool,
}

impl BinWriter {
    fn new(le: bool) -> Self {
        Self { buf: Vec::with_capacity(1024), le }
    }

    fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&if self.le { v.to_le_bytes() } else { v.to_be_bytes() });
    }

    fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&if self.le { v.to_le_bytes() } else { v.to_be_bytes() });
    }

    fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&if self.le { v.to_le_bytes() } else { v.to_be_bytes() });
    }

    fn f32(&mut self, v: f32) {
        self.buf.extend_from_slice(&if self.le { v.to_le_bytes() } else { v.to_be_bytes() });
    }

    fn f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&if self.le { v.to_le_bytes() } else { v.to_be_bytes() });
    }

    fn string(&mut self, s: &str) -> Result<(), String> {
        let len = u16::try_from(s.len()).map_err(|_| "NBT string exceeds 65535 bytes".to_string())?;
        self.u16(len);
        self.buf.extend_from_slice(s.as_bytes());
        Ok(())
    }

    fn write_root(&mut self, name: &str, tag: &Tag) -> Result<(), String> {
        if tag.type_id() != 10 {
            return Err("NBT root must be a compound { ... }".into());
        }
        self.u8(10);
        self.string(name)?;
        self.write_payload(tag)
    }

    fn write_payload(&mut self, tag: &Tag) -> Result<(), String> {
        match tag {
            Tag::Byte(v) => self.u8(*v as u8),
            Tag::Short(v) => self.u16(*v as u16),
            Tag::Int(v) => self.i32(*v),
            Tag::Long(v) => self.i64(*v),
            Tag::Float(v) => self.f32(*v),
            Tag::Double(v) => self.f64(*v),
            Tag::ByteArray(items) => {
                self.i32(i32::try_from(items.len()).map_err(|_| "byte array too long")?);
                for &v in items {
                    self.u8(v as u8);
                }
            }
            Tag::String(s) => self.string(s)?,
            Tag::List { elem_type, items } => {
                let et = items.first().map(Tag::type_id).unwrap_or(*elem_type);
                self.u8(et);
                self.i32(i32::try_from(items.len()).map_err(|_| "list too long")?);
                for it in items {
                    if it.type_id() != et {
                        return Err("all items in an NBT list must have the same type".into());
                    }
                    self.write_payload(it)?;
                }
            }
            Tag::Compound(entries) => {
                for (k, v) in entries {
                    self.u8(v.type_id());
                    self.string(k)?;
                    self.write_payload(v)?;
                }
                self.u8(0);
            }
            Tag::IntArray(items) => {
                self.i32(i32::try_from(items.len()).map_err(|_| "int array too long")?);
                for &v in items {
                    self.i32(v);
                }
            }
            Tag::LongArray(items) => {
                self.i32(i32::try_from(items.len()).map_err(|_| "long array too long")?);
                for &v in items {
                    self.i64(v);
                }
            }
        }
        Ok(())
    }
}

fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() < 18 || bytes[0] != 0x1f || bytes[1] != 0x8b || bytes[2] != 8 {
        return Err("not a gzip stream".into());
    }
    let flg = bytes[3];
    let mut pos = 10usize;
    if flg & 0x04 != 0 {
        if pos + 2 > bytes.len() {
            return Err("truncated gzip extra field".into());
        }
        let xlen = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        pos += 2 + xlen;
    }
    if flg & 0x08 != 0 {
        while pos < bytes.len() && bytes[pos] != 0 {
            pos += 1;
        }
        pos += 1;
    }
    if flg & 0x10 != 0 {
        while pos < bytes.len() && bytes[pos] != 0 {
            pos += 1;
        }
        pos += 1;
    }
    if flg & 0x02 != 0 {
        pos += 2;
    }
    if pos + 8 > bytes.len() {
        return Err("truncated gzip header".into());
    }
    let payload = &bytes[pos..bytes.len() - 8];
    miniz_oxide::inflate::decompress_to_vec(payload).map_err(|e| format!("gzip inflate failed: {e:?}"))
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let deflated = miniz_oxide::deflate::compress_to_vec(data, 6);
    let mut out = Vec::with_capacity(18 + deflated.len());
    out.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0xff]);
    out.extend_from_slice(&deflated);
    out.extend_from_slice(&crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

fn ext_lower(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

fn is_nbt_candidate_ext(ext: &str) -> bool {
    matches!(ext, "dat" | "dat_old" | "nbt" | "mcstructure" | "schem" | "schematic")
}

fn try_bedrock_headered(bytes: &[u8]) -> Option<(NbtFormat, String, Tag)> {
    if bytes.len() < 11 {
        return None;
    }
    let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let payload_len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    // Storage versions 1..=32 cover Pocket Edition through modern Bedrock builds.
    if version == 0 || version > 32 || payload_len < 3 || 8 + payload_len > bytes.len() {
        return None;
    }
    if bytes[8] != 10 {
        return None;
    }
    let mut r = BinReader::new(&bytes[8..8 + payload_len], true);
    let (name, tag) = r.read_root().ok()?;
    if r.pos != payload_len {
        return None;
    }
    Some((NbtFormat::BedrockHeadered { storage_version: version }, name, tag))
}

fn try_raw_nbt(bytes: &[u8], le: bool) -> Option<(String, Tag)> {
    if bytes.len() < 4 || bytes[0] != 10 {
        return None;
    }
    let mut r = BinReader::new(bytes, le);
    let (name, tag) = r.read_root().ok()?;
    (r.pos == bytes.len()).then_some((name, tag))
}

pub fn decode_file(path: &Path, bytes: &[u8]) -> Option<DecodedNbt> {
    let ext = ext_lower(path);
    let name_lower = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_ascii_lowercase();
    let is_level_file = matches!(name_lower.as_str(), "level.dat" | "level.dat_old");
    let size = human_size(bytes.len() as u64);

    let (format, root_name, tag) = if let Some(hit) = try_bedrock_headered(bytes) {
        hit
    } else if bytes.starts_with(&[0x1f, 0x8b]) && (is_nbt_candidate_ext(&ext) || is_level_file) {
        let raw = gunzip(bytes).ok()?;
        let (rn, t) = try_raw_nbt(&raw, false)?;
        (NbtFormat::JavaGzip, rn, t)
    } else if is_nbt_candidate_ext(&ext) || is_level_file {
        if let Some((rn, t)) = try_raw_nbt(bytes, true) {
            (NbtFormat::BedrockRaw, rn, t)
        } else if let Some((rn, t)) = try_raw_nbt(bytes, false) {
            (NbtFormat::JavaRaw, rn, t)
        } else {
            return None;
        }
    } else {
        return None;
    };

    let snbt = format_document(format, &root_name, &tag);
    let label = format.label();
    let banner = match format {
        NbtFormat::BedrockHeadered { storage_version } => crate::trf!(
            "Minecraft Bedrock NBT (v{ver}) · {size} · edit values and press Ctrl+S to save back to binary";
            ver => storage_version,
            size => size
        ),
        NbtFormat::BedrockRaw => crate::trf!(
            "Minecraft Bedrock NBT · {size} · edit values and press Ctrl+S to save back to binary";
            size => size
        ),
        NbtFormat::JavaGzip => crate::trf!(
            "Minecraft Java NBT (gzip) · {size} · edit values and press Ctrl+S to save back to binary";
            size => size
        ),
        NbtFormat::JavaRaw => crate::trf!(
            "Minecraft Java NBT · {size} · edit values and press Ctrl+S to save back to binary";
            size => size
        ),
    };
    Some(DecodedNbt { format, snbt, label, banner })
}

pub fn decode_path(path: &Path) -> Option<DecodedNbt> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > 16 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    decode_file(path, &bytes)
}

pub fn encode_file(text: &str, default_format: NbtFormat) -> Result<Vec<u8>, String> {
    let parsed = parse_snbt(text).map_err(|e| e.to_string())?;
    let format = match (default_format, parsed.storage_version) {
        (NbtFormat::BedrockHeadered { .. }, Some(v)) => NbtFormat::BedrockHeadered { storage_version: v },
        (fmt, _) => fmt,
    };
    let mut w = BinWriter::new(format.is_le());
    w.write_root(&parsed.root_name, &parsed.root)?;
    match format {
        NbtFormat::BedrockHeadered { storage_version } => {
            let len = u32::try_from(w.buf.len()).map_err(|_| "NBT payload exceeds 4 GB".to_string())?;
            let mut out = Vec::with_capacity(8 + w.buf.len());
            out.extend_from_slice(&storage_version.to_le_bytes());
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&w.buf);
            Ok(out)
        }
        NbtFormat::BedrockRaw | NbtFormat::JavaRaw => Ok(w.buf),
        NbtFormat::JavaGzip => Ok(gzip(&w.buf)),
    }
}

pub fn diagnose(text: &str) -> Vec<CodeDiag> {
    match parse_snbt(text) {
        Ok(_) => Vec::new(),
        Err(e) => vec![CodeDiag {
            row: e.row,
            col: e.col,
            end_col: e.end_col.max(e.col + 1),
            severity: Severity::Error,
            message: e.message,
            source: "nbt",
        }],
    }
}

pub fn bedrock_world_summary(root: &Path) -> Option<String> {
    let path = if root.join("level.dat").is_file() {
        root.join("level.dat")
    } else if root.join("level.dat_old").is_file() {
        root.join("level.dat_old")
    } else {
        return None;
    };
    let bytes = std::fs::read(&path).ok()?;
    let (fmt, _, tag) = try_bedrock_headered(&bytes)?;
    let ver = match fmt {
        NbtFormat::BedrockHeadered { storage_version } => storage_version,
        _ => 0,
    };
    let name = tag
        .get("LevelName")
        .and_then(Tag::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| std::fs::read_to_string(root.join("levelname.txt")).ok().map(|s| s.trim().to_string()))
        .unwrap_or_else(|| "(unnamed)".into());
    let mut parts = Vec::new();
    if let Some(gt) = tag.get("GameType").and_then(Tag::as_i64) {
        let mode = match gt {
            0 => "Survival",
            1 => "Creative",
            2 => "Adventure",
            _ => "Custom",
        };
        parts.push(mode.to_string());
    }
    if let Some(diff) = tag.get("Difficulty").and_then(Tag::as_i64) {
        let d = match diff {
            0 => "Peaceful",
            1 => "Easy",
            2 => "Normal",
            3 => "Hard",
            _ => "Custom",
        };
        parts.push(d.to_string());
    }
    if let Some(gen_id) = tag.get("Generator").and_then(Tag::as_i64) {
        let g = match gen_id {
            0 => "Old",
            1 => "Infinite",
            2 => "Flat",
            _ => "Custom",
        };
        parts.push(format!("{g} world"));
    }
    if let Some(Tag::List { items, .. }) = tag.get("lastOpenedWithVersion") {
        let nums: Vec<String> = items.iter().take(3).filter_map(Tag::as_i64).map(|n| n.to_string()).collect();
        if !nums.is_empty() {
            parts.push(format!("MC {}", nums.join(".")));
        }
    }
    if let Some(seed) = tag.get("RandomSeed").and_then(Tag::as_i64) {
        parts.push(format!("seed {seed}"));
    }
    let bp = count_json_array(&root.join("world_behavior_packs.json"));
    let rp = count_json_array(&root.join("world_resource_packs.json"));
    if bp > 0 || rp > 0 {
        parts.push(format!("{bp} BP, {rp} RP active"));
    }
    let meta = if parts.is_empty() { String::new() } else { format!(" ({})", parts.join(", ")) };
    Some(format!("Minecraft Bedrock world \"{name}\" [storage v{ver}]{meta}"))
}

fn count_json_array(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.as_array().map(Vec::len))
        .unwrap_or(0)
}

fn format_document(format: NbtFormat, root_name: &str, tag: &Tag) -> String {
    let mut out = String::with_capacity(2048);
    match format {
        NbtFormat::BedrockHeadered { storage_version } => {
            let _ = writeln!(out, "// Minecraft Bedrock NBT | storage_version: {storage_version}");
        }
        NbtFormat::BedrockRaw => out.push_str("// Minecraft Bedrock NBT (raw)\n"),
        NbtFormat::JavaGzip => out.push_str("// Minecraft Java NBT (gzip)\n"),
        NbtFormat::JavaRaw => out.push_str("// Minecraft Java NBT (raw)\n"),
    }
    if !root_name.is_empty() {
        let _ = writeln!(out, "// root_name: {}", quote_str(root_name));
    }
    out.push_str("// Types: b=byte (0b/1b), s=short, int, L=long, f=float, d=double\n");
    format_tag(tag, 0, &mut out);
    out.push('\n');
    out
}

fn tag_type_name(id: u8) -> &'static str {
    match id {
        1 => "byte",
        2 => "short",
        3 => "int",
        4 => "long",
        5 => "float",
        6 => "double",
        7 => "byte_array",
        8 => "string",
        9 => "list",
        10 => "compound",
        11 => "int_array",
        12 => "long_array",
        _ => "end",
    }
}

fn parse_tag_type_name(name: &str) -> Option<u8> {
    Some(match name.trim() {
        "end" => 0,
        "byte" => 1,
        "short" => 2,
        "int" => 3,
        "long" => 4,
        "float" => 5,
        "double" => 6,
        "byte_array" => 7,
        "string" => 8,
        "list" => 9,
        "compound" => 10,
        "int_array" => 11,
        "long_array" => 12,
        _ => return None,
    })
}

fn is_bare_key(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'+'))
}

fn quote_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn format_float(v: f32) -> String {
    if v.is_nan() {
        return "NaNf".into();
    }
    if v.is_infinite() {
        return if v.is_sign_negative() { "-Infinityf".into() } else { "Infinityf".into() };
    }
    let mut s = format!("{v}");
    if !s.contains(['.', 'e', 'E']) {
        s.push_str(".0");
    }
    s.push('f');
    s
}

fn format_double(v: f64) -> String {
    if v.is_nan() {
        return "NaNd".into();
    }
    if v.is_infinite() {
        return if v.is_sign_negative() { "-Infinityd".into() } else { "Infinityd".into() };
    }
    let mut s = format!("{v}");
    if !s.contains(['.', 'e', 'E']) {
        s.push_str(".0");
    }
    s.push('d');
    s
}

fn format_tag(tag: &Tag, indent: usize, out: &mut String) {
    match tag {
        Tag::Byte(v) => {
            let _ = write!(out, "{v}b");
        }
        Tag::Short(v) => {
            let _ = write!(out, "{v}s");
        }
        Tag::Int(v) => {
            let _ = write!(out, "{v}");
        }
        Tag::Long(v) => {
            let _ = write!(out, "{v}L");
        }
        Tag::Float(v) => out.push_str(&format_float(*v)),
        Tag::Double(v) => out.push_str(&format_double(*v)),
        Tag::String(s) => out.push_str(&quote_str(s)),
        Tag::ByteArray(items) => format_typed_array('B', items.iter().map(|v| format!("{v}b")), indent, out),
        Tag::IntArray(items) => format_typed_array('I', items.iter().map(|v| v.to_string()), indent, out),
        Tag::LongArray(items) => format_typed_array('L', items.iter().map(|v| format!("{v}L")), indent, out),
        Tag::List { elem_type, items } => {
            if items.is_empty() {
                if *elem_type == 0 {
                    out.push_str("[]");
                } else {
                    let _ = write!(out, "[/*{}*/]", tag_type_name(*elem_type));
                }
                return;
            }
            let simple = items.len() <= 8
                && items.iter().all(|it| {
                    matches!(it, Tag::Byte(_) | Tag::Short(_) | Tag::Int(_) | Tag::Long(_) | Tag::Float(_) | Tag::Double(_))
                });
            if simple {
                out.push('[');
                for (i, it) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    format_tag(it, indent, out);
                }
                out.push(']');
                return;
            }
            out.push_str("[\n");
            let pad = "  ".repeat(indent + 1);
            for (i, it) in items.iter().enumerate() {
                out.push_str(&pad);
                format_tag(it, indent + 1, out);
                if i + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&"  ".repeat(indent));
            out.push(']');
        }
        Tag::Compound(entries) => {
            if entries.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            let pad = "  ".repeat(indent + 1);
            for (i, (k, v)) in entries.iter().enumerate() {
                out.push_str(&pad);
                if is_bare_key(k) {
                    out.push_str(k);
                } else {
                    out.push_str(&quote_str(k));
                }
                out.push_str(": ");
                format_tag(v, indent + 1, out);
                if i + 1 < entries.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&"  ".repeat(indent));
            out.push('}');
        }
    }
}

fn format_typed_array(prefix: char, iter: impl ExactSizeIterator<Item = String>, indent: usize, out: &mut String) {
    let n = iter.len();
    if n == 0 {
        let _ = write!(out, "[{prefix};]");
        return;
    }
    if n <= 12 {
        let _ = write!(out, "[{prefix}; ");
        for (i, s) in iter.enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&s);
        }
        out.push(']');
        return;
    }
    let _ = writeln!(out, "[{prefix};");
    let pad = "  ".repeat(indent + 1);
    for (i, s) in iter.enumerate() {
        if i % 16 == 0 {
            out.push_str(&pad);
        } else {
            out.push(' ');
        }
        out.push_str(&s);
        if i + 1 < n {
            out.push(',');
        }
        if i % 16 == 15 || i + 1 == n {
            out.push('\n');
        }
    }
    out.push_str(&"  ".repeat(indent));
    out.push(']');
}

struct ParsedSnbt {
    storage_version: Option<u32>,
    root_name: String,
    root: Tag,
}

struct SnbtParser<'a> {
    src: &'a str,
    b: &'a [u8],
    pos: usize,
    row: u32,
    col: u32,
    storage_version: Option<u32>,
    root_name: String,
    last_hint: Option<u8>,
}

impl<'a> SnbtParser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            b: src.as_bytes(),
            pos: 0,
            row: 0,
            col: 0,
            storage_version: None,
            root_name: String::new(),
            last_hint: None,
        }
    }

    fn err(&self, msg: impl Into<String>) -> SnbtError {
        SnbtError { row: self.row, col: self.col, end_col: self.col + 1, message: msg.into() }
    }

    fn err_at(&self, row: u32, col: u32, end_col: u32, msg: impl Into<String>) -> SnbtError {
        SnbtError { row, col, end_col: end_col.max(col + 1), message: msg.into() }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let ch = *self.b.get(self.pos)?;
        self.pos += 1;
        if ch == b'\n' {
            self.row += 1;
            self.col = 0;
        } else if (ch & 0xc0) != 0x80 {
            self.col += 1;
        }
        Some(ch)
    }

    fn skip_ws(&mut self) -> Result<(), SnbtError> {
        loop {
            while let Some(c) = self.peek() {
                if c.is_ascii_whitespace() {
                    self.bump();
                } else {
                    break;
                }
            }
            if self.b.get(self.pos..self.pos + 2) == Some(b"//") {
                let start = self.pos + 2;
                while let Some(c) = self.peek() {
                    if c == b'\n' {
                        break;
                    }
                    self.bump();
                }
                let line = &self.src[start..self.pos];
                if let Some(rest) = line.split("storage_version:").nth(1) {
                    if let Ok(v) = rest.split_whitespace().next().unwrap_or("").parse::<u32>() {
                        self.storage_version = Some(v);
                    }
                }
                if let Some(rest) = line.split("root_name:").nth(1) {
                    let t = rest.trim();
                    if let Some(inner) = t.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                        self.root_name = inner.replace("\\\"", "\"").replace("\\\\", "\\");
                    }
                }
                continue;
            }
            if self.b.get(self.pos..self.pos + 2) == Some(b"/*") {
                let (sr, sc) = (self.row, self.col);
                self.bump();
                self.bump();
                let start = self.pos;
                let mut closed = false;
                while self.pos + 1 < self.b.len() {
                    if self.b[self.pos] == b'*' && self.b[self.pos + 1] == b'/' {
                        let body = &self.src[start..self.pos];
                        if let Some(tid) = parse_tag_type_name(body) {
                            self.last_hint = Some(tid);
                        }
                        self.bump();
                        self.bump();
                        closed = true;
                        break;
                    }
                    self.bump();
                }
                if !closed {
                    return Err(self.err_at(sr, sc, sc + 2, "unterminated block comment"));
                }
                continue;
            }
            break;
        }
        Ok(())
    }

    fn parse_string(&mut self) -> Result<String, SnbtError> {
        let quote = self.peek().ok_or_else(|| self.err("expected string"))?;
        if quote != b'"' && quote != b'\'' {
            return Err(self.err("expected '\"' or '\\''"));
        }
        let (sr, sc) = (self.row, self.col);
        self.bump();
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c == quote {
                self.bump();
                return Ok(out);
            }
            if c == b'\n' {
                return Err(self.err_at(sr, sc, self.col, "unterminated string literal"));
            }
            if c == b'\\' {
                self.bump();
                let esc = self.bump().ok_or_else(|| self.err("unterminated escape sequence"))?;
                match esc {
                    b'"' => out.push('"'),
                    b'\'' => out.push('\''),
                    b'\\' => out.push('\\'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let start = self.pos;
                        for _ in 0..4 {
                            self.bump().ok_or_else(|| self.err("incomplete \\u escape"))?;
                        }
                        let hex = &self.src[start..self.pos];
                        let code = u32::from_str_radix(hex, 16).map_err(|_| self.err("invalid \\u hex escape"))?;
                        let ch = char::from_u32(code).ok_or_else(|| self.err("invalid unicode scalar in \\u escape"))?;
                        out.push(ch);
                    }
                    other => out.push(other as char),
                }
            } else {
                let start = self.pos;
                self.bump();
                while self.pos < self.b.len() && (self.b[self.pos] & 0xc0) == 0x80 {
                    self.pos += 1;
                }
                out.push_str(&self.src[start..self.pos]);
            }
        }
        Err(self.err_at(sr, sc, sc + 1, "unterminated string literal"))
    }

    fn parse_key(&mut self) -> Result<String, SnbtError> {
        self.skip_ws()?;
        match self.peek() {
            Some(b'"' | b'\'') => self.parse_string(),
            Some(_) => {
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b'+') {
                        self.bump();
                    } else {
                        break;
                    }
                }
                if self.pos == start {
                    return Err(self.err("expected key name"));
                }
                Ok(self.src[start..self.pos].to_string())
            }
            None => Err(self.err("unexpected end of input while reading key")),
        }
    }

    fn parse_value(&mut self, depth: usize) -> Result<Tag, SnbtError> {
        if depth > MAX_DEPTH {
            return Err(self.err("NBT nesting too deep"));
        }
        self.skip_ws()?;
        match self.peek() {
            Some(b'{') => self.parse_compound(depth),
            Some(b'[') => self.parse_list_or_array(depth),
            Some(b'"' | b'\'') => Ok(Tag::String(self.parse_string()?)),
            Some(_) => self.parse_scalar(),
            None => Err(self.err("expected NBT value")),
        }
    }

    fn parse_compound(&mut self, depth: usize) -> Result<Tag, SnbtError> {
        self.bump(); // '{'
        let mut entries = Vec::new();
        loop {
            self.skip_ws()?;
            if self.peek() == Some(b'}') {
                self.bump();
                return Ok(Tag::Compound(entries));
            }
            let key = self.parse_key()?;
            self.skip_ws()?;
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':' after compound key"));
            }
            self.bump();
            let val = self.parse_value(depth + 1)?;
            entries.push((key, val));
            self.skip_ws()?;
            match self.peek() {
                Some(b',') => {
                    self.bump();
                }
                Some(b'}') => {
                    self.bump();
                    return Ok(Tag::Compound(entries));
                }
                _ => return Err(self.err("expected ',' or '}' in compound")),
            }
        }
    }

    fn parse_list_or_array(&mut self, depth: usize) -> Result<Tag, SnbtError> {
        self.bump(); // '['
        self.last_hint = None;
        self.skip_ws()?;
        // Typed array syntax: [B; ...], [I; ...], [L; ...]
        if matches!(self.peek(), Some(b'B' | b'I' | b'L')) && self.b.get(self.pos + 1) == Some(&b';') {
            let kind = self.bump().unwrap();
            self.bump(); // ';'
            return self.parse_typed_array(kind);
        }
        if self.peek() == Some(b']') {
            let hint = self.last_hint.take().unwrap_or(0);
            self.bump();
            return Ok(Tag::List { elem_type: hint, items: Vec::new() });
        }
        let mut items: Vec<Tag> = Vec::new();
        let mut elem_type = 0u8;
        loop {
            self.skip_ws()?;
            if self.peek() == Some(b']') {
                self.bump();
                return Ok(Tag::List { elem_type, items });
            }
            let (er, ec) = (self.row, self.col);
            let val = self.parse_value(depth + 1)?;
            if items.is_empty() {
                elem_type = val.type_id();
            } else if val.type_id() != elem_type {
                return Err(self.err_at(
                    er,
                    ec,
                    self.col,
                    format!(
                        "NBT list expects {} elements, found {}",
                        tag_type_name(elem_type),
                        tag_type_name(val.type_id())
                    ),
                ));
            }
            items.push(val);
            self.skip_ws()?;
            match self.peek() {
                Some(b',') => {
                    self.bump();
                }
                Some(b']') => {
                    self.bump();
                    return Ok(Tag::List { elem_type, items });
                }
                _ => return Err(self.err("expected ',' or ']' in list")),
            }
        }
    }

    fn parse_typed_array(&mut self, kind: u8) -> Result<Tag, SnbtError> {
        let mut bytes_out = Vec::new();
        let mut ints_out = Vec::new();
        let mut longs_out = Vec::new();
        loop {
            self.skip_ws()?;
            if self.peek() == Some(b']') {
                self.bump();
                return Ok(match kind {
                    b'B' => Tag::ByteArray(bytes_out),
                    b'I' => Tag::IntArray(ints_out),
                    _ => Tag::LongArray(longs_out),
                });
            }
            let (er, ec) = (self.row, self.col);
            let val = self.parse_scalar()?;
            match (kind, val) {
                (b'B', Tag::Byte(v)) => bytes_out.push(v),
                (b'B', Tag::Int(v)) if i8::try_from(v).is_ok() => bytes_out.push(v as i8),
                (b'I', Tag::Int(v)) => ints_out.push(v),
                (b'L', Tag::Long(v)) => longs_out.push(v),
                (b'L', Tag::Int(v)) => longs_out.push(i64::from(v)),
                _ => return Err(self.err_at(er, ec, self.col, "array element type does not match array header")),
            }
            self.skip_ws()?;
            match self.peek() {
                Some(b',') => {
                    self.bump();
                }
                Some(b']') => {
                    self.bump();
                    return Ok(match kind {
                        b'B' => Tag::ByteArray(bytes_out),
                        b'I' => Tag::IntArray(ints_out),
                        _ => Tag::LongArray(longs_out),
                    });
                }
                _ => return Err(self.err("expected ',' or ']' in typed array")),
            }
        }
    }

    fn parse_scalar(&mut self) -> Result<Tag, SnbtError> {
        let (sr, sc) = (self.row, self.col);
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'+' | b'.' | b'_') {
                self.bump();
            } else {
                break;
            }
        }
        let tok = &self.src[start..self.pos];
        if tok.is_empty() {
            return Err(self.err("unexpected character in NBT value"));
        }
        if tok == "true" {
            return Ok(Tag::Byte(1));
        }
        if tok == "false" {
            return Ok(Tag::Byte(0));
        }
        let last = tok.as_bytes()[tok.len() - 1];
        let body = &tok[..tok.len() - 1];
        if !body.is_empty() {
            match last {
                b'b' | b'B' => {
                    if let Ok(v) = body.parse::<i8>() {
                        return Ok(Tag::Byte(v));
                    }
                    return Err(self.err_at(sr, sc, self.col, format!("invalid byte value '{tok}' (-128..127)")));
                }
                b's' | b'S' => {
                    if let Ok(v) = body.parse::<i16>() {
                        return Ok(Tag::Short(v));
                    }
                    return Err(self.err_at(sr, sc, self.col, format!("invalid short value '{tok}'")));
                }
                b'l' | b'L' => {
                    if let Ok(v) = body.parse::<i64>() {
                        return Ok(Tag::Long(v));
                    }
                    return Err(self.err_at(sr, sc, self.col, format!("invalid long value '{tok}'")));
                }
                b'f' | b'F' => {
                    if let Ok(v) = parse_special_f32(body) {
                        return Ok(Tag::Float(v));
                    }
                    return Err(self.err_at(sr, sc, self.col, format!("invalid float value '{tok}'")));
                }
                b'd' | b'D' => {
                    if let Ok(v) = parse_special_f64(body) {
                        return Ok(Tag::Double(v));
                    }
                    return Err(self.err_at(sr, sc, self.col, format!("invalid double value '{tok}'")));
                }
                _ => {}
            }
        }
        if let Ok(v) = tok.parse::<i32>() {
            return Ok(Tag::Int(v));
        }
        if tok.contains(['.', 'e', 'E']) {
            if let Ok(v) = tok.parse::<f64>() {
                return Ok(Tag::Double(v));
            }
        }
        Err(self.err_at(
            sr,
            sc,
            self.col,
            format!("invalid NBT literal '{tok}' (use quotes for strings, or suffixes b/s/L/f/d for numbers)"),
        ))
    }
}

fn parse_special_f32(s: &str) -> Result<f32, ()> {
    match s {
        "NaN" => Ok(f32::NAN),
        "Infinity" => Ok(f32::INFINITY),
        "-Infinity" => Ok(f32::NEG_INFINITY),
        _ => s.parse::<f32>().map_err(|_| ()),
    }
}

fn parse_special_f64(s: &str) -> Result<f64, ()> {
    match s {
        "NaN" => Ok(f64::NAN),
        "Infinity" => Ok(f64::INFINITY),
        "-Infinity" => Ok(f64::NEG_INFINITY),
        _ => s.parse::<f64>().map_err(|_| ()),
    }
}

fn parse_snbt(text: &str) -> Result<ParsedSnbt, SnbtError> {
    let mut p = SnbtParser::new(text);
    p.skip_ws()?;
    if p.peek() != Some(b'{') {
        return Err(p.err("NBT document must start with '{'"));
    }
    let root = p.parse_compound(0)?;
    p.skip_ws()?;
    if p.peek().is_some() {
        return Err(p.err("unexpected trailing tokens after root compound"));
    }
    Ok(ParsedSnbt {
        storage_version: p.storage_version,
        root_name: p.root_name,
        root,
    })
}

// ---------- Minecraft Bedrock LevelDB inspection (db/*.log and db/*.ldb) ----------

const LDB_MAGIC: u64 = 0xdb47_7524_8b80_fb57;

pub fn leveldb_view(path: &Path, bytes: &[u8]) -> Option<View> {
    let ext = ext_lower(path);
    let size = human_size(bytes.len() as u64);
    if ext == "ldb" && bytes.len() >= 48 {
        let magic = u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().ok()?);
        if magic == LDB_MAGIC {
            return decode_ldb(bytes, &size);
        }
    }
    if ext == "log" && is_bedrock_db_parent(path) && bytes.len() >= 19 {
        if let Some(v) = decode_wal_log(bytes, &size) {
            return Some(v);
        }
    }
    None
}

fn is_bedrock_db_parent(path: &Path) -> bool {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("db"))
}

fn decode_varint(b: &[u8], pos: &mut usize) -> Option<u64> {
    let mut val = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*pos)?;
        *pos += 1;
        val |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(val);
        }
    }
    None
}

fn snappy_decompress(input: &[u8]) -> Option<Vec<u8>> {
    let mut pos = 0usize;
    let target_len = decode_varint(input, &mut pos)? as usize;
    if target_len > 16 * 1024 * 1024 {
        return None;
    }
    let mut out = Vec::with_capacity(target_len.min(65536));
    while pos < input.len() {
        let tag = input[pos];
        pos += 1;
        match tag & 0x03 {
            0 => {
                let mut len = ((tag >> 2) as usize) + 1;
                if len > 60 {
                    let extra = len - 60;
                    if pos + extra > input.len() {
                        return None;
                    }
                    let mut n = 0usize;
                    for i in 0..extra {
                        n |= (input[pos + i] as usize) << (i * 8);
                    }
                    pos += extra;
                    len = n + 1;
                }
                if pos + len > input.len() || out.len() + len > target_len {
                    return None;
                }
                out.extend_from_slice(&input[pos..pos + len]);
                pos += len;
            }
            1 => {
                let len = (((tag >> 2) & 0x07) as usize) + 4;
                let low = *input.get(pos)? as usize;
                pos += 1;
                let offset = (((tag >> 5) as usize) << 8) | low;
                copy_snappy_backref(&mut out, offset, len, target_len)?;
            }
            2 => {
                let len = ((tag >> 2) as usize) + 1;
                if pos + 2 > input.len() {
                    return None;
                }
                let offset = u16::from_le_bytes([input[pos], input[pos + 1]]) as usize;
                pos += 2;
                copy_snappy_backref(&mut out, offset, len, target_len)?;
            }
            _ => {
                let len = ((tag >> 2) as usize) + 1;
                if pos + 4 > input.len() {
                    return None;
                }
                let offset = u32::from_le_bytes([input[pos], input[pos + 1], input[pos + 2], input[pos + 3]]) as usize;
                pos += 4;
                copy_snappy_backref(&mut out, offset, len, target_len)?;
            }
        }
    }
    (out.len() == target_len).then_some(out)
}

fn copy_snappy_backref(out: &mut Vec<u8>, offset: usize, len: usize, limit: usize) -> Option<()> {
    if offset == 0 || offset > out.len() || out.len() + len > limit {
        return None;
    }
    let start = out.len() - offset;
    for i in 0..len {
        let b = out[start + i];
        out.push(b);
    }
    Some(())
}

fn read_ldb_block(file: &[u8], offset: usize, size: usize) -> Option<Vec<u8>> {
    let end = offset.checked_add(size)?;
    if end + 5 > file.len() {
        return None;
    }
    let raw = &file[offset..end];
    match file[end] {
        0 => Some(raw.to_vec()),
        1 => snappy_decompress(raw),
        2 => miniz_oxide::inflate::decompress_to_vec_zlib(raw).ok(),
        4 => miniz_oxide::inflate::decompress_to_vec(raw).ok(),
        _ => None,
    }
}

fn parse_ldb_block_entries(block: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
    if block.len() < 4 {
        return Vec::new();
    }
    let restarts = u32::from_le_bytes(block[block.len() - 4..].try_into().unwrap_or([0; 4])) as usize;
    let limit = block.len().saturating_sub(4 + restarts.saturating_mul(4));
    let mut pos = 0usize;
    let mut last_key = Vec::new();
    let mut out = Vec::new();
    while pos < limit {
        let Some(shared) = decode_varint(block, &mut pos).map(|v| v as usize) else { break };
        let Some(non_shared) = decode_varint(block, &mut pos).map(|v| v as usize) else { break };
        let Some(val_len) = decode_varint(block, &mut pos).map(|v| v as usize) else { break };
        if shared > last_key.len() || pos + non_shared + val_len > limit {
            break;
        }
        let mut key = Vec::with_capacity(shared + non_shared);
        key.extend_from_slice(&last_key[..shared]);
        key.extend_from_slice(&block[pos..pos + non_shared]);
        pos += non_shared;
        let val = block[pos..pos + val_len].to_vec();
        pos += val_len;
        last_key = key.clone();
        out.push((key, val));
    }
    out
}

fn decode_ldb(bytes: &[u8], size: &str) -> Option<View> {
    let footer = &bytes[bytes.len() - 48..bytes.len() - 8];
    let mut fpos = 0usize;
    let _meta_off = decode_varint(footer, &mut fpos)?;
    let _meta_sz = decode_varint(footer, &mut fpos)?;
    let idx_off = decode_varint(footer, &mut fpos)? as usize;
    let idx_sz = decode_varint(footer, &mut fpos)? as usize;
    let idx_block = read_ldb_block(bytes, idx_off, idx_sz)?;
    let index_entries = parse_ldb_block_entries(&idx_block);

    let mut out = String::from("// Minecraft Bedrock LevelDB table (.ldb) | read only\n\n");
    let mut total = 0usize;
    let mut nbt_count = 0usize;
    let mut truncated = false;
    for (_, handle) in &index_entries {
        let mut hpos = 0usize;
        let Some(doff) = decode_varint(handle, &mut hpos).map(|v| v as usize) else { continue };
        let Some(dsz) = decode_varint(handle, &mut hpos).map(|v| v as usize) else { continue };
        let Some(data_block) = read_ldb_block(bytes, doff, dsz) else { continue };
        for (ikey, val) in parse_ldb_block_entries(&data_block) {
            // LevelDB internal keys end with an 8-byte (seq << 8 | type) trailer.
            if ikey.len() < 8 {
                continue;
            }
            let user_key = &ikey[..ikey.len() - 8];
            let kind = ikey[ikey.len() - 8];
            total += 1;
            if total <= MAX_LEVELDB_RECORDS && out.len() < MAX_LEVELDB_CHARS {
                if write_leveldb_entry(&mut out, user_key, kind != 0, &val) {
                    nbt_count += 1;
                }
            } else {
                truncated = true;
            }
        }
    }
    if truncated {
        let _ = writeln!(out, "// ... truncated ({total} total keys in table)");
    }
    Some(View {
        text: out,
        lang: "javascript",
        label: "Bedrock LevelDB",
        banner: crate::trf!(
            "Bedrock LevelDB table · {size} · {keys} keys ({nbt} decoded NBT), read only";
            size => size,
            keys => total,
            nbt => nbt_count
        ),
    })
}

fn decode_wal_log(bytes: &[u8], size: &str) -> Option<View> {
    const BLOCK_SIZE: usize = 32_768;
    let mut pos = 0usize;
    let mut pending: Vec<u8> = Vec::new();
    let mut batches: Vec<Vec<u8>> = Vec::new();

    while pos + 7 <= bytes.len() {
        let rem = BLOCK_SIZE - (pos % BLOCK_SIZE);
        if rem < 7 {
            pos += rem;
            continue;
        }
        let len = u16::from_le_bytes([bytes[pos + 4], bytes[pos + 5]]) as usize;
        let rtype = bytes[pos + 6];
        if rtype == 0 && len == 0 {
            pos += rem;
            continue;
        }
        if !(1..=4).contains(&rtype) || pos + 7 + len > bytes.len() || len + 7 > rem {
            break;
        }
        let chunk = &bytes[pos + 7..pos + 7 + len];
        pos += 7 + len;
        match rtype {
            1 => {
                pending.clear();
                batches.push(chunk.to_vec());
            }
            2 => {
                pending.clear();
                pending.extend_from_slice(chunk);
            }
            3 => pending.extend_from_slice(chunk),
            4 => {
                pending.extend_from_slice(chunk);
                batches.push(std::mem::take(&mut pending));
            }
            _ => break,
        }
    }
    if batches.is_empty() {
        return None;
    }

    let mut out = String::from("// Minecraft Bedrock LevelDB log (.log) | read only\n\n");
    let mut total = 0usize;
    let mut nbt_count = 0usize;
    let mut truncated = false;
    for batch in &batches {
        if batch.len() < 12 {
            continue;
        }
        let count = u32::from_le_bytes([batch[8], batch[9], batch[10], batch[11]]) as usize;
        let mut bpos = 12usize;
        for _ in 0..count {
            if bpos >= batch.len() {
                break;
            }
            let kind = batch[bpos];
            bpos += 1;
            let Some(klen) = decode_varint(batch, &mut bpos).map(|v| v as usize) else { break };
            if bpos + klen > batch.len() {
                break;
            }
            let key = &batch[bpos..bpos + klen];
            bpos += klen;
            let val = if kind == 1 {
                let Some(vlen) = decode_varint(batch, &mut bpos).map(|v| v as usize) else { break };
                if bpos + vlen > batch.len() {
                    break;
                }
                let v = &batch[bpos..bpos + vlen];
                bpos += vlen;
                v
            } else {
                &[]
            };
            total += 1;
            if total <= MAX_LEVELDB_RECORDS && out.len() < MAX_LEVELDB_CHARS {
                if write_leveldb_entry(&mut out, key, kind == 1, val) {
                    nbt_count += 1;
                }
            } else {
                truncated = true;
            }
        }
    }
    if total == 0 {
        return None;
    }
    if truncated {
        let _ = writeln!(out, "// ... truncated ({total} total entries in log)");
    }
    Some(View {
        text: out,
        lang: "javascript",
        label: "Bedrock LevelDB",
        banner: crate::trf!(
            "Bedrock LevelDB log · {size} · {keys} entries ({nbt} decoded NBT), read only";
            size => size,
            keys => total,
            nbt => nbt_count
        ),
    })
}

fn describe_bedrock_key(key: &[u8]) -> String {
    if !key.is_empty() && key.iter().all(|&b| (0x20..0x7f).contains(&b)) {
        return String::from_utf8_lossy(key).into_owned();
    }
    // Chunk keys are 9/10 bytes (Overworld) or 13/14 bytes (Nether/End): cx:i32, cz:i32, [dim:i32], tag:u8, [subchunk:i8].
    if matches!(key.len(), 9 | 10 | 13 | 14) {
        let cx = i32::from_le_bytes([key[0], key[1], key[2], key[3]]);
        let cz = i32::from_le_bytes([key[4], key[5], key[6], key[7]]);
        let (dim, tag, sub) = if key.len() >= 13 {
            let d = i32::from_le_bytes([key[8], key[9], key[10], key[11]]);
            let t = key[12];
            let s = key.get(13).map(|&b| b as i8);
            (d, t, s)
        } else {
            (0, key[8], key.get(9).map(|&b| b as i8))
        };
        let dim_name = match dim {
            0 => "overworld",
            1 => "nether",
            2 => "end",
            _ => "dim",
        };
        let tag_name = match tag {
            43 => "Data3D",
            44 => "Version",
            45 => "Data2D",
            47 => "SubChunkPrefix",
            49 => "BlockEntity",
            50 => "Entity",
            51 => "PendingTicks",
            54 => "FinalizedState",
            58 => "RandomTicks",
            _ => "ChunkTag",
        };
        return match sub {
            Some(y) => format!("chunk({cx}, {cz}, {dim_name}) {tag_name}[y={y}]"),
            None => format!("chunk({cx}, {cz}, {dim_name}) {tag_name}({tag})"),
        };
    }
    let hex: String = key.iter().take(24).map(|b| format!("{b:02x}")).collect();
    if key.len() > 24 {
        format!("0x{hex}... ({} B)", key.len())
    } else {
        format!("0x{hex}")
    }
}

fn write_leveldb_entry(out: &mut String, key: &[u8], is_put: bool, val: &[u8]) -> bool {
    let label = describe_bedrock_key(key);
    if !is_put {
        let _ = writeln!(out, "// DEL {label}");
        return false;
    }
    if val.len() >= 4 && val[0] == 10 {
        let mut r = BinReader::new(val, true);
        if let Ok((_, tag)) = r.read_root() {
            let _ = writeln!(out, "// KEY {label} ({}, Bedrock NBT)", human_size(val.len() as u64));
            let mut snbt = String::new();
            format_tag(&tag, 0, &mut snbt);
            if snbt.len() > 4096 {
                let cut = (0..=4096).rev().find(|&i| snbt.is_char_boundary(i)).unwrap_or(0);
                snbt.truncate(cut);
                snbt.push_str("\n// ... (entry truncated)");
            }
            out.push_str(&snbt);
            out.push_str("\n\n");
            return true;
        }
    }
    let _ = writeln!(out, "// PUT {label} ({})", human_size(val.len() as u64));
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bedrock_level_dat() -> Vec<u8> {
        let root = Tag::Compound(vec![
            ("BiomeOverride".into(), Tag::String(String::new())),
            ("CenterMapsToOrigin".into(), Tag::Byte(0)),
            ("Difficulty".into(), Tag::Int(2)),
            ("GameType".into(), Tag::Int(1)),
            ("Generator".into(), Tag::Int(2)),
            ("LevelName".into(), Tag::String("Kea World".into())),
            ("RandomSeed".into(), Tag::Long(-123456789012345)),
            ("RainLevel".into(), Tag::Float(0.25)),
            ("abilities".into(), Tag::Compound(vec![("flying".into(), Tag::Byte(1)), ("walkSpeed".into(), Tag::Float(0.1))])),
            ("lastOpenedWithVersion".into(), Tag::List { elem_type: 3, items: vec![Tag::Int(1), Tag::Int(21), Tag::Int(50)] }),
            ("emptyCompounds".into(), Tag::List { elem_type: 10, items: Vec::new() }),
            ("chunks".into(), Tag::IntArray(vec![1, -2, 3])),
        ]);
        let mut w = BinWriter::new(true);
        w.write_root("", &root).unwrap();
        let mut out = Vec::new();
        out.extend_from_slice(&10u32.to_le_bytes());
        out.extend_from_slice(&(w.buf.len() as u32).to_le_bytes());
        out.extend_from_slice(&w.buf);
        out
    }

    #[test]
    fn bedrock_level_dat_old_round_trips_losslessly() {
        let bytes = sample_bedrock_level_dat();
        let dec = decode_file(Path::new("level.dat_old"), &bytes).expect("decode level.dat_old");
        assert_eq!(dec.format, NbtFormat::BedrockHeadered { storage_version: 10 });
        assert_eq!(dec.label, "Bedrock NBT");
        assert!(dec.snbt.contains("Difficulty: 2"), "{}", dec.snbt);
        assert!(dec.snbt.contains("CenterMapsToOrigin: 0b"), "{}", dec.snbt);
        assert!(dec.snbt.contains("RandomSeed: -123456789012345L"), "{}", dec.snbt);
        assert!(dec.snbt.contains("emptyCompounds: [/*compound*/]"), "{}", dec.snbt);

        let reencoded = encode_file(&dec.snbt, dec.format).expect("re-encode");
        assert_eq!(reencoded, bytes, "exact byte-for-byte Bedrock round trip");

        let edited = dec.snbt.replace("Difficulty: 2", "Difficulty: 3").replace("LevelName: \"Kea World\"", "LevelName: \"Updated\"");
        let saved = encode_file(&edited, dec.format).expect("encode edited");
        let dec2 = decode_file(Path::new("level.dat_old"), &saved).expect("decode edited");
        assert!(dec2.snbt.contains("Difficulty: 3"));
        assert!(dec2.snbt.contains("LevelName: \"Updated\""));
        assert_eq!(u32::from_le_bytes([saved[4], saved[5], saved[6], saved[7]]) as usize, saved.len() - 8);
    }

    #[test]
    fn java_gzip_nbt_and_raw_mcstructure_round_trip() {
        let root = Tag::Compound(vec![("size".into(), Tag::List { elem_type: 3, items: vec![Tag::Int(5), Tag::Int(5), Tag::Int(5)] })]);
        let mut raw_le = BinWriter::new(true);
        raw_le.write_root("", &root).unwrap();
        let dec_mc = decode_file(Path::new("house.mcstructure"), &raw_le.buf).unwrap();
        assert_eq!(dec_mc.format, NbtFormat::BedrockRaw);
        assert_eq!(encode_file(&dec_mc.snbt, dec_mc.format).unwrap(), raw_le.buf);

        let mut raw_be = BinWriter::new(false);
        raw_be.write_root("", &root).unwrap();
        let gz = gzip(&raw_be.buf);
        let dec_java = decode_file(Path::new("level.dat"), &gz).unwrap();
        assert_eq!(dec_java.format, NbtFormat::JavaGzip);
        let re_gz = encode_file(&dec_java.snbt, dec_java.format).unwrap();
        assert_eq!(gunzip(&re_gz).unwrap(), raw_be.buf);
    }

    #[test]
    fn snbt_diagnose_pinpoints_errors() {
        assert!(diagnose("{ a: 1b, b: \"ok\", c: true }").is_empty());
        let errs = diagnose("{\n  a: [1, 2L]\n}");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].row, 1);
        assert!(errs[0].message.contains("list expects int"), "{}", errs[0].message);
    }

    #[test]
    fn decodes_bedrock_leveldb_wal_with_embedded_nbt() {
        let mut nbt_w = BinWriter::new(true);
        nbt_w.write_root("", &Tag::Compound(vec![("Health".into(), Tag::Short(20))])).unwrap();
        let key = b"~local_player";
        let mut batch = Vec::new();
        batch.extend_from_slice(&1u64.to_le_bytes()); // seq
        batch.extend_from_slice(&1u32.to_le_bytes()); // count
        batch.push(1); // Put
        batch.push(key.len() as u8);
        batch.extend_from_slice(key);
        batch.push(nbt_w.buf.len() as u8);
        batch.extend_from_slice(&nbt_w.buf);

        let mut wal = Vec::new();
        wal.extend_from_slice(&0u32.to_le_bytes()); // crc
        wal.extend_from_slice(&(batch.len() as u16).to_le_bytes());
        wal.push(1); // Full record
        wal.extend_from_slice(&batch);

        let view = leveldb_view(Path::new("world/db/000299.log"), &wal).expect("wal view");
        assert_eq!(view.label, "Bedrock LevelDB");
        assert!(view.text.contains("~local_player"), "{}", view.text);
        assert!(view.text.contains("Health: 20s"), "{}", view.text);
    }

    #[test]
    fn bedrock_world_summary_reads_level_dat_old() {
        let dir = std::env::temp_dir().join(format!("fx-mc-world-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("level.dat_old"), sample_bedrock_level_dat()).unwrap();
        let s = bedrock_world_summary(&dir).unwrap();
        assert!(s.contains("Kea World") && s.contains("Creative") && s.contains("Normal") && s.contains("Flat world") && s.contains("MC 1.21.50"), "{s}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
