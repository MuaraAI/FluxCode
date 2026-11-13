# Changelog

FluxCode is in development and has no version numbers. Each build is named after the
moment it was made, `BuildDev-YYYY-MM-DD-HH-MM-SS` (Asia/Jakarta time). Every change
merged into `main` becomes a build on the [Releases](https://github.com/MuaraAI/FluxCode/releases)
page, and installed copies offer it on their own. Newest first.

## WSL2 workspaces: the helper inside the distro (13 November 2026)

- A WSL2 folder now reads and writes through a small helper running inside the
  distro, instead of reaching it over Windows' 9p share. Opening a folder, walking
  the tree, opening and saving a file, find-in-files and git all go through it.
  It is off by default: turn it on in **Settings > WSL**.
- The helper is FluxCode itself, built for Linux, taken from the release of the
  build that is running: the two are always the same version, and nothing extra
  ships. It is installed once per build in `~/.fluxcode/helper/<build>/`.
- If the helper cannot be installed or started, everything falls back to the
  `\\wsl.localhost\...` path, so a WSL folder still opens and saves.

## WSL2 workspaces (13 November 2026)

- A folder that lives inside a WSL2 distro opens like any other. Files are read and
  written through `\\wsl.localhost\...`, while the terminal, Run and git run inside the
  distro, so the paths, line endings and toolchains are the Linux ones.
- **File > Open Folder in WSL...** lists the installed distros and opens the chosen one's
  home folder. The status bar says `WSL: Ubuntu` while such a folder is open.
- What it does not do yet: no language server (FluxCode has none), no interactive terminal
  (the terminal runs one line at a time), and no file watcher (there is none). A folder
  under `/mnt/c` works but is slow, so keep the repo in the Linux filesystem.

## Big files no longer freeze the window (13 November 2026)

- Opening or saving a file over 256 KB now happens on a background thread, so the
  window keeps drawing while a large file, a network share or a slow disk is read.
  Small files still open the moment you click them, and a save that finishes late
  no longer hides newer unsaved text.

## Pull requests open to everyone, with automatic checks (13 November 2026)

- Pull requests from outside the MuaraAI organization are no longer closed. Anyone can
  fork, change and open one; [CONTRIBUTING.md](CONTRIBUTING.md) says how.
- Every pull request gets a **safety scan** that only reads the diff (malware patterns,
  leaked secrets, Trojan Source characters, committed executables, dependencies from
  outside crates.io, outside changes to CI and build files), and a **verdict** that
  compares it with `main` on the same runner: executable size, run time and peak memory,
  clippy and rustfmt on the added lines, and new RustSec advisories. Each leaves one
  comment that updates on every push. Limits live in `.github/pr-check.json` and are
  always read from `main`.
- Changes to CI, build, packaging, updater and network code ask the maintainers for a
  review automatically (`.github/CODEOWNERS`).

## Two new color themes, FluxMuara and FluxMuara Dark (13 November 2026)

- FluxMuara joins the color themes, with a dark companion. The light one is the Muara
  AI brand palette: a mist water canvas, white surfaces, riverbed ink text and one
  river current blue accent (`#0369A1`). FluxMuara Dark is the same river at night:
  riverbed ink surfaces (the design's code-block colors) with the accent lifted so it
  reads on the dark canvas. Both appear in the theme menu in the status bar, in
  `Ctrl+K Ctrl+T` cycling and as cards in Settings.
- A new test holds every theme to the contrast bar the palette comment states
  (4.5:1 for text on every surface, 5.1:1 on an accent fill), so a palette that
  ships unreadable fails the build instead of a reader.

## Our own Live Share relay, and a README in two languages (11 November 2026)

- Live Share now goes through MuaraAI's own relay at `live.muaraai.com` before it falls
  back to the free public one. The panel says which is in use: **Relay (MuaraAI)** when
  ours carries the session, **Relay (public)** when the fallback does. Ours is faster and
  has a bandwidth ceiling, so it stops instead of running up a bill.
- A shared session refuses to send what should never leave a PC: environment files,
  private keys, `credentials*`, `.envrc`, and build folders like `node_modules`, `target`
  and `.git`, even when a guest asks for one by name. Windows paths that try to escape the
  shared folder (a leading backslash, a drive letter, a name Windows would trim, a symlink)
  are refused.
- A guest can no longer flood the host: opens and joins are rate limited per guest and
  across the session, edits are charged by the bytes they change, and the peer table is
  bounded.
- The README and the Flux guide come in English and Indonesian (`README.id.md`,
  `docs/flux.id.md`), English first. `fluxcode --flux-to-md ... --lang id` writes the
  Indonesian file, and the tests keep all four generated files in step.
- The relay's own setup, from the VPS to the DNS record, is documented in
  [`relay/README.md`](relay/README.md).

## The agent asks before it acts, settings in a dialog (11 November 2026)

- When the agent wants to run a command or change a file, a card in the chat says what it
  wants to do, with the exact command, and the agent waits for Allow once, Always allow or
  Reject. The panel opens if it was hidden, and the taskbar button flashes while FluxCode is
  in the background. Two requests at once are shown one after the other; none is lost.
- Full access is a switch: on, every request is answered yes; off, the agent asks. What the
  agent's own config denies stays denied either way.
- The agent's settings open as a dialog from the gear in the panel's title row (model,
  permissions, session, browser, agent), so the chat stays where it was.
- The message box fits a narrow panel: the model's name is cut first, labels turn into
  icons, Send never leaves the box. The context meter is a small ring with the share.
- The model list lines up: context and output limits sit in right-aligned columns.
- Usage says when the model is free (OpenCode Zen): no plan limits to show.
- Clicking a menu, a dialog or a button over the chat no longer selects chat text.
- Terminal and Output text can be selected with the mouse (double click: a word, triple
  click: a line) and copied with Ctrl+C or the Copy button. Error cards are a quiet red
  band, without the colored edge.

## A Windows installer and a macOS build (10 November 2026)

- Windows releases have an installer: `FluxCode-<build>-windows-x64-setup.exe`. It installs
  for your account without admin rights in `%USERPROFILE%\.fluxcode\app` (where FluxCode
  updates itself), adds Start Menu and Desktop shortcuts and an entry in Settings > Apps
  with a working Uninstall, and offers to open FluxCode when it's done. The ZIP stays for a
  portable copy (and for the built-in updater). CI installs, runs and uninstalls each
  installer before it's published.
- FluxCode runs on macOS 11 and newer: `FluxCode-<build>-macos-universal.dmg`, one app for
  Apple silicon and Intel. It updates itself like the other builds. It is signed ad hoc,
  not notarized, so the first open asks once (Privacy & Security > Open Anyway). CI opens
  the packed app and checks it is still running 15 s later.

## The terminal reads like a normal terminal (10 November 2026)

- The Terminal tab has no separate input bar any more: the prompt (`folder ❯ `) is the
  line right after the output, like Windows Terminal or VS Code. A click anywhere in the
  terminal puts the caret at the prompt (links to errors still open the file).
- Each command stays in the scrollback as the prompt it was typed at, in the same style,
  so it is clear where one command starts. A quick success ends quietly; a failure, a
  stop or a command that took a second or more gets one short line, failures in red.
- The folder in the prompt is cut from the left (`…\Project\app`, the whole path on
  hover), only the folder's name on a narrow panel. Typed text is the log's mono font
  and size; the hint ("Type a command, then press Enter") is a plain sentence and only
  shows on an empty terminal or while a program waits for input.
- The prompt is never cut off at the bottom: the log shrinks above it, and a resized
  panel stays scrolled to the newest lines.

## A calmer chat while the agent answers; effort you can drag; models with their limits (10 November 2026)

- Scrolling up while an answer streams keeps the view where you put it; scrolling back to
  the bottom (or sending a message) follows the answer again. A wheel glide toward the
  bottom lands on the newest line even as it grows.
- An opened thought sits inside its step, so the rail runs on beside it to the next step.
  Thoughts are no longer in italics. The time of every step lines up in one column on the
  right, whether the step opens or not.
- Agents that send web tools by their bare name (OpenCode's `websearch`, `webfetch`) show
  "Searched the web" and "Fetched a page" instead of "Done websearch".
- Headings in answers are a step or two above the body text (a `#` was twice its size),
  and paragraphs sit a little closer.
- FluxCode's own memory is gone: the "Used 1 note from memory" lines, the Memory view and
  button, `/remember` and `/memory`, and the note asking the agent to save facts. Agents
  keep their own memory.
- Effort: press on the rail and drag; the handle follows the pointer, names the level under
  it, and settles on the nearest level when let go. The handle is an upright bar with a gap
  in the rail and stops at each level; Default (the agent decides) shows an empty rail, and
  Reset goes back to it. The labels sit under their stops.
- The model menu shows each model's context window and its longest answer ("200K context ·
  32K out") and marks free models Free, read from the model list OpenCode keeps on the PC
  (`~/.cache/opencode/models.json`); nothing is fetched. Claude's models get the same line
  after their price.
- Attached images show a thumbnail in their chip and a bigger preview on hover. When the
  chips don't fit, the row scrolls sideways (a mouse wheel moves it too) instead of cutting
  every name to two letters.
- The welcome page in a very narrow editor area shows the logo alone instead of words
  broken letter by letter.

## The taskbar shows the new icon; lighter debug builds (10 November 2026)

- After an update or a reinstall, Windows kept showing the old blue icon on the taskbar
  and in Start, because the shell caches a shortcut's icon by the exe's path. The updater
  now tells the shell the exe changed (and again on the first start after an update), and
  the installer refreshes the icon cache.
- Debug builds keep line numbers for FluxCode's own code and no debug info for the
  dependencies: the local build cache went from 48 GB to 2.5 GB. Release builds are
  unchanged.

## The agent's steps as one timeline; Run view, Output and Live share redesigned (10 November 2026)

- The agent's thoughts and tool calls (read, edit, run, search, fetch, delete, move) are
  steps on one rail, joined to the steps around them. A running step has its icon in the
  accent and a thin sweep under it; when it ends, the label cross-fades from "Reading" to
  "Read" and the time it took appears; new steps slide in once (160 ms). Only running steps
  animate, so an idle chat draws nothing.
- Run view: each project has a compact header and one full-width main action (Build),
  with the other tasks as quiet rows and rarely used ones under More; a running task shows
  a spinner and Stop. Extensions not installed yet are one line under Suggested extensions.
  "This file" shows only when F5 can run the file (no more "Run warps.yml").
- An extension's page: the title shares its line with Install or Uninstall and a close
  button (they wrap under it when narrow), one meta line, and each project as one row with
  its main task and a menu for the rest.
- Output and Terminal: log text in the editor font at a readable size and normal weight
  with a 1.6 line height; each command is one header band (what ran, where, status, time);
  the end says "Finished in 1.8s" or "Failed (exit code 1)" with a result line (what was
  built) or the reason (the first compiler error, clickable); Gradle's housekeeping lines
  are dimmed; wrapped lines indent by two characters; the top row is never cut in half at
  the bottom; the Python version shows only for Python runs.
- Live share: a short header, two choices side by side (Host this folder, Join with a
  code) that stack when narrow, the invite code first with one Copy button while hosting,
  people as a compact list with their file and line, the ways in as one line each with
  details behind a toggle, and errors as a banner that says what to do.

## The agent panel: effort slider, priced models, usage panel, web searches (10 November 2026)

- The model menu (and the other menus above the message box) gets exactly the room between
  the panel's top and the box, measured as it is drawn: it no longer runs over the panel's
  header in a short window.
- Effort is a slider of the agent's levels with a line on what each costs. New sessions
  start at medium (good results with fewer tokens) unless a level was picked before.
- Claude Agent's models are grouped by generation, newest first (Claude 5.5, Claude 5,
  Claude 4), each with its price per million tokens from Anthropic's model table: Opus 5.5
  $4 / $20, Sonnet 5.5 $2 / $10, Haiku 5.5 $0.10 / $0.50, Fable 5.1 $10 / $50, Opus 5 and
  4.x $5 / $25, Sonnet 4.6 $3 / $15, Haiku 4.5 $1 / $5.
- A usage panel (the gauge button, the context meter under the message box, or typing
  `/usage`): the plan's limits from the agent's own `/usage` (run quietly, no tokens), when
  each resets (live from the agent's `usage_update`), the context window from `/context`,
  the session's cost and tokens, and what uses the limits most. Request usage credits asks
  for confirmation, runs `/usage-credits` and shows its answer; limits themselves reset on
  their own.
- Web searches and page reads are cards: the query, then the sources with their site, each
  opening in the browser; a thin line sweeps while it searches and the rows ease in once
  when results arrive. The agent's own tool-list lookups (ToolSearch) no longer show as rows.
- The message box's model, mode and permission buttons give way in a narrow panel instead
  of pushing Send out of the box; "Default (recommended)" shows as "Default".

## Extensions, FluxCompile, OpenCode 2 and a theme picker (10 November 2026)

- An Extensions view (`Ctrl+Shift+X`, a new activity bar icon): 11 built-in extensions as
  small JSON manifests (`extensions/*.json`), with a search box and All / Installed / This
  folder, a page per extension (what it needs and whether it is found, its tasks, the
  folder's projects, a new-project form), and Install from file for more. Installed ids are
  kept in the settings; the HTML preview is one of them and installed by default.
- Paper plugins: Build, Build & deploy, Deploy last build, Start server (in the Terminal),
  Test, Clean, Run server when the project uses run-paper, Add Gradle wrapper. Projects are
  named after their plugin (the plugin.yml in the built jar); deploy writes
  `<Name>-<version>.jar` and removes older jars of the same plugin by the name inside them.
  New plugin writes a Gradle project for the server's Paper version (26.x: `paper-api:26.x.build.+`
  on Java 25, as Paper's docs say; 1.21.x: `-R0.1-SNAPSHOT` on Java 21).
- Bedrock add-ons: Check manifests, Deploy to Minecraft (the GDK build's
  `%APPDATA%\Minecraft Bedrock` folders or the Store build's), Pack `.mcaddon`/`.mcpack`
  (FluxCode's own ZIP writer), New add-on with fresh UUIDs.
- Gradle, Maven, Node.js (every package.json script, the package manager from the lock
  file), Rust, Python (the project's venv), Go, C/C++ (CMake, Make) and .NET tasks.
- FluxCompile runs Java builds with the JDK the project asks for (on JAVA_HOME and PATH),
  Gradle's local build cache unless the project turns it off, and the project's own
  parallel and configuration-cache settings. It doesn't force those two: Gradle's docs and
  28 large plugins' builds show that forcing them breaks some projects. Gradle's task lines
  fold into one line, the end says what was built (name, version, size, time, path), and a
  deploy suggests `paperweight-mappings-namespace: mojang` when the jar lacks it.
- Compiler errors from javac, kotlinc, rustc, gcc/clang, MSBuild, Gradle and Maven are red
  and open the file at the line.
- The agent's workspace overview says, for each project an installed extension is for,
  what it is, the commands that build it, the JDK and test server (Paper) or the packs
  (Bedrock), and the extension's notes.
- OpenCode 2 replaces OpenCode in the agent list: the ACP registry's `opencode-v2` build
  from opencode.ai, installed by FluxCode and checked against its SHA-256. An `opencode`
  choice and its options move to it. Measured here, both start a session in about 1.8 s
  and use about the same memory (they are the same OpenCode 2.2.x); OpenCode 2 skips the
  npm `cmd.exe` launcher and always matches the registry's version.
- The status bar's theme name opens a list of the four themes, each drawn as a small editor
  in its colors, with the current one checked.
- The welcome page fits a narrow editor area and scrolls when the window is short.

## Incoming commits in Source Control, and a smaller update notice (10 November 2026)

- Source Control fetches quietly every five minutes while a repository with an upstream is
  open, and when the window comes back after two minutes away. The fetch never prompts
  (`GIT_TERMINAL_PROMPT=0`, `GCM_INTERACTIVE=never`); a failed one leaves things as they
  were.
- When the branch is behind, an Incoming section at the top of the panel lists the commits
  (`git log HEAD..@{u}`, at most 50), drawn like the Graph with hollow dots. A commit opens
  to the files it changed (`git show --name-status`, read only when opened), and a file
  opens its diff in the Output panel. Pull runs `git pull` (with `--autostash` when you
  have changes); the close button hides the section until the upstream moves. A blue dot
  on the Source Control icon shows there is something to pull.
- The update notice is shorter: "Update ready", when the build was made, one line on what
  changed, and the two buttons. The build names are in its tooltip. Closing it leaves a
  small Update button in the title bar instead of losing the update until the next start.
- What's new names the build each release came in (`BuildDev-...` on every card, "installed
  now" on the ones in this build), instead of a date that several releases share. build.rs
  works it out from git: the commit that added the release to changelog.flux, then the first
  build tag containing it; CI now checks out the full history (3.7 MB) for this. The header
  lists the installed build and the one before it, and leaves out "Before" when the old
  build didn't record itself (it used to say "Before this you ran ."). The page is centered
  and fits narrow windows.

## A browser for the agents, and a tidier message box (10 November 2026)

- Every agent session gets a browser it can drive: the Chrome, Brave, Edge or Chromium on
  this PC (Chrome when it is there), through Google's `chrome-devtools-mcp` 1.12.0 passed
  as an ACP `mcpServers` entry. The agent opens pages, clicks, types, fills forms, reads
  console messages and network requests, takes screenshots (JPEG, at most 1280 px wide)
  and accessibility snapshots, and records performance traces; 32 tools in all.
- Light: no browser is downloaded; the tool (15 MB) is installed once into
  `~/.fluxcode/tools/browser` with the Node.js already used for npm agents, the first time
  an agent starts, and it runs only with an agent. The browser uses its own profile
  (`~/.fluxcode/browser-profile`: Chrome ignores remote debugging on the default profile
  since 136, and the agent never sees your cookies), and the tool's usage statistics to
  Google are off.
- Agent settings > Browser: the browsers found (Chrome marked recommended), Off, with a
  window or without, and where the tool stands (installing, needs Node.js 20.19+, ready).
- Tested: OpenCode opened https://example.com through it and answered "Example Domain".
- The message box: a context chip you close goes away instead of staying struck through,
  and a small "+" brings closed ones back. Chips stay on one line and narrow (their names
  cut short) instead of wrapping and making the box taller; every chip has its close
  button on the right.

## Flux documents: `.flux` opens drawn, and the README is written in it (10 November 2026)

- A `.flux` file opens drawn, with Preview and Source above it. Flux is Markdown's text
  plus one-line blocks: `@image` with `width=`, `align=`, `dark=` and `link=`,
  `@gallery`, `@video` (a poster; a click plays it in the system's player), `@flow`
  flowcharts laid out and drawn by FluxCode, `@note` / `@tip` / `@warn` / `@danger`,
  `@details`, `@columns`, `@table` with alignment and widths, `@toc`, `@en` / `@id`
  sections and `@style`. Inline it adds `{kbd ...}`, `{badge ...}`, `{color=... ...}` and
  `==highlight==`. The syntax is in `docs/flux.flux` (and `docs/flux.md`).
- It stays light at any size: one pass over the text with nothing copied (16 bytes a
  block), every table row and list item its own block, and only what is on screen drawn,
  in small groups. A document over 256 KB shows its first 32 KB at once (0.08 ms of
  reading) while the rest is read on another thread; over 1 MB its text is kept
  DEFLATE-packed in 64 KB frames, unpacked only for what is on screen; over 2 MB the
  source editor fills only when Source is shown. 1,300,003 lines (31.8 MB, a 200,000-row
  table) read in 49 ms and keep their text in 2.3 MB; opened in FluxCode the window is up
  in 0.28 s, Task Manager shows 16 MB and the process reserves 81.5 MB (this page: 5 MB
  and 47.6 MB).
- `README.md` is made from `README.flux`: `fluxcode --flux-to-md README.flux README.md`
  (`--check` only compares), or **Save as Markdown** above any Flux document. Flowcharts
  become Mermaid, callouts GitHub's alerts, `@details` `<details>`, images HTML where they
  need it. The tests fail when a generated file is out of date.
- `.gitattributes` marks `.flux` as Markdown for GitHub: Linguist already gives `.flux`
  to an unrelated query language called FLUX, and adds new formats only once they are in
  wide use.
- `changelog.flux` keeps working as before and opens drawn too.

## Live share gets its own page (10 November 2026)

- The people icon in the activity bar now opens Live share as a page in the editor area,
  like Settings, instead of squeezing it into the side bar. Its cards (host and join;
  invite code and ways in; people and activity; a guest's connection and shared files)
  sit two to a row on a wide window and wrap to one per row when it is narrow.
- The page header says where the session stands (not in a session, live, connected) with
  End session or Leave next to it. Before a session, three short steps say how it works.
- Copy invite code sits next to the code; the code box copies on click too. Enter in the
  invite box joins. Opening a shared file goes to the editor; the icon brings the page back.
- Fixed: with the side bar hidden, the editor area ran past the window's right edge by
  the activity bar's width (tabs, the page and the welcome screen were cut off there).

## One look for the agent menus, and models for GitHub Copilot (10 November 2026)

- The agent, model, mode and permission menus are drawn by FluxCode now, in one style:
  each opens inside the agent panel and as wide as it (the agent list under the title,
  the others right above the message box), so none runs off the panel or covers the box.
  The agent and model lists have a search box; Up, Down, Enter and Esc work in all four,
  and the current choice is checked and scrolled into view.
- The agent list is one line per agent (name and maker) with whether it is **On**,
  **Starting**, **Working**, **Off** or in **Error**. Every row looks the same whether or
  not the ACP registry has loaded yet.
- Effort is a row of choices at the top of the model list, so it changes without closing
  it. Mode and permission rows show their description in up to two lines.
- GitHub Copilot CLI sends no models over ACP. FluxCode reads them from
  `copilot help config`, groups them by maker, and restarts Copilot with `--model` when
  one is picked; the chat continues in the same session (Copilot can resume sessions).
  Its effort setting is left out because Copilot refuses to start when the model can't
  take it.

## Live share over the internet, without a VPN or server, and a clearer panel (10 November 2026)

- Hosting now opens every way in it can, at once: the same network, a port on the router
  (UPnP, the way game servers open theirs), the PC's public IPv6 address, and a free
  public relay (bore.pub) that works behind any router, including carrier-grade NAT.
  The panel lists each one with its address, or why it isn't open (UPnP off, a private
  router address, no IPv6 from the provider). The invite code carries every address
  that opened and updates as they open.
- Guests try every address at once, the same network first, and use the first host that
  proves it holds the code's secret: the host's first message is sealed with it, so a
  wrong or out-of-date code is told so instead of hanging. The protocol version is now
  `FXC2`; builds from before this one can't join these sessions.
- Session traffic stays end-to-end encrypted through the relay, which only passes bytes.
  The router's port is closed and the tunnel shut when the session ends; a dropped
  tunnel reopens on its own (with a new address, so the code changes).
- Tested: a guest given only the relay's address joined and co-edited a file through
  bore.pub (`cargo test session_through_the_relay -- --ignored`, about 2.7 s).
- The panel: separate cards for hosting and joining, a tick box for **Let people join over
  the internet**, a two-way choice for where a guest's files go, the invite code in a box
  that copies on click, a warning when the code changed after you copied it, and people
  with their file and line.

## What's new in an update, and the benchmark in the README (10 November 2026)

- The update card's "What's new" opens a page in FluxCode instead of the browser: every
  build between yours and the new one with its first note, then the new build's
  `changelog.flux` entries that yours doesn't have, and Update and restart. It reads one
  small text file from GitHub when clicked.
- README: a Benchmark section with FluxCode, Zed 1.27 and VS Code 1.145 measured on the
  same PC (window time, memory in Task Manager, memory reserved, idle CPU, processes,
  size on disk), drawn for GitHub's light and dark themes, with the exact numbers, the
  method and what it does not measure. `tools/bench/bench.ps1` and `chart.py` reproduce it.

## Live share: code together, with the host's PC as the server (10 November 2026)

- The people icon in the activity bar opens Live share. The host clicks Start hosting
  and sends the invite code; guests paste it and join, into their own clone of the
  repository or into a new folder.
- Every open shared file syncs keystroke by keystroke. The host orders edits with
  operational transformation (the ot.js algorithm, `src/collab/ot.rs`), so concurrent
  typing ends in the same text everywhere; tested with two windows typing into one line
  at once.
- The panel shows who is in the session, by GitHub name when signed in, and each
  person's file and line. The activity list shows each edit with lines added and removed
  and a diff; edits made by an AI agent are labelled with agent, model, effort and mode.
- Traffic goes directly between the host's PC and each guest over TCP, encrypted and
  authenticated with ChaCha20-Poly1305 under the invite code's secret, one derived key
  per connection. No relay: over the internet the host forwards TCP port 47615.
- Not yet: carets drawn in the editor, per-person undo, syncing files that aren't open.

## Less memory: 72 MB reserved, about 1 MB in Task Manager when idle (10 November 2026)

| | Before | After |
| --- | --- | --- |
| Memory reserved (commit) | 121 MB | 72 MB |
| Task Manager, idle | 53 MB | 1 MB (5 MB after a click) |
| Zed 1.27 on the same PC | 153 MB reserved, 69 MB in Task Manager | |

Measured on one Windows 11 PC, fresh profile, 9 s after launch, 3 runs each.

- FluxCode's own code was about 1 MB of it; the rest was GPUI's DirectX renderer. It now
  builds with a copy of `gpui-fast-windows` (`vendor/gpui-fast-windows`, see its
  FLUXCODE.md) that makes the two window-sized path textures, one of them 4x
  multisampled, only while frames draw paths, and keeps two swap-chain buffers instead
  of three.
- 6 s after start, and after the window has been in the background for 15 s, FluxCode
  hands the memory it isn't using back to Windows (`EmptyWorkingSet`; on Linux the
  allocator's free pages). The pages come back, a few milliseconds' worth, when used.
- `FLUXCODE_STARTUP_LOG` now writes the working set and private memory at each start-up
  step, and once more 5 s after the first frame.

## New look, any ACP agent with one click, agents that ask, and VS Code's keys (10 November 2026)

| | Before | After |
| --- | --- | --- |
| Icon | blue tile with `</>` and a lightning bolt | a meander in ivory and ochre on ink (README, The icon) |
| FluxDark | near-black with a crimson accent on every button and switch | ink and ivory, one muted ochre accent |
| FluxCream, FluxRed | red accents | umber accent on paper; FluxRed is a quieter oxblood with a clay accent |
| Agents | Mura or OpenCode, installed by hand | every agent in the ACP registry; the agent menu downloads any of them |
| No agent installed | an error after the first message | the panel lists agents to download, each with a button |
| Agent menu | names only | each agent's own icon from the ACP registry, tinted to the theme |
| Installing | text only | a liquid ring that fills with the real byte count, then a box that opens as tar writes each file |
| Model | a chip that opened the settings page | a dropdown under the message box, with effort in the same menu |
| Mode | settings page only | its own dropdown, each mode with its description |
| Permissions menu | one line per level, partly cut off | an icon, a name and one plain sentence per level |
| Agent questions | not supported | a card with lettered choices (A, B, C...), descriptions and "Type my own answer" (ACP elicitation) |
| Removing an agent | one click | type its name twice |
| Agent instruction files | find them yourself | Settings > Agent instructions (soul files): Open or Create for each |
| Shortcuts | 15 of VS Code's | VS Code's for every command FluxCode has, plus a Command Palette (`Ctrl+Shift+P`, `F1`) |
| Line editing | the editor's defaults | toggle comment, move / copy / delete line, new line above / below, go to line |
| After an update | nothing | a "What's new" page from `changelog.flux`, once, in the interface language |
| Editor settings | font size, tab size, wrap, whitespace, auto save | also font, line numbers, folding, bracket closing, smart indent, cursor padding, scrolling past the end |

- Agents FluxCode installs live in `~/.fluxcode/agents/<id>`. Native builds are
  preferred; the download is checked against the registry's SHA-256 before it is used.
  npm agents are installed into that folder and started with `node` directly. The agent
  menu can remove an agent it installed.
- Agents already on PATH (`gemini`, `copilot`, `cursor-agent`, `qwen`, `goose` and more)
  are found and used as they are.
- Agent icons are the registry's own SVGs, fetched in one curl call and kept per agent
  version, so a new logo arrives with the agent's next version. Agents without one (Mura)
  show their initial.
- The install animation is drawn with a few GPU paths at about 30 frames a second, in a
  view of its own, and stops once the install ends. It shows only what is measured: bytes
  against the server's size, files against the ZIP's entry count; with no size it swirls
  without a percentage. A failed install leaves a broken red ring, the reason in plain
  words, and Try again. Under Windows' reduce-motion setting the liquid holds still.
- The robot icon is gone from the activity bar, status bar and chat; the agent panel
  uses a chat-with-code icon and the welcome screens show the meander.
- Every text color still reaches WCAG AA (4.5:1) on every surface, and over a
  background image at the Balanced strength.
- Start-up, measured on one Windows 11 PC: the window shows about 0.21 s after launch
  (the previous build: 0.21 s) and FluxCode holds about 95 MB. Finding installed agents
  reads each PATH folder once, skips the Windows folders, and runs on its own thread
  while GPUI opens the window. `FLUXCODE_STARTUP_LOG=<file>` writes the time of each
  start-up step.
- The agent is told the interface language, so it defaults to Indonesian when FluxCode is
  in Indonesian.
- Tests cover every install failure (missing file, wrong checksum, broken archive,
  missing program), the animation's geometry (no NaN point can reach the tessellator,
  which would end the app), the line commands, the question forms and `changelog.flux`.

## About section, title bar and README screenshots (9 November 2026)

| | Before | After |
| --- | --- | --- |
| Settings | no word about the project | About FluxCode: open source, in development, links to the repository, issues and changelog |
| Long setting descriptions | ran off the right edge and pushed the buttons out of view | wrap under the title; the buttons stay visible |
| Title bar in Bahasa Indonesia | the search box covered the Bantuan menu | the box moves right of the menus; centered whenever there is room |
| README | one screenshot | seven, in pairs of the same size, each with a description |

- Settings > About FluxCode says FluxCode is open source and still in development, and
  links to the repository, the issue form and this changelog.
- Debug builds no longer stop when Settings opens (the theme cards set two hover styles).

## Update card polish (9 November 2026)

- The update card names the new build on its own line and the installed one below it.
- Its notes show the first sentence of each point instead of cutting lines mid-sentence.
- Tested end to end on Windows: a local build found `BuildDev-2026-11-09-17-58-21`,
  downloaded it with a progress bar, replaced itself and reopened as the new build.

## First public build (9 November 2026)

FluxCode is public, builds for Windows and Linux on every change, and updates itself.

| | Before | Now |
| --- | --- | --- |
| Build name | `0.9.1-dev.2026-11-09-1655` | `BuildDev-2026-11-09-HH-MM-SS` |
| Downloads | Windows | Windows x64 and Linux x64 |
| Updating | download the ZIP, run `Install.cmd` again | a card offers the new build; one click downloads it, swaps the program and restarts |
| Name in the taskbar preview and Alt+Tab | empty | FluxCode Development (BuildDev-...) |
| Repository | two editors (Tauri + CodeMirror, and GPUI in `gpui-app/`) | one: the native GPUI editor, at the root |
| Pull requests | anyone | MuaraAI members; others are closed with a pointer to Issues |
| New issues | no checks | a bot lists similar earlier issues and checks the build ID |

### Updates

- FluxCode asks GitHub for its release list 4 seconds after start and every 6 hours.
  The request is a plain `GET` with no build ID, machine ID or cookie. Settings >
  Updates turns it off; Help > Check for Updates asks right away.
- A new build shows as a card in the bottom-right corner with the first lines of its
  notes. **Update and restart** downloads it with a progress bar, puts it in place of the
  running program and opens it again with the same folders and files. **What's new** opens
  the release page; the X hides the card until the next start.
- If the download or the swap fails, the card says why and offers **Try again**; the
  running program is left as it was.

### Builds

- GitHub Actions builds Windows (`FluxCode-BuildDev-...-windows-x64.zip`) and Linux
  (`FluxCode-BuildDev-...-linux-x64.tar.gz`) and publishes both as one release. The
  release notes list the commits since the previous build.
- Linux: `./install.sh` installs for the current user (binary in `~/.local/share/fluxcode`,
  app menu entry, `fluxcode` on PATH). Dragging the title bar now moves the window on
  Linux; before, only Windows had this.
- Windows: `packaging/windows/build.ps1` builds and packs the same ZIP locally.

### Interface

- A small DEV mark next to the name in the title bar; hovering it shows the build.
- Help > About FluxCode shows the build and copies it to the clipboard, ready for an issue.

### Repository

- The Tauri + CodeMirror edition is removed: `src/`, `src-tauri/`, the web files, its test
  scripts, the Arch PKGBUILD and the `examples/` folder. It stays in the git history (last
  in commit `b485904`).
- Issue forms for bugs and ideas, an automatic check on each new issue, and a gate that
  closes pull requests from outside the organization.

### Not tested by hand yet

- The Linux build on a desktop. CI compiles it and runs the unit tests on Ubuntu 22.04.

## 0.9.1-dev (9 November 2026)

| | Before (0.9.0) | After (0.9.1) |
| --- | --- | --- |
| Files the agent changed | not visible, except the ones written through FluxCode | Changes card: each file with +lines -lines, diff and undo |
| Mouse wheel in the agent chat | jumps | glides like the editor |
| Status bar errors | full path and system message, never cleared | a short sentence, gone after 6 seconds |
| Source Control with thousands of new files | every row drawn, slow | 200 rows per section, plus folders to ignore |
| Undoing `git init` | not possible | Remove Git from This Folder; `.git` goes to the Recycle Bin |
| Agent settings | long list, needs scrolling | one screen: model as a dropdown, the rest as button rows |
| Title bar search box | click sometimes late or lost | opens on press |

- Changes card above the message box: every file the agent edited this turn, with lines
  added and removed. Click a name to open it, the diff button for a unified diff, undo to
  put that file back. Undo all and Keep all sit in the card's header.
- Source Control draws at most 200 rows per section. Over 200 untracked files, it lists
  the folders holding most of them, each with **Ignore** (adds it to `.gitignore`).
  Tested with 2,801 files: the panel opens at once.
- Status bar: Agent button with `Ctrl+L`; it lights up once on the first three launches.
- Mira (`omm`) removed; the panel runs OpenCode or Mura Harness.

## 0.9.0-dev (9 November 2026)

Measured on one Windows 11 laptop, release builds, median of 3 launches:

| | Before (0.8.2) | After (0.9.0) |
| --- | --- | --- |
| FluxCode.exe | 28.6 MB | 15.1 MB |
| Click to window | 231 ms | 226 ms |
| Idle memory (working set) | 120 MB | 123 MB |
| Languages with syntax colors | 19 | about 200 |
| Binary, Office, PDF, ZIP, SQLite files | error | read-only view |
| Dragging a folder onto the window | nothing | add to the workspace, or switch to it |
| Interface language | English | English or Bahasa Indonesia |
| Agent model, effort and mode | only inside OpenCode | in the agent panel |
| Sent messages | fixed | editable; the answer restarts from there |
| Agent permissions | auto-approve on or off | Read only, Read & write, Full access |

- Syntax colors come from syntect with bat's grammar set, which replaced 19 compiled
  tree-sitter grammars; that is where the 13 MB went.
- Files that are not text open read-only with a line saying what they are: hex for
  binaries (first 64 KB), text for Word, Excel, PowerPoint, OpenDocument and PDF, the file
  list for ZIP, JAR, WHL and APK, tables with their SQL and row counts for SQLite.
- Mouse-wheel scrolling glides in the editor; touchpads keep their own scrolling.
- Multi-root workspaces: Go to File, Find in Files, `@` and the agent cover every folder.
- The agent gets a workspace overview with its first message: project types, README,
  layout, languages, git branch and changes, open tabs.
- Mura (MuaraAI's agent harness, a separate Rust program) can run the agent panel.
- Agent memory runs in Rust with no background process, and can be switched off.

## 0.6.2 to 0.8.2 (8 and 9 November 2026)

- Python terminal: errors as cards with a clickable file and line, typo fixes such as
  `pip i` to `pip install`, consistent colors.
- Source Control: commit, push, sync, pull requests through `gh`, GitHub Actions runs.
- Go to File (`Ctrl+P`) in the middle of the window.
- Agent panel: `@file`, context chips, chat history and resuming OpenCode sessions,
  memory, `/` commands, idle shutdown that resumes the same session.
- Attachments (images, PDF, Office documents), a queue for messages sent while the agent
  answers, and switching harness without losing the chat.

## Before 0.6

FluxCode started as a Tauri + CodeMirror editor (0.1 to 0.5). The native GPUI editor
replaced it; its history is in the git log.
