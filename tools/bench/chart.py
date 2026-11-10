"""Draw the README benchmark charts from results.json (written by bench.ps1).

    python tools/bench/chart.py

Writes assets/benchmarks/benchmark-light.svg and benchmark-dark.svg (GitHub shows one
or the other through <picture>), and prints the README's exact-numbers table.

The design follows what the best benchmark charts share (esbuild, uv, Ruff, Bun, Deno,
pnpm and others): one small panel per metric with its unit and "lower is better", the
same editor order in every panel, horizontal bars on a linear axis from zero, the
exact value at each bar's end, the subject set apart by weight and one accent colour,
a transparent background, and the method, versions and machine written underneath.
"""

import json, os, statistics, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
OUT = os.path.join(ROOT, "assets", "benchmarks")

# (key, title, unit, decimals, which runs: "warm" skips run 1)
METRICS = [
    ("window_ms", "Window on screen after launch", "ms", 0, "warm"),
    ("memory_mb", "Memory in Task Manager, idle", "MB", 1, "all"),
    ("commit_mb", "Memory reserved", "MB", 0, "all"),
    ("cpu_pct", "CPU while idle", "% of one core", 2, "all"),
    ("processes", "Processes running", "", 0, "all"),
    ("disk_mb", "Size on disk", "MB", 0, "all"),
]

THEMES = {
    # Text tuned to GitHub's light and dark canvases; bars read on both.
    "light": {"text": "#1f2328", "muted": "#59636e", "grid": "rgba(127,127,127,0.25)", "accent": "#9a6533", "other": "#818b95"},
    "dark": {"text": "#e6edf3", "muted": "#9198a1", "grid": "rgba(127,127,127,0.25)", "accent": "#d0a070", "other": "#6e7681"},
}

FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans', Helvetica, Arial, sans-serif"


def short_version(v):
    """'1.27.0+stable.386.c84af5b' -> '1.27.0'."""
    return v.split("+")[0].strip()


def median_of(app, key, which):
    if key == "disk_mb":
        return app["disk_mb"]
    runs = app["runs"][1:] if which == "warm" and len(app["runs"]) > 1 else app["runs"]
    vals = [r[key] for r in runs if r.get(key) is not None]
    return statistics.median(vals) if vals else None


def fmt(v, dec):
    if v is None:
        return "n/a"
    if dec == 0:
        return f"{v:,.0f}"
    return f"{v:,.{dec}f}"


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def ratio_note(values, subject, key):
    """'2.1x less than Zed' against the best other app, when the subject leads."""
    others = [(n, v) for n, v in values if n != subject and v]
    mine = dict(values).get(subject)
    if not others or not mine:
        return ""
    # Windows counts CPU time in 15.6 ms ticks: over the 30 s window that is about
    # 0.05% of a core, so a ratio against a reading that small would be noise.
    if key == "cpu_pct" and mine < 0.1:
        return "under the 0.1% Windows can resolve here"
    name, best = min(others, key=lambda x: x[1])
    if best <= mine:
        return ""
    r = best / mine
    word = "faster than" if key == "window_ms" else "fewer than" if key == "processes" else "less than"
    return f"{r:.1f}x {word} {name}" if r < 100 else f"{r:,.0f}x {word} {name}"


