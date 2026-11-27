"""Generate vertical benchmark-styled FluxCode advantages SVGs for README (EN & ID, light & dark).

    python tools/bench/advantages.py

Writes:
  assets/benchmarks/advantages-light.svg
  assets/benchmarks/advantages-dark.svg
  assets/benchmarks/advantages-id-light.svg
  assets/benchmarks/advantages-id-dark.svg
"""

import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
OUT = os.path.join(ROOT, "assets", "benchmarks")

FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans', Helvetica, Arial, sans-serif"

THEMES = {
    "light": {
        "text": "#1f2328",
        "muted": "#59636e",
        "faint": "#6e7781",
        "grid": "rgba(127,127,127,0.22)",
        "card": "rgba(127,127,127,0.04)",
        "border": "rgba(127,127,127,0.22)",
        "accent": "#9a6533",
        "other": "#818b95",
        "dim": "rgba(127,127,127,0.35)",
    },
    "dark": {
        "text": "#e6edf3",
        "muted": "#9198a1",
        "faint": "#7d8590",
        "grid": "rgba(127,127,127,0.22)",
        "card": "rgba(255,255,255,0.025)",
        "border": "rgba(127,127,127,0.24)",
        "accent": "#d0a070",
        "other": "#6e7681",
        "dim": "rgba(127,127,127,0.35)",
    },
}

# Part A: numeric benchmarks (shorter bar = lighter & faster)
NUMERIC = [
    {
        "en": ("1. Window open after launch", "Opens right away with no splash screen or wait.", "ms, shorter is better"),
        "id": ("1. Kecepatan buka jendela", "Langsung siap dipakai begitu diklik, tanpa tunggu loading.", "ms, makin pendek makin baik"),
        "max": 3000,
        "rows": [
            ("FluxCode", 10, "10 ms  (Rust + GPUI, 81.8x faster than Zed)", "10 ms  (Rust + GPUI, 81,8x lebih cepat dari Zed)"),
            ("Zed", 818, "818 ms  (Rust + GPUI)", "818 ms  (Rust + GPUI)"),
            ("VS Code", 3000, "~3,000 ms  (Electron workbench usable)", "~3.000 ms  (Electron sampai siap pakai)"),
        ],
    },
    {
        "en": ("2. Idle memory (RAM)", "Stays light in the background so your laptop stays cool and fast.", "MB, shorter is better"),
        "id": ("2. Penggunaan RAM saat diam", "Tetap ringan di latar belakang, laptop tidak lemot atau panas.", "MB, makin pendek makin baik"),
        "max": 929.2,
        "rows": [
            ("FluxCode", 1.4, "1.4 MB  (47.3x less than Zed, 663x less than VS Code)", "1,4 MB  (47,3x lebih hemat dari Zed, 663x dari VS Code)"),
            ("Zed", 66.2, "66.2 MB  (Task Manager idle)", "66,2 MB  (Task Manager saat idle)"),
            ("VS Code", 929.2, "929.2 MB  (all Electron processes summed)", "929,2 MB  (total seluruh proses Electron)"),
        ],
    },
    {
        "en": ("3. Reserved memory", "Claims far less virtual memory on low-RAM laptops.", "MB, shorter is better"),
        "id": ("3. Memori yang dicadangkan", "Hemat memori pada laptop spek ringan maupun RAM terbatas.", "MB, makin pendek makin baik"),
        "max": 1315,
        "rows": [
            ("FluxCode", 72, "72 MB  (2.1x less than Zed)", "72 MB  (2,1x lebih hemat dari Zed)"),
            ("Zed", 154, "154 MB", "154 MB"),
            ("VS Code", 1315, "1,315 MB", "1.315 MB"),
        ],
    },
    {
        "en": ("4. Background processes", "One clean app process instead of spawning a cluster of helpers.", "processes, fewer is better"),
        "id": ("4. Jumlah proses berjalan", "Cukup satu proses bersih, tidak memecah banyak proses di Task Manager.", "proses, makin sedikit makin baik"),
        "max": 9,
        "rows": [
            ("FluxCode", 1, "1 process  (single native process)", "1 proses  (satu proses native)"),
            ("Zed", 2, "2 processes", "2 proses"),
            ("VS Code", 9, "9 processes  (renderer, GPU, extension host, helpers)", "9 proses  (renderer, GPU, extension host, helper)"),
        ],
    },
    {
        "en": ("5. Install size on disk", "Downloads in seconds and takes the space of a few photos.", "MB, shorter is better"),
        "id": ("5. Ukuran aplikasi di disk", "Cepat diunduh dan sangat hemat ruang penyimpanan.", "MB, makin pendek makin baik"),
        "max": 1044,
        "rows": [
            ("FluxCode", 16, "16 MB  (one binary, 16.3x smaller than Zed)", "16 MB  (satu file program, 16,3x lebih kecil dari Zed)"),
            ("Zed", 261, "261 MB", "261 MB"),
            ("VS Code", 1044, "1,044 MB  (over 1 GB installed)", "1.044 MB  (lebih dari 1 GB terpasang)"),
        ],
    },
]

