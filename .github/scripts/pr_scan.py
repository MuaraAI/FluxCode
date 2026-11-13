#!/usr/bin/env python3
"""Static safety scan of a pull request's diff. Nothing from the pull request is run:
its files are only read, through git, as data.

  python3 pr_scan.py --repo DIR --base SHA --head SHA --config pr-check.json \
      [--outsider] [--labels a,b] --out-json scan.json --out-md scan.md

Findings have a level:
  critical  almost certainly malicious or a leaked secret: fails the check
  high      dangerous unless a maintainer says otherwise: fails the check until a
            maintainer adds the override label (pr-check.json: override_label)
  review    allowed, but a maintainer has to read it (sensitive paths, new processes,
            new network calls, unsafe, new domains)
  info      for the record (new dependencies, sizes)
Standard library only, so it runs on any runner without installing anything.
"""
import argparse
import json
import os
import re
import subprocess
import sys

LEVELS = ["critical", "high", "review", "info"]

# The scanner and its limits are full of the patterns below; they are sensitive paths
# (an outside pull request may not touch them at all), so their lines are not scanned.
SELF = {".github/scripts/pr_scan.py", ".github/pr-check.json"}

# (level, name, regex) checked against every added line of every text file.
LINE_RULES = [
    # Remote code run straight from the internet.
    ("critical", "download piped into a shell", r"(curl|wget|iwr|Invoke-WebRequest)\b[^\n|]*\|\s*(sudo\s+)?(ba|z|da)?sh\b|\|\s*iex\b"),
    ("critical", "PowerShell encoded command", r"(?i)powershell(\.exe)?\b.*\s-(e|ec|enc|encodedcommand)\s+[A-Za-z0-9+/=]{16,}"),
    ("high", "Invoke-Expression", r"(?i)\b(Invoke-Expression|iex)\s*[\(\$'\"]"),
    ("high", "base64 decoded and run", r"(?i)(FromBase64String|base64\s+(-d|--decode)).*(\||Invoke|iex|exec|sh\b|bash\b|python)"),
    # Reverse shells and droppers.
    ("critical", "reverse shell", r"/dev/tcp/|\bnc(at)?\b[^\n]*\s-e\s|bash\s+-i\s*>&|socat\b[^\n]*exec:|\bmkfifo\b[^\n]*\bnc\b"),
    ("critical", "crypto miner", r"(?i)stratum\+(tcp|ssl)://|\bxmrig\b|coinhive|cryptonight|minexmr|nicehash"),
    # Credential and data theft.
    ("critical", "browser or app credential store", r"(?i)(Login Data|Local State|os_crypt|Cookies\.sqlite|key4\.db|logins\.json|wallet\.dat|Discord\\+Local Storage|discord/Local Storage|leveldb)"),
    ("critical", "exfiltration endpoint", r"(?i)(discord(app)?\.com/api/webhooks|api\.telegram\.org/bot|hooks\.slack\.com/services|pastebin\.com/raw|transfer\.sh|webhook\.site|requestbin|ngrok\.io|ngrok-free\.app|interact\.sh|oast\.(fun|pro|live|site|online|me))"),
    ("high", "private key or system secret path", r"(?i)(\.ssh/id_(rsa|ed25519|ecdsa)|/etc/shadow|\.aws/credentials|\.git-credentials|\.npmrc|\.pypirc|\.docker/config\.json|\.kube/config)"),
    ("critical", "Windows credential APIs", r"\b(CryptUnprotectData|CredEnumerate|LsaRetrievePrivateData|MiniDumpWriteDump)\b"),
    ("critical", "process injection", r"\b(VirtualAllocEx|WriteProcessMemory|CreateRemoteThread|NtCreateThreadEx|QueueUserAPC|SetThreadContext)\b"),
    ("high", "keyboard hook or key logging", r"\b(SetWindowsHookEx[AW]?|GetAsyncKeyState|GetKeyboardState|WH_KEYBOARD_LL)\b"),
    # Persistence and destruction.
    ("high", "autostart persistence", r"(?i)(CurrentVersion\\+Run\b|schtasks(\.exe)?\s+/create|\bcrontab\s+-|/etc/cron\.|LaunchAgents|LaunchDaemons|systemctl\s+enable|\.bashrc|\.zshrc|\.profile\b|LD_PRELOAD|DYLD_INSERT_LIBRARIES)"),
    ("critical", "destructive command", r"(?i)(rm\s+-rf\s+(/|~|\$HOME)(\s|$|\*)|format\s+c:|vssadmin\s+delete\s+shadows|cipher\s+/w|wbadmin\s+delete|bcdedit\s+/set|Remove-Item\s+-Recurse\s+-Force\s+(C:\\|\$env:USERPROFILE|~)\s*$)"),
    ("high", "security tooling turned off", r"(?i)(Set-MpPreference\s+-Disable|Add-MpPreference\s+-ExclusionPath|DisableRealtimeMonitoring|netsh\s+advfirewall\s+set\s+\w+\s+state\s+off|setenforce\s+0)"),
    # Leaked secrets.
    ("critical", "secret: private key", r"-----BEGIN ([A-Z]+ )?PRIVATE KEY-----"),
    ("critical", "secret: GitHub token", r"\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36}\b|\bgithub_pat_[A-Za-z0-9_]{60,}"),
    ("critical", "secret: API key", r"\bsk-(ant-|proj-)?[A-Za-z0-9_\-]{32,}|\bAKIA[0-9A-Z]{16}\b|\bAIza[0-9A-Za-z_\-]{35}\b|\bxox[abprs]-[A-Za-z0-9-]{10,}|\bglpat-[A-Za-z0-9_\-]{20}\b|\bnpm_[A-Za-z0-9]{36}\b"),
    # Hidden code.
    ("high", "long encoded blob", r"[A-Za-z0-9+/]{240,}={0,2}|(?:\\x[0-9a-fA-F]{2}){48,}|(?:0x[0-9a-fA-F]{2},\s*){64,}"),
    ("high", "URL to a raw IP address", r"(?i)\b(https?|wss?|tcp)://(?!127\.0\.0\.1|0\.0\.0\.0|10\.0\.0\.1\b|192\.168\.1\.1\b)\d{1,3}(\.\d{1,3}){3}"),
]

