#!/usr/bin/env python3
"""Security advisories (RustSec) a pull request brings in: cargo-deny runs on the base
and on the head, and only advisories the base does not already have are reported, so
an old advisory does not fail every pull request until it is fixed on main.

  python3 pr_deps.py --base-dir BASE --head-dir HEAD --out deps.json
"""
import argparse
import json
import subprocess
import sys


def advisories(manifest_dir):
    p = subprocess.run(["cargo", "deny", "--manifest-path", f"{manifest_dir}/Cargo.toml", "--format", "json",
                        "--log-level", "error", "check", "advisories"], capture_output=True, text=True, errors="replace")
    found = {}
    for line in (p.stderr + "\n" + p.stdout).splitlines():
        try:
            d = json.loads(line)
        except ValueError:
            continue
        f = d.get("fields") or {}
        adv = f.get("advisory") or {}
        if not adv.get("id"):
            continue
        pkg = ""
        for label in f.get("labels") or []:
            pkg = label.get("span") or pkg
        found[adv["id"]] = {"id": adv["id"], "kind": f.get("code") or "", "severity": f.get("severity") or "",
                            "package": adv.get("package") or pkg, "title": adv.get("title") or f.get("message") or ""}
    return found, p.returncode


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-dir", required=True)
    ap.add_argument("--head-dir", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    base, _ = advisories(a.base_dir)
    head, code = advisories(a.head_dir)
    new = [v for k, v in head.items() if k not in base]
    fixed = [v for k, v in base.items() if k not in head]
    json.dump({"new": new, "fixed": fixed, "existing": len(set(head) & set(base)), "head_exit": code},
              open(a.out, "w", encoding="utf-8"), indent=1)
    print(f"advisories: {len(new)} new, {len(fixed)} fixed, {len(set(head) & set(base))} already on main")
    return 0


if __name__ == "__main__":
    sys.exit(main())
