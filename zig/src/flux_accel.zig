// C-ABI SIMD kernels for hot paths in FluxCode; built into a static archive when Zig is installed.

pub export fn flux_zig_active() u32 {
    return 1;
}

pub export fn flux_has_zero(ptr: [*]const u8, len: usize) u8 {
    if (len == 0) return 0;
    const Vec = @Vector(32, u8);
    const zeros: Vec = @splat(0);
    var i: usize = 0;
    while (i + 32 <= len) : (i += 32) {
        const chunk: Vec = ptr[i..][0..32].*;
        if (@reduce(.Or, chunk == zeros)) return 1;
    }
    while (i < len) : (i += 1) {
        if (ptr[i] == 0) return 1;
    }
    return 0;
}

pub export fn flux_scan_file_stats(
    ptr: [*]const u8,
    len: usize,
    long_threshold: usize,
    out_crlf: *u8,
    out_long_bytes: *usize,
    out_short_bytes: *usize,
    out_max_line: *usize,
) void {
    var crlf: u8 = 0;
    var long_bytes: usize = 0;
    var short_bytes: usize = 0;
    var max_line: usize = 0;
    var line_start: usize = 0;

    const Vec = @Vector(32, u8);
    const nls: Vec = @splat('\n');
    var i: usize = 0;
    while (i + 32 <= len) : (i += 32) {
        const chunk: Vec = ptr[i..][0..32].*;
        var m: u32 = @bitCast(chunk == nls);
        while (m != 0) {
            const bit: usize = @ctz(m);
            const nl_pos = i + bit;
            var line_end = nl_pos;
            if (nl_pos > line_start and ptr[nl_pos - 1] == '\r') {
                crlf = 1;
                line_end -= 1;
            }
            const line_len = line_end - line_start;
            if (line_len > max_line) max_line = line_len;
            if (line_len >= long_threshold) {
                long_bytes += line_len;
            } else {
                short_bytes += line_len;
            }
            line_start = nl_pos + 1;
            m &= m - 1;
        }
    }
    while (i < len) : (i += 1) {
        if (ptr[i] == '\n') {
            var line_end = i;
            if (i > line_start and ptr[i - 1] == '\r') {
                crlf = 1;
                line_end -= 1;
            }
            const line_len = line_end - line_start;
            if (line_len > max_line) max_line = line_len;
            if (line_len >= long_threshold) {
                long_bytes += line_len;
            } else {
                short_bytes += line_len;
            }
            line_start = i + 1;
        }
    }
    if (line_start < len) {
        const line_len = len - line_start;
        if (line_len > max_line) max_line = line_len;
        if (line_len >= long_threshold) {
            long_bytes += line_len;
        } else {
            short_bytes += line_len;
        }
    }

    out_crlf.* = crlf;
    out_long_bytes.* = long_bytes;
    out_short_bytes.* = short_bytes;
    out_max_line.* = max_line;
}

inline fn matchBytes(a: [*]const u8, b: [*]const u8, len: usize) bool {
    var k: usize = 0;
    while (k < len) : (k += 1) {
        if (a[k] != b[k]) return false;
    }
    return true;
}

pub export fn flux_find_bytes(hay: [*]const u8, hay_len: usize, needle: [*]const u8, needle_len: usize) usize {
    if (needle_len == 0) return 0;
    if (needle_len > hay_len) return hay_len;

    const first = needle[0];
    const last = needle[needle_len - 1];
    const limit = hay_len - needle_len + 1;

    const Vec = @Vector(32, u8);
    const first_v: Vec = @splat(first);
    const last_v: Vec = @splat(last);
    var i: usize = 0;

    if (needle_len == 1) {
        while (i + 32 <= limit) : (i += 32) {
            const chunk: Vec = hay[i..][0..32].*;
            const m: u32 = @bitCast(chunk == first_v);
            if (m != 0) return i + @ctz(m);
        }
        while (i < limit) : (i += 1) {
            if (hay[i] == first) return i;
        }
        return hay_len;
    }

    const tail_off = needle_len - 1;
    while (i + 32 <= limit) : (i += 32) {
        const v0: Vec = hay[i..][0..32].*;
        const v1: Vec = hay[i + tail_off ..][0..32].*;
        var m: u32 = @as(u32, @bitCast(v0 == first_v)) & @as(u32, @bitCast(v1 == last_v));
        while (m != 0) {
            const pos = i + @ctz(m);
            if (needle_len == 2 or matchBytes(hay + pos + 1, needle + 1, needle_len - 2)) {
                return pos;
            }
            m &= m - 1;
        }
    }
    while (i < limit) : (i += 1) {
        if (hay[i] == first and hay[i + tail_off] == last) {
            if (needle_len == 2 or matchBytes(hay + i + 1, needle + 1, needle_len - 2)) {
                return i;
            }
        }
    }
    return hay_len;
}