# Rust and script lines that are fine on their own but a maintainer must see.
REVIEW_RULES = [
    ("starts a process", r"\bCommand::new\s*\(|std::process::Command|\bShellExecute\w*\b|\bCreateProcess\w*\b|subprocess\.|os\.system\(|Start-Process\b"),
    ("opens a network connection", r"\b(TcpStream|UdpSocket|TcpListener)::|\b(reqwest|ureq|hyper|isahc|curl)::|\burllib\.request|\bhttp\.client\b|\bWebSocket\b|Invoke-RestMethod|Invoke-WebRequest"),
    ("unsafe Rust", r"\bunsafe\s*(\{|fn\b|impl\b)"),
    ("foreign code or dynamic loading", r"\bextern\s+\"(C|system|stdcall)\"|\blibloading\b|\bLoadLibrary\w*\b|\bdlopen\b|#\[link\("),
    ("embeds a file at build time", r"\binclude_bytes!\s*\(|\binclude_str!\s*\("),
    ("reads a secret-looking environment variable", r"(?i)env::var(_os)?\s*\(\s*\"[A-Z0-9_]*(TOKEN|SECRET|PASSWORD|API_KEY|PRIVATE)[A-Z0-9_]*\""),
    ("deletes files recursively", r"\bremove_dir_all\s*\(|shutil\.rmtree|rm\s+-rf\b|Remove-Item\s+[^\n]*-Recurse"),
    ("changes file permissions", r"\bset_permissions\s*\(|\bchmod\b|icacls\b"),
]

# Invisible and direction-changing characters: the "Trojan Source" attack hides code
# that reads differently to people than to the compiler.
BIDI = re.compile("[\u202a-\u202e\u2066-\u2069\u200b-\u200f\u2028\u2029\u2060-\u2064\ufeff]")
# Cyrillic or Greek letters right next to Latin ones: look-alike identifiers.
HOMOGLYPH = re.compile(r"[A-Za-z_][\u0370-\u03ff\u0400-\u04ff]|[\u0370-\u03ff\u0400-\u04ff][A-Za-z_]")
URL = re.compile(r"\b(?:https?|wss?)://([A-Za-z0-9.-]+)")

