
const MAX_CELLS: usize = 4_000_000;

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
    if am.len().saturating_mul(bm.len()) > MAX_CELLS {
        return None;
    }
    let (n, m) = (am.len(), bm.len());
    let mut lcs = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i * (m + 1) + j] = if am[i] == bm[j] {
                lcs[(i + 1) * (m + 1) + j + 1] + 1
            } else {
                lcs[(i + 1) * (m + 1) + j].max(lcs[i * (m + 1) + j + 1])
            };
        }
    }
    let mut out: Vec<(Op, usize, usize)> = (0..pre).map(|k| (Op::Same, k, k)).collect();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && am[i] == bm[j] {
            out.push((Op::Same, pre + i, pre + j));
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i * (m + 1) + j + 1] > lcs[(i + 1) * (m + 1) + j]) {
            out.push((Op::Add, pre + i, pre + j));
            j += 1;
        } else {
            out.push((Op::Del, pre + i, pre + j));
            i += 1;
        }
    }
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
}
