//! Line diff for the agent's changes: how many lines it added and removed in a
//! file, and a unified diff to read them. Common lines at the start and end are
//! cut off first, so a small edit in a big file costs almost nothing; the middle
//! goes through a longest-common-subsequence table, capped in size.

/// Above this many cells in the middle part, counts are estimated and no diff text is made.
const MAX_CELLS: usize = 4_000_000;

#[derive(Debug, PartialEq, Clone, Copy)]
enum Op {
    Same,
    Del,
    Add,
}

/// (op, index in old, index in new) for every line, in order; None if too large.
fn ops(a: &[&str], b: &[&str]) -> Option<Vec<(Op, usize, usize)>> {
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..].iter().rev().zip(b[pre..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (am, bm) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    if am.len().saturating_mul(bm.len()) > MAX_CELLS {
        return None;
    }
    let (n, m) = (am.len(), bm.len());
    // lcs[i][j] = longest common subsequence of am[i..] and bm[j..]
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
        // On a tie the removal comes first, so a changed line reads "-old" then "+new".
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

/// Lines added and removed going from `old` to `new`.
pub fn counts(old: &str, new: &str) -> (usize, usize) {
    let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    match ops(&a, &b) {
        Some(o) => (o.iter().filter(|x| x.0 == Op::Add).count(), o.iter().filter(|x| x.0 == Op::Del).count()),
        // Too big to diff exactly: compare line multisets.
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

/// A unified diff (3 lines of context) with `---`/`+++` headers named `name`.
pub fn unified(name: &str, old: &str, new: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let Some(o) = ops(&a, &b) else {
        return format!("--- a/{name}\n+++ b/{name}\n(The file is too large to show line by line.)\n");
    };
    let mut out = format!("--- a/{name}\n+++ b/{name}\n");
    let changed: Vec<usize> = o.iter().enumerate().filter(|(_, x)| x.0 != Op::Same).map(|(k, _)| k).collect();
    let mut k = 0;
    while k < changed.len() {
        // A hunk: changes closer than 6 lines apart stay together, with 3 lines around.
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

#[cfg(test)]
mod tests {
    use super::{counts, unified};

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