EXEC_EXT = {".exe", ".dll", ".so", ".dylib", ".bin", ".com", ".scr", ".msi", ".sys", ".node",
            ".jar", ".class", ".pyc", ".wasm", ".zip", ".7z", ".rar", ".tar", ".gz", ".xz",
            ".bz2", ".zst", ".cab", ".apk", ".dmg", ".pkg", ".deb", ".rpm", ".a", ".lib", ".o", ".obj"}
EXEC_MAGIC = [(b"MZ", "Windows executable"), (b"\x7fELF", "Linux executable"),
              (b"\xcf\xfa\xed\xfe", "macOS executable"), (b"\xca\xfe\xba\xbe", "macOS or Java binary"),
              (b"PK\x03\x04", "ZIP archive"), (b"\x00asm", "WebAssembly"), (b"7z\xbc\xaf", "7-Zip archive")]


def git(repo, *args, binary=False):
    cmd = ["git", "-C", repo, "-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false",
           "-c", "diff.external=", *args]
    out = subprocess.run(cmd, check=True, capture_output=True).stdout
    return out if binary else out.decode("utf-8", "replace")


def changed_files(repo, base, head):
    """[(status, path, old_path)] between the merge base and head."""
    out = git(repo, "diff", "--no-ext-diff", "--no-textconv", "--name-status", "-M", "-z", f"{base}...{head}")
    parts = out.split("\0")
    files, i = [], 0
    while i < len(parts) and parts[i]:
        status = parts[i]
        if status[0] in "RC":
            files.append((status[0], parts[i + 2], parts[i + 1]))
            i += 3
        else:
            files.append((status[0], parts[i + 1], None))
            i += 2
    return files


def added_lines(repo, base, head, path):
    """{line number in head: text} for the lines this pull request adds to path."""
    diff = git(repo, "diff", "--no-ext-diff", "--no-textconv", "-U0", f"{base}...{head}", "--", path)
    lines, n = {}, 0
    for raw in diff.splitlines():
        if raw.startswith("@@"):
            m = re.search(r"\+(\d+)", raw)
            n = int(m.group(1)) if m else 0
        elif raw.startswith("+") and not raw.startswith("+++"):
            lines[n] = raw[1:]
            n += 1
    return lines


def blob(repo, rev, path):
    try:
        return git(repo, "show", f"{rev}:{path}", binary=True)
    except subprocess.CalledProcessError:
        return None


def is_binary(data):
    return data is not None and b"\0" in data[:8192]


def under(path, prefixes):
    return any(path == p.rstrip("/") or path.startswith(p if p.endswith("/") else p + "/") or path == p for p in prefixes)


