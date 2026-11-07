# FluxCode: Tauri (WebView2) vs GPUI (native)

Status: **prototype**. The GPUI edition lives in [`gpui-app/`](../gpui-app/) next to the
Tauri app; nothing in `src/` or `src-tauri/` was changed. Measured 2026-11-07.

## Setup

- Machine: Windows 11 Home 10.0.26100, AMD Radeon integrated graphics (DirectX 11), same session.
- Tauri: `fluxcode.exe` 0.5.2-dev release build (`npx tauri build --no-bundle`), current WebView2
  switches (GPU off, `--in-process-gpu`, browser services off).
- GPUI: `fluxcode-gpui.exe` release build (`opt-level="s"`, fat LTO, same profile as Tauri),
  gpui-kit 0.9.0 (gpui-pre 0.3.11 + gpui-component), toolchain `x86_64-pc-windows-gnu`.
- Script: [`scripts/bench-gpui-vs-tauri.ps1`](../scripts/bench-gpui-vs-tauri.ps1), 5 runs per app
  (median) after one warm-up run, opening this repository as the workspace. Tauri runs use a
  throw-away WebView2 profile, so its numbers are for a clean session.
- Memory is summed over the whole process tree (Tauri: `fluxcode.exe` + every `msedgewebview2.exe`).
  *Private* = committed private bytes (includes GPU-driver allocations that are not resident).
  *Working set* = resident pages; for Tauri the sum double-counts pages shared between the
  WebView2 processes, so it overstates Tauri a bit.
- "Large file" = a generated 60k-line / 3.5 MB JavaScript file.
- CPU = CPU time of the process tree divided by wall time (100 % = one core), while typing
  200 characters (40 ms apart), while scrolling 200 wheel notches, and 5 s idle afterwards.

## Results

| Metric | Tauri (WebView2) | GPUI | Difference |
|---|---|---|---|
| Executable size | 3.9 MB (+ system WebView2 runtime) | 26.0 MB (self-contained) | GPUI 6.7x bigger file |
| Start to window shown (median) | 430 ms | 213 ms | GPUI ~2x faster |
| Start to painted UI (median) | 506 ms | 273 ms | GPUI ~1.9x faster |
| Processes | 6 | 1 | |
| Idle private memory | 113 MB | 101 MB | GPUI -11 % |
| Idle working set | 337 MB | 59 MB | GPUI -82 % |
| Large file, private | 144 MB | 114 MB (large-file mode) / 283 MB (highlighting on) | see notes |
| Large file, working set | 387 MB | 78 MB (large-file mode) / 242 MB (highlighting on) | |
| CPU while typing (large file) | 77 % | 10 % (large-file mode) / 46 % (highlighting on) | GPUI 1.7-7x less |
| CPU while scrolling (large file) | 61 % | 8 % (large-file mode) / 18 % (highlighting on) | GPUI 3-7x less |
| CPU idle after input | 9.4 % | 2.8 % | |

Raw start-to-paint runs (ms): Tauri 516, 506, 492, 521, 503; GPUI 257, 274, 273, 257, 275.
The "highlighting on" column is from the first benchmark run, before the 2 MB large-file limit
was added (same build otherwise); all other numbers are from the final run.

## What the numbers say

- **Start-up**: GPUI shows a painted UI in about half the time, since there is no browser engine to boot.
- **CPU**: typing and especially scrolling cost much less CPU in GPUI (GPU-drawn text, no DOM/JS).
- **Process count**: 1 process vs 6.
- **Idle memory**: resident memory (working set) is far lower for GPUI. *Committed private* memory is
  closer than expected: ~100 MB of the GPUI number is committed by the DirectX driver and GPUI's
  atlases but only ~58 MB is resident. This is the first thing to optimise next (smaller atlases,
  fewer fonts, checking what gpui-kit initialises).
- **Large files**: gpui-component's tree-sitter highlighter keeps highlight state for the whole
  document. With highlighting on, the 3.5 MB file costs +180 MB, which is worse than CodeMirror
  (CodeMirror only highlights what is visible). The GPUI edition therefore opens files above 2 MB as
  plain text (like VS Code's large-file mode); the table shows both.
- **Binary size**: the GPUI exe is bigger (26 MB: 20 tree-sitter grammars, icon set, GPU renderer).
  The Tauri exe is small because it borrows the system WebView2 runtime (~300+ MB, preinstalled on
  Windows 11). Dropping grammars nobody needs, or loading them lazily, would shrink the GPUI exe.

## Feature parity (GPUI edition)

| Area | Tauri app | GPUI edition |
|---|---|---|
| Custom title bar + menus | ✅ | ✅ title bar, window buttons; menu labels only (no dropdowns yet) |
| Activity bar | ✅ | ✅ Explorer works; Search / Debug / OpenCode show "not ported yet" |
| Explorer tree + file icons | ✅ SVG icons, lazy folders, create/rename | ✅ tree with folder icons + coloured file badges; no create/rename/delete yet; folders read up front (20k entry cap, skips node_modules/target/.git) |
| Open folder / open from CLI | ✅ | ✅ (`fluxcode-gpui <folder> [files…]`, Ctrl+K Ctrl+O) |
| Tabs | ✅ | ✅ open/switch/close, dirty dot, Ctrl+Tab, Ctrl+W |
| Editor + syntax highlighting | ✅ CodeMirror 6 | ✅ gpui-component editor, tree-sitter (JS/TS/TSX/HTML/CSS/JSON/MD/PY/RS/PHP/TOML/YAML/sh/C/C++/Go/Java/Lua/SQL), line numbers, indent guides; undo/redo and find come with the gpui-component editor (not separately tested); files > 2 MB open without highlighting |
| Save | ✅ | ✅ Ctrl+S |
| Status bar | ✅ | ✅ cursor Ln/Col, language, messages |
| Completion + Emmet | ✅ | ❌ (gpui-component has a `CompletionProvider` hook; the JS logic in `src/complete/` must be ported to Rust) |
| Search in files | ✅ | ❌ |
| Terminal | ✅ | ❌ (FluxCode's terminal is not a PTY, so it is a plain output view + runner to port) |
| Run / Debug (Python, Node) | ✅ | ❌ |
| OpenCode ACP panel, inline Keep/Undo | ✅ | ❌ |
| Live Preview | ✅ (WebView) | ❌ (possible with `gpui-webview`, which loads WebView2 only while the preview is open) |
| Settings UI, GitHub sign-in | ✅ | ❌ |
| Window state, session restore | ✅ | ❌ |

## Licensing

`gpui`/`gpui-pre`, `gpui-component`, `gpui-kit`, `gpui-webview`: Apache-2.0. Zed's `editor` and
`terminal` crates are GPL and are **not** used. Third-party notices need the gpui-kit/Lucide entries
before a release.

## Building

See [`gpui-app/README.md`](../gpui-app/README.md). Release builds use `gpui-app/tools/fxc-shim`
(Windows' own `d3dcompiler_47.dll`) instead of the Windows SDK's `fxc.exe`.