# Part B: built-in capabilities (status level: "full", "partial", "none")
FEATURES = [
    {
        "en": ("6. Built-in AI agents & RAM optimizer", "Chat with AI right inside the editor without freezing your laptop, and undo any turn."),
        "id": ("6. Agen AI bawaan & hemat RAM otomatis", "Asisten AI langsung siap pakai tanpa bikin RAM penuh, setiap perubahan bisa di-undo."),
        "rows": [
            ("FluxCode", "full", "ACP (OpenCode 2, Claude, Codex, Gemini, Copilot), FIFO queue, Force Send, EcoQoS ~94% trim", "ACP (OpenCode 2, Claude, Codex, Gemini, Copilot), antrean FIFO, Force Send, EcoQoS ~94%"),
            ("Zed", "partial", "Built-in agent + ACP (no live EcoQoS RAM trim or Job Object isolation)", "Agen bawaan + ACP (tanpa pemangkasan RAM EcoQoS atau isolasi Job Object)"),
            ("VS Code", "partial", "GitHub Copilot + third-party extensions", "GitHub Copilot + ekstensi pihak ketiga"),
        ],
    },
    {
        "en": ("7. VS Code Marketplace extensions", "Install your favorite icons, themes, and AI extensions from the same ecosystem."),
        "id": ("7. Ekstensi VS Code Marketplace", "Pasang ikon, tema, dan ekstensi AI favorit dari ekosistem yang sama."),
        "rows": [
            ("FluxCode", "full", "Almost all VS Code extensions work (icons, themes, AI); report bugs in GitHub Issues", "Hampir semua ekstensi VS Code didukung (ikon, tema, AI); laporkan bug di GitHub Issues"),
            ("Zed", "partial", "Separate Zed extension catalog", "Katalog ekstensi tersendiri (bukan VS Code Marketplace)"),
            ("VS Code", "full", "Original full marketplace catalog", "Katalog marketplace asli terlengkap"),
        ],
    },
    {
        "en": ("8. Built-in WizTree disk analyzer", "See which folders eat your storage instantly, without installing a separate tool."),
        "id": ("8. Penganalisis disk WizTree bawaan", "Langsung lihat folder yang paling makan ruang penyimpanan tanpa aplikasi tambahan."),
        "rows": [
            ("FluxCode", "full", "Built-in Rust treemap (Ctrl+Shift+Y) + @wiztree AI context in 0-1 tool calls", "Treemap Rust bawaan (Ctrl+Shift+Y) + konteks AI @wiztree dalam 0-1 tool call"),
            ("Zed", "none", "Not built in", "Tidak tersedia"),
            ("VS Code", "partial", "Requires third-party extensions", "Perlu pasang ekstensi tambahan"),
        ],
    },
    {
        "en": ("9. Non-text & game files", "Open and edit databases, PDFs, Office docs, ZIPs, and Minecraft files in one app."),
        "id": ("9. File non-teks & file game", "Buka dan edit database, PDF, dokumen Office, ZIP, hingga file Minecraft langsung."),
        "rows": [
            ("FluxCode", "full", "Editable Bedrock/Java NBT, LevelDB, SQLite, PDF, Office, ZIP, and Hex built in", "Edit NBT Bedrock/Java, LevelDB, SQLite, PDF, Office, ZIP, dan Hex bawaan"),
            ("Zed", "partial", "Images only", "Hanya gambar"),
            ("VS Code", "partial", "Images built in; other formats need extensions", "Gambar bawaan; format lain perlu ekstensi"),
        ],
    },
    {
        "en": ("10. Multi-language terminal & error jumps", "When code fails, click the error card to jump straight to the exact broken line."),
        "id": ("10. Terminal multibahasa & klik error", "Saat kode error, klik kartunya untuk langsung lompat ke baris yang bermasalah."),
        "rows": [
            ("FluxCode", "full", "Multi-tab shell with clickable file:line:col error cards across 15+ languages", "Terminal multi-tab dengan kartu error file:baris:kolom lintas 15+ bahasa"),
            ("Zed", "partial", "Standard integrated terminal", "Terminal bawaan standar"),
            ("VS Code", "partial", "Integrated terminal + task problem matchers", "Terminal bawaan + problem matcher per task"),
        ],
    },
    {
        "en": ("11. Git, project kits & WSL2", "Manage commits, incoming changes, project templates, and Windows-Linux files smoothly."),
        "id": ("11. Git, kit proyek & WSL2", "Kelola commit, cek pembaruan tim, template proyek, dan file Windows-Linux dengan cepat."),
        "rows": [
            ("FluxCode", "full", "Commit graph, incoming fetch, 11 built-in manifests, fast WSL2 disk helper", "Grafik commit, cek incoming, 11 manifest bawaan, helper disk cepat WSL2"),
            ("Zed", "partial", "Built-in Git, SSH remote", "Git bawaan, remote SSH"),
            ("VS Code", "partial", "Built-in Git; SSH, Containers, and WSL via extensions", "Git bawaan; SSH, Container, dan WSL lewat ekstensi"),
        ],
    },
    {
        "en": ("12. Privacy & zero telemetry", "Does not track your habits or send usage data back to any server."),
        "id": ("12. Privasi & nol telemetri", "Tidak melacak aktivitas atau mengirim data penggunaan ke server mana pun."),
        "rows": [
            ("FluxCode", "full", "Zero telemetry (only automatic network call is the update check)", "Nol telemetri (satu-satunya request otomatis hanya cek update)"),
            ("Zed", "partial", "Telemetry on by default (opt-out)", "Telemetri aktif secara bawaan (bisa dimatikan)"),
            ("VS Code", "partial", "Telemetry on by default (opt-out)", "Telemetri aktif secara bawaan (bisa dimatikan)"),
        ],
    },
    {
        "en": ("13. Honest trade-offs (not built yet)", "Still in active development; mature language servers and a debugger are on the roadmap."),
        "id": ("13. Catatan jujur (yang belum ada)", "Masih tahap pengembangan aktif; language server mendalam dan debugger sedang disiapkan."),
        "rows": [
            ("FluxCode", "partial", "Dev builds; LSP and debugger planned (Run supported)", "Build pengembangan; LSP dan debugger terjadwal (Run sudah ada)"),
            ("Zed", "full", "Stable release with built-in LSP and debugger", "Rilis stabil dengan LSP dan debugger bawaan"),
            ("VS Code", "full", "Stable release with debugger and extension LSPs", "Rilis stabil dengan debugger dan LSP via ekstensi"),
        ],
    },
]