inline fn asciiLower(b: u8) u8 {
    return if (b >= 'A' and b <= 'Z') b | 0x20 else b;
}

inline fn asciiLowerVec(v: @Vector(32, u8)) @Vector(32, u8) {
    const Vec = @Vector(32, u8);
    const is_upper = (v -% @as(Vec, @splat('A'))) <= @as(Vec, @splat('Z' - 'A'));
    return @select(u8, is_upper, v | @as(Vec, @splat(0x20)), v);
}

inline fn matchAsciiLower(hay: [*]const u8, needle_lower: [*]const u8, len: usize) bool {
    var k: usize = 0;
    while (k < len) : (k += 1) {
        if (asciiLower(hay[k]) != asciiLower(needle_lower[k])) return false;
    }
    return true;
}

pub export fn flux_find_ascii_ci(hay: [*]const u8, hay_len: usize, needle_lower: [*]const u8, needle_len: usize) usize {
    if (needle_len == 0) return 0;
    if (needle_len > hay_len) return hay_len;

    const first = asciiLower(needle_lower[0]);
    const last = asciiLower(needle_lower[needle_len - 1]);
    const limit = hay_len - needle_len + 1;

    const Vec = @Vector(32, u8);
    const first_v: Vec = @splat(first);
    const last_v: Vec = @splat(last);
    var i: usize = 0;

    if (needle_len == 1) {
        while (i + 32 <= limit) : (i += 32) {
            const chunk = asciiLowerVec(hay[i..][0..32].*);
            const m: u32 = @bitCast(chunk == first_v);
            if (m != 0) return i + @ctz(m);
        }
        while (i < limit) : (i += 1) {
            if (asciiLower(hay[i]) == first) return i;
        }
        return hay_len;
    }

    const tail_off = needle_len - 1;
    while (i + 32 <= limit) : (i += 32) {
        const v0 = asciiLowerVec(hay[i..][0..32].*);
        const v1 = asciiLowerVec(hay[i + tail_off ..][0..32].*);
        var m: u32 = @as(u32, @bitCast(v0 == first_v)) & @as(u32, @bitCast(v1 == last_v));
        while (m != 0) {
            const pos = i + @ctz(m);
            if (needle_len == 2 or matchAsciiLower(hay + pos + 1, needle_lower + 1, needle_len - 2)) {
                return pos;
            }
            m &= m - 1;
        }
    }
    while (i < limit) : (i += 1) {
        if (asciiLower(hay[i]) == first and asciiLower(hay[i + tail_off]) == last) {
            if (needle_len == 2 or matchAsciiLower(hay + i + 1, needle_lower + 1, needle_len - 2)) {
                return i;
            }
        }
    }
    return hay_len;
}

inline fn isBoundary(b: u8) bool {
    return switch (b) {
        '\\', '/', '_', '-', '.', ' ' => true,
        else => false,
    };
}

pub export fn flux_fuzzy_score(q: [*]const u8, q_len: usize, p: [*]const u8, p_len: usize, out_score: *i32) u8 {
    if (q_len == 0) {
        out_score.* = 0;
        return 1;
    }
    var name_start: usize = 0;
    var k: usize = p_len;
    while (k > 0) {
        k -= 1;
        if (p[k] == '\\' or p[k] == '/') {
            name_start = k + 1;
            break;
        }
    }

    var score: i32 = 0;
    var at: usize = 0;
    var has_prev = false;
    var prev: usize = 0;
    var qi: usize = 0;
    while (qi < q_len) : (qi += 1) {
        const qc = q[qi];
        const found = flux_find_bytes(p + at, p_len - at, @ptrCast(&qc), 1);
        if (found >= p_len - at) return 0;
        const pos = at + found;
        score += 1;
        if (has_prev and prev + 1 == pos) {
            score += 5;
        }
        if (pos >= name_start) {
            score += 3;
        }
        if (pos == 0 or isBoundary(p[pos - 1])) {
            score += 4;
        }
        prev = pos;
        has_prev = true;
        at = pos + 1;
    }

    const name_len = p_len - name_start;
    if (name_len >= q_len and matchBytes(p + name_start, q, q_len)) {
        score += 100;
    } else if (flux_find_bytes(p + name_start, name_len, q, q_len) < name_len) {
        score += 60;
    } else if (flux_find_bytes(p, p_len, q, q_len) < p_len) {
        score += 25;
    }

    out_score.* = score * 10 - @as(i32, @intCast(p_len));
    return 1;
}

