#!/usr/bin/env python3
"""Performance of a pull request against its base, on the same runner, in one go.

Both binaries are built the same way and run in turn (base, head, base, head, ...), so
a slow or busy runner slows both alike and the comparison stays fair. Measured:

  size_bytes    the executable's size
  <workload>    median wall time (ms) and peak memory (KB) of a headless run:
                startup     `fluxcode --flux-to-md` on a missing file (start and exit)
                readme      README.flux to Markdown, both languages
                large       a ~3 MB Flux document to Markdown
  crates        packages in Cargo.lock

The window itself cannot be opened on a Linux runner without a GPU; tools/bench
measures that on a real PC.

  python3 pr_perf.py --base-bin B --head-bin H --base-dir BASE --head-dir HEAD --out perf.json
"""
import argparse
import json
import os
import random
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time


def run(cmd):
    start = time.perf_counter()
    p = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    _, status, usage = os.wait4(p.pid, 0)
    p.returncode = os.waitstatus_to_exitcode(status)
    return (time.perf_counter() - start) * 1000, usage.ru_maxrss, p.returncode


def crates(lock):
    try:
        return len(re.findall(r"^\[\[package\]\]", open(lock, encoding="utf-8").read(), re.M))
    except FileNotFoundError:
        return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-bin", required=True)
    ap.add_argument("--head-bin", required=True)
    ap.add_argument("--base-dir", required=True)
    ap.add_argument("--head-dir", required=True)
    ap.add_argument("--runs", type=int, default=40)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()

    tmp = tempfile.mkdtemp(prefix="fluxbench-")
    # Inputs come from the base tree, so both sides read exactly the same files.
    readme = os.path.join(a.base_dir, "README.flux")
    large = os.path.join(tmp, "large.flux")
    parts = [open(os.path.join(a.base_dir, p), encoding="utf-8").read()
             for p in ("README.flux", "docs/flux.flux") if os.path.exists(os.path.join(a.base_dir, p))]
    with open(large, "w", encoding="utf-8") as f:
        size = 0
        while size < 3_000_000:
            for p in parts:
                f.write(p + "\n\n")
                size += len(p)
    out_md = os.path.join(tmp, "out.md")
    workloads = {
        "startup": lambda b: [b, "--flux-to-md", os.path.join(tmp, "missing.flux"), out_md],
        "readme": lambda b: [b, "--flux-to-md", readme, out_md, "--lang", "id"],
        "large": lambda b: [b, "--flux-to-md", large, out_md],
    }
    bins = {"base": a.base_bin, "head": a.head_bin}
    result = {"runs": a.runs, "size_bytes": {k: os.path.getsize(v) for k, v in bins.items()},
              "crates": {"base": crates(os.path.join(a.base_dir, "Cargo.lock")),
                         "head": crates(os.path.join(a.head_dir, "Cargo.lock"))},
              "workloads": {}}
    for name, cmd in workloads.items():
        times = {"base": [], "head": []}
        rss = {"base": [], "head": []}
        codes = {"base": set(), "head": set()}
        for side in ("base", "head"):  # warm the page cache
            run(cmd(bins[side]))
        for i in range(a.runs):
            order = ["base", "head"] if random.random() < 0.5 else ["head", "base"]
            for side in order:
                ms, kb, code = run(cmd(bins[side]))
                times[side].append(ms)
                rss[side].append(kb)
                codes[side].add(code)
        result["workloads"][name] = {
            side: {"median_ms": round(statistics.median(times[side]), 3),
                   "p90_ms": round(sorted(times[side])[int(len(times[side]) * 0.9) - 1], 3),
                   "rss_kb": int(statistics.median(rss[side])),
                   "exit": sorted(codes[side])}
            for side in ("base", "head")
        }
        print(name, json.dumps(result["workloads"][name]))
    json.dump(result, open(a.out, "w", encoding="utf-8"), indent=1)
    shutil.rmtree(tmp, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