def panel(x, y, w, metric, apps, t, subject):
    key, title, unit, dec, which = metric
    values = [(a["name"], median_of(a, key, which)) for a in apps]
    top = max((v for _, v in values if v), default=1)
    label_w, row_h, bar_h = 92, 26, 14
    plot_w = w - label_w - 150
    parts = []
    sub = f"{unit}, lower is better" if unit else "lower is better"
    parts.append(f'<text x="{x}" y="{y + 14}" font-size="13.5" font-weight="600" fill="{t["text"]}">{esc(title)}</text>')
    parts.append(f'<text x="{x}" y="{y + 31}" font-size="11.5" fill="{t["muted"]}">{esc(sub)}</text>')
    gy = y + 44
    # Gridlines at 0, 50%, 100% of the largest value (linear axis from zero).
    for f in (0, 0.5, 1):
        gx = x + label_w + plot_w * f
        parts.append(f'<line x1="{gx:.1f}" y1="{gy - 4}" x2="{gx:.1f}" y2="{gy + row_h * len(values) - 6}" stroke="{t["grid"]}" stroke-width="1"/>')
    for i, (name, v) in enumerate(values):
        ry = gy + i * row_h
        me = name == subject
        weight = "700" if me else "400"
        parts.append(f'<text x="{x}" y="{ry + 11}" font-size="12.5" font-weight="{weight}" fill="{t["text"]}">{esc(name)}</text>')
        bw = 0 if not v else max(2.0, plot_w * v / top)
        color = t["accent"] if me else t["other"]
        parts.append(f'<rect x="{x + label_w}" y="{ry}" width="{bw:.1f}" height="{bar_h}" rx="2" fill="{color}"/>')
        val = fmt(v, dec) + (f" {unit}" if unit and unit != "% of one core" else ("%" if unit == "% of one core" else ""))
        parts.append(f'<text x="{x + label_w + bw + 6:.1f}" y="{ry + 11}" font-size="12" font-weight="{weight}" fill="{t["text"]}">{esc(val)}</text>')
        if me:
            note = ratio_note(values, subject, key)
            if note:
                nx = x + label_w + bw + 6 + 7.2 * len(val) + 8
                parts.append(f'<text x="{nx:.1f}" y="{ry + 11}" font-size="11.5" fill="{t["muted"]}">{esc(note)}</text>')
    return "\n".join(parts), gy + row_h * len(values)


def svg(data, theme, subject="FluxCode"):
    t = THEMES[theme]
    apps = data["apps"]
    width, col_w, gap = 900, 430, 40
    out, y, h = [], 8, 0
    for i in range(0, len(METRICS), 2):
        row_bottom = y
        for j, m in enumerate(METRICS[i:i + 2]):
            p, bottom = panel(8 + j * (col_w + gap), y, col_w, m, apps, t, subject)
            out.append(p)
            row_bottom = max(row_bottom, bottom)
        y = row_bottom + 26
    m = data["machine"]
    versions = " · ".join(f'{a["name"]} {short_version(a["version"])}' for a in apps)
    foot = [
        f'{data["date"]} · {m["os"]} · {m["cpu"]} · {m["ram_gb"]} GB RAM',
        versions,
        "Median of 5 runs, fresh empty profile each, telemetry and update checks off, no folder open. Window: runs 2 to 5 (warm).",
        "Memory and CPU summed over all of an app's processes. Reproduce: tools/bench/bench.ps1",
    ]
    for k, line in enumerate(foot):
        out.append(f'<text x="8" y="{y + k * 17}" font-size="11.5" fill="{t["muted"]}">{esc(line)}</text>')
    h = y + len(foot) * 17
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{h}" viewBox="0 0 {width} {h}" font-family="{FONT}">\n'
            f'<title>FluxCode, Zed and VS Code: start-up, memory, CPU, processes and size</title>\n'
            + "\n".join(out) + "\n</svg>\n")


def table(data):
    apps = data["apps"]
    head = "| | " + " | ".join(f'{a["name"]} {short_version(a["version"])}' for a in apps) + " |"
    rows = [head, "| --- |" + " --- |" * len(apps)]
    for key, title, unit, dec, which in METRICS:
        cells = []
        for a in apps:
            v = median_of(a, key, which)
            if key == "disk_mb" or not a["runs"]:
                cells.append(fmt(v, dec))
                continue
            runs = a["runs"][1:] if which == "warm" and len(a["runs"]) > 1 else a["runs"]
            vals = [r[key] for r in runs if r.get(key) is not None]
            lo, hi = min(vals), max(vals)
            cells.append(f"{fmt(v, dec)} ({fmt(lo, dec)} to {fmt(hi, dec)})" if lo != hi else fmt(v, dec))
        u = f" ({unit})" if unit else ""
        rows.append(f"| {title}{u} | " + " | ".join(cells) + " |")
    return "\n".join(rows)


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "results.json")
    data = json.load(open(path, encoding="utf-8-sig"))
    os.makedirs(OUT, exist_ok=True)
    for theme in THEMES:
        with open(os.path.join(OUT, f"benchmark-{theme}.svg"), "w", encoding="utf-8", newline="\n") as f:
            f.write(svg(data, theme))
    print(table(data))


if __name__ == "__main__":
    main()
