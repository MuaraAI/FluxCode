# Changelog

FluxCode is in development and has no version numbers. Each build is named after the
moment it was made, `BuildDev-YYYY-MM-DD-HH-MM-SS` (Asia/Jakarta time). Every change
merged into `main` becomes a build on the [Releases](https://github.com/MuaraAI/FluxCode/releases)
page, and installed copies offer it on their own. Newest first.

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
