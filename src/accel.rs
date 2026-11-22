// Hot-path kernels for binary/line scanning, search, fuzzy matching, and ridge rasterization.
// Uses the static Zig SIMD library when available and falls back cleanly to pure Rust.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStats {
    pub crlf: bool,
    pub long_bytes: usize,
    pub short_bytes: usize,
    pub max_line: usize,
}

impl FileStats {
    pub fn dominated_by_long_lines(&self) -> bool {
        self.long_bytes > 0 && self.long_bytes > self.short_bytes
    }
}

#[cfg(flux_zig)]
extern "C" {
    fn flux_zig_active() -> u32;
    fn flux_has_zero(ptr: *const u8, len: usize) -> u8;
    fn flux_scan_file_stats(
        ptr: *const u8,
        len: usize,
        long_threshold: usize,
        out_crlf: *mut u8,
        out_long_bytes: *mut usize,
        out_short_bytes: *mut usize,
        out_max_line: *mut usize,
    );
    fn flux_find_bytes(hay: *const u8, hay_len: usize, needle: *const u8, needle_len: usize) -> usize;
    fn flux_find_ascii_ci(hay: *const u8, hay_len: usize, needle_lower: *const u8, needle_len: usize) -> usize;
    fn flux_fuzzy_score(q: *const u8, q_len: usize, p: *const u8, p_len: usize, out_score: *mut i32) -> u8;
    fn flux_raster_ridge(tops3: *const f32, w: usize, h: usize, r: u8, g: u8, b: u8, dst: *mut u8);
}

pub fn zig_enabled() -> bool {
    #[cfg(flux_zig)]
    {
        unsafe { flux_zig_active() == 1 }
    }
    #[cfg(not(flux_zig))]
    {
        false
    }
}

pub fn has_zero(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    #[cfg(flux_zig)]
    {
        unsafe { flux_has_zero(bytes.as_ptr(), bytes.len()) != 0 }
    }
    #[cfg(not(flux_zig))]
    {
        fallback::has_zero(bytes)
    }
}

pub fn scan_file_stats(bytes: &[u8], long_threshold: usize) -> FileStats {
    if bytes.is_empty() {
        return FileStats { crlf: false, long_bytes: 0, short_bytes: 0, max_line: 0 };
    }
    #[cfg(flux_zig)]
    {
        let mut crlf = 0u8;
        let (mut long_bytes, mut short_bytes, mut max_line) = (0usize, 0usize, 0usize);
        unsafe {
            flux_scan_file_stats(bytes.as_ptr(), bytes.len(), long_threshold, &mut crlf, &mut long_bytes, &mut short_bytes, &mut max_line);
        }
        FileStats { crlf: crlf != 0, long_bytes, short_bytes, max_line }
    }
    #[cfg(not(flux_zig))]
    {
        fallback::scan_file_stats(bytes, long_threshold)
    }
}

pub fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > hay.len() {
        return None;
    }
    #[cfg(flux_zig)]
    {
        let pos = unsafe { flux_find_bytes(hay.as_ptr(), hay.len(), needle.as_ptr(), needle.len()) };
        (pos < hay.len()).then_some(pos)
    }
    #[cfg(not(flux_zig))]
    {
        fallback::find_bytes(hay, needle)
    }
}

pub fn find_ascii_ci(hay: &[u8], needle_lower: &[u8]) -> Option<usize> {
    if needle_lower.is_empty() {
        return Some(0);
    }
    if needle_lower.len() > hay.len() {
        return None;
    }
    #[cfg(flux_zig)]
    {
        let pos = unsafe { flux_find_ascii_ci(hay.as_ptr(), hay.len(), needle_lower.as_ptr(), needle_lower.len()) };
        (pos < hay.len()).then_some(pos)
    }
    #[cfg(not(flux_zig))]
    {
        fallback::find_ascii_ci(hay, needle_lower)
    }
}

pub fn fuzzy_score(q: &[u8], p: &[u8]) -> Option<i32> {
    if q.is_empty() {
        return Some(0);
    }
    #[cfg(flux_zig)]
    {
        let mut score = 0i32;
        let ok = unsafe { flux_fuzzy_score(q.as_ptr(), q.len(), p.as_ptr(), p.len(), &mut score) };
        (ok != 0).then_some(score)
    }
    #[cfg(not(flux_zig))]
    {
        fallback::fuzzy_score(q, p)
    }
}

