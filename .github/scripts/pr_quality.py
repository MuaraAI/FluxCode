#!/usr/bin/env python3
"""Code quality of the lines a pull request adds: clippy and compiler warnings,
rustfmt, and a few patterns that make code harder to keep. Only the added lines count,
so old code that predates these checks does not fail anyone's pull request.

  cargo clippy --bin fluxcode --message-format=json -- -W clippy::all ... > clippy.json
  python3 pr_quality.py --repo DIR --base SHA --clippy clippy.json --config pr-check.json --out quality.json
"""
import argparse
import json
import os
import re
import subprocess
import sys

PATTERNS = [
    ("warn", "`unwrap()` / `expect()` (panics instead of handling the error)", r"\.(unwrap|expect)\s*\("),
    ("warn", "`todo!` / `unimplemented!` left in", r"\b(todo|unimplemented)!\s*\("),
    ("fail", "`dbg!` left in", r"\bdbg!\s*\("),
    ("warn", "`println!` / `eprintln!` (use the log or the UI)", r"\be?println!\s*\("),
    ("warn", "`thread::sleep` (blocks the thread; on the UI thread it freezes the window)", r"thread::sleep\s*\("),
    ("warn", "`block_on` (blocks the thread until the future ends)", r"\bblock_on\s*\("),
    ("warn", "`#[allow(...)]` hides a warning", r"#!?\[allow\("),
    ("warn", "`.clone()` inside a loop header", r"\bfor\b.*\bin\b.*\.clone\(\)"),
    ("warn", "`collect` then iterate again", r"\.collect::<Vec<[^>]*>>\(\)\s*\.(iter|into_iter)\(\)"),
    ("warn", "`std::fs::read_to_string` of a whole file", r"fs::read_to_string\s*\("),
]


def git(repo, *args):
    return subprocess.run(["git", "-C", repo, "-c", "core.hooksPath=/dev/null", *args],
                          check=True, capture_output=True).stdout.decode("utf-8", "replace")


def added(repo, base):
    """{path: {line: text}} of lines added since base, in the working tree's numbering."""
    out = {}
    diff = git(repo, "diff", "--no-ext-diff", "--no-textconv", "-U0", base, "HEAD", "--", "*.rs")
    path, n = None, 0
    for raw in diff.splitlines():
        if raw.startswith("+++ "):
            path = raw[6:] if raw.startswith("+++ b/") else None
            if path:
                out.setdefault(path, {})
        elif raw.startswith("@@"):
            m = re.search(r"\+(\d+)", raw)
            n = int(m.group(1)) if m else 0
        elif raw.startswith("+") and path:
            out[path][n] = raw[1:]
            n += 1
    return out


def new_files(repo, base):
    return set(git(repo, "diff", "--name-only", "--diff-filter=A", base, "HEAD", "--", "*.rs").split())


def clippy(path, lines, fail_groups):
    """Lints whose main span sits on an added line."""
    res = []
    try:
        msgs = [json.loads(l) for l in open(path, encoding="utf-8", errors="replace") if l.startswith("{")]
    except FileNotFoundError:
        return res, False
    errors = False
    seen = set()
    for m in msgs:
        if m.get("reason") != "compiler-message":
            continue
        d = m["message"]
        if d.get("level") == "error":
            errors = True
        if d.get("level") not in ("warning", "error"):
            continue
        span = next((s for s in d.get("spans", []) if s.get("is_primary")), None)
        if not span:
            continue
        f = span["file_name"].replace("\\", "/")
        ln = span["line_start"]
        if f not in lines or not any(l in lines[f] for l in range(ln, span["line_end"] + 1)):
            continue
        code = (d.get("code") or {}).get("code") or ""
        key = (f, ln, code)
        if key in seen:
            continue
        seen.add(key)
        group = clippy_group(d)
        level = "fail" if d["level"] == "error" or group in fail_groups else "warn"
        res.append({"level": level, "kind": f"clippy::{group}" if code.startswith("clippy::") else "rustc",
                    "code": code, "path": f, "line": ln, "message": d["message"]})
    return res, errors


def clippy_group(d):
    # Clippy names the group in a child note: "`#[warn(clippy::perf)]` implied by ..."
    for c in d.get("children", []):
        m = re.search(r"clippy::(correctness|suspicious|perf|complexity|style|pedantic|nursery|cargo)\b", c.get("message", ""))
        if m:
            return m.group(1)
    code = (d.get("code") or {}).get("code") or ""
    return "lint" if code.startswith("clippy::") else "rustc"


def rustfmt(repo, lines, edition):
    """rustfmt's complaints that touch added lines."""
    res = []
    for f in lines:
        p = subprocess.run(["rustfmt", "--check", "--color", "never", "--edition", edition, f],
                           cwd=repo, capture_output=True, text=True, errors="replace")
        text = p.stdout
        for m in re.finditer(r"^Diff in (.+?)(?: at line |:)(\d+):?\s*$([\s\S]*?)(?=^Diff in |\Z)", text, re.M):
            fp = m.group(1).removeprefix("\\\\?\\")
            if os.path.isabs(fp):
                try:
                    fp = os.path.relpath(fp, os.path.abspath(repo))
                except ValueError:
                    pass
            fp = fp.replace("\\", "/")
            start = int(m.group(2))
            span = sum(1 for l in m.group(3).splitlines() if l.startswith((" ", "-")))
            if fp in lines and any(l in lines[fp] for l in range(start, start + max(span, 1))):
                res.append({"level": "warn", "kind": "rustfmt", "code": "", "path": fp, "line": start,
                            "message": "not formatted the way `cargo fmt` would"})
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True)
    ap.add_argument("--base", required=True)
    ap.add_argument("--clippy", required=True)
    ap.add_argument("--config", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    cfg = json.load(open(a.config, encoding="utf-8"))["quality"]
    edition = re.search(r'^edition\s*=\s*"(\d+)"', open(os.path.join(a.repo, "Cargo.toml"), encoding="utf-8").read(), re.M)
    edition = edition.group(1) if edition else "2021"

    lines = added(a.repo, a.base)
    issues, compile_error = clippy(a.clippy, lines, set(cfg["fail_lint_groups"]))
    issues += rustfmt(a.repo, lines, edition)
    for f, ls in lines.items():
        for n, text in ls.items():
            code = text.split("//")[0]
            for level, name, rx in PATTERNS:
                if re.search(rx, code):
                    issues.append({"level": level, "kind": "pattern", "code": "", "path": f, "line": n, "message": name})
            if len(text) > cfg["max_line_length"]:
                issues.append({"level": "warn", "kind": "pattern", "code": "", "path": f, "line": n,
                               "message": f"line is {len(text)} characters long"})
    for f in new_files(a.repo, a.base):
        count = len(lines.get(f, {}))
        if count > cfg["max_new_file_lines"]:
            issues.append({"level": "warn", "kind": "size", "code": "", "path": f, "line": None,
                           "message": f"new file with {count} lines: split it into modules"})
    total_added = sum(len(v) for v in lines.values())
    json.dump({"added_rust_lines": total_added, "compile_error": compile_error, "issues": issues},
              open(a.out, "w", encoding="utf-8"), indent=1)
    print(f"{total_added} added Rust lines, {len(issues)} issue(s), compile error: {compile_error}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
