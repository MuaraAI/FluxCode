# FluxCode

A native code editor for Windows and Linux, written in Rust and drawn by the GPU.
One small program with an AI agent panel, a Python-friendly terminal, built-in Source
Control, and read-only views for files other editors refuse to open. No telemetry.

A [MuaraAI](https://github.com/MuaraAI) project, built by a first-semester university
student in Indonesia ([MyKineID](https://github.com/MyKineID)). Open source, and still in
development.

[![Latest build](https://img.shields.io/github/v/release/MuaraAI/FluxCode?display_name=tag&label=latest%20build)](https://github.com/MuaraAI/FluxCode/releases/latest)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

<a href="assets/screenshots/agent.png"><img src="assets/screenshots/agent.png" width="100%" alt="FluxCode with main.py open. In the agent panel on the right, OpenCode has added a docstring to greet(); the Changes card lists main.py with one added line, and a prompt asks before the agent runs a Python command."></a>

The agent added a docstring (line 23), the Changes card lists the file with Undo, and
because access is set to Read & write, it has to ask before running a command.

## Download

Get the newest build from [Releases](https://github.com/MuaraAI/FluxCode/releases/latest).

| System | File | Install |
| --- | --- | --- |
| Windows 10/11, x64 | `FluxCode-BuildDev-...-windows-x64.zip` | Extract, double-click `Install.cmd`. No admin rights. |
| Linux, x64 (glibc 2.35+, Vulkan) | `FluxCode-BuildDev-...-linux-x64.tar.gz` | Extract, run `./install.sh`. No root. |

There are no version numbers yet: every change merged into `main` becomes a build named
after the moment it was made, like `BuildDev-2026-11-09-18-19-54`. Installed copies offer
the new one in a card and update themselves in one click. Settings > Updates turns the
check off.

## A closer look

<p align="center">
  <a href="assets/screenshots/python-error.png"><img src="assets/screenshots/python-error.png" width="49%" alt="A Python run fails with ModuleNotFoundError. The Output panel shows a card with the file and line, the code line, a hint, and a Fix button that runs py -m pip install tabulate."></a>
  <a href="assets/screenshots/sqlite.png"><img src="assets/screenshots/sqlite.png" width="49%" alt="A SQLite database opened as text: each table and view with its CREATE statement and row count, under a line saying the view is read-only."></a>
</p>

A Python error becomes a card with the file, the line and a hint; for a missing module, Fix
installs it. A SQLite database opens as its tables, SQL and row counts, like Word, Excel,
PDF and ZIP files do, instead of an error.

<p align="center">
  <a href="assets/screenshots/source-control.png"><img src="assets/screenshots/source-control.png" width="49%" alt="Source Control after git init in a game server folder: 2801 untracked files, with the folders holding most of them listed first, each with an Ignore button."></a>
  <a href="assets/screenshots/go-to-file.png"><img src="assets/screenshots/go-to-file.png" width="49%" alt="Go to File in the FluxMono theme: typing wbench finds workbench.rs in the src folder."></a>
</p>

Source Control stays fast with thousands of new files and offers to ignore the folders
that hold them. Go to File (`Ctrl+P`) finds a file from a few letters of its name.

<p align="center">
  <a href="assets/screenshots/themes.png"><img src="assets/screenshots/themes.png" width="49%" alt="Settings: four color themes as cards (FluxDark, FluxCream, FluxMono, FluxRed), then language and interface size."></a>
  <a href="assets/screenshots/update-indonesian.png"><img src="assets/screenshots/update-indonesian.png" width="49%" alt="The FluxCream theme with the interface in Bahasa Indonesia. A card in the corner offers a newer build, with its notes and a button to update and restart."></a>
</p>

Four color themes, three interface sizes and your own background image. The interface
also comes in Bahasa Indonesia, shown here with the update card in the FluxCream theme.

## What else it does

- Syntax colors for about 200 languages, picked by file name, extension or `#!` line.
- Multi-root workspaces: drag a folder onto the window to add it or switch to it.
- The agent panel (`Ctrl+L`) runs any [ACP](https://agentclientprotocol.com) agent, such
  as [OpenCode](https://opencode.ai) or MuaraAI's Mura. Pick the model, effort and mode
  there; edit a sent message and the answer restarts from it; the agent gets an overview
  of the workspace and a local memory you can read, edit or switch off.
- Source Control commits, pushes, pulls and syncs, shows a commit graph, opens pull
  requests and GitHub Actions runs through `gh`, and Remove Git undoes a `git init`.
- Settings > About FluxCode links to this repository, the issue tracker and the changelog.

## FluxCode, Zed and VS Code

All three are good editors made for different people. This is where each stands today.

| | FluxCode | Zed | VS Code |
| --- | --- | --- | --- |
| Built with | Rust, GPUI (GPU-drawn) | Rust, GPUI (GPU-drawn) | TypeScript on Electron (Chromium + Node.js) |
| Systems | Windows, Linux | macOS, Linux, Windows | Windows, macOS, Linux, web |
| What you download | one program, about 16 MB | an installer | an installer |
| Telemetry | none | on by default, can be turned off | on by default in Microsoft's build, can be turned off |
| Language smarts (go to definition, rename, diagnostics) | not yet: colors and word completion | built in, through language servers | through extensions |
| Debugger | not yet (Run only) | yes | yes |
| Extensions | none | yes | the largest catalog |
| AI agent | any ACP agent, three access levels, per-turn changes with undo, local memory | its own agent and ACP agents | GitHub Copilot and extensions |
| Files that are not text | hex, PDF, Office, ZIP, SQLite (read-only) | images | images; more through extensions |
| Real-time collaboration | no | built in | Live Share extension |
| Remote development | no | SSH | SSH, containers, WSL |
| Interface languages | English, Bahasa Indonesia | English | many, through language packs |
| Maturity | development builds | stable releases | stable, widely used |
| License | GPL-3.0-or-later | GPL-3.0 (the editor) | MIT for Code - OSS; Microsoft's build has its own license |

Pick FluxCode if you want a small, quiet editor on Windows or Linux with an agent you can
keep on a short leash, and you open logs, databases and documents next to your code.
Pick Zed for macOS, language servers out of the box and pair programming.
Pick VS Code when you need a specific extension, a debugger or remote development.

## Privacy

FluxCode sends nothing about you. The only request it makes by itself is the update
check: a plain `GET` of the release list from github.com, with no build ID, machine ID or
cookie, at start and every 6 hours (Settings > Updates turns it off). Everything else goes
out only when you start it: the agent you run, `git`, and GitHub when you sign in.

## Shortcuts

| Action | Keys |
| --- | --- |
| Go to File / Find in Files | `Ctrl+P` / `Ctrl+Shift+F` |
| Open File / Open Folder | `Ctrl+O` / `Ctrl+K Ctrl+O` |
| Save / Save All / Close tab | `Ctrl+S` / `Ctrl+Alt+S` / `Ctrl+W` |
| Agent panel | `Ctrl+L` |
| Explorer / Source Control / Run | `Ctrl+Shift+E` / `Ctrl+Shift+G` / `Ctrl+Shift+D` |
| Side bar / Panel / Terminal | `Ctrl+B` / `Ctrl+J` / ``Ctrl+` `` |
| Run file / Stop | `F5` / `Shift+F5` |
| Settings / Next theme | `Ctrl+,` / `Ctrl+K Ctrl+T` |

## Build from source

Rust stable, then:

```sh
cargo build --release          # target/release/fluxcode(.exe)
cargo test --bin fluxcode
```

- Windows: HLSL shaders need `fxc.exe` from the Windows SDK, or set `GPUI_FXC_PATH` to
  `tools/fxc-shim` built with `cargo build --release --manifest-path tools/fxc-shim/Cargo.toml`.
  With the GNU toolchain, use a `CARGO_TARGET_DIR` without spaces (MinGW `windres`).
  `packaging/windows/build.ps1 -Install` builds, packs and installs like a release.
- Linux: `pkg-config clang libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev
  libx11-xcb-dev libxcb1-dev libfontconfig1-dev libfreetype6-dev libvulkan-dev` (Debian
  and Ubuntu names).
- Builds name themselves from `FLUXCODE_BUILD` (`BuildDev-YYYY-MM-DD-HH-MM-SS`); without
  it, from the current UTC time.

## Contributing

FluxCode is open source, and issues are open to everyone: report a bug or suggest an idea
through [Issues](https://github.com/MuaraAI/FluxCode/issues/new/choose). A bot checks each
new issue for earlier reports of the same problem and for an outdated build.

Pull requests are accepted only from members of the MuaraAI organization; others are
closed automatically. Details in [CONTRIBUTING.md](CONTRIBUTING.md). What changed in each
build: [CHANGELOG.md](CHANGELOG.md).

## License

[GPL-3.0-or-later](LICENSE). Third-party components: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
