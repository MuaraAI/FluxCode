# FluxCode

A lightweight code editor for Windows with the VS Code look and feel, built on
**Tauri 2 (Rust)** + **CodeMirror 6**. There's no bundled Chromium: the UI runs
in Windows' built-in WebView2, and everything that touches the machine (files,
processes) runs in Rust.

> Status: **development build `0.2.0-dev.2026-11-07-1026`** — not a final
> release. Python first; the roadmap is Python → Rust → HTML/CSS/JS.
> Dev builds are versioned `<version>-dev.<YYYY-MM-DD>-<HHMM>` and show a
> **DEV** badge in the title bar.

## Flux Terminal

FluxCode's own terminal, written from scratch (no xterm.js, no PTY, no shell
process idling in the background). Open it with ``Ctrl+` `` or the
**TERMINAL** tab in the panel.

- **No `cd` needed**: click any part of the path bar to jump there, use the
  ▾ menu to dive into a subfolder or the drive button to switch drives, or
  just type a folder name, `..`, `...`, `~`, `-` (back) or `D:` and press
  Enter. Commands still work as usual (`cd` too)
- **Blocks**: every command shows its folder, status (✓ / ✗ exit code),
  duration and actions (copy output, run again, stop)
- Native, clickable `ls` / `dir` (folders open, files open in the editor)
- Colored command line, Tab path completion with a picker, ghost suggestions
  from history and folder names (→ to accept), ↑/↓ history
- Smooth caret that glides and fades, typed characters ease in (can be
  turned off), bar / block / line cursor, font size, shell
  (cmd, PowerShell, pwsh) — all in Terminal Settings (⚙)
- ANSI colors and progress bars (`\r`) render; Python tracebacks and
  `C:\path:line` are clickable
- Multiple terminals, `input()`-style stdin for running programs, Ctrl+C,
  Ctrl+L
- **Permissions first**: the terminal only works in folders you allowed —
  the open workspace (for the session), a folder allowed once or always, or
  full disk access. A dialog asks before entering any new folder, and
  Terminal Settings can revoke everything. This is a consent gate for where
  commands run, not a sandbox: a command you run can still reach any path
  the Windows account can.
- Limits: full-screen TUI programs (vim, htop, interactive REPL UIs) need a
  real PTY and aren't supported yet

## What works now

- VS Code layout: custom title bar with menus (File, Edit, Selection, View,
  Run, Help), a centered command center, layout toggles and window buttons;
  activity bar, explorer, search, tabs (with preview tabs), breadcrumbs,
  output panel, status bar, resizable side bar and panel
- FluxCode's own dark theme and **its own icon set**, drawn for this project
  (not codicons/Seti): UI icons, file-type badges, folder icons
- Smooth, cheap animations (opacity/transform only): palette, menus, tabs,
  folder chevrons, rows, panel, toasts, smooth caret. Respects the OS
  "reduce motion" setting
- **Search view (Ctrl+Shift+F)**: find in files (Rust, background thread),
  Match Case / Whole Word, matches highlighted in the results *and* in the
  open editor
- Explorer: lazy folder loading, in-place expand/collapse, keyboard
  navigation, new file/folder inline, rename (F2), refresh, collapse all,
  dirty markers
- Word wrap (Alt+Z), zoom (Ctrl+= / Ctrl+- / Ctrl+0)
- Editor: syntax highlighting with Dark+ colors, indent guides, folding,
  bracket matching/auto-close, multi-cursor, search/replace, autocomplete,
  go to line, VS Code keybindings
- **Python:** highlighting, smart indent, keyword + local completion,
  **Run Python File (F5)** with live output, `input()` support, Stop, and
  clickable traceback lines. Uses the workspace `.venv`/`venv` automatically,
  otherwise `python`/`py` from PATH
- Quick Open (Ctrl+P), Command Palette (Ctrl+Shift+P / F1), `:line` jump
- Atomic saves, CRLF/LF preserved (toggle in the status bar), UTF-8 BOM handled,
  binary and very large files refused instead of freezing
- Session restore (folder, open files, layout); `fluxcode <folder|file>` on the
  command line

## Keybindings

| Action | Keys |
| --- | --- |
| Command Palette | `Ctrl+Shift+P`, `F1` |
| Go to File | `Ctrl+P` |
| Open Folder | `Ctrl+K Ctrl+O` |
| Open File | `Ctrl+O` |
| New File | `Ctrl+N` |
| Save / Save As / Save All | `Ctrl+S` / `Ctrl+Shift+S` / `Ctrl+K S` |
| Close Editor / Close All | `Ctrl+W` / `Ctrl+K Ctrl+W` |
| Next / Previous Editor | `Ctrl+Tab`, `Ctrl+PageDown` / `Ctrl+Shift+Tab`, `Ctrl+PageUp` |
| Toggle Side Bar / Panel | `Ctrl+B` / `Ctrl+J` |
| Toggle Terminal / New Terminal | ``Ctrl+` `` / ``Ctrl+Shift+` `` |
| Run Python File / Stop | `F5`, `Ctrl+F5` / `Shift+F5` |
| Find / Replace / Go to Line | `Ctrl+F` / `Ctrl+H` / `Ctrl+G` |
| Find in Files / Explorer | `Ctrl+Shift+F` / `Ctrl+Shift+E` |
| Word Wrap / Zoom | `Alt+Z` / `Ctrl+=`, `Ctrl+-`, `Ctrl+0` |
| Select next occurrence | `Ctrl+D` |
| Toggle comment | `Ctrl+/` |
| Move line / Copy line | `Alt+↑↓` / `Shift+Alt+↑↓` |
| Delete line | `Ctrl+Shift+K` |

## Why it's light

| | VS Code (Electron) | FluxCode (Tauri) |
| --- | --- | --- |
| Browser engine | Bundled Chromium per app | Shared, OS-provided WebView2 |
| Backend | Node.js | Native Rust |
| UI | Large TS workbench | Plain DOM, no framework |
| Editor | Monaco | CodeMirror 6, renders only visible lines |
| Languages | Loaded via extensions | Lazy-loaded chunks, only when a file needs one |

The release build also uses fat LTO, `opt-level = "s"`, `panic = "abort"` and
stripped symbols. WebView2 background services (translate, sync, component
updates, SmartScreen pings) are switched off.

WebView2 GPU acceleration is **off by default**: the GPU process alone used
~155 MB, and text editing doesn't need it. What remains of it runs inside
the browser process (`--in-process-gpu`), saving one more process. To turn
hardware acceleration back on, start FluxCode with the environment variable
`FLUXCODE_GPU=1`.

Measured on this machine (Windows 11, `examples/python` open, all processes
including WebView2, after ~10 s idle):

| | Processes | Private memory | Working set |
| --- | --- | --- | --- |
| **FluxCode 0.2 dev**, terminal open + commands run (exe 3.6 MB) | 6 | **~131 MB** | ~408 MB |
| FluxCode 0.2, editor only | 6 | ~119 MB | ~361 MB |
| FluxCode 0.1 | 7 | ~136 MB | ~398 MB |
| FluxCode with `FLUXCODE_GPU=1` | 7 | ~275 MB | ~475 MB |
| VS Code, no extensions, fresh profile | 9 | ~1,577 MB | ~1,859 MB |

Idle CPU use is close to zero (~16 ms over 5 s while the editor has focus,
from the smooth caret fade).

## Build from source (Windows)

Requirements: Node.js 20+, Rust (GNU toolchain), MinGW-w64 binutils
(e.g. `winget install BrechtSanders.WinLibs.POSIX.UCRT`), WebView2 (built into
Windows 10/11).

```bash
npm install
npx tauri dev                # run with hot reload
npx tauri build --no-bundle  # optimized FluxCode exe
npx tauri build              # + NSIS installer
```

Build output goes to `C:/Users/User/.fluxcode-target` (see
`src-tauri/.cargo/config.toml`), because MinGW's `windres` can't handle spaces
in the project path.

UI-only preview in a browser, with a mocked backend: `npm run dev`, then open
`http://localhost:1420/dev.html`.

