#!/usr/bin/env python3
"""Turns the raw results of the pull request check (quality.json, perf.json, deps.json,
jobs.json) into one comment and one verdict. It runs from main, with main's limits
(pr-check.json): the pull request's own copy of the limits is never used.

The results were produced by the pull request's code, so they are treated as untrusted
data: numbers are checked to be numbers and every string is escaped before it is shown.

  python3 pr_report.py --dir results/ --config pr-check.json --out-md report.md --out-json verdict.json
"""
import argparse
import json
import os
import sys


def load(d, name):
    try:
        with open(os.path.join(d, name), encoding="utf-8") as f:
            v = json.load(f)
        return v if isinstance(v, dict) else None
    except (OSError, ValueError):
        return None


def num(v):
    return v if isinstance(v, (int, float)) and not isinstance(v, bool) else None


def esc(s, n=140):
    s = str(s if s is not None else "")
    s = s.replace("`", "'").replace("|", "\\|").replace("\n", " ").replace("<", "&lt;").replace("@", "@\u200b")
    return s[:n]


def pct(base, head):
    return None if not base or head is None else (head - base) * 100.0 / base


def fmt_pct(p):
    return "n/a" if p is None else f"{p:+.1f}%"


def judge(p, warn, fail, abs_ok=False):
    if p is None or abs_ok:
        return "✅"
    return "❌" if p > fail else "⚠️" if p > warn else "✅"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", required=True)
    ap.add_argument("--config", required=True)
    ap.add_argument("--out-md", required=True)
    ap.add_argument("--out-json", required=True)
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    cfg = json.load(open(a.config, encoding="utf-8"))
    P, Q = cfg["perf"], cfg["quality"]
    jobs = load(a.dir, "jobs.json") or {}
    quality, perf, deps = load(a.dir, "quality.json"), load(a.dir, "perf.json"), load(a.dir, "deps.json")
    fails, warns = [], []
    out = ["### Quality, performance and dependencies", ""]

    # Jobs that did not finish.
    for job in ("quality", "perf", "deps"):
        state = esc(jobs.get(job, "missing"), 20)
        if state != "success":
            fails.append(f"the `{job}` job ended with `{state}` (it does not build, a test fails, or it was cancelled)")

    # Performance.
    out += ["#### Performance (this pull request against its base, same runner)", ""]
    if perf and isinstance(perf.get("workloads"), dict):
        out += ["| Measure | Base | This PR | Change | |", "| --- | --- | --- | --- | --- |"]
        sb, sh = num((perf.get("size_bytes") or {}).get("base")), num((perf.get("size_bytes") or {}).get("head"))
        p = pct(sb, sh)
        mark = judge(p, P["size_warn_pct"], P["size_fail_pct"])
        if sb and sh:
            out.append(f"| Executable size | {sb / 1e6:.2f} MB | {sh / 1e6:.2f} MB | {fmt_pct(p)} | {mark} |")
        (fails if mark == "❌" else warns if mark == "⚠️" else []).append(f"executable size {fmt_pct(p)}")
        names = {"startup": "Start and exit", "readme": "README.flux to Markdown", "large": "3 MB document to Markdown"}
        for key, label in names.items():
            w = perf["workloads"].get(key)
            if not isinstance(w, dict):
                fails.append(f"no result for `{key}`")
                continue
            b, h = w.get("base") or {}, w.get("head") or {}
            bt, ht = num(b.get("median_ms")), num(h.get("median_ms"))
            br, hr = num(b.get("rss_kb")), num(h.get("rss_kb"))
            pt, pr = pct(bt, ht), pct(br, hr)
            small = bt is not None and ht is not None and (ht - bt) < P["time_min_ms"]
            mt = judge(pt, P["time_warn_pct"], P["time_fail_pct"], small)
            mr = judge(pr, P["rss_warn_pct"], P["rss_fail_pct"])
            if bt is not None and ht is not None:
                out.append(f"| {label}: time (median) | {bt:.1f} ms | {ht:.1f} ms | {fmt_pct(pt)} | {mt} |")
            if br is not None and hr is not None:
                out.append(f"| {label}: peak memory | {br / 1024:.1f} MB | {hr / 1024:.1f} MB | {fmt_pct(pr)} | {mr} |")
            for m, what, pv in ((mt, "time", pt), (mr, "memory", pr)):
                if m == "❌":
                    fails.append(f"{label}: {what} {fmt_pct(pv)}")
                elif m == "⚠️":
                    warns.append(f"{label}: {what} {fmt_pct(pv)}")
            if b.get("exit") != h.get("exit"):
                fails.append(f"{label}: exit code changed from {esc(b.get('exit'), 20)} to {esc(h.get('exit'), 20)}")
        cb, ch = num((perf.get("crates") or {}).get("base")), num((perf.get("crates") or {}).get("head"))
        if cb is not None and ch is not None:
            d = ch - cb
            mark = "❌" if d >= P["new_crates_fail"] else "⚠️" if d >= P["new_crates_warn"] else "✅"
            out.append(f"| Crates in Cargo.lock | {cb} | {ch} | {d:+d} | {mark} |")
            if mark == "❌":
                fails.append(f"{d} new crates")
            elif mark == "⚠️":
                warns.append(f"{d} new crate(s): say why in the pull request")
        out += ["", f"<sub>Median of {esc(perf.get('runs'), 6)} interleaved runs per side. ✅ within limits · ⚠️ above "
                f"{P['time_warn_pct']}% (time) / {P['size_warn_pct']}% (size) / {P['rss_warn_pct']}% (memory) · ❌ above "
                f"{P['time_fail_pct']}% / {P['size_fail_pct']}% / {P['rss_fail_pct']}%. Opening the window is not measured "
                "here (no GPU on the runner): `tools/bench` measures that on a PC.</sub>", ""]
    else:
        out += ["No performance results (see the `perf` job).", ""]

    # Quality.
    out += ["#### Code quality (only the lines this pull request adds)", ""]
    if quality and isinstance(quality.get("issues"), list):
        issues = [i for i in quality["issues"] if isinstance(i, dict)]
        bad = [i for i in issues if i.get("level") == "fail"]
        meh = [i for i in issues if i.get("level") != "fail"]
        if quality.get("compile_error"):
            fails.append("it does not compile")
        if bad:
            fails.append(f"{len(bad)} lint(s) from {', '.join(Q['fail_lint_groups'])} or a leftover `dbg!`")
        if meh:
            warns.append(f"{len(meh)} style or maintainability note(s)")
        out.append(f"{num(quality.get('added_rust_lines')) or 0} Rust lines added · ❌ {len(bad)} must fix · ⚠️ {len(meh)} to consider")
        if issues:
            out += ["", "| | Where | Check | Message |", "| --- | --- | --- | --- |"]
            for i in (bad + meh)[:50]:
                where = f"`{esc(i.get('path'))}`" + (f":{num(i.get('line'))}" if num(i.get("line")) else "")
                check = esc(i.get("code") or i.get("kind"), 60)
                out.append(f"| {'❌' if i.get('level') == 'fail' else '⚠️'} | {where} | `{check}` | {esc(i.get('message'))} |")
            if len(issues) > 50:
                out.append(f"| | | | … and {len(issues) - 50} more (see the job log) |")
        out.append("")
    else:
        out += ["No quality results (see the `quality` job).", ""]

    # Dependencies.
    out += ["#### Dependencies (RustSec advisories)", ""]
    if deps and isinstance(deps.get("new"), list):
        new = [d for d in deps["new"] if isinstance(d, dict)]
        if new:
            fails.append(f"{len(new)} new security advisory(ies) in dependencies")
            out += ["| Advisory | Crate | Kind | Title |", "| --- | --- | --- | --- |"]
            out += [f"| `{esc(d.get('id'), 30)}` | `{esc(d.get('package'), 40)}` | {esc(d.get('kind'), 20)} | {esc(d.get('title'))} |" for d in new[:30]]
        else:
            out.append("No new advisories.")
        fixed = deps.get("fixed") if isinstance(deps.get("fixed"), list) else []
        if fixed:
            out.append(f"\nThis pull request clears {len(fixed)} advisory(ies) that main has. 🎉")
        out.append("")
    else:
        out += ["No dependency results (see the `deps` job).", ""]

    if fails:
        head = "**❌ Not ready:** " + "; ".join(fails) + "."
    elif warns:
        head = "**⚠️ Passed with notes:** " + "; ".join(warns) + "."
    else:
        head = "**✅ Passed:** no slowdown, no new lint, no new advisory."
    out.insert(2, head + "\n")
    md = "\n".join(out) + "\n"
    open(a.out_md, "w", encoding="utf-8").write(md)
    json.dump({"conclusion": "failure" if fails else "success", "fails": fails, "warns": warns},
              open(a.out_json, "w", encoding="utf-8"), indent=1)
    print(md)
    return 0


if __name__ == "__main__":
    sys.exit(main())
