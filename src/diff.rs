
const MAX_D: usize = 4096;

#[derive(Debug, PartialEq, Clone, Copy)]
enum Op {
    Same,
    Del,
    Add,
}

fn ops(a: &[&str], b: &[&str]) -> Option<Vec<(Op, usize, usize)>> {
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..].iter().rev().zip(b[pre..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (am, bm) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let (n, m) = (am.len(), bm.len());
    let mut out: Vec<(Op, usize, usize)> = (0..pre).map(|k| (Op::Same, k, k)).collect();
    if n == 0 || m == 0 {
        for i in 0..n {
            out.push((Op::Del, pre + i, pre));
        }
        for j in 0..m {
            out.push((Op::Add, pre + n, pre + j));
        }
        for k in 0..suf {
            out.push((Op::Same, a.len() - suf + k, b.len() - suf + k));
        }
        return Some(out);
    }

    let max_d = (n + m).min(MAX_D);
    let offset = max_d as isize;
    let mut v = vec![0isize; 2 * max_d + 1];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut found_d = None;

    'outer: for d in 0..=max_d {
        let di = d as isize;
        trace.push(v[(max_d - d)..=(max_d + d)].to_vec());
        for k in (-di..=di).step_by(2) {
            let mut x = if k == -di || (k != di && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize]) {
                v[(k + 1 + offset) as usize]
            } else {
                v[(k - 1 + offset) as usize] + 1
            };
            let mut y = x - k;
            while (x as usize) < n && (y as usize) < m && am[x as usize] == bm[y as usize] {
                x += 1;
                y += 1;
            }
            v[(k + offset) as usize] = x;
            if x as usize >= n && y as usize >= m {
                found_d = Some(d);
                break 'outer;
            }
        }
    }

    let final_d = found_d?;
    let mut x = n as isize;
    let mut y = m as isize;
    let mut rev: Vec<(Op, usize, usize)> = Vec::new();
    for d in (1..=final_d).rev() {
        let snap = &trace[d];
        let di = d as isize;
        let k = x - y;
        let prev_k = if k == -di || (k != di && snap[(k - 1 + di) as usize] < snap[(k + 1 + di) as usize]) {
            k + 1
        } else {
            k - 1
        };
        let prev_x = snap[(prev_k + di) as usize];
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            rev.push((Op::Same, pre + x as usize, pre + y as usize));
        }
        if x == prev_x {
            y -= 1;
            rev.push((Op::Add, pre + x as usize, pre + y as usize));
        } else {
            x -= 1;
            rev.push((Op::Del, pre + x as usize, pre + y as usize));
        }
    }
    while x > 0 && y > 0 {
        x -= 1;
        y -= 1;
        rev.push((Op::Same, pre + x as usize, pre + y as usize));
    }
    rev.reverse();
    out.extend(rev);
    for k in 0..suf {
        out.push((Op::Same, a.len() - suf + k, b.len() - suf + k));
    }
    Some(out)
}

pub fn counts(old: &str, new: &str) -> (usize, usize) {
    let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    match ops(&a, &b) {
        Some(o) => (o.iter().filter(|x| x.0 == Op::Add).count(), o.iter().filter(|x| x.0 == Op::Del).count()),
        None => {
            let mut seen: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
            for l in &a {
                *seen.entry(l).or_default() += 1;
            }
            let mut added = 0;
            for l in &b {
                let e = seen.entry(l).or_default();
                if *e > 0 {
                    *e -= 1;
                } else {
                    added += 1;
                }
            }
            (added, seen.values().filter(|v| **v > 0).map(|v| *v as usize).sum())
        }
    }
}