pub export fn flux_raster_ridge(tops3: [*]const f32, w: usize, h: usize, r: u8, g: u8, b: u8, dst: [*]u8) void {
    if (w == 0 or h == 0) return;
    const hf: f32 = @floatFromInt(h);
    const solid = [4]u8{ b, g, r, 255 };
    const stride = w * 4;

    var x: usize = 0;
    while (x < w) : (x += 1) {
        const t0 = tops3[x * 3];
        const t1 = tops3[x * 3 + 1];
        const t2 = tops3[x * 3 + 2];
        const t_min = @min(t0, @min(t1, t2));
        const t_max = @max(t0, @max(t1, t2));
        if (!(t_min <= t_max)) continue;

        const y_start_f = @max(@floor(t_min), 0.0);
        const y_start: usize = if (y_start_f >= hf) h else @intFromFloat(y_start_f);
        const y_end_f = @min(@max(@ceil(t_max), 0.0), hf);
        const y_end: usize = @max(y_start, @as(usize, @intFromFloat(y_end_f)));

        var y: usize = y_start;
        while (y < y_end) : (y += 1) {
            const yf: f32 = @floatFromInt(y + 1);
            const c0 = @min(@max(yf - t0, 0.0), 1.0);
            const c1 = @min(@max(yf - t1, 0.0), 1.0);
            const c2 = @min(@max(yf - t2, 0.0), 1.0);
            const cov = (c0 + c1 + c2) / 3.0;
            if (cov <= 0.0) continue;
            const alpha: u8 = @intFromFloat(@round(cov * 255.0));
            const off = y * stride + x * 4;
            dst[off..][0..4].* = [4]u8{ b, g, r, alpha };
        }
        while (y < h) : (y += 1) {
            const off = y * stride + x * 4;
            dst[off..][0..4].* = solid;
        }
    }
}

const std = @import("std");

test "has_zero detects null byte at any alignment" {
    var buf: [97]u8 = @splat('a');
    try std.testing.expectEqual(@as(u8, 0), flux_has_zero(&buf, buf.len));
    for (0..buf.len) |idx| {
        buf[idx] = 0;
        try std.testing.expectEqual(@as(u8, 1), flux_has_zero(&buf, buf.len));
        buf[idx] = 'a';
    }
}

test "scan_file_stats handles crlf, long lines, and trailing no-newline" {
    var sample: [7 + 320 + 6]u8 = undefined;
    const head = "short\r\n";
    const mid: [320]u8 = @splat('x');
    const tail = "\r\ntail";
    sample[0..7].* = head.*;
    sample[7..327].* = mid;
    sample[327..333].* = tail.*;
    var crlf: u8 = 0;
    var long_b: usize = 0;
    var short_b: usize = 0;
    var max_l: usize = 0;
    flux_scan_file_stats(&sample, sample.len, 300, &crlf, &long_b, &short_b, &max_l);
    try std.testing.expectEqual(@as(u8, 1), crlf);
    try std.testing.expectEqual(@as(usize, 320), long_b);
    try std.testing.expectEqual(@as(usize, 9), short_b);
    try std.testing.expectEqual(@as(usize, 320), max_l);
}

test "find_bytes and find_ascii_ci across SIMD boundaries" {
    const hay = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ__EndMarker!";
    const n1 = "xyz";
    try std.testing.expectEqual(@as(usize, 33), flux_find_bytes(hay.ptr, hay.len, n1.ptr, n1.len));
    const n2 = "endmarker";
    try std.testing.expectEqual(@as(usize, 64), flux_find_ascii_ci(hay.ptr, hay.len, n2.ptr, n2.len));
    const miss = "nope";
    try std.testing.expectEqual(hay.len, flux_find_ascii_ci(hay.ptr, hay.len, miss.ptr, miss.len));
}

test "fuzzy_score matches path ranking" {
    var s1: i32 = 0;
    var s2: i32 = 0;
    try std.testing.expectEqual(@as(u8, 1), flux_fuzzy_score("app", 3, "src\\app.py", 10, &s1));
    try std.testing.expectEqual(@as(u8, 1), flux_fuzzy_score("app", 3, "apple\\pie\\x.txt", 15, &s2));
    try std.testing.expect(s1 > s2);
    try std.testing.expectEqual(@as(u8, 0), flux_fuzzy_score("zz", 2, "app.py", 6, &s1));
}

test "raster_ridge produces smooth AA edge and solid bottom" {
    const tops = [_]f32{ 2.5, 2.5, 2.5, 0.0, 0.0, 0.0 };
    var px: [2 * 5 * 4]u8 = @splat(0);
    flux_raster_ridge(&tops, 2, 5, 10, 20, 30, &px);
    try std.testing.expectEqual([4]u8{ 0, 0, 0, 0 }, px[0..4].*);
    try std.testing.expectEqual([4]u8{ 30, 20, 10, 128 }, px[(2 * 2 + 0) * 4 ..][0..4].*);
    try std.testing.expectEqual([4]u8{ 30, 20, 10, 255 }, px[(3 * 2 + 0) * 4 ..][0..4].*);
    try std.testing.expectEqual([4]u8{ 30, 20, 10, 255 }, px[(0 * 2 + 1) * 4 ..][0..4].*);
}