## Project layout

```
src/                 frontend (plain JS + CSS)
  main.js            workbench: explorer, tabs, panel, quick open, commands
  editor.js          CodeMirror setup, theme, indent guides, search highlights
  terminal.js        Flux Terminal (lazy-loaded)
  languages.js       lazy language registry (add Rust / web here)
  icons.js           inline SVG icons + file-type glyphs
src-tauri/src/lib.rs Rust backend: file system, search, process runner, Python detection
src-tauri/src/terminal.rs  terminal runner, native listing, folder-access grants
scripts/shot.ps1     dev helper: screenshot / drive the app window
examples/python/     files for testing the Python workflow
```

## Roadmap

1. **Python** (now): language server (basedpyright/ruff) for diagnostics,
   go-to-definition and hover
2. **Rust**: `lang-rust`, `cargo run`/`cargo check` with clickable errors,
   rust-analyzer
3. **HTML/CSS/JS/TS**: languages, Emmet, live preview
4. Flux Terminal: optional PTY mode for full-screen programs; Git status,
   minimap, settings UI, bracket pair colors
5. Extensions: an Open VSX–compatible host is a long-term goal. Microsoft's
   proprietary extensions (Pylance, C# Dev Kit, Remote-SSH, …) are licensed
   only for Microsoft's VS Code, so FluxCode will use open alternatives
   (basedpyright, rust-analyzer, …)

## License

MIT