pub fn raster_ridge(tops3: &[f32], w: usize, h: usize, rgb: [u8; 3], dst: &mut [u8]) {
    assert_eq!(tops3.len(), w * 3);
    assert_eq!(dst.len(), w * h * 4);
    if w == 0 || h == 0 {
        return;
    }
    #[cfg(flux_zig)]
    unsafe {
        flux_raster_ridge(tops3.as_ptr(), w, h, rgb[0], rgb[1], rgb[2], dst.as_mut_ptr());
    }
    #[cfg(not(flux_zig))]
    {
        fallback::raster_ridge(tops3, w, h, rgb, dst);
    }
}

#[cfg(any(not(flux_zig), test))]
pub(crate) mod fallback {
    use super::FileStats;

    pub fn has_zero(bytes: &[u8]) -> bool {
        bytes.contains(&0)
    }

    pub fn scan_file_stats(bytes: &[u8], long_threshold: usize) -> FileStats {
        let mut crlf = false;
        let (mut long_bytes, mut short_bytes, mut max_line) = (0usize, 0usize, 0usize);
        let mut start = 0usize;
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                let mut end = i;
                if i > start && bytes[i - 1] == b'\r' {
                    crlf = true;
                    end -= 1;
                }
                let len = end - start;
                max_line = max_line.max(len);
                if len >= long_threshold {
                    long_bytes += len;
                } else {
                    short_bytes += len;
                }
                start = i + 1;
            }
        }
        if start < bytes.len() {
            let len = bytes.len() - start;
            max_line = max_line.max(len);
            if len >= long_threshold {
                long_bytes += len;
            } else {
                short_bytes += len;
            }
        }
        FileStats { crlf, long_bytes, short_bytes, max_line }
    }

    pub fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
        if needle.is_empty() {
            return Some(0);
        }
        if needle.len() > hay.len() {
            return None;
        }
        if needle.len() == 1 {
            return hay.iter().position(|&b| b == needle[0]);
        }
        let (first, last) = (needle[0], needle[needle.len() - 1]);
        let tail = needle.len() - 1;
        let limit = hay.len() - needle.len() + 1;
        for i in 0..limit {
            if hay[i] == first && hay[i + tail] == last && hay[i + 1..i + tail] == needle[1..tail] {
                return Some(i);
            }
        }
        None
    }

    pub fn find_ascii_ci(hay: &[u8], needle_lower: &[u8]) -> Option<usize> {
        if needle_lower.is_empty() {
            return Some(0);
        }
        if needle_lower.len() > hay.len() {
            return None;
        }
        let first = needle_lower[0].to_ascii_lowercase();
        if needle_lower.len() == 1 {
            return hay.iter().position(|b| b.to_ascii_lowercase() == first);
        }
        let last = needle_lower[needle_lower.len() - 1].to_ascii_lowercase();
        let tail = needle_lower.len() - 1;
        let limit = hay.len() - needle_lower.len() + 1;
        for i in 0..limit {
            if hay[i].to_ascii_lowercase() == first
                && hay[i + tail].to_ascii_lowercase() == last
                && hay[i + 1..i + tail].eq_ignore_ascii_case(&needle_lower[1..tail])
            {
                return Some(i);
            }
        }
        None
    }

    pub fn fuzzy_score(q: &[u8], p: &[u8]) -> Option<i32> {
        if q.is_empty() {
            return Some(0);
        }
        let name_start = p.iter().rposition(|c| *c == b'\\' || *c == b'/').map_or(0, |i| i + 1);
        let mut score = 0i32;
        let mut at = 0usize;
        let mut prev: Option<usize> = None;
        for &qc in q {
            let pos = at + p[at..].iter().position(|&b| b == qc)?;
            score += 1;
            if prev.is_some_and(|pv| pv + 1 == pos) {
                score += 5;
            }
            if pos >= name_start {
                score += 3;
            }
            if pos == 0 || matches!(p[pos - 1], b'\\' | b'/' | b'_' | b'-' | b'.' | b' ') {
                score += 4;
            }
            prev = Some(pos);
            at = pos + 1;
        }
        let name = &p[name_start..];
        if name.starts_with(q) {
            score += 100;
        } else if find_bytes(name, q).is_some() {
            score += 60;
        } else if find_bytes(p, q).is_some() {
            score += 25;
        }
        Some(score * 10 - p.len() as i32)
    }

    pub fn raster_ridge(tops3: &[f32], w: usize, h: usize, rgb: [u8; 3], dst: &mut [u8]) {
        let [r, g, b] = rgb;
        let solid = [b, g, r, 255];
        let stride = w * 4;
        for x in 0..w {
            let t0 = tops3[x * 3];
            let t1 = tops3[x * 3 + 1];
            let t2 = tops3[x * 3 + 2];
            let t_min = t0.min(t1).min(t2);
            let t_max = t0.max(t1).max(t2);
            if !(t_min <= t_max) {
                continue;
            }
            let y_start = (t_min.floor().max(0.) as usize).min(h);
            let y_end = (t_max.ceil().max(0.) as usize).min(h).max(y_start);
            for y in y_start..y_end {
                let yf = (y + 1) as f32;
                let cov = ((yf - t0).clamp(0., 1.) + (yf - t1).clamp(0., 1.) + (yf - t2).clamp(0., 1.)) / 3.;
                if cov <= 0. {
                    continue;
                }
                let i = y * stride + x * 4;
                dst[i..i + 4].copy_from_slice(&[b, g, r, (cov * 255.).round() as u8]);
            }
            for y in y_end..h {
                let i = y * stride + x * 4;
                dst[i..i + 4].copy_from_slice(&solid);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zig_and_fallback_match_on_zero_and_stats() {
        assert_eq!(zig_enabled(), cfg!(flux_zig));
        for len in 0..=130 {
            let mut buf = vec![b'a'; len];
            assert_eq!(has_zero(&buf), fallback::has_zero(&buf));
            if len > 0 {
                for &at in &[0, len / 2, len - 1] {
                    buf[at] = 0;
                    assert!(has_zero(&buf));
                    assert_eq!(has_zero(&buf), fallback::has_zero(&buf));
                    buf[at] = b'a';
                }
            }
        }

        let cases = [
            b"".as_slice(),
            b"one line without newline",
            b"a\nb\nc\n",
            b"short\r\nsecond\r\ntail",
            b"\n\r\n",
        ];
        for sample in cases {
            assert_eq!(scan_file_stats(sample, 10), fallback::scan_file_stats(sample, 10));
        }
        let mut big = Vec::new();
        big.extend(std::iter::repeat_n(b'x', 400));
        big.extend_from_slice(b"\r\nhi");
        let st = scan_file_stats(&big, 300);
        assert_eq!(st, fallback::scan_file_stats(&big, 300));
        assert!(st.crlf && st.dominated_by_long_lines());
        assert_eq!(st.max_line, 400);
    }

    #[test]
    fn zig_and_fallback_match_on_search_and_fuzzy() {
        let mut hay = vec![b'z'; 140];
        hay[0..3].copy_from_slice(b"AbC");
        hay[31..35].copy_from_slice(b"FoO1");
        hay[63..71].copy_from_slice(b"needle99");
        hay[129..138].copy_from_slice(b"EndMarker");

        for n in [b"".as_slice(), b"a", b"F", b"foo1", b"needle99", b"endmarker", b"missing", b"zzzzzzzzzzzz"] {
            assert_eq!(find_ascii_ci(&hay, n), fallback::find_ascii_ci(&hay, n), "ci {n:?}");
        }
        for n in [b"".as_slice(), b"A", b"FoO1", b"needle99", b"EndMarker", b"endmarker", b"missing"] {
            assert_eq!(find_bytes(&hay, n), fallback::find_bytes(&hay, n), "exact {n:?}");
        }

        let pairs = [
            ("app", "src\\app.py"),
            ("app", "apple\\pie\\x.txt"),
            ("req", "requirements.txt"),
            ("zz", "app.py"),
            ("", "anything.rs"),
            ("main", "main"),
        ];
        for (q, p) in pairs {
            assert_eq!(fuzzy_score(q.as_bytes(), p.as_bytes()), fallback::fuzzy_score(q.as_bytes(), p.as_bytes()), "{q} in {p}");
        }
    }

    #[test]
    fn zig_and_fallback_match_on_ridge_rasterization() {
        let (w, h) = (37usize, 24usize);
        let tops3: Vec<f32> = (0..w * 3)
            .map(|i| {
                let x = i as f32 / (w * 3) as f32;
                (0.3 + 0.4 * (x * 9.0).sin()) * h as f32
            })
            .collect();
        let mut a = vec![0u8; w * h * 4];
        let mut b = vec![0u8; w * h * 4];
        raster_ridge(&tops3, w, h, [0x1a, 0x2b, 0x3c], &mut a);
        fallback::raster_ridge(&tops3, w, h, [0x1a, 0x2b, 0x3c], &mut b);
        assert_eq!(a, b);
    }
}
