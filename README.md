# FluxCode

A lightweight code editor for Windows and Linux with the VS Code look and feel, built on
**Tauri 2 (Rust)** + **CodeMirror 6**. There's no bundled Chromium: the UI runs
in the system's native webview (WebView2 on Windows, WebKitGTK on Linux), and
everything that touches the machine (files, processes, the terminal, the preview server)
runs in Rust.

> Status: **development build `0.4.2-dev.2026-11-07-1206`** — not a final
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

- **Appearance** (gear in the activity bar, or Ctrl+K Ctrl+T): 8 theme
  presets (dark and light) or your own **Accent / Background / Text** colors
  with color pickers. Every other color — panels, borders, selections, syntax
  highlighting, terminal ANSI colors — is derived from those and, with
  **auto contrast**, adjusted to meet WCAG contrast ratios (text 7:1, muted
  text and code 4.5:1), with live contrast scores in the panel
- **Two interface fonts** (Appearance, or View → Font — Jersey / Font — VS
  Code Default):
  - **Jersey** (SIL OFL, pixel style): all 8 Jersey families on Fontsource were
    compared sharp and blurred (to simulate low vision); Jersey 15 stayed the
    most legible, so it is used for all interface text, Jersey 25 only for the
    big wordmark. The "Charted" variants were left out: their grid background
    cuts contrast. No synthetic bold
  - **VS Code default**: Segoe UI, like VS Code on Windows
  - Code always stays in a monospace font (Cascadia Code / Consolas)
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
| Python | highlight, smart indent, completion | run with live output, `input()`, clickable tracebacks, `.venv` detection |
| JavaScript / TypeScript | highlight, JSX/TSX | run with Node.js (TypeScript via Node's type stripping), clickable stack traces |
| HTML / CSS | highlight, auto-close tags | **Live Preview** inside FluxCode |
| JSON, Markdown, Rust | highlight | — |

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
- Terminal Settings (⚙): shell (bash, zsh, fish, sh on Linux; cmd, PowerShell,
  pwsh on Windows), font size, cursor style, smooth caret, hidden files, revoke
  permissions — popovers always stay inside the window

- **Permissions first**: the terminal only works in folders you allowed —
  the open workspace (for the session), a folder allowed once or always, or
  full disk access. This is a consent gate for where commands run, not a
  sandbox: a command can still reach any path your operating system account can


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

Executable: 3.6 MB. Idle CPU: near zero.

## Keybindings

| Action | Keys |
| --- | --- |
| Command Palette / Go to File | `Ctrl+Shift+P`, `F1` / `Ctrl+P` |
| Open Folder / Open File / New File | `Ctrl+K Ctrl+O` / `Ctrl+O` / `Ctrl+N` |
| Save / Save As / Save All | `Ctrl+S` / `Ctrl+Shift+S` / `Ctrl+K S` |
| Close Editor / Close All | `Ctrl+W` / `Ctrl+K Ctrl+W` |
| Next / Previous Editor | `Ctrl+Tab` / `Ctrl+Shift+Tab` |
| Toggle Side Bar / Panel | `Ctrl+B` / `Ctrl+J` |
| Toggle Terminal / New Terminal | ``Ctrl+` `` / ``Ctrl+Shift+` `` |
| Run or Preview / Stop | `F5` / `Shift+F5` |
| Find / Replace / Go to Line | `Ctrl+F` / `Ctrl+H` / `Ctrl+G` |
| Find in Files / Explorer | `Ctrl+Shift+F` / `Ctrl+Shift+E` |
| Word Wrap / Zoom | `Alt+Z` / `Ctrl+=`, `Ctrl+-`, `Ctrl+0` |
| Next occurrence / Comment / Delete line | `Ctrl+D` / `Ctrl+/` / `Ctrl+Shift+K` |
| Move line / Copy line | `Alt+↑↓` / `Shift+Alt+↑↓` |

## Build from source (Windows)

Requirements: Node.js 22+, Rust (GNU toolchain), MinGW-w64 binutils
(e.g. `winget install BrechtSanders.WinLibs.POSIX.UCRT`), WebView2 (built into
Windows 10/11).

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

`scripts/test-app.mjs` drives the real app through WebView2's DevTools
protocol (no real keyboard/mouse input, throwaway profile): it runs every
example, checks the results, saves screenshots and measures memory.

## Project layout

```
src/                     frontend (plain JS + CSS)
  main.js                workbench: explorer, tabs, panel, preview, auto save, commands
  editor.js              CodeMirror setup, theme, indent guides, search highlights
  terminal.js            Flux Terminal (lazy-loaded)
  languages.js           lazy language registry
  icons.js               FluxCode icon set + file-type badges
  theme.js               theme engine: presets, derived palette, auto contrast, fonts
  appearance.js          Appearance panel (lazy-loaded)
src-tauri/src/lib.rs     file system, search, process runner, Python/Node detection
src-tauri/src/terminal.rs    terminal runner, native listing, folder-access grants
src-tauri/src/preview.rs     Live Preview server
src-tauri/src/window_state.rs  work-area placement, remembered window bounds
scripts/test-app.mjs     end-to-end test (CDP)
examples/                python/, js/ (JS + TS), web/ (HTML + CSS + JS)
```

## Roadmap

1. Language servers: basedpyright/ruff (Python), TypeScript, HTML/CSS
   diagnostics, go-to-definition and hover
2. Emmet for HTML/CSS; npm scripts runner
3. Rust: `cargo run`/`cargo check` with clickable errors, rust-analyzer
4. Flux Terminal: optional PTY mode for full-screen programs; Git status,
   minimap, settings UI, bracket pair colors
5. Extensions: an Open VSX–compatible host is a long-term goal. Microsoft's
   proprietary extensions (Pylance, C# Dev Kit, Remote-SSH, …) are licensed
   only for Microsoft's VS Code, so FluxCode uses open alternatives

## License

FluxCode is **source-available**, not open source — see [LICENSE](LICENSE).
You may read the code, use the official builds, and fork/build it **only to
contribute back** to this repository (see [CONTRIBUTING.md](CONTRIBUTING.md)).
Redistributing it, publishing forks or rebrands, or reusing its code in other
products needs written permission from MuaraAI. Third-party components keep
their own licenses ([THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
