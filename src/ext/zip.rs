//! A small ZIP writer for `.mcpack` and `.mcaddon` files: deflate from miniz_oxide
//! (already compiled for the editor), stored when that isn't smaller, UTF-8 names.

const TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = TABLE[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

struct Entry {
    name: String,
    crc: u32,
    packed: u32,
    size: u32,
    offset: u32,
    method: u16,
}

#[derive(Default)]
pub struct Zip {
    buf: Vec<u8>,
    entries: Vec<Entry>,
}

fn u16le(v: &mut Vec<u8>, n: u16) {
    v.extend_from_slice(&n.to_le_bytes());
}
fn u32le(v: &mut Vec<u8>, n: u32) {
    v.extend_from_slice(&n.to_le_bytes());
}

/// 1980-01-01 00:00, the earliest DOS date: the same bytes for the same files.
const DOS_DATE: u16 = (1 << 5) | 1;

impl Zip {
    /// Add a file. `name` uses `/` between folders.
    pub fn add(&mut self, name: &str, data: &[u8]) {
        let crc = crc32(data);
        let deflated = miniz_oxide::deflate::compress_to_vec(data, 6);
        let (method, body): (u16, &[u8]) = if deflated.len() < data.len() { (8, &deflated) } else { (0, data) };
        let offset = self.buf.len() as u32;
        let b = &mut self.buf;
        u32le(b, 0x0403_4b50);
        u16le(b, 20);
        u16le(b, 0x0800);
        u16le(b, method);
        u16le(b, 0);
        u16le(b, DOS_DATE);
        u32le(b, crc);
        u32le(b, body.len() as u32);
        u32le(b, data.len() as u32);
        u16le(b, name.len() as u16);
        u16le(b, 0);
        b.extend_from_slice(name.as_bytes());
        b.extend_from_slice(body);
        self.entries.push(Entry { name: name.to_string(), crc, packed: body.len() as u32, size: data.len() as u32, offset, method });
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn finish(mut self) -> Vec<u8> {
        let start = self.buf.len() as u32;
        for e in &self.entries {
            let b = &mut self.buf;
            u32le(b, 0x0201_4b50);
            u16le(b, 20);
            u16le(b, 20);
            u16le(b, 0x0800);
            u16le(b, e.method);
            u16le(b, 0);
            u16le(b, DOS_DATE);
            u32le(b, e.crc);
            u32le(b, e.packed);
            u32le(b, e.size);
            u16le(b, e.name.len() as u16);
            u16le(b, 0);
            u16le(b, 0);
            u16le(b, 0);
            u16le(b, 0);
            u32le(b, 0);
            u32le(b, e.offset);
            b.extend_from_slice(e.name.as_bytes());
        }
        let size = self.buf.len() as u32 - start;
        let n = self.entries.len() as u16;
        let b = &mut self.buf;
        u32le(b, 0x0605_4b50);
        u16le(b, 0);
        u16le(b, 0);
        u16le(b, n);
        u16le(b, n);
        u32le(b, size);
        u32le(b, start);
        u16le(b, 0);
        self.buf
    }
}

/// One file out of a ZIP (a jar): found in the central directory, stored or deflated.
pub fn read_entry(zip: &[u8], name: &str) -> Option<Vec<u8>> {
    let u16at = |i: usize| zip.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32at = |i: usize| zip.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    // The end record is in the last 64 KB (after an optional comment).
    let from = zip.len().saturating_sub(22 + 65_535);
    let end = (from..zip.len().saturating_sub(21)).rev().find(|&i| u32at(i) == Some(0x0605_4b50))?;
    let count = u16at(end + 10)? as usize;
    let mut at = u32at(end + 16)? as usize;
    for _ in 0..count {
        if u32at(at)? != 0x0201_4b50 {
            return None;
        }
        let method = u16at(at + 10)?;
        let packed = u32at(at + 20)? as usize;
        let size = u32at(at + 24)? as usize;
        let (nlen, xlen, clen) = (u16at(at + 28)? as usize, u16at(at + 30)? as usize, u16at(at + 32)? as usize);
        let offset = u32at(at + 42)? as usize;
        let entry = zip.get(at + 46..at + 46 + nlen)?;
        at += 46 + nlen + xlen + clen;
        if entry != name.as_bytes() {
            continue;
        }
        let (ln, lx) = (u16at(offset + 26)? as usize, u16at(offset + 28)? as usize);
        let body = zip.get(offset + 30 + ln + lx..offset + 30 + ln + lx + packed)?;
        return match method {
            0 => Some(body.to_vec()),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(body, size.max(1) * 2 + 1024).ok(),
            _ => None,
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_entry_back() {
        let mut z = Zip::default();
        z.add("a.txt", b"first");
        z.add("plugin.yml", "name: Kiw\nversion: 1.2.3\n".repeat(5).as_bytes());
        let bytes = z.finish();
        assert_eq!(read_entry(&bytes, "a.txt").as_deref(), Some(&b"first"[..]));
        assert!(String::from_utf8(read_entry(&bytes, "plugin.yml").unwrap()).unwrap().starts_with("name: Kiw"));
        assert_eq!(read_entry(&bytes, "missing"), None);
    }

    #[test]
    fn crc_of_the_check_string() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn entries_read_back() {
        let text = "{\"format_version\": 2}\n".repeat(40);
        let mut z = Zip::default();
        z.add("BP/manifest.json", text.as_bytes());
        z.add("BP/pack_icon.png", &[1, 2, 3]);
        let bytes = z.finish();
        // End record: two entries.
        let end = bytes.len() - 22;
        assert_eq!(&bytes[end..end + 4], &[0x50, 0x4b, 0x05, 0x06]);
        assert_eq!(u16::from_le_bytes([bytes[end + 10], bytes[end + 11]]), 2);
        // First entry: deflated, and it inflates back to the text.
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 8);
        let packed = u32::from_le_bytes(bytes[18..22].try_into().unwrap()) as usize;
        let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
        let body = &bytes[30 + name_len..30 + name_len + packed];
        assert_eq!(miniz_oxide::inflate::decompress_to_vec(body).unwrap(), text.as_bytes());
    }
}