COPY = {
    "en": {
        "title": "Why FluxCode vs Zed & VS Code",
        "sub": "Stacked vertically top to bottom: everyday benefit first, programmer detail next.",
        "part_a": "PART 1  /  SPEED & FOOTPRINT (SHORTER BAR WINS)",
        "part_b": "PART 2  /  BUILT-IN CAPABILITIES (OUT OF THE BOX)",
        "legend": "Filled accent square = built in  ·  Grey square = partial or via extension  ·  Empty square = not available",
        "foot": "Windows 11 Home (AMD Ryzen 5 7535HS, 15 GB RAM) · empty profiles · median of 5 runs · Nov 2026",
    },
    "id": {
        "title": "Keunggulan FluxCode vs Zed & VS Code",
        "sub": "Disusun vertikal ke bawah: manfaat mudah dipahami untuk awam, detail teknis untuk programmer.",
        "part_a": "BAGIAN 1  /  KECEPATAN & RINGAN (BATANG MAKIN PENDEK MAKIN BAIK)",
        "part_b": "BAGIAN 2  /  FITUR BAWAAN (LANGSUNG ADA TANPA RIBET)",
        "legend": "Kotak warna = bawaan lengkap  ·  Kotak abu = sebagian / perlu ekstensi  ·  Kotak kosong = belum tersedia",
        "foot": "Windows 11 Home (AMD Ryzen 5 7535HS, RAM 15 GB) · profil kosong · median 5 kali uji · Nov 2026",
    },
}


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def render(lang, theme):
    t = THEMES[theme]
    c = COPY[lang]
    w = 780
    pad = 16
    card_w = w - pad * 2
    out = []
    y = 28

    out.append(f'<text x="{pad}" y="{y}" font-size="18" font-weight="700" fill="{t["text"]}">{esc(c["title"])}</text>')
    y += 20
    out.append(f'<text x="{pad}" y="{y}" font-size="12.5" fill="{t["muted"]}">{esc(c["sub"])}</text>')
    y += 28

    # Part A heading
    out.append(f'<text x="{pad}" y="{y}" font-size="11" font-weight="700" letter-spacing="0.06em" fill="{t["accent"]}">{esc(c["part_a"])}</text>')
    y += 12

    label_x = pad + 14
    bar_x = pad + 96
    plot_w = 210
    row_h = 24
    bar_h = 13

    for item in NUMERIC:
        title, plain, unit_note = item[lang]
        card_h = 126
        out.append(f'<rect x="{pad}" y="{y}" width="{card_w}" height="{card_h}" rx="8" fill="{t["card"]}" stroke="{t["border"]}" stroke-width="1"/>')
        out.append(f'<text x="{label_x}" y="{y + 22}" font-size="13.5" font-weight="700" fill="{t["text"]}">{esc(title)}</text>')
        out.append(f'<text x="{w - pad - 14}" y="{y + 22}" text-anchor="end" font-size="11" fill="{t["faint"]}">{esc(unit_note)}</text>')
        out.append(f'<text x="{label_x}" y="{y + 40}" font-size="12" fill="{t["muted"]}">{esc(plain)}</text>')

        gy = y + 54
        for frac in (0.0, 0.5, 1.0):
            gx = bar_x + plot_w * frac
            out.append(f'<line x1="{gx:.1f}" y1="{gy - 3}" x2="{gx:.1f}" y2="{gy + row_h * 3 - 6}" stroke="{t["grid"]}" stroke-width="1"/>')

        for i, (name, val, text_en, text_id) in enumerate(item["rows"]):
            ry = gy + i * row_h
            me = name == "FluxCode"
            weight = "700" if me else "400"
            bw = max(3.0, plot_w * val / item["max"])
            color = t["accent"] if me else t["other"]
            txt = text_en if lang == "en" else text_id
            out.append(f'<text x="{label_x}" y="{ry + 11}" font-size="12" font-weight="{weight}" fill="{t["text"]}">{esc(name)}</text>')
            out.append(f'<rect x="{bar_x}" y="{ry}" width="{bw:.1f}" height="{bar_h}" rx="2" fill="{color}"/>')
            out.append(f'<text x="{bar_x + bw + 8:.1f}" y="{ry + 11}" font-size="11.5" font-weight="{weight}" fill="{t["text"] if me else t["muted"]}">{esc(txt)}</text>')

        y += card_h + 10

    y += 16
    # Part B heading
    out.append(f'<text x="{pad}" y="{y}" font-size="11" font-weight="700" letter-spacing="0.06em" fill="{t["accent"]}">{esc(c["part_b"])}</text>')
    y += 16
    out.append(f'<text x="{pad}" y="{y}" font-size="11" fill="{t["faint"]}">{esc(c["legend"])}</text>')
    y += 12

    for item in FEATURES:
        title, plain = item[lang]
        card_h = 122
        out.append(f'<rect x="{pad}" y="{y}" width="{card_w}" height="{card_h}" rx="8" fill="{t["card"]}" stroke="{t["border"]}" stroke-width="1"/>')
        out.append(f'<text x="{label_x}" y="{y + 22}" font-size="13.5" font-weight="700" fill="{t["text"]}">{esc(title)}</text>')
        out.append(f'<text x="{label_x}" y="{y + 40}" font-size="12" fill="{t["muted"]}">{esc(plain)}</text>')

        gy = y + 54
        for i, (name, level, text_en, text_id) in enumerate(item["rows"]):
            ry = gy + i * row_h
            me = name == "FluxCode"
            weight = "700" if me else "400"
            txt = text_en if lang == "en" else text_id
            out.append(f'<text x="{label_x}" y="{ry + 11}" font-size="12" font-weight="{weight}" fill="{t["text"]}">{esc(name)}</text>')
            mx = bar_x
            my = ry + 2
            if level == "full":
                fill = t["accent"] if me else t["other"]
                out.append(f'<rect x="{mx}" y="{my}" width="9" height="9" rx="2" fill="{fill}"/>')
            elif level == "partial":
                fill = t["accent"] if me else t["other"]
                out.append(f'<rect x="{mx}" y="{my}" width="9" height="9" rx="2" fill="none" stroke="{fill}" stroke-width="1.5"/>')
                out.append(f'<rect x="{mx + 2}" y="{my + 4.5}" width="5" height="2.5" rx="0.8" fill="{fill}"/>')
            else:
                out.append(f'<rect x="{mx}" y="{my}" width="9" height="9" rx="2" fill="none" stroke="{t["dim"]}" stroke-width="1.2"/>')
            out.append(f'<text x="{mx + 18}" y="{ry + 11}" font-size="11.5" font-weight="{weight}" fill="{t["text"] if me else t["muted"]}">{esc(txt)}</text>')

        y += card_h + 10

    y += 10
    out.append(f'<text x="{pad}" y="{y}" font-size="11" fill="{t["faint"]}">{esc(c["foot"])}</text>')
    h = y + 18

    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="{FONT}">\n'
        f'<title>{esc(c["title"])}</title>\n'
        + "\n".join(out)
        + "\n</svg>\n"
    )


def main():
    os.makedirs(OUT, exist_ok=True)
    for lang, prefix in (("en", "advantages"), ("id", "advantages-id")):
        for theme in THEMES:
            path = os.path.join(OUT, f"{prefix}-{theme}.svg")
            with open(path, "w", encoding="utf-8", newline="\n") as f:
                f.write(render(lang, theme))
            print("wrote", os.path.relpath(path, ROOT))


if __name__ == "__main__":
    main()