pub fn unified(name: &str, old: &str, new: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let Some(o) = ops(&a, &b) else {
        return format!("--- a/{name}\n+++ b/{name}\n(The file is too large to show line by line.)\n");
    };
    let mut out = format!("--- a/{name}\n+++ b/{name}\n");
    let changed: Vec<usize> = o.iter().enumerate().filter(|(_, x)| x.0 != Op::Same).map(|(k, _)| k).collect();
    let mut k = 0;
    while k < changed.len() {
        let start = changed[k].saturating_sub(3);
        let mut end = changed[k];
        while k + 1 < changed.len() && changed[k + 1] <= end + 6 {
            k += 1;
            end = changed[k];
        }
        let end = (end + 3).min(o.len() - 1);
        let hunk = &o[start..=end];
        let old_n = hunk.iter().filter(|x| x.0 != Op::Add).count();
        let new_n = hunk.iter().filter(|x| x.0 != Op::Del).count();
        out.push_str(&format!("@@ -{},{} +{},{} @@\n", hunk[0].1 + 1, old_n, hunk[0].2 + 1, new_n));
        for (op, i, j) in hunk {
            match op {
                Op::Same => out.push_str(&format!(" {}\n", a[*i])),
                Op::Del => out.push_str(&format!("-{}\n", a[*i])),
                Op::Add => out.push_str(&format!("+{}\n", b[*j])),
            }
        }
        k += 1;
    }
    out
}

pub fn typing_frames(old: &str, new: &str, max: usize) -> Vec<(String, u32)> {
    let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let Some(o) = ops(&a, &b) else { return Vec::new() };
    let mut cuts: Vec<usize> = Vec::new();
    for (k, x) in o.iter().enumerate() {
        let next_del = o.get(k + 1).is_some_and(|y| y.0 == Op::Del);
        if x.0 == Op::Add || (x.0 == Op::Del && !next_del) {
            cuts.push(k + 1);
        }
    }
    if cuts.is_empty() {
        return Vec::new();
    }
    let picked: Vec<usize> = if cuts.len() <= max.max(1) {
        cuts
    } else {
        let n = max.max(1);
        (1..=n).map(|s| cuts[(s * cuts.len()).div_ceil(n) - 1]).collect()
    };
    let last = picked.len() - 1;
    picked
        .into_iter()
        .enumerate()
        .map(|(s, k)| {
            let done: Vec<&str> = o[..k].iter().filter(|x| x.0 != Op::Del).map(|x| b[x.2]).collect();
            let line = done.len().saturating_sub(1) as u32;
            if s == last {
                return (new.to_string(), line);
            }
            let rest = o[k..].iter().filter(|x| x.0 != Op::Add).map(|x| a[x.1]);
            (done.iter().copied().chain(rest).collect::<Vec<_>>().join("\n"), line)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{counts, typing_frames, unified};

    #[test]
    fn typing_goes_line_by_line() {
        let f = typing_frames("a\nb\nc\n", "a\nB\nx\nc\n", 10);
        let texts: Vec<&str> = f.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(texts, vec!["a\nc", "a\nB\nc", "a\nB\nx\nc\n"]);
        assert_eq!(f[2].1, 2);
        assert_eq!(typing_frames("a\n", "a\n", 10).len(), 0);
        let many = typing_frames("", &"l\n".repeat(100), 8);
        assert_eq!(many.len(), 8);
        assert_eq!(many[7].0, "l\n".repeat(100));
    }

    #[test]
    fn counts_and_unified() {
        let old = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let new = "a\nb\nC\nd\ne\nf\ng\nh\ni\n";
        assert_eq!(counts(old, new), (2, 1));
        assert_eq!(counts("", "x\ny\n"), (2, 0));
        assert_eq!(counts("x\n", ""), (0, 1));
        let u = unified("f.txt", old, new);
        assert!(u.starts_with("--- a/f.txt\n+++ b/f.txt\n@@ -1,"), "{u}");
        assert!(u.contains("-c\n+C\n"), "{u}");
        assert!(u.contains("+i\n"), "{u}");
    }

    #[test]
    fn large_file_diff_stays_fast_and_exact() {
        let old_lines: Vec<String> = (0..5_000).map(|i| format!("line {i}")).collect();
        let mut new_lines = old_lines.clone();
        for ix in (200..4_800).step_by(500) {
            new_lines[ix] = format!("changed {ix}");
        }
        let old = old_lines.join("\n") + "\n";
        let new = new_lines.join("\n") + "\n";
        assert_eq!(counts(&old, &new), (10, 10));
        let u = unified("big.txt", &old, &new);
        assert!(u.contains("-line 200\n+changed 200\n"), "{u}");
    }
}
