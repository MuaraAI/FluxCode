"""FluxIcon: FluxCode's own file icons, drawn from scratch by this script.

Every icon is a 16x16 line drawing with one stroke weight, round ends and round
joins, so the set reads as one family. Languages get a monogram set in FluxIcon's
own line lettering (below); data, config, media and tooling get a pictogram.
Colour is not in the SVG: the editor tints each icon (one colour per icon,
listed here), so the same files work in every theme.

Run from the repository root:

    python -I tools/fluxicon/gen.py

It writes `assets/fluxicon/*.svg`, `src/fluxicon.rs` (the icon list, the name and
extension map, the embedded SVGs) and `tools/fluxicon/preview.html`.
"""

import math
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT_SVG = os.path.join(ROOT, "assets", "fluxicon")
OUT_RS = os.path.join(ROOT, "src", "fluxicon.rs")
OUT_HTML = os.path.join(ROOT, "tools", "fluxicon", "preview.html")

SW = 1.5  # the family's stroke weight


def f(v):
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


# --------------------------------------------------------------------- lettering
# FluxIcon's line lettering: each glyph is a list of strokes in a unit box
# (x 0..1 left to right, y 0..1 top to bottom). Curves are sampled ellipse arcs.

def arc(cx, cy, rx, ry, a0, a1, n=None):
    """Points on an ellipse from angle a0 to a1 (degrees, 0 = right, 90 = down)."""
    n = n or max(6, int(abs(a1 - a0) / 15))
    return [(cx + rx * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
             cy + ry * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]


def L(*pts):
    return [tuple(p) for p in pts]


GLYPHS = {
    "A": (1.0, [L((0, 1), (0.5, 0), (1, 1)), L((0.2, 0.64), (0.8, 0.64))]),
    "B": (0.9, [L((0, 1), (0, 0), (0.5, 0)) + arc(0.5, 0.25, 0.42, 0.25, -90, 90) + L((0, 0.5)),
                L((0.5, 0.5)) + arc(0.52, 0.75, 0.48, 0.25, -90, 90) + L((0, 1))]),
    "C": (0.95, [arc(0.55, 0.5, 0.55, 0.5, -45, -315)]),
    "D": (0.95, [L((0, 0), (0, 1), (0.42, 1)) + arc(0.42, 0.5, 0.58, 0.5, 90, -90) + L((0, 0))]),
    "E": (0.85, [L((1, 0), (0, 0), (0, 1), (1, 1)), L((0, 0.5), (0.8, 0.5))]),
    "F": (0.85, [L((1, 0), (0, 0), (0, 1)), L((0, 0.5), (0.8, 0.5))]),
    "G": (1.0, [arc(0.5, 0.5, 0.5, 0.5, -45, -360), L((1, 0.5), (0.55, 0.5))]),
    "H": (0.9, [L((0, 0), (0, 1)), L((1, 0), (1, 1)), L((0, 0.5), (1, 0.5))]),
    "I": (0.5, [L((0.5, 0), (0.5, 1)), L((0, 0), (1, 0)), L((0, 1), (1, 1))]),
    "J": (0.8, [L((1, 0), (1, 0.68)) + arc(0.5, 0.68, 0.5, 0.32, 0, 180)]),
    "K": (0.9, [L((0, 0), (0, 1)), L((1, 0), (0, 0.62)), L((0.36, 0.4), (1, 1))]),
    "L": (0.8, [L((0, 0), (0, 1), (1, 1))]),
    "M": (1.15, [L((0, 1), (0, 0), (0.5, 0.62), (1, 0), (1, 1))]),
    "N": (0.95, [L((0, 1), (0, 0), (1, 1), (1, 0))]),
    "O": (1.05, [arc(0.5, 0.5, 0.5, 0.5, 0, 360, 28)]),
    "P": (0.9, [L((0, 1), (0, 0), (0.5, 0)) + arc(0.5, 0.27, 0.5, 0.27, -90, 90) + L((0, 0.54))]),
    "Q": (1.05, [arc(0.5, 0.5, 0.5, 0.5, 0, 360, 28), L((0.62, 0.7), (1.02, 1.04))]),
    "R": (0.9, [L((0, 1), (0, 0), (0.5, 0)) + arc(0.5, 0.27, 0.5, 0.27, -90, 90) + L((0, 0.54)), L((0.45, 0.54), (1, 1))]),
    "S": (0.9, [arc(0.5, 0.25, 0.5, 0.25, -25, -270) + arc(0.5, 0.75, 0.5, 0.25, -90, 155)]),
    "T": (0.95, [L((0, 0), (1, 0)), L((0.5, 0), (0.5, 1))]),
    "U": (0.9, [L((0, 0), (0, 0.62)) + arc(0.5, 0.62, 0.5, 0.38, 180, 0) + L((1, 0))]),
    "V": (1.0, [L((0, 0), (0.5, 1), (1, 0))]),
    "W": (1.25, [L((0, 0), (0.24, 1), (0.5, 0.35), (0.76, 1), (1, 0))]),
    "X": (0.95, [L((0, 0), (1, 1)), L((1, 0), (0, 1))]),
    "Y": (0.95, [L((0, 0), (0.5, 0.5), (1, 0)), L((0.5, 0.5), (0.5, 1))]),
    "Z": (0.9, [L((0, 0), (1, 0), (0, 1), (1, 1))]),
    "+": (0.85, [L((0.5, 0.18), (0.5, 0.82)), L((0, 0.5), (1, 0.5))]),
    "#": (0.95, [L((0.38, 0.05), (0.22, 0.95)), L((0.8, 0.05), (0.64, 0.95)), L((0.04, 0.34), (1, 0.34)), L((0, 0.66), (0.96, 0.66))]),
    "0": (0.9, [arc(0.5, 0.5, 0.5, 0.5, 0, 360, 28)]),
    "1": (0.6, [L((0.05, 0.25), (0.65, 0), (0.65, 1))]),
}


def monogram(text):
    """A monogram: the letters, centred, in the family's stroke."""
    n = len(text)
    sw = {1: 1.7, 2: 1.55}.get(n, 1.3)
    height = {1: 9.0, 2: 8.0}.get(n, 7.5)
    gap = {1: 0, 2: 1.45}.get(n, 0.95)
    widths = [GLYPHS[ch][0] for ch in text]
    max_unit = {1: 6.6, 2: 5.0}.get(n, 3.55)
    room = 15.0 - sw - gap * (n - 1)
    unit = min(max_unit, room / sum(widths))
    total = unit * sum(widths) + gap * (n - 1)
    x = (16 - total) / 2
    y0 = (16 - height) / 2
    paths = []
    for ch, wf in zip(text, widths):
        w = unit * wf
        for stroke in GLYPHS[ch][1]:
            pts = [(x + px_ * w, y0 + py * height) for px_, py in stroke]
            d = "M" + " L".join(f"{f(a)} {f(b)}" for a, b in pts)
            paths.append(d)
        x += w + gap
    return paths, sw


# --------------------------------------------------------------------- pictograms
# Hand-drawn on the 16 grid. A dot is a zero-length stroke (round caps make it round).

def gear():
    pts = []
    teeth = 8
    for i in range(teeth * 4):
        a = math.radians(i * 360 / (teeth * 4) - 90 + 360 / (teeth * 8))
        r = 6.2 if (i % 4) in (1, 2) else 4.7
        pts.append((8 + r * math.cos(a), 8 + r * math.sin(a)))
    d = "M" + " L".join(f"{f(a)} {f(b)}" for a, b in pts) + "Z"
    return [d, circle(8, 8, 1.9)]


def circle(cx, cy, r):
    return f"M{f(cx - r)} {f(cy)} A{f(r)} {f(r)} 0 1 0 {f(cx + r)} {f(cy)} A{f(r)} {f(r)} 0 1 0 {f(cx - r)} {f(cy)}"


def dot(x, y):
    return f"M{f(x)} {f(y)} h0.01"


PAGE = "M3.75 1.75H9.5L12.75 5V14.25H3.75Z"
FOLD = "M9.5 1.75V5H12.75"

PICTOS = {
    "file": [PAGE, FOLD],
    "text": [PAGE, FOLD, "M6 8.25H10.5", "M6 11H10.5", "M6 5.5H7.5"],
    "code": [PAGE, FOLD, "M7 7.75L5.5 9.5L7 11.25", "M9.5 7.75L11 9.5L9.5 11.25"],
    "markup": ["M5 4.25L1.75 8L5 11.75", "M11 4.25L14.25 8L11 11.75", "M9.25 3.25L6.75 12.75"],
    "xml": ["M5 4.25L1.75 8L5 11.75", "M11 4.25L14.25 8L11 11.75", dot(8, 8)],
    "braces": ["M6 2.25C4.5 2.25 4.5 3.25 4.5 4.75V6.5C4.5 7.5 3.75 8 2.75 8C3.75 8 4.5 8.5 4.5 9.5V11.25C4.5 12.75 4.5 13.75 6 13.75",
               "M10 2.25C11.5 2.25 11.5 3.25 11.5 4.75V6.5C11.5 7.5 12.25 8 13.25 8C12.25 8 11.5 8.5 11.5 9.5V11.25C11.5 12.75 11.5 13.75 10 13.75",
               dot(8, 8)],
    "hash": ["M6.5 2.5L5.25 13.5", "M10.75 2.5L9.5 13.5", "M3 6H13.5", "M2.5 10H13"],
    "markdown": ["M1.75 11.5V4.5L4.75 8L7.75 4.5V11.5", "M11.75 4.5V11.25", "M9.5 9L11.75 11.25L14 9"],
    "image": ["M2 3.25H14V12.75H2Z", "M2 11.25L5.75 7.5L8.75 10.5L10.5 8.75L14 12.25", circle(10.75, 5.9, 1.05)],
    "vector": ["M2.75 12.75C4.75 3.75 11.25 12.25 13.25 3.25", circle(2.75, 12.75, 1.2), circle(13.25, 3.25, 1.2), "M5.5 3.5H9.5", dot(4, 3.5)],
    "video": ["M2 3.5H14V12.5H2Z", "M6.75 6V10L10.25 8Z"],
    "audio": [circle(4.75, 11.75, 1.85), circle(11.25, 10.25, 1.85), "M6.6 11.75V3.5L13.1 2V10.25"],
    "font": ["M1.5 12.75L4.75 3.25L8 12.75", "M2.6 9.5H6.9", circle(11.4, 10.4, 2.2), "M13.6 7.6V12.75"],
    "archive": ["M3 1.75H13V14.25H3Z", "M8 1.75V3", "M8 4.25V5.5", "M8 6.75V8", "M6.75 9.25H9.25V12H6.75Z"],
    "table": ["M2 3H14V13H2Z", "M2 6.5H14", "M2 9.75H14", "M6.5 3V13"],
    "slides": ["M2 2.75H14V10.25H2Z", "M8 10.25V13.25", "M5.25 13.25H10.75", "M5 7.75L7 5.75L8.75 7.5L11 5"],
    "notebook": ["M4.25 1.75H13.25V14.25H4.25Z", "M2.5 4.5H6", "M2.5 8H6", "M2.5 11.5H6", "M8.25 5.5H11", "M8.25 8.5H11"],
    "log": [dot(2.75, 4), dot(2.75, 8), dot(2.75, 12), "M5.5 4H13.5", "M5.5 8H11.5", "M5.5 12H12.5"],
    "database": ["M2.5 4A5.5 2 0 1 0 13.5 4A5.5 2 0 1 0 2.5 4", "M2.5 4V12A5.5 2 0 0 0 13.5 12V4", "M2.5 8A5.5 2 0 0 0 13.5 8"],
    "lock": ["M5 7V5A3 3 0 0 1 11 5V7", "M3.5 7H12.5V14H3.5Z", "M8 9.75V11.25"],
    "key": [circle(5, 10.75, 2.75), "M7 8.75L13.25 2.5", "M11.25 4.5L13 6.25", "M9.6 6.15L10.85 7.4"],
    "badge": [circle(8, 6.25, 4.25), "M5.6 9.75L4.75 14.25L8 12.6L11.25 14.25L10.4 9.75", dot(8, 6.25)],
    "shield": ["M8 1.75L13 3.75V7.75C13 11 10.75 13.25 8 14.25C5.25 13.25 3 11 3 7.75V3.75Z", "M5.75 8L7.25 9.5L10.25 6.5"],
    "prompt": ["M2.75 4.25L6.75 8L2.75 11.75", "M8.5 12H13.25"],
    "git": [circle(4.5, 3.5, 1.6), circle(4.5, 12.5, 1.6), circle(11.5, 5.25, 1.6), "M4.5 5.1V10.9", "M11.5 6.85C11.5 9.5 4.5 8.25 4.5 10.9"],
    "containers": ["M1.75 9H14.25V13.25H1.75Z", "M5.9 9V13.25", "M10.1 9V13.25", "M5.9 4.75H10.1V9"],
    "package": ["M8 1.75L14 5V11L8 14.25L2 11V5Z", "M2 5L8 8.25L14 5", "M8 8.25V14.25"],
    "gear": gear(),
    "sliders": ["M2.25 4.5H8.4", "M11.6 4.5H13.75", circle(10, 4.5, 1.6),
                "M2.25 8H4.4", "M7.6 8H13.75", circle(6, 8, 1.6),
                "M2.25 11.5H9.4", "M12.6 11.5H13.75", circle(11, 11.5, 1.6)],
    "tools": ["M2.75 2.75H10.25V6.25H2.75Z", "M6.5 6.25V14", "M10.25 4.5H13.25"],
    "flask": ["M5.75 1.75H10.25", "M6.75 1.75V6L2.9 12.6A1.1 1.1 0 0 0 3.85 14.25H12.15A1.1 1.1 0 0 0 13.1 12.6L9.25 6V1.75", "M4.7 9.75H11.3"],
    "info": [circle(8, 8, 6.1), "M8 7.25V11.25", dot(8, 4.85)],
    "history": ["M2.75 8A5.25 5.25 0 1 0 4.3 4.3", "M2.25 1.75V4.75H5.25", "M8 5V8L10 9.25"],
    "chip": ["M4.25 4.25H11.75V11.75H4.25Z", "M6.5 2V4.25", "M9.5 2V4.25", "M6.5 11.75V14", "M9.5 11.75V14",
             "M2 6.5H4.25", "M2 9.5H4.25", "M11.75 6.5H14", "M11.75 9.5H14"],
    "cube": ["M2.5 5.25H10.5V13.25H2.5Z", "M5.5 2.75H13.5V10.75H10.5", "M2.5 5.25L5.5 2.75", "M10.5 5.25L13.5 2.75", "M5.5 2.75V5.25"],
    "diff": ["M8 2.25V8.25", "M5 5.25H11", "M5 12.25H11"],
    "lambda": ["M3.5 2.25H5.5L12.75 13.75", "M8.4 7.4L3.25 13.75"],
    "flux": ["M1.75 6C3.75 3.75 5.75 3.75 8 6S12.25 8.25 14.25 6", "M1.75 10.5C3.75 8.25 5.75 8.25 8 10.5S12.25 12.75 14.25 10.5"],
    "book": ["M8 3.5C6.5 2.5 4.25 2.25 1.75 2.75V13C4.25 12.5 6.5 12.75 8 13.75C9.5 12.75 11.75 12.5 14.25 13V2.75C11.75 2.25 9.5 2.5 8 3.5Z", "M8 3.5V13.75"],
    "folder": ["M1.75 4A1 1 0 0 1 2.75 3H6.25L7.75 4.75H13.25A1 1 0 0 1 14.25 5.75V12.25A1 1 0 0 1 13.25 13.25H2.75A1 1 0 0 1 1.75 12.25Z"],
    "folder-open": ["M1.75 12.5V4A1 1 0 0 1 2.75 3H6.25L7.75 4.75H12.25A1 1 0 0 1 13.25 5.75V7",
                    "M1.75 12.5L3.7 7.6A1 1 0 0 1 4.65 7H14A.75 .75 0 0 1 14.7 8L12.95 12.6A1 1 0 0 1 12 13.25H2.5A.75 .75 0 0 1 1.75 12.5Z"],
}

# --------------------------------------------------------------------- the set
# (id, drawing, tint, extensions, file names). A drawing is a pictogram name or
# "mono:XY". File names are matched whole and lower-cased; a name ending in "*"
# matches by prefix ("readme*"), one starting with "*" by suffix ("*.test.ts").

ICONS = [
    # --- languages: general purpose
    ("python", "mono:PY", 0x4F9FD8, "py pyw pyi pyx pxd pyz rpy", "setup.py"),
    ("javascript", "mono:JS", 0xE8C547, "js mjs cjs es6", ""),
    ("typescript", "mono:TS", 0x3B8EEA, "ts mts cts", ""),
    ("ts-decl", "mono:TS", 0x7FA6C9, "", "*.d.ts *.d.mts *.d.cts"),
    ("jsx", "mono:JSX", 0x61DAFB, "jsx", ""),
    ("tsx", "mono:TSX", 0x3B8EEA, "tsx", ""),
    ("rust", "mono:RS", 0xDEA584, "rs", ""),
    ("go", "mono:GO", 0x4FD1C5, "go", ""),
    ("c", "mono:C", 0x6C9BD2, "c i", ""),
    ("c-header", "mono:H", 0xA48ADB, "h", ""),
    ("cpp", "mono:C++", 0x5C8DD6, "cpp cc cxx c++ ixx cppm tpp ipp inl", ""),
    ("cpp-header", "mono:H", 0x5C8DD6, "hpp hh hxx h++", ""),
    ("csharp", "mono:C#", 0x9B6CD8, "cs csx", ""),
    ("fsharp", "mono:F#", 0x3A9BD5, "fs fsi fsx fsscript", ""),
    ("vb", "mono:VB", 0x6F86D9, "vb vbs vba bas", ""),
    ("java", "mono:J", 0xE76F51, "java jav", ""),
    ("class", "chip", 0xE76F51, "class", ""),
    ("kotlin", "mono:KT", 0xA97BFF, "kt kts", ""),
    ("scala", "mono:SC", 0xDE3423, "scala sc sbt", ""),
    ("groovy", "mono:GR", 0x4298B8, "groovy gvy gy gsh", "jenkinsfile"),
    ("clojure", "mono:CLJ", 0x63B132, "clj cljs cljc edn", ""),
    ("swift", "mono:SW", 0xF05138, "swift", ""),
    ("objc", "mono:OC", 0x6C9BD2, "m mm", ""),
    ("dart", "mono:DT", 0x2CB7F6, "dart", ""),
    ("ruby", "mono:RB", 0xE0464B, "rb rbw rake gemspec ru erb", "rakefile guardfile vagrantfile"),
    ("php", "mono:PHP", 0x8C8FD8, "php phtml php3 php4 php5 phps phpt", ""),
    ("perl", "mono:PL", 0x6C8ECF, "pl pm pod t psgi", ""),
    ("raku", "mono:RK", 0xC7B4E8, "raku rakumod p6 pm6", ""),
    ("lua", "mono:LU", 0x5A7FE8, "lua luau rockspec", ""),
    ("r", "mono:R", 0x3F7FCF, "r rmd rdata rds", ""),
    ("julia", "mono:JL", 0xA270BA, "jl", ""),
    ("haskell", "mono:HS", 0x8F6CC4, "hs lhs hsc", ""),
    ("ocaml", "mono:ML", 0xEE7A00, "ml mli mll mly", ""),
    ("reason", "mono:RE", 0xDD4B39, "re rei res resi", ""),
    ("purescript", "mono:PU", 0x9CA3AF, "purs", ""),
    ("elm", "mono:ELM", 0x60B5CC, "elm", ""),
    ("erlang", "mono:ER", 0xB83998, "erl hrl", ""),
    ("elixir", "mono:EX", 0x9B6CC9, "ex exs eex heex leex", ""),
    ("gleam", "mono:GL", 0xFFAFF3, "gleam", ""),
    ("zig", "mono:ZG", 0xF7A41D, "zig zon", ""),
    ("nim", "mono:NI", 0xEBC64F, "nim nims nimble", ""),
    ("crystal", "mono:CR", 0xC9CED6, "cr", ""),
    ("d", "mono:D", 0xC74B3F, "d di", ""),
    ("v", "mono:V", 0x5D87BF, "v vsh", ""),
    ("odin", "mono:OD", 0x3882D2, "odin", ""),
    ("haxe", "mono:HX", 0xEA8220, "hx hxml", ""),
    ("mojo", "mono:MO", 0xFF6A33, "mojo", ""),
    ("fortran", "mono:F", 0x7C6FD1, "f f77 f90 f95 f03 f08 for ftn", ""),
    ("cobol", "mono:CB", 0x5E81AC, "cob cbl cpy", ""),
    ("pascal", "mono:PA", 0xD9A74A, "pas pp dpr dpk lpr", ""),
    ("ada", "mono:AD", 0x5A9BD8, "adb ads ada gpr", ""),
    ("lisp", "lambda", 0xC792EA, "lisp lsp cl fasl el scm ss sld rkt rktl", ""),
    ("prolog", "mono:PRO", 0xE8704A, "pro", ""),
    ("asm", "mono:ASM", 0x9AA5B1, "asm s nasm masm a51 inc", ""),
    ("wasm", "mono:WA", 0x7A6BF0, "wasm wat wast", ""),
    ("solidity", "mono:SOL", 0x8A9AB8, "sol", ""),
    ("cuda", "mono:CU", 0x76B900, "cu cuh", ""),
    ("shader", "mono:SL", 0xE58BB8, "glsl vert frag geom comp tesc tese hlsl wgsl shader cginc fx metal", ""),
    ("verilog", "mono:HDL", 0x6CB38A, "sv svh vh vhd vhdl", ""),
    ("arduino", "mono:INO", 0x00A3A8, "ino pde", ""),
    ("gdscript", "mono:GD", 0x5A9BD8, "gd gdshader tscn tres godot", ""),
    ("coffee", "mono:CF", 0xC8A97E, "coffee cson iced", ""),
    ("apex", "mono:AX", 0x4AA8E0, "apex trigger", ""),
    ("qsharp", "mono:Q#", 0x9B6CD8, "qs", ""),
    ("wolfram", "mono:WL", 0xDD4444, "wl wls nb", ""),
    ("matlab", "mono:MAT", 0xE8853A, "mat mlx", ""),
    ("tcl", "mono:TCL", 0x8FB8DE, "tcl tk", ""),
    ("vim", "mono:VI", 0x43B05C, "vim vimrc", ".vimrc _vimrc .gvimrc"),
    ("applescript", "mono:AS", 0xA8B1BC, "applescript scpt", ""),
    ("autohotkey", "mono:AHK", 0x6DB33F, "ahk", ""),
    # --- shells
    ("shell", "prompt", 0x5FD38D, "sh bash zsh fish ksh csh tcsh ash dash command awk", ".bashrc .zshrc .profile .bash_profile .zprofile .bash_aliases"),
    ("powershell", "mono:PS", 0x4C8FD6, "ps1 psm1 psd1 ps1xml pssc", ""),
    ("batch", "prompt", 0xA8B1BC, "bat cmd btm", ""),
    # --- web
    ("html", "markup", 0xE8704A, "html htm xhtml shtml", ""),
    ("css", "hash", 0x5AA7E8, "css pcss postcss", ""),
    ("scss", "mono:SC", 0xDB6A9F, "scss sass", ""),
    ("less", "mono:LE", 0x5B85C7, "less", ""),
    ("stylus", "mono:ST", 0xB6D96C, "styl stylus", ""),
    ("vue", "mono:VU", 0x41B883, "vue", ""),
    ("svelte", "mono:SV", 0xFF5A1F, "svelte", ""),
    ("astro", "mono:AST", 0xFF7A3D, "astro", ""),
    ("handlebars", "mono:HB", 0xF4A261, "hbs handlebars mustache", ""),
    ("pug", "mono:PG", 0xC9A27E, "pug jade", ""),
    ("haml", "mono:HM", 0xECD9A0, "haml slim", ""),
    ("ejs", "mono:EJ", 0xB4CA65, "ejs", ""),
    ("liquid", "mono:LQ", 0x76C7C0, "liquid", ""),
    ("twig", "mono:TW", 0x9ACD5E, "twig", ""),
    ("razor", "mono:RZ", 0x9B6CD8, "cshtml razor vbhtml", ""),
    ("jinja", "mono:JJ", 0xC9A27E, "j2 jinja jinja2 njk nunjucks", ""),
    ("graphql", "mono:GQ", 0xE535AB, "graphql gql graphqls", ""),
    # --- documents and markup
    ("markdown", "markdown", 0x8FB8FF, "md markdown mdown mkd mdx", ""),
    ("rst", "mono:RST", 0x8FB8FF, "rst rest", ""),
    ("asciidoc", "mono:ADC", 0xE76F51, "adoc asciidoc", ""),
    ("latex", "mono:TEX", 0x67A6A2, "tex ltx sty cls dtx ins bib bst", ""),
    ("typst", "mono:TY", 0x3AC7D4, "typ", ""),
    ("org", "mono:ORG", 0x77AA99, "org", ""),
    ("text", "text", 0x9AA5B1, "txt text rtf nfo me 1st", "robots.txt"),
    ("flux", "flux", 0xC792EA, "flux flx", ""),
    ("pdf", "mono:PDF", 0xE5534B, "pdf", ""),
    ("doc", "text", 0x4F8FE0, "doc docx odt dot dotx pages wpd", ""),
    ("sheet", "table", 0x3FB27F, "xls xlsx xlsm ods numbers", ""),
    ("slides", "slides", 0xE8704A, "ppt pptx odp", ""),
    ("ebook", "book", 0xD9A74A, "epub mobi azw azw3 fb2", ""),
    ("notebook", "notebook", 0xF2884B, "ipynb", ""),
    ("diff", "diff", 0x5FD38D, "diff patch rej", ""),
    ("log", "log", 0x9AA5B1, "log out", ""),
    # --- data and config
    ("json", "braces", 0xD9A74A, "json jsonc json5 jsonl ndjson geojson topojson har webmanifest map babelrc eslintrc prettierrc stylelintrc", ""),
    ("yaml", "sliders", 0xE0646E, "yaml yml", ""),
    ("toml", "sliders", 0xC88B5A, "toml", ""),
    ("ini", "sliders", 0x9AA5B1, "ini cfg conf config properties prefs reg inf desktop service", ".htaccess .npmrc .nvmrc .yarnrc .yarnrc.yml .editorconfig .gitconfig nginx.conf"),
    ("xml", "xml", 0xE8A04A, "xml xsd xsl xslt xaml plist resx svgz rss atom wsdl xib storyboard manifest", ""),
    ("csv", "table", 0x5FD38D, "csv tsv psv tab", ""),
    ("columnar", "table", 0x5AA7E8, "parquet avro orc arrow feather", ""),
    ("proto", "mono:PB", 0x6C9BD2, "proto", ""),
    ("terraform", "mono:TF", 0x8C6CE8, "tf tfvars tfstate hcl nomad", ""),
    ("nix", "mono:NX", 0x7EB2DD, "nix", ""),
    ("bicep", "mono:BI", 0x4AA8E0, "bicep", ""),
    ("env", "key", 0xE8C547, "env", ".env .env.* *.env"),
    # --- databases
    ("sql", "database", 0xE8A04A, "sql psql pgsql plsql mysql tsql ddl dml dbml cql cypher hql prql", ""),
    ("sqlite", "database", 0x4F9FD8, "sqlite sqlite3 db db3 s3db sl3 duckdb", ""),
    ("db-file", "database", 0xA48ADB, "mdb accdb frm ibd myd dbf realm rdb ldb kdbx dump", ""),
    ("prisma", "database", 0x8C98E8, "prisma", ""),
    ("mongo", "database", 0x5FB56B, "bson", ""),
    # --- media
    ("image", "image", 0xC792EA, "png jpg jpeg jfif gif webp bmp ico icns avif tif tiff heic heif tga dds raw cr2 nef psd xcf kra", ""),
    ("svg", "vector", 0xF2B441, "svg ai eps", ""),
    ("video", "video", 0xE5534B, "mp4 m4v mov webm mkv avi wmv flv mpg mpeg 3gp ogv", ""),
    ("audio", "audio", 0x4FD1C5, "mp3 wav ogg oga flac m4a aac opus wma aiff mid midi", ""),
    ("font", "font", 0x9AA5B1, "ttf otf woff woff2 eot fon fnt pfb", ""),
    ("model3d", "cube", 0x6CB3D8, "fbx gltf glb stl blend dae 3ds ply usd usda usdz abc", ""),
    # --- binaries, archives, security
    ("binary", "chip", 0x9AA5B1, "exe dll so dylib o obj a lib bin dat pdb elf ko sys msi", ""),
    ("archive", "archive", 0xC88B5A, "zip 7z rar gz tgz tar bz2 xz zst lz lzma cab iso dmg img whl jar war ear apk aab ipa deb rpm nupkg crate gem vsix snap appimage", ""),
    ("lock", "lock", 0x9AA5B1, "lock lockb", "package-lock.json npm-shrinkwrap.json yarn.lock pnpm-lock.yaml bun.lockb cargo.lock poetry.lock pipfile.lock composer.lock gemfile.lock go.sum flake.lock mix.lock pubspec.lock podfile.lock packages.lock.json uv.lock"),
    ("cert", "shield", 0x5FD38D, "pem crt cer der csr p7b p7c", ""),
    ("key", "key", 0xD9A74A, "key pub p12 pfx jks keystore gpg asc sig", "id_rsa id_ed25519 id_ecdsa id_dsa authorized_keys known_hosts"),
    # --- tooling, by file name
    ("git", "git", 0xF05033, "", ".gitignore .gitattributes .gitmodules .gitkeep .git-blame-ignore-revs .mailmap"),
    ("docker", "containers", 0x2496ED, "dockerfile", "dockerfile dockerfile.* containerfile .dockerignore docker-compose.yml docker-compose.yaml docker-compose.*.yml docker-compose.*.yaml compose.yml compose.yaml"),
    ("npm", "package", 0xE0464B, "", "package.json .npmignore"),
    ("cargo", "package", 0xDEA584, "", "cargo.toml"),
    ("pypackage", "package", 0x4F9FD8, "", "pyproject.toml pipfile requirements.txt requirements-*.txt requirements_*.txt setup.cfg environment.yml"),
    ("gomod", "package", 0x4FD1C5, "", "go.mod go.work"),
    ("gem", "package", 0xE0464B, "", "gemfile"),
    ("composer", "package", 0x8C8FD8, "", "composer.json"),
    ("maven", "package", 0xE76F51, "", "pom.xml"),
    ("gradle", "package", 0x5FB56B, "gradle", "build.gradle build.gradle.kts settings.gradle settings.gradle.kts gradle.properties gradlew gradlew.bat"),
    ("dotnet", "package", 0x9B6CD8, "csproj fsproj vbproj vcxproj sln props targets nuspec", ""),
    ("pubspec", "package", 0x2CB7F6, "", "pubspec.yaml"),
    ("swiftpkg", "package", 0xF05138, "", "package.swift podfile"),
    ("mix", "package", 0x9B6CC9, "", "mix.exs"),
    ("cabal", "package", 0x8F6CC4, "cabal", "stack.yaml"),
    ("build", "tools", 0x9AA5B1, "mk mak cmake ninja bazel bzl", "makefile gnumakefile cmakelists.txt justfile meson.build build.ninja workspace build.bazel taskfile.yml procfile"),
    ("tsconfig", "gear", 0x3B8EEA, "", "tsconfig.json tsconfig.*.json jsconfig.json"),
    ("tool-config", "gear", 0x9AA5B1, "", "*.config.js *.config.mjs *.config.cjs *.config.ts *.config.mts .prettierrc .prettierrc.* .eslintrc .eslintrc.* .stylelintrc .babelrc .browserslistrc vercel.json netlify.toml turbo.json nx.json lerna.json .gitlab-ci.yml .travis.yml appveyor.yml azure-pipelines.yml renovate.json"),
    ("readme", "info", 0x8FB8FF, "", "readme readme.* read.me"),
    ("license", "badge", 0xD9A74A, "", "license license.* licence licence.* copying copying.* unlicense"),
    ("changelog", "history", 0x8FB8FF, "", "changelog changelog.* history history.* changes changes.* releases.md"),
    ("contributing", "book", 0x8FB8FF, "", "contributing contributing.* code_of_conduct.md security.md authors authors.* codeowners"),
    # --- tests, by name pattern (tinted like their language)
    ("test-js", "flask", 0xE8C547, "", "*.test.js *.spec.js *.test.mjs *.spec.mjs *.test.cjs *.spec.cjs *.test.jsx *.spec.jsx"),
    ("test-ts", "flask", 0x3B8EEA, "", "*.test.ts *.spec.ts *.test.tsx *.spec.tsx *.test.mts *.spec.mts"),
    ("test-py", "flask", 0x4F9FD8, "", "test_*.py *_test.py conftest.py"),
    ("test-go", "flask", 0x4FD1C5, "", "*_test.go"),
    ("test-rb", "flask", 0xE0464B, "", "*_spec.rb *_test.rb"),
    ("test-rs", "flask", 0xDEA584, "", "*_test.rs"),
    # --- fallbacks (no extensions: chosen by the editor)
    ("file", "file", None, "", ""),
    ("code", "code", None, "", ""),
    ("folder", "folder", None, "", ""),
    ("folder-open", "folder-open", None, "", ""),
]


def svg_for(drawing):
    if drawing.startswith("mono:"):
        paths, sw = monogram(drawing[5:])
    else:
        paths, sw = PICTOS[drawing], SW
    body = "".join(f'<path d="{d}"/>' for d in paths)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" '
            f'stroke="#000" stroke-width="{f(sw)}" stroke-linecap="round" stroke-linejoin="round">{body}</svg>\n')