def cargo_lock_packages(text):
    pkgs = {}
    for block in (text or "").split("[[package]]")[1:]:
        name = re.search(r'^name = "(.+)"', block, re.M)
        ver = re.search(r'^version = "(.+)"', block, re.M)
        src = re.search(r'^source = "(.+)"', block, re.M)
        sum_ = re.search(r'^checksum = "(.+)"', block, re.M)
        if name and ver:
            pkgs[(name.group(1), ver.group(1))] = (src.group(1) if src else None, sum_.group(1) if sum_ else None)
    return pkgs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True)
    ap.add_argument("--base", required=True)
    ap.add_argument("--head", required=True)
    ap.add_argument("--config", required=True)
    ap.add_argument("--outsider", action="store_true")
    ap.add_argument("--labels", default="")
    ap.add_argument("--out-json", required=True)
    ap.add_argument("--out-md", required=True)
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    cfg = json.load(open(a.config, encoding="utf-8"))
    labels = {l for l in a.labels.split(",") if l}
    known = set(cfg["known_domains"])

    findings = []

    def add(level, rule, path=None, line=None, detail=""):
        findings.append({"level": level, "rule": rule, "path": path, "line": line, "detail": detail[:160]})

    files = changed_files(a.repo, a.base, a.head)
    new_domains = {}
    for status, path, old in files:
        ext = os.path.splitext(path)[1].lower()
        if under(path, cfg["sensitive_paths"]) or (old and under(old, cfg["sensitive_paths"])):
            if a.outsider and (under(path, cfg["outsider_blocked_paths"]) or (old and under(old, cfg["outsider_blocked_paths"]))):
                add("high", "outside contributor changes CI, build or packaging files", path,
                    detail="These run with the project's trust (CI, the build script, installers). Only a maintainer can approve this.")
            else:
                add("review", "sensitive path", path)
        if status == "D":
            continue
        data = blob(a.repo, a.head, path)
        if data is None:
            continue
        if is_binary(data) or ext in EXEC_EXT:
            kind = next((k for magic, k in EXEC_MAGIC if data.startswith(magic)), None)
            if kind or ext in EXEC_EXT:
                add("critical", "executable or archive committed", path, detail=kind or ext)
            elif ext not in cfg["allowed_binary_ext"]:
                add("high", "unknown binary file", path, detail=f"{len(data)} bytes")
            elif len(data) > 2_000_000:
                add("review", "large binary file", path, detail=f"{len(data) / 1e6:.1f} MB")
            continue
        if path in SELF:
            continue
        # Executable bit on a new script.
        mode = git(a.repo, "ls-tree", a.head, "--", path).split(" ")[0] if path else ""
        if mode == "100755" and status == "A":
            add("review", "new executable file", path)

        lines = added_lines(a.repo, a.base, a.head, path)
        is_code = ext in {".rs", ".py", ".ps1", ".sh", ".cmd", ".bat", ".js", ".ts", ".toml", ".yml", ".yaml", ".nsi", ".json"}
        for n, text in lines.items():
            if BIDI.search(text):
                add("critical", "invisible or text-direction character (Trojan Source)", path, n, repr(BIDI.search(text).group()))
            if ext == ".rs" and HOMOGLYPH.search(text) and not text.lstrip().startswith("//"):
                add("high", "look-alike letters (Cyrillic/Greek next to Latin)", path, n, text.strip())
            for level, name, rx in LINE_RULES:
                if re.search(rx, text):
                    add(level, name, path, n, text.strip())
            if is_code:
                for name, rx in REVIEW_RULES:
                    if re.search(rx, text):
                        add("review", name, path, n, text.strip())
            for host in URL.findall(text):
                host = host.lower().rstrip(".")
                if host not in known and not host.endswith((".github.com", ".githubusercontent.com", ".rust-lang.org", ".microsoft.com")) \
                        and "." in host and host not in {"x.y", "host"}:
                    new_domains.setdefault(host, (path, n))
            if is_code and len(text) > 1000:
                add("high", "very long line (minified or obfuscated code)", path, n, f"{len(text)} characters")

        # Dependency files.
        if path == "Cargo.toml" or path.endswith("/Cargo.toml"):
            for n, text in lines.items():
                if re.search(r"\b(git|registry|registry-index)\s*=", text):
                    add("high", "dependency from outside crates.io", path, n, text.strip())
                if re.search(r"\bpath\s*=", text) and "vendor/" not in text:
                    add("review", "dependency from a local path", path, n, text.strip())
                if re.search(r"^\s*build\s*=|^\s*\[patch|links\s*=", text):
                    add("high", "build script or patched crate", path, n, text.strip())
        if path == "Cargo.lock":
            old_pkgs = cargo_lock_packages((blob(a.repo, a.base, "Cargo.lock") or b"").decode("utf-8", "replace"))
            new_pkgs = cargo_lock_packages(data.decode("utf-8", "replace"))
            old_names = {n for n, _ in old_pkgs}
            for key, (src, csum) in new_pkgs.items():
                if src and not src.startswith("registry+https://github.com/rust-lang/crates.io-index") \
                        and not src.startswith("sparse+https://index.crates.io"):
                    add("high", "locked crate from outside crates.io", path, detail=f"{key[0]} {key[1]} {src}")
                if key in old_pkgs and old_pkgs[key][1] and csum and old_pkgs[key][1] != csum:
                    add("critical", "checksum of an existing crate changed", path, detail=f"{key[0]} {key[1]}")
                if key not in old_pkgs:
                    add("info", "new crate" if key[0] not in old_names else "crate version changed", path, detail=f"{key[0]} {key[1]}")
        if path.startswith(".github/workflows/"):
            for n, text in lines.items():
                if text.lstrip().startswith("#"):
                    continue
                if re.search(r"^\s*(pull_request_target|workflow_run)\s*:|\$\{\{\s*(secrets\.|github\.token)|secrets:\s*inherit|"
                             r"permissions:\s*write-all|^\s*(contents|actions|packages|id-token|deployments|security-events|statuses)\s*:\s*write|GITHUB_TOKEN", text):
                    add("high", "workflow gains a trigger, permission or secret", path, n, text.strip())
                if re.search(r"uses:\s*[^@\s]+@(?![0-9a-f]{40}\b)(main|master|HEAD|latest)\b", text):
                    add("high", "workflow uses an action from a moving branch", path, n, text.strip())
        if os.path.basename(path) in {"build.rs", "install.ps1", "install.sh", "Install.cmd", "installer.nsi"}:
            for n, text in lines.items():
                if re.search(r"(?i)https?://|TcpStream|Command::new|Invoke-WebRequest|curl|wget", text):
                    add("high", "build or install script downloads or runs something", path, n, text.strip())

    for host, (path, n) in sorted(new_domains.items()):
        add("review", "new web address", path, n, host)

    override = cfg["override_label"] in labels
    blocking = [f for f in findings if f["level"] == "critical" or (f["level"] == "high" and not override)]
    needs_review = any(f["level"] in ("high", "review") for f in findings)
    result = {
        "files": len(files),
        "findings": findings,
        "counts": {lvl: sum(f["level"] == lvl for f in findings) for lvl in LEVELS},
        "blocking": len(blocking),
        "needs_review": needs_review,
        "override": override,
        "outsider": a.outsider,
    }
    json.dump(result, open(a.out_json, "w", encoding="utf-8"), indent=1)
    open(a.out_md, "w", encoding="utf-8").write(render(result, cfg))
    print(render(result, cfg))
    return 0


