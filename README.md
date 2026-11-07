# FluxCode

A lightweight code editor for Windows and Linux with the VS Code look and feel, built on
**Tauri 2 (Rust)** + **CodeMirror 6**. There's no bundled Chromium: the UI runs
in the system's native webview (WebView2 on Windows, WebKitGTK on Linux), and
everything that touches the machine (files, processes, the terminal, the preview server)
runs in Rust.

There is also a **native GPUI edition** (no WebView, one process, ~2x faster start and
~80 % less resident memory); see [GPUI edition](#gpui-edition-native-no-webview).

FluxCode is **free and open source** under the
[GNU General Public License v3.0 or later](LICENSE).

> Status: **development build `0.5.2-dev.2026-11-07-1545`** — not a final
> release. Focus: Python apps and the web (HTML, CSS, JavaScript/TypeScript).
> Dev builds are versioned `<version>-dev.<YYYY-MM-DD>-<HHMM>` and show a
> **DEV** badge in the title bar.

## Install

### Windows
Download `FluxCode_<version>_x64-setup.exe` from the
[Releases](https://github.com/MuaraAI/FluxCode/releases) page and run it. It
installs for your user only (no admin rights) into
`%LOCALAPPDATA%\FluxCode`, adds FluxCode to the Start Menu, Windows Search and
the Desktop, and is removed from Settings → Apps → Installed apps. A portable
zip is attached too.

### Linux (Debian, Ubuntu, Arch, CachyOS, ARM64)

Download the binary package for your distro from the [Releases](https://github.com/MuaraAI/FluxCode/releases) page:

- **Debian / Ubuntu (`.deb`)** (x86_64 & arm64):
  ```bash
  sudo dpkg -i fluxcode_<version>_amd64.deb # or arm64.deb
  sudo apt-get install -f # install system runtime dependencies if missing
  ```
- **Arch Linux / CachyOS**:
  Install using the `PKGBUILD` provided in `packages/arch/`:
  ```bash
  cd packages/arch
  makepkg -si
  ```
- **Universal Linux (`.AppImage`)** (x86_64 & aarch64):
  ```bash
  chmod +x FluxCode_<version>_x86_64.AppImage
  ./FluxCode_<version>_x86_64.AppImage
  ```

System dependencies on Linux: `webkit2gtk-4.1`, `libayatana-appindicator`, `openssl`.
## What works now

**Workbench**
- VS Code layout: menus (File, Edit, Selection, View, Run, Terminal, Help),
  centered command center, layout toggles, tabs (with preview tabs),
  breadcrumbs, panel (Output + Terminal), status bar. Seamless frameless window
  on Windows and native desktop decorations on Linux (GNOME, KDE, Hyprland, etc.)
- Opens inside the monitor's work area (never under taskbars; maximized
  on small screens) and reopens where you left it
- **Settings** (gear in the activity bar, Ctrl+, or the command palette):
  one searchable VS Code-style page — Appearance, Editor, Terminal, Accounts,
  OpenCode. Changes apply instantly and update in place (the page never
  jumps); **Reset to default** asks to confirm first
- **Color theme** (Ctrl+K Ctrl+T, with live preview while you arrow through
  the list): 8 presets (dark and light) or your own **Accent / Background /
  Text** colors. Every other color — panels, borders, selections, syntax
  highlighting, terminal ANSI colors — is derived from those and, with
  **auto contrast**, adjusted to meet WCAG contrast ratios (text 7:1, muted
  text and code 4.5:1), with live contrast scores
- **Fonts**: the interface uses the VS Code default on Windows (Segoe UI,
  three sizes); code and the terminal use Cascadia Code, Consolas or
  JetBrains Mono. Line numbers are zero-padded (`01`, `02` … `10`) so the
  gutter never shifts
- **Command palette** (Ctrl+Shift+P) also finds every setting: theme, fonts,
  interface size, terminal shell / font size / cursor, GitHub sign-in,
  OpenCode — each opens a quick pick or the right Settings section. Narrow
  windows fold the menu bar into a ☰ button so it never collides with the
  command center
- **Run and Debug** view (Ctrl+Shift+D): what F5 does for the active file and
  the runners found on this machine (Python, Node.js, Live Preview)
- **One icon system**: every file type is the same 16×16 tile (tinted
  background + simple glyph) so names line up; flat folders; UI icons on one
  24px grid with 1.5px strokes
- **Live Preview** with a compact header (file + folder), Fit / Tablet (768px)
  / Phone (390px) sizes, a thin loading bar instead of a white flash, and it
  follows whichever HTML file you switch to
- **Its own icon set** (UI icons, file-type badges, folder icons), drawn for
  this project
- Smooth, cheap animations (opacity/transform only); respects "reduce motion"
- **Auto Save** (on by default, File → Auto Save): saves 0.7 s after you stop
  typing and when you switch to another app
- Search view (Ctrl+Shift+F): find in files with highlighted results in the
  list and in the editor; Quick Open (Ctrl+P), Command Palette (Ctrl+Shift+P)

**Languages** (each loaded only when a file needs it)

| Language | Editing | F5 does |
| --- | --- | --- |
| Python | highlight, smart indent, completion, snippets, `import` of stdlib / PyPI / `requirements.txt` modules | run with live output, `input()`, clickable tracebacks, `.venv` detection |
| JavaScript / TypeScript | highlight, JSX/TSX, every browser API, Node.js globals and modules, npm packages (`package.json` first) in `import`/`require`, snippets | run with Node.js (TypeScript via Node's type stripping), clickable stack traces |
| HTML / CSS | highlight, auto-close tags, **Emmet** (`!`, `ul>li*3>a`, `div.card#main`, `input:email`, `m10-20`, `df`, `bgc#fff` …), tag completion in text | **Live Preview** inside FluxCode |
| PHP / Laravel / Blade | highlight, PHP functions, facades (`Route::`, `DB::` …), Eloquent / request methods after `->`, helpers, `use Illuminate\…`, Blade directives, snippets | — |
| JSON | `package.json` / `composer.json` keys, npm and Packagist package names in dependencies | — |
| Markdown, Rust | highlight | — |

**Completion** is VS Code-like: suggestions appear as you type, **Tab** or
Enter accepts them, Tab then walks through the snippet's fields, and Tab on an
Emmet abbreviation expands it directly. Words from the open file are offered
last. Everything is static data loaded with the language — no language
server process, so it costs nothing while idle.

**Live Preview** (HTML/CSS/JS): a tiny Rust server on `127.0.0.1` serves the
workspace folder; the preview pane reloads after every save (auto save makes
it live while you type). No polling: pages wait on a long-poll request.
Open in Browser is one click. Requests outside the folder are refused.

## Flux Terminal

FluxCode's own terminal, written from scratch (no xterm.js, no PTY, no shell
idling in the background). ``Ctrl+` `` or the **TERMINAL** tab; its path bar
and tools sit in the panel header to leave room for output.

- **No `cd` needed**: click any part of the path bar (long paths fold into
  `…`), use ▾ for subfolders or the root `/` / drive button, or type a folder
  name, `..`, `...`, `~`, `-` (back), `D:` or absolute `/path` and press Enter

- **Inline suggestions** while you type (history, files/folders, commands),
  shown next to the text with a `Tab` hint: **Tab** (or →) completes it;
  with no suggestion, Tab opens a completion picker
- **Blocks**: folder, status (✓ / ✗ exit code), duration, copy / rerun / stop
- Native, clickable `ls` / `dir`; colored command line; ANSI colors and
  progress bars; clickable tracebacks; multiple terminals; Ctrl+C, Ctrl+L
- Bare `python` / `py` / `node` start their interactive REPL
- Terminal Settings (⚙, or Settings → Terminal): shell (cmd, PowerShell,
  pwsh on Windows; bash, zsh, fish, sh on Linux), font size, cursor style,
  smooth caret, hidden files, revoke
  permissions — the popover updates in place and always stays inside the
  window. Clear (the icon next to ✕, or Ctrl+L) fades the old blocks out;
  the drive menu marks the current drive
- Signed in to GitHub (Settings → Accounts)? `gh` and token-aware tools get
  `GH_TOKEN` / `GITHUB_TOKEN` automatically
- **Permissions first**: the terminal only works in folders you allowed —
  the open workspace (for the session), a folder allowed once or always, or
  full disk access. This is a consent gate for where commands run, not a
  sandbox: a command can still reach any path your operating system account can


## OpenCode (AI agent)

The **OpenCode** view (robot icon, Ctrl+Shift+A) runs the
[OpenCode](https://opencode.ai) CLI inside FluxCode over the
[Agent Client Protocol](https://agentclientprotocol.com) (ACP). Install it
once (`npm i -g @opencode/cli`, v2 or newer) and sign in with
`opencode auth login`; FluxCode finds it on PATH or in the npm global folder.

- Starts `opencode acp` only when you send the first message, in the open
  folder; **Stop when idle** (default on) quits it after 10 minutes and the
  next message starts it again
- Streams the answer, the agent's plan, tool calls (with diffs and output)
  and thoughts into a light transcript
- **Live edits, like Cursor**: a file the agent changes opens in the editor
  at once with the hunks inline — added lines green, removed lines red — and
  **Keep / Undo** on every hunk. A **Changes** list in the view shows every
  file touched this session with +/− counts and Keep / Undo per file or for
  all of them
- Mode, model and effort are picked from light custom menus (no native
  `<select>`); the status shows the context tokens used
- Permission requests appear inline (allow once / always / reject), or set
  **Auto-approve tools** in Settings; modes (build / plan) and `/commands`
  come from OpenCode itself
- The active file can be attached to each prompt (chip in the composer)

**GitHub** (Settings → Accounts): sign in with a personal access token
(verified through api.github.com, stored in your FluxCode config folder) or
reuse the GitHub CLI's login. The token is passed to Flux Terminal commands
and to OpenCode as `GH_TOKEN`.

## Resource use

GPU acceleration in WebView2 is off by default (its GPU process alone used
~155 MB) and what remains runs in-process. Set `FLUXCODE_GPU=1` to turn it on.

Measured by `scripts/test-app.mjs` on this machine (Windows 11, all
processes including WebView2 and the programs being run):

| Moment | Processes | Private memory |
| --- | --- | --- |
| Idle, `examples/` open | 6 | **~112 MB** |
| After running Python and Node | 6 | ~135 MB |
| Live Preview open (one extra page process) | 7 | ~175 MB |
| Preview closed again | 6 | ~159 MB |
| VS Code, no extensions, fresh profile (for comparison) | 9 | ~1,577 MB |

Executable: 3.6 MB. Idle CPU: near zero. Completion data and the OpenCode
view are lazy chunks: nothing is loaded until a matching file or the view is
opened.

About the OpenCode process: the CLI is a ~260 MB Bun executable that uses
roughly 150–300 MB of memory of its own while a session is open. FluxCode
cannot make that smaller — it only makes sure the process exists while you
use it (start on first message, stop when idle). A whole-stack total under
50 MB is therefore not reachable with WebView2 plus OpenCode; FluxCode's own
share stays as above.

## GPUI edition (native, no WebView)

[`gpui-app/`](gpui-app/) is a second FluxCode front end built on
[GPUI](https://gpui.rs) (Zed's GPU-accelerated Rust UI framework) and
[gpui-component](https://github.com/longbridge/gpui-kit). The UI is drawn by the GPU
(DirectX 11 on Windows) in **one process**, without a browser engine. It reuses the Rust
backend logic of the Tauri app (search, processes, OpenCode/ACP, preview server, GitHub).

Benchmark on the same machine (Windows 11, AMD Radeon integrated graphics), release builds,
median of 5 runs, this repository open; Tauri numbers include every WebView2 process
([full report](docs/gpui-vs-tauri.md), script: `scripts/bench-gpui-vs-tauri.ps1`):

| | Tauri (WebView2) | GPUI | |
| --- | --- | --- | --- |
| Start to painted UI | 506 ms | **273 ms** | ~1.9x faster |
| Processes | 6 | **1** | |
| Idle memory, private | 113 MB | **101 MB** | -10 % |
| Idle memory, working set | 336 MB | **60 MB** | -82 % |
| Large file (60k lines), working set | 386 MB | **76 MB** | highlighting off above 2 MB |
| CPU while typing in the large file | 77 % | **11 %** | % of one core |
| CPU while scrolling the large file | 63 % | **11 %** | |
| CPU idle | 10 % | **2 %** | |
| Executable | **3.9 MB** (+ system WebView2) | 26.4 MB (self-contained) | |

Already in the GPUI edition: explorer (new/rename/delete to the Recycle Bin), tabs,
tree-sitter editor for 20 languages, auto save, find in files, terminal, Run (F5) with
Output, completion + Emmet, OpenCode panel with Keep/Undo, Live Preview, settings,
GitHub sign-in, session restore. Not yet: debugger, Quick Open, window position restore,
`.flx` viewer. The parity table is in [docs/gpui-vs-tauri.md](docs/gpui-vs-tauri.md).

Build and install it for your user (Start Menu + Settings → Apps entry):

```powershell
cd gpui-app
cargo build --release --manifest-path tools/fxc-shim/Cargo.toml   # SDK-free HLSL compiler shim
$env:GPUI_FXC_PATH = "<target>/release/fxc-shim.exe"
cargo build --release
powershell -ExecutionPolicy Bypass -File install.ps1
```

See [gpui-app/README.md](gpui-app/README.md) for the details (target dir without spaces
for MinGW, the optional in-window preview).

## Keybindings

| Action | Keys |
| --- | --- |
| Command Palette / Go to File | `Ctrl+Shift+P`, `F1` / `Ctrl+P` |
| Open Folder / Open File / New File | `Ctrl+K Ctrl+O` / `Ctrl+O` / `Ctrl+N` |
| Save / Save As / Save All | `Ctrl+S` / `Ctrl+Shift+S` / `Ctrl+K S` |
| Close Editor / Close All | `Ctrl+W` / `Ctrl+K Ctrl+W` |
| Next / Previous Editor | `Ctrl+Tab` / `Ctrl+Shift+Tab` |
| Toggle Side Bar / Panel | `Ctrl+B` / `Ctrl+J` |
| Settings / Color Theme | `Ctrl+,` / `Ctrl+K Ctrl+T` |
| Run and Debug / OpenCode view | `Ctrl+Shift+D` / `Ctrl+Shift+A` |
| Accept suggestion / expand Emmet / next snippet field | `Tab` |
| Toggle Terminal / New Terminal | ``Ctrl+` `` / ``Ctrl+Shift+` `` |
| Run or Preview / Stop | `F5` / `Shift+F5` |
| Find / Replace / Go to Line | `Ctrl+F` / `Ctrl+H` / `Ctrl+G` |
| Find in Files / Explorer | `Ctrl+Shift+F` / `Ctrl+Shift+E` |
| Word Wrap / Zoom | `Alt+Z` / `Ctrl+=`, `Ctrl+-`, `Ctrl+0` |
| Next occurrence / Comment / Delete line | `Ctrl+D` / `Ctrl+/` / `Ctrl+Shift+K` |
| Move line / Copy line | `Alt+↑↓` / `Shift+Alt+↑↓` |

## Build from source

**Windows.** Requirements: Node.js 22+, Rust (GNU toolchain), MinGW-w64
binutils (e.g. `winget install BrechtSanders.WinLibs.POSIX.UCRT`), WebView2
(built into Windows 10/11).

```bash
npm install
npx tauri dev                # run with hot reload
npx tauri build --no-bundle  # optimized FluxCode exe
npx tauri build              # exe + NSIS installer (bundle/nsis/*-setup.exe)
node scripts/test-app.mjs    # end-to-end test of the built exe
```

Build output goes to `C:/Users/User/.fluxcode-target` (see
`src-tauri/.cargo/config.toml`), because MinGW's `windres` can't handle spaces
in the project path.

**Linux** (Debian/Ubuntu shown; the same code, WebKitGTK instead of WebView2):

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf build-essential curl wget file libssl-dev
npm install
CARGO_TARGET_DIR=target npx tauri dev
CARGO_TARGET_DIR=target npx tauri build --bundles deb,appimage   # bundle/deb/*.deb, bundle/appimage/*.AppImage
```

`CARGO_TARGET_DIR` overrides the Windows-only path in `.cargo/config.toml`.
On Linux the terminal runs commands through `bash` (or `sh`/`zsh`/`pwsh` if
installed), the drive menu lists `/` and mounted volumes, and OpenCode is
found on PATH or in `~/.opencode/bin`. Not yet exercised on real hardware:
the Linux build is compiled and bundled by CI (`.github/workflows/build.yml`),
which produces `.deb` and `.AppImage` artifacts for every push and pull
request, labelled as development builds.

`scripts/test-app.mjs` drives the real app through WebView2's DevTools
protocol (no real keyboard/mouse input, throwaway profile): it runs every
example, checks the results, saves screenshots and measures memory.

## Project layout

```
src/                     frontend (plain JS + CSS)
  main.js                workbench: explorer, tabs, panel, preview, auto save, commands, palette
  editor.js              CodeMirror setup, theme, Tab behaviour, indent guides, search highlights
  complete/              completion: emmet.js, html.js, css.js, javascript.js, python.js, php.js, json.js
  terminal.js            Flux Terminal (lazy-loaded)
  agent.js               OpenCode over ACP: JSON-RPC client + transcript view (lazy-loaded)
  settings.js            Settings panel (lazy-loaded)
  prefs.js               shared preference store (terminal, agent, editor)
  languages.js           lazy language registry
  icons.js               FluxCode icon set + file-type badges
  theme.js               theme engine: presets, derived palette, auto contrast, fonts
src-tauri/src/lib.rs     file system, search, process runner, Python/Node detection
src-tauri/src/acp.rs     spawns `opencode acp` and bridges its stdio to the view
src-tauri/src/github.rs  GitHub account (token / gh), verified with the system curl
src-tauri/src/terminal.rs    terminal runner, native listing, folder-access grants
src-tauri/src/preview.rs     Live Preview server
src-tauri/src/window_state.rs  work-area placement, remembered window bounds
scripts/test-app.mjs     end-to-end test (CDP)
examples/                python/, js/ (JS + TS), web/ (HTML + CSS + JS)
```

## Roadmap

1. Language servers: basedpyright/ruff (Python), TypeScript, HTML/CSS
   diagnostics, go-to-definition and hover
2. npm / composer scripts runner; OpenCode: session history, image input
3. Rust: `cargo run`/`cargo check` with clickable errors, rust-analyzer
4. Flux Terminal: optional PTY mode for full-screen programs; Git status,
   minimap, settings UI, bracket pair colors
5. Extensions: an Open VSX–compatible host is a long-term goal. Microsoft's
   proprietary extensions (Pylance, C# Dev Kit, Remote-SSH, …) are licensed
   only for Microsoft's VS Code, so FluxCode uses open alternatives

## License

Copyright (C) 2026 MuaraAI and contributors.

FluxCode is free software: you can redistribute it and/or modify it under the
terms of the **GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version** — see [LICENSE](LICENSE). Anyone may use, study, share and change it;
if you distribute a modified version, you must share its source under the same
license. FluxCode comes with no warranty.

Third-party components keep their own licenses
([THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)); all of them are compatible
with the GPL v3.
