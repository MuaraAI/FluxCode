# FluxCode: Tauri (WebView2) vs GPUI (native)

Status: **working GPUI edition, most features ported** (see parity table). The GPUI edition lives in [`gpui-app/`](../gpui-app/) next to the
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

| Metric | Tauri (WebView2) | GPUI (full edition) | Difference |
|---|---|---|---|
| Executable size | 3.9 MB (+ system WebView2 runtime) | 26.4 MB (self-contained) | GPUI 6.8x bigger file |
| Start to window shown (median) | 436 ms | 212 ms | GPUI ~2x faster |
| Start to painted UI (median) | 506 ms | 273 ms | GPUI ~1.9x faster |
| Processes | 6 | 1 | |
| Idle private memory | 113 MB | 101 MB | GPUI -10 % |
| Idle working set | 336 MB | 60 MB | GPUI -82 % |
| Large file, private | 142 MB | 117 MB (large-file mode) / 283 MB (highlighting on) | see notes |
| Large file, working set | 386 MB | 76 MB (large-file mode) / 242 MB (highlighting on) | |
| CPU while typing (large file, auto save on) | 77 % | 11 % (large-file mode) / 46 % (highlighting on) | GPUI 1.7-7x less |
| CPU while scrolling (large file) | 63 % | 11 % (large-file mode) / 18 % (highlighting on) | GPUI 3.5-6x less |
| CPU idle after input | 10 % | 2.2 % | |

Raw start-to-paint runs (ms): Tauri 544, 506, 497, 499, 550; GPUI 260, 273, 275, 259, 275.
Final run with every ported feature (search, terminal, OpenCode, preview server, settings,
completion). The "highlighting on" values come from the first run, before the 2 MB large-file
limit existed; the shell-only prototype measured the same start/idle numbers (273 ms, 101 MB),
so the extra features cost nothing until they are used.

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

✅ = works and was checked in the running app or by a unit test; ⚠️ = works with a difference; ❌ = missing.

| Area | Tauri app | GPUI edition |
|---|---|---|
| Custom title bar + menus | ✅ | ✅ window buttons, File / Edit / View / Run / Terminal / Help dropdown menus |
| Activity bar | ✅ | ✅ Explorer, Search, Run, OpenCode, Settings |
| Explorer tree + file icons | ✅ SVG icons, lazy folders, create/rename | ⚠️ folder icons + coloured file badges (not the SVG set); new file/folder, rename, delete (to the Recycle Bin), copy path, reveal, via right-click / toolbar / F2 / Del; folders are read up front (20k entry cap, skips node_modules/target/.git) |
| Open folder / file / CLI | ✅ | ✅ Ctrl+K Ctrl+O, Ctrl+O, `fluxcode-gpui <folder> [files…]` |
| Tabs | ✅ | ✅ open/switch/close, dirty dot, Ctrl+Tab, Ctrl+W |
| Editor + syntax highlighting | ✅ CodeMirror 6 | ✅ gpui-component editor, tree-sitter for 20 languages, line numbers, indent guides, undo/redo/find from gpui-component; files > 2 MB open without highlighting |
| Save / auto save / line endings | ✅ | ✅ atomic save, CRLF kept, auto save 1 s after typing (on by default, like Tauri) |
| Completion + Emmet | ✅ | ⚠️ document words + language keywords + Emmet HTML (`!`, `ul>li.item$*3`, `#id.cls`, `[attr]`, `{text}`, `input:email`) and CSS (`m10`, `df`, `bgc#fff`, `w50p`); no snippet tab stops (gpui-component has none), so the cursor lands at the end |
| Search in files | ✅ | ✅ same search code as Tauri (match case, whole word, 5k hit cap), click opens at the line |
| Terminal | ✅ | ⚠️ same model (one command at a time through cmd.exe, `cd`/`cls` built in, stdin for prompts, Stop); ANSI colours are stripped; the folder access-grant prompt of the Tauri terminal is not ported |
| Run file | ✅ | ✅ F5 → Output panel (Python with workspace venv, Node, tsx, PowerShell, batch, cargo, go, php, lua, ruby) |
| Debugger (breakpoints) | in progress on another branch | ❌ |
| OpenCode ACP panel | ✅ | ⚠️ chat (Markdown), thinking, tool calls, plan, permission prompts (or auto-approve), agent reads/writes files, changed-file list with Keep / Undo per file; no inline diff in the editor, no mode/model picker |
| Live Preview | ✅ in-window WebView | ⚠️ same server and reload-on-save, opened in the system browser by default so no WebView2 is loaded; in-window WebView2 with `--features embedded-preview` (then `WebView2Loader.dll` must sit next to the exe) |
| Settings | ✅ | ✅ font size, tab size, word wrap, auto save, OpenCode auto-approve (JSON in `%APPDATA%\FluxCode-GPUI`) |
| GitHub sign-in | ✅ | ✅ token or GitHub CLI login (same `curl` check as Tauri), passed as GH_TOKEN to terminal + OpenCode (UI checked; sign-in itself not exercised here) |
| Session restore | ✅ | ✅ last folder, open files and active tab when started without arguments |
| Window position/size restore | ✅ | ❌ |
| Quick Open (Ctrl+P), command palette | ✅ | ❌ |
| `.flx` changelog viewer, explorer SVG icon fixes | in progress on another branch | ❌ |

## Licensing

`gpui`/`gpui-pre`, `gpui-component`, `gpui-kit`, `gpui-webview`: Apache-2.0, compatible with
FluxCode's GPL-3.0-or-later license. Zed's `editor` and `terminal` crates are not used (they are
GPL-3.0 and would now be license-compatible, but gpui-component's editor was enough). The GPUI dependencies are listed in THIRD_PARTY_NOTICES.md.

## Building

See [`gpui-app/README.md`](../gpui-app/README.md). Release builds use `gpui-app/tools/fxc-shim`
(Windows' own `d3dcompiler_47.dll`) instead of the Windows SDK's `fxc.exe`.