def esc(s):
    """Text from the pull request, made safe to show inside a table cell."""
    s = (s or "").replace("`", "'").replace("|", "\\|").replace("\n", " ").replace("<", "&lt;").replace("@", "@\u200b")
    return s[:160]


def render(r, cfg):
    icon = {"critical": "🛑", "high": "⛔", "review": "👀", "info": "ℹ️"}
    out = ["### Safety scan", ""]
    c = r["counts"]
    if r["blocking"]:
        out.append(f"**Blocked: {r['blocking']} finding(s) must be resolved.**"
                   + ("" if c["critical"] else f" A maintainer who has read them can add the `{cfg['override_label']}` label."))
    elif r["needs_review"]:
        out.append("**Passed, with things a maintainer has to read before merging.**"
                   + (f" (`{cfg['override_label']}` was added by a maintainer.)" if r["override"] else ""))
    else:
        out.append("**Passed.** Nothing suspicious in the added lines.")
    out.append("")
    out.append(f"{r['files']} file(s) scanned · " + " · ".join(f"{icon[l]} {l}: {c[l]}" for l in LEVELS))
    shown = [f for f in r["findings"] if f["level"] != "info"]
    if shown:
        out += ["", "| | Finding | Where | Detail |", "| --- | --- | --- | --- |"]
        order = {l: i for i, l in enumerate(LEVELS)}
        for f in sorted(shown, key=lambda f: (order[f["level"]], f["path"] or "", f["line"] or 0))[:60]:
            where = f"`{esc(f['path'])}`" + (f":{f['line']}" if f["line"] else "") if f["path"] else ""
            detail = f"`{esc(f['detail'])}`" if f["detail"] else ""
            out.append(f"| {icon[f['level']]} | {esc(f['rule'])} | {where} | {detail} |")
        if len(shown) > 60:
            out.append(f"| | … and {len(shown) - 60} more | | |")
    info = [f for f in r["findings"] if f["level"] == "info"]
    if info:
        out += ["", "<details><summary>Dependency changes (" + str(len(info)) + ")</summary>", ""]
        out += [f"- {esc(f['rule'])}: `{esc(f['detail'])}`" for f in info[:100]]
        out += ["", "</details>"]
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    sys.exit(main())
