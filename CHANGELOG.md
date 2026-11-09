# Changelog

FluxCode is in development and has no version numbers. Each build is named after the
moment it was made, `BuildDev-YYYY-MM-DD-HH-MM-SS` (Asia/Jakarta time). Every change
merged into `main` becomes a build on the [Releases](https://github.com/MuaraAI/FluxCode/releases)
page, and installed copies offer it on their own. Newest first.

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
- Updating from this build to the next one, which needs a second release to exist.

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
