# FluxCode Roadmap

FluxCode is built in the open at [MuaraAI](https://github.com/MuaraAI) by engineers with years of Rust systems experience and 6+ years of building and running production SaaS products.

We do not treat a roadmap as a wishlist. Every milestone below is scoped to what we can realistically ship, benchmark, and maintain with our engineering workflow (combining hands-on Rust systems work with AI-assisted development and strict CI checks on binary size, startup latency, and memory).

## How We Ship

- **Continuous `BuildDev` releases**: every change merged into `main` is compiled, tested, and published automatically across Windows, Linux, and macOS.
- **Hard performance budgets**: CI compares every pull request against `main` for executable size, startup/parse time, and peak RSS (`pr-check.json`). Features that break our ~50 ms startup or ~1.4 MB idle RAM baseline do not merge.
- **Zero-telemetry, light-first core**: no bundled Electron/Chromium runtime, no hidden background polling, and no new crates added to `Cargo.toml` without a clear reason.

---

## Shipped Foundation (2026)

What is already live in `main` today:

- [x] **Single-process Rust + GPUI editor** (~16 MB binary, ~50 ms startup, ~1.4 MB idle RAM on Windows 11).
- [x] **VS Code layout & keybindings** (activity bar, dockable left/right sidebars, tabs, bottom panel, Command Palette, Go to File, Find in Files).
- [x] **Agent Client Protocol (ACP) workspace** (full-screen & side-panel modes, 1-click installs from the ACP registry, per-turn file diffs & undo, git worktrees, token saver, usage-limit auto-resume).
- [x] **GitHub Copilot inline ghost-text completions** via `@github/copilot-language-server` in a separate process.
- [x] **Built-in Git Source Control**, Python-aware terminal (`F5` run + error cards), and **Live Share** (encrypted peer-to-peer / relay collaborative editing).
- [x] **Native VS Code Marketplace browser** (themes & TextMate grammars loaded natively in Rust without running JS).
- [x] **Read-only instant viewers** for SQLite, PDF, Word/Excel/PowerPoint, ZIP/archives, and images.
- [x] **Interactive Markdown & `.flux` Preview** with vertical scrollbar, zoom (`50%` to `250%`), and freehand drawing annotations.
- [x] **WSL2 native Linux helper** (`src/remote/`) for fast file reads/writes and git operations inside WSL distros.
- [x] **Contributor AI standards** (`.agents/skills/` with `fluxcode-dev`, `antislop`, and `ponytail`).

---

## Phase 1: Security Hardening & Core Stability (Late 2026 - Q1 2027)

Closing the security and reliability items identified in our own audit (`docs/security-review-2026-11-14.md`) before scaling up new platforms.

- [ ] **Workspace trust model**: prompt once before auto-running `git`, project `.venv` Python, build wrappers (`gradlew`/`mvnw`), or project agent instruction files in untrusted folders.
- [ ] **Signed & checksum-verified self-updates**: publish SHA-256 checksums with every `BuildDev` release and verify downloads in `src/update.rs` before replacing the binary.
- [ ] **OS credential store for tokens**: move GitHub tokens out of plaintext JSON into Windows Credential Manager, macOS Keychain, and Linux `libsecret`, and stop injecting tokens into untrusted child processes.
- [ ] **Agent path containment & permission clarity**: canonicalize ACP `fs/read_text_file` and `fs/write_text_file` paths against the workspace root and default fresh installs to safer write-confirmation behavior.
- [ ] **Dirty-tab close guard**: prompt before closing unsaved buffers or quitting when auto-save is turned off.
- [ ] **Preview annotation export** ([#111](https://github.com/MuaraAI/FluxCode/issues/111)) and **configurable smooth-scroll sensitivity** ([#112](https://github.com/MuaraAI/FluxCode/issues/112)).

---

## Phase 2: Lightweight LSP & Power-User Editing (Q1 - Q2 2027)

Adding real language intelligence while keeping idle memory and startup speed intact.

- [ ] **Opt-in LSP client**: connect to local language servers (`rust-analyzer`, `pyright`, `typescript-language-server`, `gopls`, `clangd`) over stdio only when a matching project is opened.
- [ ] **Inline diagnostics, hover docs & go-to-definition**: build on top of our existing wavy underline diagnostics (`src/brackets.rs`) and symbol jump UI.
- [ ] **Multi-cursor & split editors**: side-by-side split panes, synchronized diff editing, and multi-cursor selection (`Ctrl+D`, `Alt+Click`).
- [ ] **macOS notarization & Linux polish**: notarized universal macOS `.dmg` builds plus Wayland/X11 clipboard and IME improvements.

---

## Phase 3: FluxCode for Android (2027)

Bringing the same native Rust + GPU codebase to Android phones, tablets, foldables, and DeX setups without bundling a WebView. Full discussion in [#117](https://github.com/MuaraAI/FluxCode/discussions/117).

- [ ] **Q1 2027 - `aarch64-linux-android` build foundation**: isolate desktop-only OS calls behind clean `cfg` gates, stand up the native Vulkan rendering surface, and wire Android lifecycle & low-memory trim hooks.
- [ ] **Q2 2027 - Touch ergonomics & Android IME**: software keyboard composition (auto-correct off in code buffers), hardware keyboard shortcuts (`Ctrl+P`, `Ctrl+L`, `Ctrl+S`), pinch-to-zoom, selection handles, and a compact coding symbol bar (`Tab`, `Esc`, `{ }`, `[ ]`, `->`). Adaptive single-pane layout on phones and multi-pane on tablets/foldables.
- [ ] **Q3 2027 - Remote workspaces & mobile AI agent flow**: connect from Android to your desktop FluxCode or a remote Linux/SSH machine over `src/remote/` to run builds and ACP AI agents on real hardware without draining phone battery, plus local SAF/Termux file access and touch/stylus annotations in Markdown/`.flux` preview.
- [ ] **Q4 2027 - Android Public Beta & Release**: signed `FluxCode-...-android-arm64.apk` on GitHub Releases alongside F-Droid and Play Store distribution.

---

## Phase 4: Extensibility & Long-Term Sustainability (Late 2027+)

- [ ] **Sandboxed WASM extensions**: lightweight, capability-scoped plugins in WebAssembly so community extensions can run safely across desktop and Android without a Node.js runtime.
- [ ] **Remote SSH workspaces**: first-class SSH remote editing built on the same framed helper protocol already powering WSL2.
- [ ] **Versioned `v1.0` release**: graduate from continuous `BuildDev-*` builds to semantic versioning with stable and nightly update channels.

---

## Track Progress & Contribute

- Live commits and builds: [Releases](https://github.com/MuaraAI/FluxCode/releases) and [CHANGELOG.md](CHANGELOG.md)
- Vote on upcoming work or suggest ideas: [Discussions (#114)](https://github.com/MuaraAI/FluxCode/discussions/114)
- Ready-to-pick tasks: [Good First Issues](https://github.com/MuaraAI/FluxCode/labels/good%20first%20issue)