def main():
    os.makedirs(OUT_SVG, exist_ok=True)
    seen_ext, seen_name = {}, {}
    for old in os.listdir(OUT_SVG):
        if old.endswith(".svg"):
            os.remove(os.path.join(OUT_SVG, old))
    for ident, drawing, tint, exts, names in ICONS:
        with open(os.path.join(OUT_SVG, ident + ".svg"), "w", encoding="utf-8", newline="\n") as fh:
            fh.write(svg_for(drawing))
        for e in exts.split():
            if e in seen_ext:
                sys.exit(f"extension .{e} is in both {seen_ext[e]} and {ident}")
            seen_ext[e] = ident
        for n in names.split():
            if n in seen_name:
                sys.exit(f"name {n} is in both {seen_name[n]} and {ident}")
            seen_name[n] = ident
    write_rust()
    langs = sum(1 for i in ICONS if i[3] or i[4])
    print(f"{len(ICONS)} icons, {len(seen_ext)} extensions, {len(seen_name)} file names ({langs} mapped kinds)")


def const(ident):
    return ident.upper().replace("-", "_")


def write_rust():
    out = []
    w = out.append
    w("//! FluxIcon: FluxCode's own file icons. GENERATED by `tools/fluxicon/gen.py`;")
    w("//! edit the script and run `python -I tools/fluxicon/gen.py`, not this file.")
    w("//!")
    w("//! Each icon is a 16x16 line drawing (one stroke weight, round ends) that the")
    w("//! editor tints: languages are monograms in FluxIcon's own line lettering, data,")
    w("//! config, media and tools are pictograms. `for_name` picks the icon and its tint")
    w("//! for a file name: whole names and name patterns first, then the extension.")
    w("")
    w("use gpui_kit::SharedString;")
    w("")
    w("/// One FluxIcon: the asset path of its SVG.")
    w("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    w("pub struct FluxIcon(pub &'static str);")
    w("")
    w("impl gpui_kit::assets::IconNamed for FluxIcon {")
    w("    fn path(self) -> SharedString {")
    w("        self.0.into()")
    w("    }")
    w("}")
    w("")
    for ident, drawing, tint, exts, names in ICONS:
        w(f'pub const {const(ident)}: FluxIcon = FluxIcon("fluxicon/{ident}.svg");')
    w("")
    w("/// The SVGs, embedded (about 0.3 KiB each).")
    w("pub static SVGS: &[(&str, &[u8])] = &[")
    for ident, *_ in ICONS:
        w(f'    ("fluxicon/{ident}.svg", include_bytes!("../assets/fluxicon/{ident}.svg")),')
    w("];")
    w("")
    w("/// The SVG at `path`, if it is a FluxIcon.")
    w("pub fn load(path: &str) -> Option<&'static [u8]> {")
    w('    if !path.starts_with("fluxicon/") {')
    w("        return None;")
    w("    }")
    w("    SVGS.iter().find(|(p, _)| *p == path).map(|(_, b)| *b)")
    w("}")
    w("")
    w("/// Every FluxIcon path, for the asset source's listing.")
    w("pub fn paths() -> impl Iterator<Item = &'static str> {")
    w("    SVGS.iter().map(|(p, _)| *p)")
    w("}")
    w("")

    def tint_rs(t):
        return "None" if t is None else f"Some(0x{t:06X})"

    exact, prefix, suffix = [], [], []
    for ident, drawing, tint, exts, names in ICONS:
        for n in names.split():
            if n.startswith("*"):
                suffix.append((n[1:], ident, tint))
            elif n.endswith("*"):
                prefix.append((n[:-1], ident, tint))
            elif "*" in n:
                a, b = n.split("*", 1)
                prefix.append((a, ident, tint, b))
            else:
                exact.append((n, ident, tint))
    w("/// The icon for a whole lower-cased file name or a name pattern.")
    w("fn by_name(name: &str) -> Option<(FluxIcon, Option<u32>)> {")
    w("    let hit = match name {")
    for n, ident, tint in exact:
        w(f'        "{n}" => ({const(ident)}, {tint_rs(tint)}),')
    w("        _ => {")
    # longest suffix first, so "*.d.ts" wins over a shorter one
    for s, ident, tint in sorted(suffix, key=lambda x: -len(x[0])):
        w(f'            if name.ends_with("{s}") {{ return Some(({const(ident)}, {tint_rs(tint)})); }}')
    for p in sorted(prefix, key=lambda x: -len(x[0])):
        if len(p) == 4:
            a, ident, tint, b = p
            w(f'            if name.starts_with("{a}") && name.ends_with("{b}") && name.len() >= {len(a) + len(b)} {{ return Some(({const(ident)}, {tint_rs(tint)})); }}')
        else:
            a, ident, tint = p
            w(f'            if name.starts_with("{a}") {{ return Some(({const(ident)}, {tint_rs(tint)})); }}')
    w("            return None;")
    w("        }")
    w("    };")
    w("    Some(hit)")
    w("}")
    w("")
    w("/// The icon for a lower-cased extension (no dot).")
    w("fn by_ext(ext: &str) -> Option<(FluxIcon, Option<u32>)> {")
    w("    Some(match ext {")
    for ident, drawing, tint, exts, names in ICONS:
        es = exts.split()
        if es:
            pats = " | ".join(f'"{e}"' for e in es)
            w(f"        {pats} => ({const(ident)}, {tint_rs(tint)}),")
    w("        _ => return None,")
    w("    })")
    w("}")
    w("")
    w("/// The icon and tint for a file name, or None when nothing in the set knows it")
    w("/// (the caller falls back to the generic code or file icon).")
    w("pub fn for_name(name: &str) -> Option<(FluxIcon, Option<u32>)> {")
    w("    let lower = name.to_ascii_lowercase();")
    w("    if let Some(hit) = by_name(&lower) {")
    w("        return Some(hit);")
    w("    }")
    w("    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or(\"\");")
    w("    by_ext(ext)")
    w("}")
    w("")
    w("/// How many file kinds the set tells apart.")
    w("#[allow(dead_code)]")
    w(f"pub const KINDS: usize = {len(ICONS)};")
    with open(OUT_RS, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(out) + "\n")


def write_preview():
    cells = []
    for ident, drawing, tint, exts, names in ICONS:
        color = "#c9d1d9" if tint is None else f"#{tint:06x}"
        svg = svg_for(drawing).replace('stroke="#000"', f'stroke="{color}"')
        big = svg.replace('width="16" height="16"', 'width="48" height="48"')
        label = (exts.split() or names.split() or [""])[0]
        mid = svg.replace('width="16" height="16"', 'width="20" height="20"')
        cells.append(f'<div class="c"><div class="i">{big}</div><div class="s">{svg}{mid}</div><b>{ident}</b><span>{label}</span></div>')
    html = ("<!doctype html><meta charset=utf-8><title>FluxIcon</title><style>"
            "body{background:#16181d;color:#c9d1d9;font:12px system-ui;margin:24px}"
            ".g{display:grid;grid-template-columns:repeat(auto-fill,minmax(110px,1fr));gap:10px}"
            ".c{background:#1f232a;border-radius:8px;padding:10px;display:flex;flex-direction:column;align-items:center;gap:4px}"
            ".s{display:flex;gap:8px;align-items:center}b{font-weight:600}span{color:#7d8590}"
            "</style><h1>FluxIcon</h1><div class=g>" + "".join(cells) + "</div>")
    with open(OUT_HTML, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(html)


if __name__ == "__main__":
    main()
