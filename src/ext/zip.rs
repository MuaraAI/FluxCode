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

const DOS_DATE: u16 = (1 << 5) | 1;

impl Zip {
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

pub struct Listed {
    pub name: String,
    pub size: usize,
    method: u16,
    packed: usize,
    offset: usize,
}

fn u16at(zip: &[u8], i: usize) -> Option<u16> {
    let end = i.checked_add(2)?;
    zip.get(i..end).map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn u32at(zip: &[u8], i: usize) -> Option<u32> {
    let end = i.checked_add(4)?;
    zip.get(i..end)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

pub fn list(zip: &[u8]) -> Option<Vec<Listed>> {
    let from = zip.len().saturating_sub(22 + 65_535);
    let end = (from..zip.len().saturating_sub(21))
        .rev()
        .find(|&i| u32at(zip, i) == Some(0x0605_4b50))?;
    let count = u16at(zip, end.checked_add(10)?)? as usize;
    let mut at = u32at(zip, end.checked_add(16)?)? as usize;
    let mut out = Vec::with_capacity(count.min(zip.len() / 46));
    for _ in 0..count {
        if u32at(zip, at)? != 0x0201_4b50 {
            return None;
        }
        let method = u16at(zip, at.checked_add(10)?)?;
        let packed = u32at(zip, at.checked_add(20)?)? as usize;
        let size = u32at(zip, at.checked_add(24)?)? as usize;
        let (nlen, xlen, clen) = (
            u16at(zip, at.checked_add(28)?)? as usize,
            u16at(zip, at.checked_add(30)?)? as usize,
            u16at(zip, at.checked_add(32)?)? as usize,
        );
        let offset = u32at(zip, at.checked_add(42)?)? as usize;
        let name_start = at.checked_add(46)?;
        let name_end = name_start.checked_add(nlen)?;
        let name = String::from_utf8_lossy(zip.get(name_start..name_end)?).into_owned();
        at = name_end.checked_add(xlen)?.checked_add(clen)?;
        out.push(Listed {
            name,
            size,
            method,
            packed,
            offset,
        });
    }
    Some(out)
}

pub fn read(zip: &[u8], e: &Listed, max: usize) -> Option<Vec<u8>> {
    if e.size > max {
        return None;
    }
    if u32at(zip, e.offset)? != 0x0403_4b50 {
        return None;
    }
    let (ln, lx) = (
        u16at(zip, e.offset.checked_add(26)?)? as usize,
        u16at(zip, e.offset.checked_add(28)?)? as usize,
    );
    let start = e.offset.checked_add(30)?.checked_add(ln)?.checked_add(lx)?;
    let end = start.checked_add(e.packed)?;
    let body = zip.get(start..end)?;
    match e.method {
        0 => Some(body.to_vec()),
        8 => miniz_oxide::inflate::decompress_to_vec_with_limit(
            body,
            e.size.max(1).saturating_mul(2).saturating_add(1024).min(max),
        )
        .ok(),
        _ => None,
    }
}

pub fn read_entry(zip: &[u8], name: &str) -> Option<Vec<u8>> {
    let e = list(zip)?.into_iter().find(|e| e.name == name)?;
    read(zip, &e, usize::MAX)
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
        let end = bytes.len() - 22;
        assert_eq!(&bytes[end..end + 4], &[0x50, 0x4b, 0x05, 0x06]);
        assert_eq!(u16::from_le_bytes([bytes[end + 10], bytes[end + 11]]), 2);
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 8);
        let packed = u32::from_le_bytes(bytes[18..22].try_into().unwrap()) as usize;
        let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
        let body = &bytes[30 + name_len..30 + name_len + packed];
        assert_eq!(miniz_oxide::inflate::decompress_to_vec(body).unwrap(), text.as_bytes());
    }

    #[test]
    fn malformed_offsets_never_overflow() {
        let mut z = Zip::default();
        z.add("a.txt", b"ok");
        let mut bytes = z.finish();
        // Corrupt local-header name length near u16::MAX on a huge offset entry.
        let bad = Listed {
            name: "bad".into(),
            size: 1,
            method: 0,
            packed: usize::MAX,
            offset: usize::MAX - 10,
        };
        assert_eq!(read(&bytes, &bad, usize::MAX), None);
        // Corrupt EOCD central directory offset to u32::MAX.
        let end = bytes.len() - 22;
        bytes[end + 16..end + 20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(list(&bytes).is_none());
    }
}
