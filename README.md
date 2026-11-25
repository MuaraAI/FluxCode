<!-- Made from README.flux by FluxCode (fluxcode --flux-to-md README.flux README.md). Edit README.flux, not this file. -->

# FluxCode

**English** · [Bahasa Indonesia](README.id.md) · [Roadmap (2026-2027)](ROADMAP.md) · [Discussions](https://github.com/MuaraAI/FluxCode/discussions)

FluxCode is a native, GPU-rendered code editor for Windows, macOS, and Linux written in
Rust. It ships as a single 16 MB binary with an integrated AI agent workspace, a
Python-aware terminal, built-in Git, and instant read-only views for binary, archive,
database, and Office files. It opens a window in ~50 ms, idles at 1.4 MB of RAM in one
process ([benchmark](#benchmark)), and sends zero telemetry.

Built at [MuaraAI](https://github.com/MuaraAI) by engineers with deep Rust systems
experience and over six years of building and running production SaaS products. Open source
under GPL-3.0-or-later and actively developed in the open, with continuous builds from
`main` and a public [2026-2027 Roadmap](ROADMAP.md) (security hardening, opt-in LSP, and
native Android in 2027).

[![Latest build](https://img.shields.io/github/v/release/MuaraAI/FluxCode?display_name=tag&label=latest%20build)](https://github.com/MuaraAI/FluxCode/releases/latest)
[![Last commit](https://img.shields.io/github/last-commit/MuaraAI/FluxCode?label=last%20commit)](https://github.com/MuaraAI/FluxCode/commits/main)
[![Build](https://github.com/MuaraAI/FluxCode/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/MuaraAI/FluxCode/actions/workflows/build.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

<a href="assets/screenshots/agent-workspace.png"><img src="assets/screenshots/hero.jpg" width="100%" alt="A 16 MB code editor with AI agents built in: the title over a dusk sky, and below it FluxCode with the agent filling the window, rising out of a bank of clouds. Saved chats on the left, the conversation in the middle, the files it changed on the right."></a>

Full-screen agent workspace: saved threads on the left, active conversation in the center,
and live file diffs on the right, floating over a GPU-drawn background. When you're deep
in code, hit `Ctrl+L` to dock the agent as a compact side panel.

## Benchmark

On a standard Windows 11 laptop, FluxCode puts its first frame on screen in **~50 ms** and
drops to **1.4 MB** in Task Manager at idle, all inside a single process. On the same
machine, Zed takes 818 ms and 66.2 MB; VS Code takes ~3 s and 929.2 MB across nine
processes.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/benchmarks/benchmark-dark.svg">
  <img src="assets/benchmarks/benchmark-light.svg" width="100%" alt="Six bar charts, lower is better in each. Window on screen after launch: FluxCode about 50 ms (timed by hand), Zed 818 ms, VS Code 3,000 ms. Memory in Task Manager when idle: FluxCode 1.4 MB, Zed 66.2 MB, VS Code 929.2 MB. Memory reserved: FluxCode 72 MB, Zed 154 MB, VS Code 1,315 MB. CPU while idle: FluxCode 0.05%, Zed 1.46%, VS Code 5.89% of one core. Processes: FluxCode 1, Zed 2, VS Code 9. Size on disk: FluxCode 16 MB, Zed 261 MB, VS Code 1,044 MB.">
</picture>

The difference shows up where your editor spends most of its life: sitting open in the
background while you think, read docs, or run builds. FluxCode idles at 1.4 MB of RAM
(vs. 66.2 MB for Zed and 929.2 MB for VS Code) and takes just 16 MB on disk. Lower is
better across every chart above.

*Median of 5 runs each on a PC with an AMD Ryzen 5 7535HS and 15 GB of RAM, 10 November 2026. Every
app started from a fresh, empty profile with telemetry and update checks off and no folder
open.*

<details><summary>Exact numbers, method, and what this does not measure</summary>

Median, with the lowest and highest of the runs in brackets:

| | FluxCode BuildDev-2026-11-10-04-41-34 | Zed 1.27.0 | VS Code 1.145.2 |
| --- | --- | --- | --- |
| Window on screen after launch (ms) | about 50, timed by hand (bench.ps1: 221) | 818 (387 to 1,322) | 3,000 |
| Memory in Task Manager, idle (MB) | 1.4 (1.0 to 4.8) | 66.2 (65.2 to 71.9) | 929.2 (867.7 to 985.4) |
| Memory reserved (MB) | 72 (72 to 72) | 154 (153 to 156) | 1,315 (1,249 to 1,399) |
| CPU while idle (% of one core) | 0.05 (0.00 to 0.68) | 1.46 (1.15 to 2.60) | 5.89 (5.57 to 8.12) |
| Processes running | 1 | 2 | 9 |
| Size on disk (MB) | 16 | 261 | 1,044 |

- **Window**: from launch to the first visible window, runs 2 to 5 (run 1 reads the program
  from disk cold). VS Code's figure is the time until the workbench is usable, about
  3 s: its window shows before the workbench inside it has loaded.
- **FluxCode's window time** is about 50 ms, timed by hand by the maintainer. `bench.ps1`
  logs 221 ms for it, but its PowerShell loop calls `Get-Process` on every poll, so that
  figure includes the script's own time. Zed's and VS Code's figures come from the same
  script.
- **Memory in Task Manager**: the private working set, 9 s after launch, summed over every
  process of the app. FluxCode hands memory it isn't using back to Windows 6 s after start,
  so this is its idle figure; it is about 5 MB after a click and about 30 MB while the agent
  install animation runs. Zed and VS Code keep theirs.
- **Memory reserved**: private bytes (commit), the memory the app has claimed whether or
  not it is in RAM right now.
- **CPU while idle**: CPU time from 9 s to 39 s after launch, as a share of one core.
  Windows counts it in 15.6 ms steps, so FluxCode's 0.05% is at the limit of what it can
  tell apart from zero.
- **Size on disk**: the install folder; for FluxCode, its one program file.
- **Not measured here**: typing latency, opening very large files, working in a big
  project, or anything with extensions and language servers loaded. Zed and VS Code do
  more in those areas (see the comparison table below: FluxCode has no debugger,
  extensions or language servers yet).
- **Reproduce**: close all three apps, then run
  `powershell -ExecutionPolicy Bypass -File tools/bench/bench.ps1 -FluxExe <path to fluxcode.exe>`
  and `python tools/bench/chart.py`. The raw runs are in
  [`tools/bench/results.json`](tools/bench/results.json).

</details>

## Download

Grab the latest build from [Releases](https://github.com/MuaraAI/FluxCode/releases/latest).

| System | File | Install |
| --- | --- | --- |
| Windows 10/11, x64 | `FluxCode-BuildDev-...-windows-x64-setup.exe` | Run the installer. No admin rights required: installs per-user with Start Menu and Desktop shortcuts. |
| macOS 11+, Apple Silicon and Intel (preview) | `FluxCode-BuildDev-...-macos-universal.dmg` | Open the DMG and drag FluxCode into Applications. |
| Linux, x64 (glibc 2.35+, Vulkan) | `FluxCode-BuildDev-...-linux-x64.tar.gz` | Extract and run `./install.sh` (no root required). |
| Windows, portable | `FluxCode-BuildDev-...-windows-x64.zip` | Extract anywhere and run `FluxCode.exe` directly (`Install.cmd` is included if you want to install later). |

The macOS build is currently a preview and not yet notarized by Apple, so macOS will ask
once on first launch (on macOS 15+, open it and click **System Settings > Privacy &
Security > Open Anyway**; on older versions, right-click FluxCode > **Open**). If you hit
any macOS-specific quirks, [open an issue](https://github.com/MuaraAI/FluxCode/issues/new).

We ship continuous `BuildDev-YYYY-MM-DD-HH-MM-SS` builds straight from `main`. Installed
copies notify you when a new build lands and update in one click (you can turn update
checks off anytime under **Settings > Updates**).

## A closer look

<table>
  <tr>
    <td width="50%" valign="top">
      <a href="assets/screenshots/token-saver.png"><img src="assets/screenshots/token-saver.png" width="100%" alt="The token saver panel over the chat: Caveman on at the Full level, Lean code off, Quiet tools on, about 36 percent fewer output tokens."></a>
      <br><sub><b>Token saver</b></sub>
    </td>
    <td width="50%" valign="top">
      <a href="assets/screenshots/usage-limit.png"><img src="assets/screenshots/usage-limit.png" width="100%" alt="A banner above the message box: Usage limit reached, resets 15.45 in 2 hours, with a Resume at reset checkbox and a Continue now button."></a>
      <br><sub><b>When a usage limit is reached</b></sub>
    </td>
  </tr>
</table>

**Token saver** (the leaf icon under the prompt box) trims verbose agent prose, favors
focused diffs over full-file dumps, and keeps tool calls quiet. And when you hit a provider's
rate or quota cap, FluxCode parses the reset window so you can either **Continue now** or
tick **Resume at reset** and let it pick right back up automatically.

<table>
  <tr>
    <td width="50%" valign="top">
      <a href="assets/screenshots/side-bars-search.png"><img src="assets/screenshots/side-bars-search.png" width="100%" alt="Search moved to the right side bar, with the Explorer on the left: typing right_view lists 23 results in 4 files as you type, each match tinted, long files folded to six lines."></a>
      <br><sub><b>Search in either side bar</b></sub>
    </td>
    <td width="50%" valign="top">
      <a href="assets/screenshots/extensions-marketplace.png"><img src="assets/screenshots/extensions-marketplace.png" width="100%" alt="The Extensions view searching the VS Code Marketplace for prettier: 256 results with icons, installs, ratings and Install buttons, and Prettier's page with its details and README."></a>
      <br><sub><b>The whole VS Code Marketplace</b></sub>
    </td>
  </tr>
</table>

Right-click any activity icon to dock that view on the left or the right. Project-wide
search streams results as you type, and the Extensions view searches and installs directly
from the VS Code Marketplace.

<a href="assets/screenshots/backgrounds.png"><img src="assets/screenshots/backgrounds.png" width="100%" alt="The twelve animated backgrounds in the FluxDark theme, drawn live: Clouds at dusk, a moonlit Ocean, Mountains in fog, an Aurora over snowy ridges, a pine Forest with fireflies, a Lake between hills, a Beach at sunset, Dunes under a low sun, a City skyline with lit windows, Rain, Snow, and a Galaxy with a ringed planet."></a>

<table>
  <tr>
    <td width="50%" valign="top">
      <a href="assets/screenshots/settings-backgrounds.png"><img src="assets/screenshots/settings-backgrounds.png" width="100%" alt="Settings, Appearance: Animated background as a grid of twelve live previews (Clouds, Ocean, Mountains, Aurora, Forest, Lake, Beach, Dunes, City, Rain, Snow, Galaxy), each drawn in the FluxSky theme's colors, with Clouds chosen; Background animation and Background visibility below."></a>
      <br><sub><b>Twelve backgrounds, previewed live</b></sub>
    </td>
    <td width="50%" valign="top">
      <a href="assets/screenshots/backgrounds-themes.png"><img src="assets/screenshots/backgrounds-themes.png" width="100%" alt="The Mountains background in four themes: FluxDark, FluxRed and FluxMuara Dark at night, FluxCream as a pale day scene."></a>
      <br><sub><b>One scene in four themes</b></sub>
    </td>
  </tr>
</table>

Twelve procedural scenes (clouds, ocean, mountains, aurora, forest, lake, beach, dunes,
city, rain, snow, galaxy) rendered straight from your active theme palette, with zero
external assets to download. By default they stay static at 0% CPU; if you flip on
**Background animation**, FluxCode benchmarks its own frame cost on your hardware first and
tells you honestly how light or heavy it runs.

## What makes it different

- **Muscle-memory compatible with VS Code.** Same layout (activity bar, sidebars, tabs,
  bottom panel, status bar) and the same keybindings you already use (`Ctrl+Shift+P`,
  `Ctrl+P`, `Ctrl+G`, `Ctrl+/`, `Alt+Up/Down`). It isn't a bloated Electron clone: there's
  no Node runtime in the editor process, and VS Code color themes & grammars load natively
  in Rust without running extension JavaScript.
- **Systems-level efficiency.** Built in Rust on top of GPUI (the GPU UI framework behind
  Zed) with custom Windows D3D11 swap-chain & path-texture optimizations and incremental
  per-line syntax highlighting. In our [benchmark](#benchmark), it opens a window in ~50 ms,
  reserves 72 MB, and trims its idle working set down to 1.4 MB.
- **First-class AI agent workflow (ACP).** Speaks the Agent Client Protocol out of the box
  and supports every agent in the ACP registry (OpenCode, Claude Code, Codex, Gemini CLI,
  Copilot, Cursor, Qwen, Command Code, and more). Install missing agents in one click, pick
  models and effort levels inline, review file diffs per turn, or roll changes back with
  one click.
- **GitHub Copilot ghost-text completions.** Talks to the official
  `@github/copilot-language-server` over stdio in a separate process so the editor stays
  tiny and responsive. Install once with `npm install -g @github/copilot-language-server`
  and sign in under **Settings > AI agents and GitHub**.
- **Crafted by seasoned Rust & SaaS engineers.** Designed by developers who have written
  Rust in production and shipped commercial SaaS platforms for the past 6+ years: approachable
  enough for a beginner with one-click setup and read-only safety modes, yet tunable down to
  every editor, font, keybinding, and per-agent instruction file for veterans.

## Extensions and FluxCompile

The Extensions view (`Ctrl+Shift+X`) is the VS Code Marketplace: all of it, loaded a page
at a time as you scroll, searched as you type (an earlier search comes back instantly),
sorted by relevance, installs, rating, last update or name, and filtered by category.
Install puts the extension's VSIX into `~/.fluxcode/vscode-extensions`; Update and
Uninstall are on its page, and the folder button installs a local `.vsix` file. VS Code's
JavaScript extension host isn't part of FluxCode, so an extension's own code doesn't run
here; its page says what it declares (themes, languages, snippets).

FluxCode's own project support is always on, with nothing to install: Paper plugins,
Bedrock add-ons, Gradle, Maven, Node.js, Rust, Python, Go, C/C++ (CMake and Make), .NET and
the HTML preview. Each is one small JSON manifest: the files that mark its projects, the
tasks it adds, the tools it needs (with a winget id to install them) and notes for the AI
agent. The Run view lists the folder's projects with their tasks, and the agent is told
what each project is and how it builds.

- **Paper plugins (Minecraft Java).** Build, Build & deploy, Start server, Test, Clean. The
  project's name is its plugin's name, read from the `plugin.yml` inside the built jar. Deploy
  copies the jar into the test server's `plugins` folder as `<Name>-<version>.jar` and
  removes older builds of the same plugin (found by the name inside each jar). The server is
  the folder above the plugin with a `plugins` folder and a `paper-*.jar`. New plugin writes
  a Gradle project for that server's version, with the Gradle wrapper.
- **Bedrock add-ons.** Check manifests (UUIDs, versions, module types, script entries,
  dependencies between the packs), Deploy to Minecraft (into the game's development pack
  folders) and Pack `.mcaddon`, which FluxCode zips itself. New add-on writes a behavior
  and a resource pack that depend on each other.
- **FluxCompile** is how FluxCode runs a Java build. It uses the JDK the project asks for
  (a Gradle toolchain or Maven `release`, found in the usual install folders or a
  `tools/jdk-*` folder next to the project), Gradle's local build cache unless the project
  turns it off, and the project's own settings for parallel builds and the configuration
  cache. It folds Gradle's task lines into one progress line and ends with what was built.
  A compiler error opens the file at its line. Forcing the configuration cache or parallel
  builds from outside can break a build, so FluxCompile doesn't; we checked how 28 large
  plugins (LuckPerms, EssentialsX, WorldEdit, Geyser, ViaVersion and others) build before
  choosing.
- Measured on a multi-module Paper plugin (55 Gradle tasks, a 609 KB jar): the first
  build took 14.6 s (40 tasks run, 15 from the cache), a build with nothing changed 1.9 s
  with Gradle's daemon warm.

## Flux documents

This README is written in Flux (`README.flux`); the `README.md` GitHub shows is made from
it. A `.flux` file is Markdown's text plus one-line blocks for what Markdown leaves to
HTML: image width and dark-theme versions, images side by side, video, flowcharts,
callouts, collapsible sections, columns, a table of contents and sections per language.
FluxCode opens it drawn, with Preview and Source above it.

- **Light at any size**: one pass over the text, nothing copied, only the rows on screen
  drawn, and a large document's text kept packed. A 1.3-million-line document (31.8 MB, a
  200,000-row table in it) shows its first screen after 0.08 ms of reading, opens in
  0.3 s, and shows 16 MB in Task Manager (its text kept in 2.3 MB).
- **Readable on GitHub**: `fluxcode --flux-to-md README.flux README.md` (or **Save as
  Markdown** in FluxCode) writes GitHub Markdown; flowcharts become Mermaid and callouts
  GitHub's alerts. Add `--lang id` for the Indonesian sections, which ship as
  `README.id.md`. The tests fail when any of those files disagree.

> [!TIP]
> **The whole syntax**
> [docs/flux.md](docs/flux.md) describes every block, written in Flux itself
> ([docs/flux.flux](docs/flux.flux)).

## What else it does

- Syntax colors for about 200 languages, picked by file name, extension or `#!` line.
- Multi-root workspaces: drag a folder onto the window to add it or switch to it.
- The agent panel (`Ctrl+L`) runs any agent in the [ACP registry](https://agentclientprotocol.com),
  such as OpenCode 2, Claude Agent, Codex, Gemini CLI or GitHub Copilot. An
  agent that isn't on your PC has a download button in the agent menu (with the agent's own
  icon from the registry); FluxCode puts it in
  `~/.fluxcode/agents` (npm packages too, not in your global npm) and starts it only while
  you use the panel. Model, effort, mode and permissions are buttons under the message
  box; each opens a list right above it with a search box, and the agent menu says which
  agent is on. GitHub Copilot, which offers no models over ACP, gets its model list from
  Copilot itself and restarts with the one you pick, in the same chat. Edit a sent message
  and the answer restarts from it; the agent gets an overview of the workspace. Memory is
  the agent's own (FluxCode adds none of its own).
- **Effort and models, with what they cost.** Effort is a slider (low to max) that starts at
  medium: drag it, or click a level. Each model shows how many tokens it reads at once and
  writes in one answer, and free models are marked Free (from the model list OpenCode keeps
  on your PC). Claude Agent's models are grouped by generation, newest first, each with its
  price per million tokens, since the newer ones do the same work with fewer tokens.
- **Usage without typing `/usage`.** A meter under the message box shows how full the
  context is; it, the gauge button or typing `/usage` opens a panel with the plan's limits
  (the 5-hour and weekly ones), when each resets, the context window, the session's cost and
  what uses the limits most, all read from the agent itself. Limits reset by themselves;
  Request usage credits asks first, then asks the plan for more and shows the answer.
- **Web searches you can follow.** When the agent searches the web or reads a page, the
  chat shows the query and its sources, each one click away.
- **What the agent is doing, as it does it.** Thinking, reading, editing, running a command
  and searching are steps on one line: a running step has a thin moving line under it, and
  when it ends its label turns from "Reading" to "Read" and shows how long it took.
- **A browser for every agent.** FluxCode hands each agent session a browser it can
  drive: the Chrome, Brave or Edge already on your PC (Chrome when it is there), through
  Google's [chrome-devtools-mcp](https://github.com/ChromeDevTools/chrome-devtools-mcp)
  over the Chrome DevTools Protocol. The agent opens pages, clicks and types, reads the
  console and network requests, takes screenshots and accessibility snapshots, and records
  performance traces. Nothing is downloaded but the 15 MB tool, installed once into
  `~/.fluxcode/tools/browser`; the browser gets a profile of its own, never your cookies
  or passwords, and Google's usage statistics are turned off. Agent settings > Browser
  picks the browser, a window or none, or turns it off.
- Source Control commits, pushes, pulls and syncs, shows a commit graph, opens pull
  requests and GitHub Actions runs through `gh`, and Remove Git undoes a `git init`.
- Source Control tells you when the remote has commits you don't. It runs a quiet
  `git fetch` every five minutes (and when you come back to the window), never asking
  for a password, and an Incoming section at the top lists the new commits: title and
  author, then the files each one changed, and a file's diff in the Output panel. Pull
  brings them in, or put them aside until more arrive. A blue dot on the Source Control
  icon says they are there.
- After an update, a "What's new" page opens once with the notes since your last build
  (from [`changelog.flux`](changelog.flux), in the interface language). Help > What's New
  opens it again.
- OpenCode 2 is the registry's native OpenCode build from opencode.ai, which FluxCode
  installs and checks against its SHA-256. An OpenCode 2 you already installed (npm's
  `@opencode/cli`, or the opencode.ai installer) is used as it is instead; OpenCode 1 is
  not. An existing OpenCode choice moves to OpenCode 2 with its settings.
- Agents already on your PC are found the way Windows finds programs: on the PATH as
  Windows has it now and in the folders installers use (npm, pnpm, bun, Scoop, WinGet,
  `~/.local/bin`), so nothing is downloaded twice.
- The color theme's name in the status bar opens the seven themes, each shown as a small
  editor in its colors.
- Markdown files draw themselves: a README opens as text, and an eye icon above it
  turns it into the drawn page GitHub shows (headings, tables, lists, local images,
  GitHub's `> [!NOTE]` callouts), with Source to go back. Flux documents keep their
  own Preview and Source.
- Settings > About FluxCode links to this repository, the issue tracker and the changelog.
- **Side bars on either side.** Explorer, Search, Source Control, Run, Extensions and the
  agent can each live in the left or the right side bar, as in VS Code's secondary side
  bar: right-click an activity bar icon to move it. The right side bar's header switches
  between the views that live there.
- **Find in files while you type.** Results update after a short pause in typing, each
  match is tinted, and a file with many matches folds to its first six lines with an
  "N more matches" row. In a git repository only the files git tracks or would not ignore
  are read, on several threads.
- **The agent's full-screen workspace.** The full-screen button in the agent's header turns
  it into three columns: chats, the conversation, and the files it changed with their
  diffs and undo. The columns step aside on narrow windows.
- **Token saver.** Caveman (Lite, Full or Ultra) asks for shorter answers, Lean code for
  diffs instead of whole files, Quiet tools for reading and printing only what the task
  needs. The guidance goes out with a message when the packs change and every few turns,
  so it costs almost nothing itself.
- **Usage limit reached.** A banner above the message box shows when the limit resets.
  Continue now sends the cut-off message again; Resume at reset does it by itself once the
  limit window has reset.
- **Disk Usage (WizTree) for AI agents.** Built-in Rust disk tree and treemap analyzer
  that feeds a compact folder hierarchy (largest, smallest, waste hotspots, extensions,
  and largest files) directly to the agent via `@wiztree`, **Ask Agent**, or
  `fluxcode --wiztree`, so agents inspect workspace storage without manual directory scans.
- **Minecraft Bedrock data.** Open and edit binary `level.dat`, `level.dat_old`,
  `.mcstructure`, and `.nbt` files as clean SNBT with lossless binary save, plus read-only
  inspection of LevelDB `db/*.log` and `db/*.ldb` files.
- **Multi-language terminal.** Parses errors and stack traces across Python, JavaScript,
  TypeScript, Rust, Go, Java, Kotlin, C/C++, C#, Zig, PHP, Ruby, Lua, Dart, Swift, Shell,
  and PowerShell with one-click file:line:col jump links and inline Tab cycling.
- **Liquid glass and animated backgrounds.** Panels can turn translucent over the system's
  blur, with floating glass cards. Twelve animated backgrounds play behind them, drawn
  live in the theme's colors (no video and no download). They stand still by default;
  Background animation makes them move 24 frames a second while the window is in front
  (about 10 to 15% of one CPU core) and a few behind other windows, and turning it on
  first measures what your PC can take.

## FluxCode, Zed and VS Code

All three are good editors made for different people. This is where each stands today.

FluxCode is the lightest of the three by a wide margin, and the gap grows the longer the
editor stays open. It uses 1.4 MB idle against 66.2 MB for Zed and 929.2 MB for VS Code,
and takes 16 MB on disk against 261 MB and 1,044 MB. It opens in about 50 ms, against 818 ms
and about 3 s. What it gives up is clear in the table: no debugger, no language servers,
and VS Code extensions install but their code doesn't run.

| | FluxCode | Zed | VS Code |
| --- | --- | --- | --- |
| Built with | Rust, GPUI (GPU-drawn) | Rust, GPUI (GPU-drawn) | TypeScript on Electron (Chromium + Node.js) |
| Systems | Windows, macOS, Linux | macOS, Linux, Windows | Windows, macOS, Linux, web |
| What you download | one program, about 16 MB | an installer | an installer |
| Layout and keys | VS Code's layout and VS Code's keys for the commands it has | its own layout; a VS Code key map is available | the original |
| Start, memory, CPU, size | about 50 ms, 1.4 MB idle, 1 process, 16 MB ([benchmark](#benchmark)) | 0.82 s, 66 MB, 2 processes, 261 MB | about 3 s, 929 MB, 9 processes, 1 GB |
| Telemetry | none | on by default, can be turned off | on by default in Microsoft's build, can be turned off |
| Language smarts (go to definition, rename, diagnostics) | not yet: colors and word completion | built in, through language servers | through extensions |
| Debugger | not yet (Run only) | yes | yes |
| Extensions | the VS Code Marketplace to browse and install (their code doesn't run), plus 11 built-in project manifests | yes | the largest catalog |
| AI agent | any ACP agent, three access levels, per-turn changes with undo; GitHub Copilot inline completions | its own agent and ACP agents | GitHub Copilot and extensions |
| Files that are not text | hex, PDF, Office, ZIP, SQLite, LevelDB (read-only); Bedrock/Java NBT (editable) | images | images; more through extensions |
| Remote development | WSL2 (Windows) | SSH | SSH, containers, WSL |
| Interface languages | English, Bahasa Indonesia | English | many, through language packs |
| Maturity | development builds | stable releases | stable, widely used |
| License | GPL-3.0-or-later | GPL-3.0 (the editor) | MIT for Code - OSS; Microsoft's build has its own license |

Pick FluxCode if you want a small, quiet editor on Windows or Linux with an agent you can
keep on a short leash, your code can live in a WSL2 distro, and you open logs, databases
and documents next to your code.
Pick Zed for macOS, language servers out of the box and pair programming.
Pick VS Code when you need a specific extension, a debugger or remote development beyond WSL.

## Other editors at a glance

These editors were not in the benchmark, so this table has no measured numbers for
them. It compares only what each project documents: what it is built with, where it
runs, and what ships built in.

| Editor | Built with | Systems | Language servers | Debugger | Extensions | AI agent |
| --- | --- | --- | --- | --- | --- | --- |
| Sublime Text | C++ | Windows, macOS, Linux | through the LSP package, not built in | none built in | Python plugins, through Package Control | none built in |
| Notepad++ | C++ (Win32, Scintilla) | Windows only | through plugins, not built in | none built in | plugins, through Plugin Admin | none built in |
| Lapce | Rust | Windows, macOS, Linux | built in | none built in | WASI plugins | none built in |
| Helix | Rust | terminal, on Windows, macOS and Linux | built in | built in, through DAP (experimental) | none yet | none built in |
| Neovim | C, with Lua | terminal, on Windows, macOS and Linux | built in | through plugins, not built in | Lua plugins, a large ecosystem | none built in, through plugins |

FluxCode sits closest to Lapce and Helix in spirit: a small Rust editor that stays out
of the way. What sets it apart from them is the graphical interface, the VS Code
layout and keys, and the agent panel. None of the five editors above was measured in
the benchmark, so do not read this table as a speed ranking.

## Privacy

FluxCode sends nothing about you. The only request it makes by itself is the update
check: a plain `GET` of the release list from github.com, with no build ID, machine ID or
cookie, at start and every 6 hours (Settings > Updates turns it off). Everything else goes
out only when you start it: the agent you run, `git`, GitHub when you sign in, and the new
build's `changelog.flux` when you click What's new on the update card. The
agent menu reads the public ACP agent list from cdn.agentclientprotocol.com, at most once
a day and only when you open the menu.
The first time an agent starts, FluxCode installs the agents' browser tool
(chrome-devtools-mcp) from the npm registry; Agent settings > Browser > Off skips it.
Extensions download nothing by themselves: Add Gradle wrapper asks services.gradle.org for
the current version and fetches the wrapper from Gradle's GitHub repository, and an
Install button on an extension's page runs `winget` in the Terminal, only when you click.
The Extensions view asks marketplace.visualstudio.com for the list while it is open, and
downloads an extension's VSIX only when you click Install.

## Shortcuts

VS Code's keys on Windows, macOS and Linux. `Ctrl+Shift+P` (or `F1`) lists every command
with its keys, and Settings > Keyboard shortcuts shows the same list.

| Action | Keys |
| --- | --- |
| Command Palette | `Ctrl+Shift+P` / `F1` |
| Go to File / Go to Line | `Ctrl+P` / `Ctrl+G` |
| Find / Replace / Find in Files | `Ctrl+F` / `Ctrl+H` / `Ctrl+Shift+F` |
| Open File / Open Folder | `Ctrl+O` / `Ctrl+K Ctrl+O` |
| Save / Save All | `Ctrl+S` / `Ctrl+K S` |
| Close tab / Close all / Reopen closed | `Ctrl+W` / `Ctrl+K Ctrl+W` / `Ctrl+Shift+T` |
| Next / previous tab | `Ctrl+Tab` or `Ctrl+PageDown` / `Ctrl+Shift+Tab` or `Ctrl+PageUp` |
| Toggle line comment | `Ctrl+/` |
| Move line up / down | `Alt+Up` / `Alt+Down` |
| Copy line up / down | `Shift+Alt+Up` / `Shift+Alt+Down` |
| Add cursor above / below | `Ctrl+Alt+Up` / `Ctrl+Alt+Down` |
| Delete line | `Ctrl+Shift+K` |
| New line below / above | `Ctrl+Enter` / `Ctrl+Shift+Enter` |
| Indent / outdent | `Ctrl+]` / `Ctrl+[` |
| Word wrap | `Alt+Z` |
| Editor font bigger / smaller / reset | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` |
| Side bar / Panel / Terminal | `Ctrl+B` / `Ctrl+J` / ``Ctrl+` `` |
| Explorer / Source Control / Run / Extensions | `Ctrl+Shift+E` / `Ctrl+Shift+G` / `Ctrl+Shift+D` / `Ctrl+Shift+X` |
| Run file / Stop | `F5` / `Shift+F5` |
| Agent panel | `Ctrl+L` |
| Settings / Next theme | `Ctrl+,` / `Ctrl+K Ctrl+T` |

`Ctrl+L` opens the agent (as in Cursor) where VS Code would select a line.

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
- macOS: Xcode (for the Metal shaders). `packaging/macos/bundle.sh BUILD BINARY OUT` packs
  a binary as `FluxCode.app`, a `.dmg` and the update archive, like a release.
- Builds name themselves from `FLUXCODE_BUILD` (`BuildDev-YYYY-MM-DD-HH-MM-SS`); without
  it, from the current UTC time.

## Contributing

FluxCode is open source, and issues are open to everyone: report a bug or suggest an idea
through [Issues](https://github.com/MuaraAI/FluxCode/issues/new/choose). A bot checks each
new issue for earlier reports of the same problem and for an outdated build.

Pull requests are welcome from anyone. Fork the repository, branch from `main`, and open
a pull request; for a new feature or a new dependency, open an issue first so the idea
can be agreed on before the code is written.

Every pull request is checked automatically, and every push is checked again:

| Check | What it looks at | Fails when |
| --- | --- | --- |
| **Safety scan** | Every added line and file, read without running anything: malware patterns (download-and-run, reverse shells, miners, credential theft, process injection), leaked tokens and keys, invisible or right-to-left characters, encoded blobs, committed executables, dependencies from outside crates.io | Anything malicious or secret turns up, or someone outside the team changes CI, build or packaging files |
| **Performance** | The pull request and `main`, built the same way on the same runner and run 40 times each: executable size, start-up time, Markdown conversion time, peak memory, crate count | Size grows more than 10%, a run gets more than 25% slower, or memory grows more than 40% (smaller changes are flagged as warnings) |
| **Code quality** | Only the lines the pull request adds: clippy, rustfmt, `unwrap()`, `dbg!`, `thread::sleep`, very long lines and files | A clippy `correctness`, `suspicious` or `perf` lint, a compile error or a leftover `dbg!` |
| **Dependencies** | The RustSec advisory database, through cargo-deny | A dependency brings in an advisory `main` does not already have |
| **Build** | Windows, Linux and macOS builds, unit tests, install and start | Anything does not build, a test fails, or the app does not start |

The limits are read from `main`, so a pull request cannot loosen its own. The checks
help; they do not merge anything: a maintainer reads every pull request first. The full
list, how to run them locally and what each warning means are in
[CONTRIBUTING.md](CONTRIBUTING.md). What changed in each build:
[CHANGELOG.md](CHANGELOG.md).

## Roadmap

FluxCode is under active, continuous development with hard CI budgets on binary size,
startup speed, and memory. Here is where we are headed (full checklist and technical scope
in [ROADMAP.md](ROADMAP.md)):

| Phase | Target | Focus |
| --- | --- | --- |
| **Foundation** | Shipped (2026) | Single-process Rust + GPUI editor (~50 ms startup, 1.4 MB idle RAM), ACP agent workspace, Copilot ghost text, Git, WizTree AI context, Bedrock NBT, multi-language terminal, WSL2 helper, Markdown/`.flux` preview with zoom & annotations |
| **Phase 1: Hardening** | Late 2026 - Q1 2027 | Workspace trust model, SHA-256 verified self-updates, OS credential store for tokens, ACP path containment, dirty-tab close guards |
| **Phase 2: Language Intelligence** | Q1 - Q2 2027 | Opt-in lightweight LSP client (`rust-analyzer`, `pyright`, `tsserver`, `gopls`), inline diagnostics & go-to-definition, multi-cursor & split editors, notarized macOS builds |
| **Phase 3: FluxCode for Android** | 2027 | Native `aarch64-linux-android` Vulkan build (zero WebView), touch & Android IME ergonomics, remote desktop/SSH + ACP agent offload, signed APK / F-Droid / Play Store release ([#117](https://github.com/MuaraAI/FluxCode/discussions/117)) |
| **Phase 4: Extensibility & v1.0** | Late 2027+ | Sandboxed WASM extensions, first-class Remote SSH workspaces, semantic `v1.0` release channels |

## The icon

The mark is a meander: one unbroken line that turns only at right angles and never
crosses itself. The pattern is named after the river Maeander in western Anatolia (the
Büyük Menderes today). The Greek geographer Strabo wrote that it winds so much that
every winding course came to be called after it. One of the oldest known meanders is
carved into mammoth-ivory bracelets found at Mezin, in Ukraine, more than 15,000 years
ago; Greek potters of the Geometric period later painted it around whole vases.

FluxCode takes three things from it. The name: a flux is a flow, and the meander is a
river drawn as a rule. The structure: code also runs as one path that turns into a
block and comes back out, the way a call returns to where it started. The colors: ivory
for the Mezin carvings and ochre for fired clay, on ink. The line is split at its center
into two hooks of the same shape, turned 180 degrees and holding each other, so the mark
looks the same upside down. The app's FluxDark and FluxCream themes use the same three
colors.

## License

Copyright (C) 2026 MuaraAI and contributors.

[GPL-3.0-or-later](LICENSE): everyone is free to use, study, change and share
FluxCode, and every copy carries the same freedom. Third-party components:
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
