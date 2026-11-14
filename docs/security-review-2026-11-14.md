# FluxCode security review, 2026-11-14

- Repo: MuaraAI/FluxCode
- Reviewed commit: `11d95f3` on `main` (merge of PR #53, Copilot inline completions)
- Date: 14 November 2026
- Method: manual code review, read only. Eight reviewers worked in parallel, one per area: credentials and secrets, file system safety, network and remote, input parsing and rendering, agent and AI command execution, editor core bugs and robustness, extensions and supply chain, UI and state bugs with security impact. No code was changed for this review and no fixes are included in this report. Every finding below cites a file and line the reviewer actually read. Findings reported by more than one reviewer were merged into one entry and the duplicate is noted.
- Result: 1 Critical, 19 High, 24 Medium, 19 Low. Total 63 findings.

## How to read this report

Findings are grouped by severity. Each finding has a title, a location (file and line numbers at commit `11d95f3`), evidence (what the code does), impact (what an attacker or a bad file can do with it), and a suggested fix. Severity means:

- Critical: an agent, a file, or a folder can take over the user's machine or read and write arbitrary files with no user action beyond the normal workflow.
- High: arbitrary code execution, credential theft, or data loss, reachable by opening a folder, opening a file, clicking once, or installing an update.
- Medium: code execution or data loss that needs an extra step, a compromised third party, or a crafted file, or a missing control that makes a High finding easier.
- Low: a real defect with limited impact, a hardening gap, or a process gap in CI and dependencies.

The single most important pattern in this review: FluxCode has no workspace trust model. Opening a folder runs `git` in it, runs the folder's own Python binary, prefers the folder's own build wrappers, turns `package.json` scripts into Run buttons, and reads the folder's instruction files into the agent, all without asking. A second pattern: the agent's permission checks trust the agent itself (it labels its own tool kind, it supplies its own file paths, and its file tools bypass the permission dialog entirely). Fix those two patterns first and most of the High findings shrink with them.

## Critical

### Agent bypasses the permission dialog entirely through ACP `fs/write_text_file`, with no path containment

- Location: `src/agent.rs:1458-1505`
- Evidence: `agent_on_request` handles `("fs/write_text_file", Some(p))` by calling `core::write_text(&p, &content, crlf)` directly. The only gate is an arm above it that refuses when `self.settings.access() == "read"`. The path `p` comes verbatim from `params.get("path")` via `PathBuf::from`, with no canonicalisation and no check that it sits under the workspace root or the agent working directory, and no per-file prompt. `fs/read_text_file` in the same block is likewise unchecked. A second reviewer found the same gap independently (`src/agent.rs:1393`, `src/agent.rs:1459-1510`, `src/settings.rs:217-221`): the default access level is `"write"`, which auto-allows both reads and writes, so no permission prompt appears at all by default.
- Impact: a compromised, prompt-injected, or malicious agent can read or overwrite any file the user can access (for example `~/.ssh/id_ed25519`, files in `~/.fluxcode`, tokens, or files outside the project) without the user ever seeing a permission card. This defeats the `session/request_permission` gate that governs the agent's own edit and execute tools.
- Suggested fix: canonicalise every agent file-tool path and require it to stay under the workspace root or the agent worktree, and prompt for any path outside it. Treat `fs/read_text_file` and `fs/write_text_file` with the same permission flow as the agent's own edit tools.

## High

### Self-update downloads and executes a new editor binary with no signature or hash verification

- Location: `src/update.rs:91-107, 136, 226-244, 290-293`; `.github/workflows/build.yml:319`
- Evidence: `check()` takes the download URL straight from the GitHub API JSON (`asset["browser_download_url"]`) and `download()` fetches it with `curl -f -L`. `install()` accepts the bytes when only `bytes.len() >= 1MB` and `looks_like_exe()` matches magic bytes (MZ, ELF, or Mach-O), plus the xz CRC for `.update.xz`. The workflow publishes with plain `gh release create "$BUILD" dist/*` and uploads no checksum or signature file. There is no hash, no signature, and no allow-listed host anywhere in the update path. Three reviewers confirmed this independently. The same codebase does this correctly elsewhere: the WSL helper fetch verifies SHA-256 against the release digest (`src/remote/client.rs:179-190`).
- Impact: anyone who can publish or replace a BuildDev release asset (a compromised repo or GitHub account, compromised CI, or a poisoned API response) gets arbitrary code execution on every user with update checks on, which is the default (`update_check: true`, `src/settings.rs:171`). A size check and magic bytes do not authenticate a binary.
- Suggested fix: publish a checksum file with every release and verify the downloaded bytes against it in `install()` before writing the binary. Reject the update when the checksum is missing or does not match.

### GitHub token stored in plaintext and injected into every child process

- Location: `src/github.rs:29-37, 44-49`; `src/settings.rs:21-27, 38-42`; `src/agent.rs:921`; `src/panel.rs:248`; `src/scm.rs:318`; `src/ext/view.rs:377, 425`
- Evidence: `save()` does a plain `fs::write` of the account JSON to `<config>/FluxCode-GPUI/github.json`, with no `set_permissions` call anywhere, so the token sits unencrypted on disk and readable under a typical Linux umask. `apply_env()` then adds `GH_TOKEN` and `GITHUB_TOKEN` to every command FluxCode spawns: the agent CLI, the integrated terminal, git and `gh` commands, F5 Run, and extension tasks (including `npm run <script>` built from the workspace's own `package.json`). Four reviewers confirmed parts of this finding. The token handling is otherwise careful: `verify()` passes the token to curl through `--config -` on stdin, never on the command line, and the token input is masked in the UI.
- Impact: any other local user or process can read the token from disk, and any script, extension task, or agent-invoked command can read it from its own environment and exfiltrate it. The token's power is whatever scopes the user's personal access token carries.
- Suggested fix: store the token in the OS credential store (Keychain, Credential Manager, or libsecret) and stop passing it to the terminal and the agent process. Pass it only to the specific `git` and `gh` commands that need it, or not at all when the system credential helper already provides it.

### No workspace trust model: opening a folder automatically runs `git` in it

- Location: `src/workbench.rs:557-574`, `src/scm.rs:346-372`, `src/scm.rs:387`
- Evidence: `open_folder` unconditionally calls `scm_refresh`, which runs `git rev-parse --show-toplevel`, `git status --porcelain=v2 --branch -z -uall`, and `git log` in the opened folder. There is no trust prompt and no config isolation (only one `-c i18n.logOutputEncoding=UTF-8` override on the log call). A repo's `.git/config` can set `core.fsmonitor` or `core.hooksPath` to an arbitrary command that `git status` executes.
- Impact: merely opening an untrusted or cloned folder executes repo-controlled code with the user's privileges, before any user action.
- Suggested fix: ask the user to trust a folder the first time it is opened, and until it is trusted, run `git` with `-c core.fsmonitor=false -c core.hooksPath=/dev/null` (or the platform equivalent) and skip the status and log calls. Record trust per folder.

### Opening a folder executes the workspace's own Python binary

- Location: `src/workbench.rs:569`, `src/panel.rs:1503-1527`, `src/proc.rs:54-66`
- Evidence: `open_folder` calls `detect_python`, and `proc::python` prefers `<root>/.venv`, `venv`, or `env` (`Scripts/python.exe` on Windows, `bin/python` elsewhere) when present, then runs it with `--version`. There is no trust check or prompt.
- Impact: a planted `.venv/Scripts/python.exe` or `.venv/bin/python` in an opened folder runs arbitrary code on folder open.
- Suggested fix: only use the system Python for the probe, or do not run any workspace binary until the folder is trusted (see the workspace trust finding above).

### Closing a tab or quitting silently discards unsaved changes when auto-save is off

- Location: `src/workbench.rs:1036-1060, 1279-1281, 409-414`; `src/settings.rs:150, 283-293`; `src/workbench.rs:1135-1145`
- Evidence: `close()` does `self.docs.remove(ix)` with no dirty check or prompt, `on_quit` is just `cx.quit()`, and `on_app_quit` only kills processes. Auto-save defaults to true, which hides this, but a user who turns it off loses edits with no warning. The session file stores only file paths, so unsaved content is also lost across restarts, with no backup.
- Impact: one stray Ctrl+W or window close permanently loses unsaved work, and there is no recovery path.
- Suggested fix: prompt before closing a dirty tab and before quitting with dirty tabs, and keep a backup copy of unsaved content in the session file.

### Extension manifest is arbitrary shell code execution with no permission model, sandbox, or install warning

- Location: `src/ext/mod.rs:49-63, 197-206`; `src/ext/view.rs:251-258, 727-730`; `src/proc.rs:35-48`
- Evidence: `TaskDef` has only `run: String` ("The command line, run in the project folder"). `ext_run` passes it to `shell_command`, which runs `sh -c <line>` (or `cmd.exe /c` on Windows) through `Procs::spawn` with the user's full privileges and the working directory set to the project folder. `install_file` only checks `parse()` (id alphanumeric, name non-empty) and copies the file verbatim to `~/.fluxcode/extensions/<id>/extension.json`. No signature, hash, publisher, version, or capability check exists anywhere (no `permission`, `sandbox`, or `capabilit*` string in `src/ext/`). The Install button calls `ext_install` directly, which appends the id to `settings.extensions` and saves, with no confirmation dialog or warning that tasks run shell commands. A second reviewer confirmed the same gap (`src/ext/mod.rs:158-189, 197-205`, `src/ext/view.rs:376`).
- Impact: anyone who gets a user to install one `extension.json` (file pick, download, or shared file) gets full user-privilege code execution the first time the user clicks any of its tasks, and the extension page shows the command but never warns that it is unsandboxed.
- Suggested fix: show the exact command a task will run before the first run of an extension, and warn at install time that extension tasks run shell commands with full privileges. Longer term, add a capability list to the manifest so a task that only needs to run a fixed tool cannot run an arbitrary shell line.

### CI bakes the relay secret into builds that run pull-request code

- Location: `.github/workflows/build.yml:47` (Windows job), `.github/workflows/build.yml:134` (Linux job)
- Evidence: both jobs set `FLUXCODE_RELAY_SECRET: ${{ secrets.FLUXCODE_RELAY_SECRET }}` at job level while the workflow triggers on `pull_request` and the job runs `cargo build` on the PR checkout. The file's own comment admits it: "A PR from a branch in this repo does get the secret: keep untrusted code off same-repo branches or rotate after." A PR's `build.rs` or build script runs with that secret in its environment and can exfiltrate it. Related: the same secret is compiled into every release binary through `option_env!("FLUXCODE_RELAY_SECRET")` (`src/collab/reach.rs:43`), so anyone with the binary can extract it with `strings` and use the MuaraAI relay for their own tunnels.
- Impact: any contributor (or compromised account) who can open a same-repo-branch PR can steal the relay secret, which is baked into release binaries for the MuaraAI relay. The secret must be rotated.
- Suggested fix: remove the secret from the PR build jobs entirely and pass it only to the release job, after rotating the current secret. Longer term, do not bake the relay secret into the binary; fetch a short-lived credential at runtime instead.

### Project-supplied wrapper and venv executables are preferred over system tools and run unsandboxed

- Location: `src/ext/mod.rs:366-377, 437-450`; `src/proc.rs:53-70`
- Evidence: `wrapper()` returns `./gradlew`, `.\gradlew.bat`, or `./mvnw` when that file exists in the project folder, otherwise the system `gradle` or `mvn`. `fill()` substitutes `{gradle}` and `{mvn}` with it, and `python()` returns `<project>/.venv/bin/python` (or `Scripts/python.exe`) when present. These are then executed through the same shell spawn as extension tasks, with no hash, prompt, or distinction from a trusted system tool.
- Impact: opening or cloning an untrusted project and clicking its normal Build or Run task executes attacker-controlled code shipped in the repo (a trojaned `gradlew`, `mvnw`, or venv Python) with full user privileges.
- Suggested fix: treat a project-supplied wrapper or venv binary as untrusted until the folder is trusted, and show which binary will run (project wrapper versus system tool) in the task list before it runs.

### Live Preview serves the entire workspace, including secrets, to JavaScript in any previewed HTML page

- Location: `src/preview.rs:144-151, 22`; `src/preview_ui.rs:46-47`
- Evidence: `handle()` only rejects non-`Component::Normal` path components, then does `root.join(rel_path)` and `std::fs::read(&file)` for any file under the root, with no allowlist by extension. `content_type()` falls back to `application/octet-stream` for anything else. The server binds `127.0.0.1:0` with no token, Host, or Origin check, and the embedded WebView is a plain `wry::WebViewBuilder::new().with_url(&url)` with no content security policy or sandboxing.
- Impact: opening Live Preview on an untrusted or cloned HTML file lets its script same-origin `fetch()` any workspace file (`.env`, `.git/config`, credentials, private keys) and exfiltrate it to the internet, since the preview page can also reach the network. The default (non-embedded) build has the same exposure through the system browser. This is localhost-only, so it needs a malicious file in the workspace or another local process or page that can reach loopback; it is not remotely exploitable on its own.
- Suggested fix: serve only the file types the preview needs (HTML, CSS, JS, images), refuse everything else, and add a random per-preview token to the URL. Resolve symlinks and refuse paths outside the root (see the next finding).

### Preview server follows symlinks out of the workspace root

- Location: `src/preview.rs:147-151`
- Evidence: `let mut file = root.lock().unwrap().join(rel_path);` followed by `std::fs::read(&file)`. There is no `canonicalize()` or containment check after joining, and no symlink check anywhere in `preview.rs`. A second reviewer confirmed the same gap and added that there is no stop path for `preview_server` anywhere in the code, so once started it listens for the whole life of the app (`src/preview_ui.rs:30-36`, `src/workbench.rs:339`).
- Impact: a symlink inside the workspace pointing outside it (for example to `~/.ssh`, `/etc`, or another project) makes those files servable to preview-page JavaScript or any local process that can reach the loopback server, defeating the `..` traversal check at line 144.
- Suggested fix: canonicalise the joined path and refuse it when it does not stay under the canonicalised workspace root. Stop the preview server when the preview is closed.

### Clicking a link in an untrusted Markdown or Flux document opens any local file or executable with the system handler, no confirmation

- Location: `src/fluxdoc/view.rs:336-339, 491-493, 610-614`
- Evidence: in `rich()`, a link that is not `http(s)://`, `mailto:`, or `#` goes to `cx.open_with_system(&base.join(u))`. `path()` returns an absolute path as-is, so `PathBuf::join` with an absolute link discards the document base. The same pattern is used for image `link=` (lines 546-549) and for video playback (lines 582, 610-614: any non-`http` target is opened with the system handler).
- Impact: a malicious `.md` or `.flux` file (for example in a cloned repo) can present a plausible link or a play button that, on one click, launches an arbitrary local executable, script, or UNC path, with no warning, no allowlist, and no restriction to safe extensions.
- Suggested fix: only open `http(s)://` and `mailto:` links directly. For `file://` and relative links, show a confirmation that names the exact path and, for executables, refuse or warn explicitly.

### Agent self-labels its tool kind, so it can get execute commands auto-approved

- Location: `src/agent.rs:1409-1430`
- Evidence: in `session/request_permission`, `kind` is taken from `params.pointer("/toolCall/kind")`, which the agent supplies. Then `reads = matches!(kind, "read"|"search"|"fetch"|"think")` and `writes = matches!(kind, "edit"|"delete"|"move")` decide auto-approval, and the code auto-selects the first option whose kind starts with `allow` or `reject` and responds without showing UI. The displayed `title` and `detail` are also agent-supplied (`/toolCall/title`, `/toolCall/rawInput/command`, and similar).
- Impact: an agent that labels an `execute` tool call as `read` or `edit` gets it auto-approved under the default, read, and write levels, and can mislabel a destructive command as benign in the one dialog the user does see.
- Suggested fix: derive the tool kind on the FluxCode side from the method name, never from the agent-supplied `kind` field, and always show the exact command in the permission dialog.

### Default access level is "write": file edits and deletes are auto-approved on a fresh install

- Location: `src/settings.rs:216-222, 164-165`; `src/agent.rs:1411-1417`
- Evidence: `Settings::default()` sets `agent_access: String::new()`. `access()` falls through to `"write"` when the string is empty (and to `"full"` when the legacy `agent_auto_approve` is set). Under `"write"`, the permission handler auto-approves `read`, `search`, `fetch`, and `think`, and `edit`, `delete`, and `move`. Only `execute` asks.
- Impact: out of the box, the agent can edit, delete, and move files with no confirmation. The user must discover the setting and change the level to `read` to get prompts for writes.
- Suggested fix: change the default access level to `read`, so writes ask first, and say so in the agent panel the first time the agent runs.

### No sandbox at all: the agent process runs unsandboxed as the user

- Location: `src/agent.rs:919-930`
- Evidence: `agent_start` builds the agent command, adds flag args and env, then `self.procs.spawn(cmd, Some(self.agent_cwd().as_path()))`. A grep over `src/*.rs` and `src/agent/*.rs` finds no sandbox, seccomp, chroot, bubblewrap, Landlock, or allowlist mechanism (the only "sandbox" hit is an unrelated JSON-schema completion list in `src/complete.rs:343`). The git worktree (`src/agent/worktree.rs:3-15`) is explicitly only a separate checkout, not isolation.
- Impact: any agent the user picks (16 third-party CLIs) runs with the user's full filesystem, network, and credential access. A prompt-injected agent's shell, file, and network tools are limited only by the agent's own permission behaviour, not by FluxCode.
- Suggested fix: run the agent with the least privilege the platform allows (for example a read-only view of the home directory outside the workspace), and at minimum never hand it the GitHub token or the browser profile (see the findings below).

### Agent binary is executed from PATH or an unverified `launch.json` with no hash, signature, or ownership check

- Location: `src/acp.rs:200-230, 326-345`; `src/agent.rs:449-466`
- Evidence: `find()` first reads `~/.fluxcode/agents/<id>/launch.json` and deserialises `Launch { cmd, args, env }`, then falls back to `which(exe)` (first PATH folder wins) for the names in `KNOWN`. `Launch::command()` executes `cmd` with those args and env. `find_mura` and `find_opencode` accept any `p.is_file()` candidate on PATH or in `~/.local/bin`, `~/.cargo/bin`, and similar. No hash, signature, or writability or ownership check exists anywhere in this path.
- Impact: anything that can plant a file earlier on PATH, in `~/.local/bin`, or in `~/.fluxcode/agents/<id>/launch.json` (for example a malicious repo script, another installer, or a prompt-injected agent writing that file through the `fs/write_text_file` hole above) gets executed as the user the next time the agent starts.
- Suggested fix: pin the agent binary by hash at install time and verify it before launch, and warn when a `launch.json` overrides the installed binary.

### Registry agent install trusts the registry for code execution: checksum optional, npm install scripts run

- Location: `src/acp.rs:22, 372, 376-379, 493-508, 745-803, 223-225`
- Evidence: the registry is fetched over HTTPS only from `https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json` and saved and parsed with no signature verification. In `install_into()`, the Binary method verifies SHA-256 only `if let Some(want)` (the field is `Option`), and the Npm method runs `npm install ... <package>` and the Uv method runs `uvx <package>` with no hash or pinning check of any kind. Registry-supplied `env` maps are also applied verbatim to the agent process through `c.env(k, v)`, so a malicious registry entry could set dangerous keys (for example `LD_PRELOAD` or `NODE_OPTIONS`). Three reviewers confirmed parts of this finding.
- Impact: compromise of the registry CDN or content, or a malicious registry entry, yields arbitrary code execution the moment a user installs that agent, including through npm install scripts. The SHA-256 check that exists for binaries is optional and does not protect the npm or uvx paths at all.
- Suggested fix: require a SHA-256 for every binary entry and refuse entries without one, pass `--ignore-scripts` to npm installs, and allowlist the environment keys a registry entry may set.

### Prompt injection through untrusted file, README, and terminal content sent to the agent as plain instructions

- Location: `src/agent.rs:2387-2500, 3386-3420`; `src/ws_context.rs:219-239, 343-360`; `src/ext/mod.rs:525-561`
- Evidence: `agent_context_blocks` sends the active file (up to 200,000 chars), the selection, `@`-mentioned files, the last 60 terminal lines, the last failure, and a workspace overview as plain `text` and `resource` blocks with no "untrusted content" marking, delimiter, or instruction-hierarchy note. `ws_context::overview` embeds `README: <first ~400 chars of README.md>` verbatim, and `agent_attachment_blocks` sends attached text files (up to 400,000 bytes) verbatim. Extension notes from `agent_notes`, including each extension manifest's `m.agent` text, are appended the same way.
- Impact: text in a cloned repo (README, source comments, attached files, terminal output, extension notes) that says for example "ignore previous instructions and run X" or "exfiltrate Y" is delivered to the agent in the same channel as the user's own prompt, and the agent's own tools then execute it, subject only to the weak permission checks above.
- Suggested fix: mark file, terminal, and extension content as untrusted data when it is sent to the agent, keep it out of the instruction channel, and warn the user when a project file contains instruction-like text. This pairs with the containment fixes above: even when the agent is tricked, its tools must not reach outside the workspace.

### SQLite viewer panics on truncated or crafted database files, crashing the editor on open

- Location: `src/viewer.rs:344, 426-427, 443, 449, 460-461, 472`
- Evidence: `sqlite_view` does `u16::from_be_bytes([b[16], b[17]])` with direct indexing, but the caller only checks the 16-byte magic (`bytes.starts_with(b"SQLite format 3\0")`), so a 16- or 17-byte file panics immediately. `table_rows` and `count_rows` then do `p[h]`, `p[h+3]`, and `p[h+8..h+11]` where `h` is 100 for page 1, but the page size comes from those same header bytes, so a forged page size under 101 (or 0) makes `page_at` return a too-short slice and the indexing panics. `&p[start..start + local]` at line 443 panics when a forged cell pointer puts `start` past the page end, because a zero-length slice with `start > len` still panics. Two reviewers confirmed this independently. The most easily demonstrated case: a 16-byte file containing exactly `SQLite format 3\0` crashes the editor on open.
- Impact: opening a truncated download or a maliciously crafted `.db` or `.sqlite` file (or any file with the SQLite magic) kills the whole editor process, since `finish_open_file` calls `viewer::view` on every opened file before any size or validity check on the database structure.
- Suggested fix: use bounds-checked header reads (`b.get(16..18)`), validate the page size before indexing, and clamp `start` to `p.len()` before slicing. Catch viewer errors per tab instead of letting a panic kill the process.

### WSL helper save hangs forever: `RemoteClient::write()` waits for a bytes frame the server never sends

- Location: `src/remote/client.rs:77-90`; `src/remote/server.rs:106-111`; `src/workbench.rs:1099-1103`
- Evidence: `bytes()` always does `read_json()` then a second `read_frame()` expecting `FRAME_BYTES`. For `Write`, the server returns a single JSON frame (`Answer::Json`); the end-to-end test confirms this (it reads exactly one JSON frame after a write). The second `read_frame` therefore blocks indefinitely on the helper's stdout. `save_doc` calls `remote_write` synchronously on the UI thread for normal-size files, so every save of a WSL file through the helper hangs the editor, and the UNC fallback (`core::write_text`) is never reached because `remote_write` never returns. `Read` is unaffected (the server sends JSON plus bytes).
- Impact: saving any file in a WSL workspace through the helper freezes the UI permanently. The user cannot save and cannot fall back to the UNC path. This is a reliability bug, not a security vulnerability, and it is listed here because it destroys the user's work in the same way a data-loss bug does.
- Suggested fix: make `bytes()` read the second frame only for operations the server documents as sending one, and add a timeout to the helper read so a stuck helper fails the save with an error instead of freezing the editor.

## Medium

### Agent worktrees duplicate `.env` files inside the repo and merge commits with `git add -A`

- Location: `src/agent/worktree.rs:109-116, 118-135`
- Evidence: `create()` copies every top-level file whose name starts with `.env` into `<repo>/.fluxcode/worktrees/<stamp>/`. `merge()` runs `git add -A` then commits on the agent branch and merges into the user's checkout. The only thing keeping the worktree out of git is an entry appended to local `.git/info/exclude`, which is not shared and not a gitignore rule. A second reviewer confirmed the `.env` copy.
- Impact: project secrets are duplicated into a second location inside the repository, visible to the agent and anything it runs. When `.env` is not gitignored, or the agent writes a new secret file that is not ignored, `git add -A` commits it onto the merged branch.
- Suggested fix: do not copy `.env*` files into the worktree, and never merge with `git add -A`. Stage only the files the agent actually changed.

### Agent undo can permanently delete arbitrary files outside the workspace

- Location: `src/agent.rs:1999-2003, 2101-2112`; `src/core.rs:117-121`
- Evidence: `agent_track_edit` resolves tool paths with `abs()`: absolute paths pass through unchanged and relative paths are joined to the agent working directory with no `..` filtering and no workspace check. A path the agent reports lands in `agent.changes`. `agent_undo` then runs `core::delete_path(&ch.path)` when `before` is `None` (a file the agent "created"). On macOS and Linux `delete_path` is a permanent `remove_dir_all` or `remove_file`.
- Impact: a user clicking Undo or Discard on an agent change whose path points outside the workspace (for example through `../../`) permanently deletes that file or directory, with no containment check and no recovery on non-Windows.
- Suggested fix: apply the same workspace containment as the file-tool fix (see the Critical finding) to every path recorded in `agent.changes`, and refuse to delete anything outside the workspace.

### Agent registry `rel_path` and tar extraction are not contained to the install directory

- Location: `src/acp.rs:912-915, 943-975, 764-782`
- Evidence: `rel_path` filters only empty components and `"."` (`rel.split(['/', '\\']).filter(|p| !p.is_empty() && *p != ".")`), so `..` segments pass through. `install_into` (Binary method) builds `exe = rel_path(dir, rel)` from the registry entry's `cmd`, checks only `exe.is_file()`, runs `chmod 755` on it on Unix, and returns it as the `Launch` command to execute. The same helper is used for the `bin` path from an npm package's `package.json`. `unpack()` runs system `tar -xvf <archive> -C <dir>` with no `--no-absolute-filenames`, strip, or symlink and traversal check, so safety depends on whichever tar happens to be installed. Three reviewers confirmed parts of this finding.
- Impact: a malicious or compromised registry entry (or npm package `bin` field) with `cmd` such as `../../...` can point the launched agent binary at an arbitrary existing executable outside `~/.fluxcode/agents/<id>`, which the editor then runs. A malicious archive can write outside the install directory.
- Suggested fix: reject `..` components in `rel_path`, and validate tar entries (no absolute paths, no `..`, no symlinks escaping the directory) before extracting.

### Delete is permanent on macOS and Linux despite the UI promising the Recycle Bin

- Location: `src/core.rs:76-121, 117-121`; `src/explorer.rs:172`; `src/scm.rs:676-690, 1333-1352, 863`
- Evidence: the delete confirm asks "Move {name} to the Recycle Bin?" but `delete_path` only uses the Recycle Bin under `#[cfg(windows)]`. Elsewhere it does `fs::remove_dir_all` or `fs::remove_file`. The same false label is used for SCM "Remove Git" and for SCM discard of untracked files. Two reviewers confirmed this independently.
- Impact: on Linux and macOS a delete the user believes is undoable permanently destroys the file or folder, including the whole `.git` directory.
- Suggested fix: use the platform trash on macOS and Linux too (for example the `trash` crate), and until then change the label to say the delete is permanent.

### Stale right-click target can make Delete or rename act on the wrong file

- Location: `src/explorer.rs:148-151, 207-217`; `src/workbench.rs:98`
- Evidence: building a context menu sets `ctx_target` to the right-clicked path, and it is only ever cleared by `take()` in `target_dir`, `target_item`, or `on_remove_folder`. When the menu is dismissed without choosing, the stale value stays. A later Delete keypress (bound to `ExplorerDelete` in the Explorer context) calls `target_item()`, which prefers the stale `ctx_target` over the current selection.
- Impact: a destructive action can target a file the user is no longer pointing at, and the confirm dialog shows only the basename (see the next finding), so the mistake is easy to miss.
- Suggested fix: clear `ctx_target` when the context menu closes, and make `target_item()` prefer the current selection unless the action came from the menu itself.

### Delete, rename, and new-file confirms show only the file basename, never the full path

- Location: `src/explorer.rs:169-172, 318-320`
- Evidence: `short()` returns only `file_name()`. All pending-op labels use it ("Move {basename} to the Recycle Bin?", "Rename {basename}"). Explorer rows are also truncated (`.truncate()`).
- Impact: in a multi-folder workspace, or with same-named files in different folders, the user cannot tell from the confirm dialog which file is about to be deleted or renamed.
- Suggested fix: show the path relative to the workspace root in every destructive confirm dialog.

### Commit auto-saves all dirty tabs and stages everything (`git add -A`) when nothing is staged

- Location: `src/scm.rs:524-536`
- Evidence: `scm_commit` first saves every dirty open editor, then when `staged` is empty pushes `git_step(&["add", "-A"])` ("smart commit") before committing, with no per-file review step in that path.
- Impact: clicking Commit can silently persist and commit files the user never intended to include, including unrelated or sensitive files.
- Suggested fix: never auto-stage. When nothing is staged, show the file list and ask which files to include.

### Gradle wrapper is downloaded from the internet with no checksum or signature check

- Location: `src/ext/java.rs:340-365` (`current_gradle`, `add_wrapper`)
- Evidence: `current_gradle()` fetches `https://services.gradle.org/versions/current` through `curl -f -L`. `add_wrapper()` then downloads `gradlew`, `gradlew.bat`, and `gradle/wrapper/gradle-wrapper.jar` from `https://raw.githubusercontent.com/gradle/gradle/v{v}/...` with `curl -f -L -o`, checks only curl's exit status, then runs `chmod 0755 gradlew`, and the project builds with it. Two reviewers confirmed this independently.
- Impact: a tampered or intercepted wrapper download (or a compromised Gradle repo or tag) becomes the project's build executable and runs with full privileges on the next build.
- Suggested fix: verify the Gradle distribution checksum published by Gradle before using the wrapper, and pin the wrapper version instead of taking "current".

### `package.json` scripts become Run tasks whose actual commands are never shown

- Location: `src/ext/mod.rs:412-416, 432-441`; `src/ext/view.rs:934`
- Evidence: `npm_scripts()` reads only the script names from `package.json`. Each becomes a task labelled for example `npm run dev` with line `npm run <name>`. The extension page renders this as the generic text "One task per script in package.json", and the Run tooltip shows only `npm run dev`, never the script body.
- Impact: a malicious or compromised `package.json` script (for example a `dev` script that exfiltrates files) runs with full privileges when the user clicks the harmless-looking Dev or Start button, without the user ever seeing what will execute.
- Suggested fix: show the full script body next to each generated task and in the Run tooltip, and ask for confirmation the first time a script from a newly opened folder runs.

### Winget tool installs auto-accept all source and package agreements

- Location: `src/ext/view.rs:906-911`
- Evidence: the Needs "Install" button runs `winget install -e --id {w} --accept-source-agreements --accept-package-agreements` in the Terminal, where `{w}` comes from the extension manifest's `Need.winget` field. Only a tooltip shows the package id.
- Impact: a malicious extension manifest can trigger installation of an arbitrary winget package with all consent prompts pre-accepted, bypassing the user's chance to review the package.
- Suggested fix: drop both `--accept-*` flags so winget asks the user, and show the package id in the button or dialog itself.

### npm script names from `package.json` are interpolated unquoted into a shell line

- Location: `src/ext/mod.rs:412-440`; `src/ext/view.rs:376`
- Evidence: `npm_scripts()` returns raw script keys from `package.json`, and `tasks()` builds `line: format!("{pm} run {s}")` with no quoting or validation. `ext/view.rs:376` passes that line to `shell_command` (`sh -c` or `cmd /c`).
- Impact: a hostile `package.json` script name containing shell metacharacters (`;`, `&&`, `|`) can execute extra commands when the user clicks that script task. Built-in manifests do not use `{name}` in a `run` line, so this is limited to the `npm.scripts` path.
- Suggested fix: validate script names against a safe character set and refuse names with shell metacharacters, or pass the script name as a separate argv element instead of building a shell line.

### Office-file ZIP decompression has no output cap, allowing a ZIP bomb despite the 20 MB declared-size check

- Location: `src/docs.rs:100-108, 200-201`
- Evidence: `zip_read()` rejects only when the declared uncompressed size in the central directory exceeds 20 MB, then calls `miniz_oxide::inflate::decompress_to_vec(data)` with no limit. The declared size is attacker-controlled header data and need not match the actual inflated size. Two reviewers confirmed this independently. The same codebase does this correctly elsewhere: `src/ext/zip.rs:150` uses `decompress_to_vec_with_limit(body, size.max(1) * 2 + 1024)`.
- Impact: a small crafted `.docx`, `.xlsx`, or `.pptx` whose entry declares a small size but inflates to gigabytes exhausts memory when the file is opened or attached to the agent.
- Suggested fix: use the same capped decompression as `src/ext/zip.rs` everywhere, and refuse entries whose actual output passes the cap.

### Preview server has no authentication, request-size, file-size, or connection limits

- Location: `src/preview.rs:22-35, 113-151`
- Evidence: `TcpListener::bind("127.0.0.1:0")`, one thread spawned per accepted connection (`thread::spawn` in the accept loop), long-poll `/__flux/wait` holds a thread for up to 25 seconds, and `std::fs::read(&file)` loads the whole file into memory with no cap. No token, no Host or Origin validation, no per-IP or concurrent-connection limit.
- Impact: any local process (or any page or script able to reach loopback) can read workspace files and can exhaust threads and memory with many connections or a huge file. Localhost-only bounds the exposure, so this is Medium on its own. It amplifies the two High preview findings above.
- Suggested fix: add a per-preview token, cap concurrent connections and file size, and close idle connections.

### SQLite viewer recurses without a depth bound on attacker-controlled page pointers

- Location: `src/viewer.rs:420-452, 454-478`
- Evidence: `table_rows` and `count_rows` recurse into child pages named by 4-byte values read from the file itself. The `budget` (20,000 or 50,000) caps total calls, not recursion depth, so a self-referential page (a child equal to its own page) recurses tens of thousands of frames deep.
- Impact: a crafted database can drive the viewer into deep recursion and risk a stack overflow crash while merely opening the file.
- Suggested fix: pass an explicit depth limit (for example 32) and stop recursing past it.

### PDF text extraction inflates FlateDecode streams with no output size cap

- Location: `src/viewer.rs:199-201`
- Evidence: `pdf_text` calls `miniz_oxide::inflate::decompress_to_vec_zlib(raw)` on every stream whose dictionary mentions FlateDecode, and keeps the whole output. There is no cap on the decompressed size, and the loop walks every `stream` and `endstream` pair in the file.
- Impact: a small crafted PDF (a classic decompression bomb) can force the editor to allocate gigabytes and freeze or be killed for out-of-memory use just by opening the file, since any file starting with `%PDF` is routed here automatically.
- Suggested fix: cap the total decompressed output (for example at the same 20 MB used for Office files) and stop extracting past it.

### Emmet expansion recurses per nesting level, so a deeply nested abbreviation can overflow the stack

- Location: `src/complete.rs:629-667` (recursive `render` at line 663), `src/complete.rs:547-620` (`parse` builds arbitrary depth)
- Evidence: `parse` is iterative and accepts any `>` nesting depth, but `render` recurses once per child level (`render(c, &inner, base, out, false)`). The abbreviation is the whole whitespace-free tail of the current line, so a long generated or minified HTML line such as `div>div>div>...` thousands of levels deep produces equally deep recursion.
- Impact: triggering completion in an HTML file with such a line can overflow the stack and abort the editor process.
- Suggested fix: cap the nesting depth `parse` accepts (for example 64 levels) and refuse deeper abbreviations.

### Spreadsheet column index folds without a cap, letting a crafted `.xlsx` force a gigantic allocation

- Location: `src/docs.rs:158-160, 186-190`
- Evidence: `col_index` folds column letters with `n * 26 + ...` and no bound, and `xlsx_sheet` then does `vec![String::new(); width]` where `width` is the max column index. A cell reference like `r="ZZZZZZZZZZ"` yields an astronomic width (and overflows `usize` arithmetic in debug builds).
- Impact: opening a crafted `.xlsx` or `.ods` file can panic on capacity overflow or attempt a multi-gigabyte allocation, crashing or freezing the editor.
- Suggested fix: cap the column count (for example at the 16,384 columns a real spreadsheet allows) and refuse references past it.

### Agents' standing-instruction files are auto-read from the project with no warning

- Location: `src/acp.rs:68-84`
- Evidence: `instruction_files(id)` maps each agent to project files it reads as standing instructions, for example `mura => [Project("MURA.md"), Project("AGENTS.md")]`, `claude-acp => [Home(".claude/CLAUDE.md"), Project("CLAUDE.md")]`, and the default `Project("AGENTS.md")`.
- Impact: opening an untrusted repository and starting the agent lets repo-controlled instruction files steer the agent before the user types anything. FluxCode shows no warning that project files are being treated as instructions.
- Suggested fix: show the user which instruction files were loaded from the project and ask before following project-supplied instructions in an untrusted folder.

### Copilot sign-in forwards a server-supplied command name and arguments to `workspace/executeCommand` unchecked

- Location: `src/copilot.rs:460-492`
- Evidence: `sign_in` reads `result.get("command")` from the language server's `signIn` response, takes its `command` string and `arguments` verbatim, and when `!command.is_empty()` sends them back through `workspace/executeCommand`. No allowlist (for example only the GitHub device-flow command) is applied.
- Impact: a compromised or malicious Copilot language server can make FluxCode ask it to run an arbitrary server-side command. Impact is bounded by what the server itself will execute, but FluxCode adds no check.
- Suggested fix: allowlist the one device-flow command and refuse any other command the server returns.

### Copilot server can make FluxCode open an arbitrary URI through `window/showDocument`

- Location: `src/copilot.rs:579-588, 658-670`
- Evidence: on `window/showDocument`, FluxCode takes the server-supplied `uri` string and passes it to `open_url_native()`, which runs `cmd /c start`, `open`, or `xdg-open` with no scheme or allowlist check. Two reviewers confirmed this independently.
- Impact: a compromised or malicious local Copilot language server could open `file://` paths or phishing URLs on the user's machine. Narrow: it requires the server binary (a user-installed npm package) to be malicious.
- Suggested fix: allow only `https://` URIs from the server, and ask before opening anything else.

### Copilot server discovery trusts PATH and an environment variable with no verification

- Location: `src/copilot.rs:521-575`
- Evidence: `find_server` honours `FLUXCODE_COPILOT_SERVER` (any path, or a `.js` file run through `node`), then `copilot-language-server[.exe|.cmd]` found by `find_on_path` (`is_file()` only), then `node <npm root>/@github/copilot-language-server/dist/language-server.js`. No hash or signature check.
- Impact: a planted binary earlier on PATH, or a set `FLUXCODE_COPILOT_SERVER`, runs as the Copilot server and receives the full text of every file the user edits (document sync in `copilot.inline`).
- Suggested fix: verify the server binary against the installed npm package hash, and warn when `FLUXCODE_COPILOT_SERVER` overrides the default.

### Agent-driven browser (MCP) is on by default and shares the user's browser profile directory

- Location: `src/browser.rs:180-210`; `src/agent.rs:879-882`
- Evidence: `mcp_server` passes `--userDataDir <profile_dir()>` and `--workspace <workspace>` plus the temp dir to the chrome-devtools MCP server, and `agent_mcp_servers` includes it in every `session/new` and `session/load` when the browser state is Ready. The default `agent_browser` is `"auto"` (`src/settings.rs`), so this is on unless the user turns it off.
- Impact: the agent can drive a browser sharing the user's profile (cookies and sessions in that profile) and read and write files under the workspace and temp dir through the browser tool, expanding the agent's reach beyond the editor.
- Suggested fix: use a separate throwaway browser profile for the agent-driven browser, and make the browser tool opt-in instead of on by default.

### Live Share accept loop spawns an unbounded thread per unauthenticated TCP connection

- Location: `src/collab/net.rs:244-275` (`accept_loop`), `src/collab/net.rs:216` (`host`)
- Evidence: `host()` binds `0.0.0.0:47615` and `accept_loop` does `std::thread::spawn` for every incoming stream before any authentication. The pre-auth thread blocks up to 10 seconds on the handshake read. `MAX_GUESTS = 16` (`src/collab/mod.rs:44`) only caps guests after the handshake, and the relay's `MAX_TUNNELS = 32` (`src/collab/reach.rs`) does not apply to direct TCP. There is no per-IP or global connection cap or handshake rate limit.
- Impact: any unauthenticated remote peer (LAN, or internet through the UPnP mapping or relay the host opens) can open thousands of connections and exhaust threads and memory, crashing the host editor.
- Suggested fix: cap pre-auth connections (for example 32) with a short handshake timeout, and rate-limit new connections per IP.

### Live Share guest writes host-supplied snapshots to disk without resolving symlinks

- Location: `src/collab/mod.rs:695-709` (guest `Snapshot` handler), `src/collab/mod.rs:202-207` (`abs_of`)
- Evidence: on `Snapshot`, the guest builds the path with `abs_of` (only `safe_components`: rejects `..`, absolute paths, and `:`) and then does `std::fs::create_dir_all(dir)` and `std::fs::write(&abs, &text)`. There is no `canonicalize` plus inside-root check, unlike the host side, which does exactly that check in `collab_host_open` (`src/collab/mod.rs:635-641`).
- Impact: a malicious host, combined with a pre-existing symlink inside the guest's shared folder (for example in a cloned repo), can write arbitrary content outside the shared folder, for example `~/.bashrc` or `~/.ssh/authorized_keys`, as the guest user.
- Suggested fix: apply the same canonicalise-and-contain check the host side already uses to the guest snapshot path.

## Low

### Agent chat history, including user-pasted text, is saved in plaintext

- Location: `src/agent.rs:148, 574-597`; `src/settings.rs:38-42`
- Evidence: `save_chat()` collects every user message and agent text reply and writes them through `save_json("agent-history.json", ...)`, a plain `fs::write` with default permissions, no encryption, and no redaction.
- Impact: anything the user pastes into agent chat (API keys, tokens, private text) persists indefinitely on disk in readable JSON until history is truncated.
- Suggested fix: warn before saving pasted secrets, or redact strings that look like tokens before writing history.

### Same-named workspace roots are indistinguishable in tabs, breadcrumbs, and search

- Location: `src/workspace.rs:40-56`; `src/workbench.rs:1980`
- Evidence: `rel_display` prefixes with `name_of(root)` (the basename) when multi-folder, so `/a/proj` and `/b/proj` both display as `proj\...`. Tabs show only `d.name`. `rel_display` also takes the first root whose prefix matches, so a file under a nested root can be labelled under the outer root.
- Impact: the user can edit, save, or run actions believing a file belongs to one folder when it belongs to another. The UI gives no way to tell them apart.
- Suggested fix: show enough of the parent path to disambiguate roots with the same basename.

### Explorer delete of an open file discards its unsaved edits without prompting

- Location: `src/explorer.rs:283-289`
- Evidence: `commit_pending` Delete closes every open doc whose path starts with the deleted path through `self.close(ix, ...)`, which drops dirty content silently (see the High finding on closing tabs).
- Impact: deleting a file that has unsaved edits destroys both the file and the edits in one click.
- Suggested fix: prompt when a deleted file has unsaved edits, and offer to save first. This is fixed together with the dirty-tab prompt above.

### Find-in-files follows symlinked files (and reads secrets) while the tree does not follow symlinked folders

- Location: `src/core.rs:201-207`; `src/remote/server.rs:159-162`; `src/files.rs:196-214`
- Evidence: `core::search` uses `e.metadata()` (which follows symlinks) and reads any file under the root, including symlinks pointing outside the workspace and files such as `.env` inside it. Hit previews surface their contents. The explorer walk and the helper `scan` use `file_type().is_dir()` and `is_file()`, which do not follow symlinks, but the helper `list()` uses `e.metadata()`, so a symlink-to-directory inside a WSL workspace is reported as a folder and expandable outside the workspace.
- Impact: search results can expose and display contents of files outside the workspace reached through symlinks, and of credential files inside it, inconsistent with the tree's no-follow behaviour.
- Suggested fix: skip symlinked files in search, or resolve them and refuse results outside the root.

### Atomic save uses a predictable sibling temp name and `fs::write` follows symlinks

- Location: `src/core.rs:48-56`; `src/remote/server.rs:175-184`
- Evidence: `write_text` and `write_file` write to `.{name}.fluxcode-tmp` next to the target, then rename over it. When that temp path is a pre-planted symlink, `fs::write` follows it and truncates and writes the link target. The rename itself replaces a symlinked target file rather than writing through it, so only the temp path is the hole.
- Impact: a local actor with write access to the directory can redirect a save to an arbitrary file. This needs a pre-planted symlink and directory write access, so it is Low.
- Suggested fix: create the temp file with `OpenOptions::create_new(true)` and refuse when it already exists, and use a random suffix.

### Extra workspace roots are not protected from whole-folder delete the way the main root is

- Location: `src/explorer.rs:244-254, 132-148`
- Evidence: Rename and Delete both filter out only `self.root` (`filter(|p| Some(p) != self.root.as_ref())`). An added folder (`workspace.extra`) still gets the full context menu including Delete, so its entire contents can be deleted in one confirmed action. Combined with the permanent-delete behaviour on non-Windows above, there is no recovery.
- Impact: deleting a secondary workspace folder wipes the whole folder, while the main root is deliberately guarded against exactly that.
- Suggested fix: apply the same root guard to every workspace root, and ask for the folder name to be typed before a whole-folder delete.

### Locked tree carries unmaintained crates with no patched version (bincode, paste, rustls-pemfile)

- Location: `Cargo.lock:588-589` (bincode 1.3.3), `Cargo.lock:5587-5588` (paste 1.0.15), `Cargo.lock:6771-6772` (rustls-pemfile 2.2.0)
- Evidence: read from rustsec.org on 2026-11-14: RUSTSEC-2025-0141 (bincode, unmaintained, "no patched versions"), RUSTSEC-2024-0436 (paste, unmaintained, "no patched versions"), RUSTSEC-2025-0134 (rustls-pemfile, unmaintained, "no patched versions", repo archived August 2025, migrate to `rustls-pki-types` PemObject). All are transitive (through the gpui-kit, syntect, and reqwest stack), not direct deps in `Cargo.toml`.
- Impact: these crates will never receive security fixes. A future vulnerability in any of them stays unpatched in FluxCode until the dependency chain is replaced.
- Suggested fix: track the replacement of these crates through their parent dependencies, and record the known advisories in a `deny.toml` ignore list with a removal date so they stay visible.

### PR dependency check only reports new advisories, so pre-existing ones on main never fail a PR

- Location: `.github/scripts/pr_deps.py:40-47`; `.github/workflows/pr-check.yml:151-169` (deps job). No `deny.toml` or `.cargo/audit.toml` exists in the repo.
- Evidence: `pr_deps.py` computes `new = head - base` and `fixed = base - head`. An advisory already present on main (for example the three unmaintained crates above) is counted as "existing" and never blocks. `cargo deny check advisories` runs with no repo `deny.toml` to set thresholds or an ignore list.
- Impact: the advisory gate cannot drive the locked tree clean. Known-bad dependencies accumulate on main with no CI pressure to remove them.
- Suggested fix: fail the check when the total advisory count grows, and add a `deny.toml` with an explicit ignore list that expires.

### PR perf job rebuilds the PR side without `--locked`, unlike the base side

- Location: `.github/workflows/pr-check.yml:121` versus `.github/workflows/pr-check.yml:126`
- Evidence: "Build main" runs `cargo build --release --locked --manifest-path build/Cargo.toml`, but "Build the pull request" runs the same command without `--locked`, so a PR that changes `Cargo.toml` without updating `Cargo.lock` resolves fresh dependency versions instead of failing.
- Impact: the measured PR binary may be built against different dependency versions than the lockfile pins, weakening reproducibility of the perf comparison and letting a lockfile drift slip through unnoticed.
- Suggested fix: add `--locked` to the PR build step.

### Generated Markdown emits unescaped Flux image arguments as raw HTML attributes

- Location: `src/fluxdoc/md.rs:256-276`
- Evidence: `image_md()` escapes `"` only in `alt` (`.replace('"', "&quot;")`). `path`, `width`, `align`, `link`, and `dark` are interpolated verbatim into `<img src="{path}" width="{w}" align="{al}" ...>`, `<source srcset="{d}">`, and `<a href="{l}">`.
- Impact: a Flux document with a crafted image path, width, or link can break out of the attribute in the exported `.md`. On GitHub the renderer sanitises event handlers, but the generated file itself carries attacker-controlled HTML, and any less-strict renderer of that file is exposed.
- Suggested fix: escape quotes and angle brackets in every interpolated attribute value.

### Malformed `settings.json` silently discards all settings to defaults

- Location: `src/settings.rs:30-36`
- Evidence: `load_json()` does `serde_json::from_str(...).ok().unwrap_or_default()`. Any parse error or single wrong-typed field makes the whole `Settings` revert to defaults, with no error surfaced and no backup.
- Impact: one bad value (a hand-edited file, a truncated write, or type confusion such as a string where a number is expected) silently wipes the user's theme, agent access level, Copilot, WSL, and extension choices on next start. Loaded numeric values such as `font_size` are also not clamped on load (only the UI stepper clamps). An extreme value flows straight into editor and terminal sizing (`src/workbench.rs:2129`, `src/panel.rs:1176`).
- Suggested fix: keep the last good settings file as a backup, report the parse error in the UI, and clamp numeric settings on load.

### Terminal autocorrect silently rewrites the command to a different file

- Location: `src/term.rs:276`; `src/panel.rs:194-216` (autocorrect applied in `run_in_terminal`)
- Evidence: `autocorrect()` rewrites for example `py app` to `py app.py` and `pyhton` to `python` based on closest-match and fuzzy file lookup (`fix_file`, `closest`), and `run_in_terminal` runs the rewritten line. The rewrite is shown as a "Fixed" note line, but execution is not confirmed.
- Impact: a typo can silently execute a different script than the user typed when a close-matching file exists in the working directory. Low severity because the correction is displayed and the match must be unique and close, but it is an integrity risk in a shell.
- Suggested fix: ask for confirmation before running a corrected command, or only correct the command name and never the file argument.

### Completion and line-edit code slices strings at editor-supplied byte offsets with no char-boundary check

- Location: `src/complete.rs:108-109, 116, 127, 153`; `src/editor_cmds.rs:24-30`
- Evidence: `completions` does `&text[..offset.min(text.len())]` and `text[offset..]`, and `line_block` does `text[..sel.start.min(text.len())]` and `text[end_at..]`. Any of these panics when the offset lands mid-character. All in-repo callers get offsets from the gpui editor, which normally keeps cursors on boundaries, so this needs a dependency invariant to break. Notably `src/copilot.rs:747` does guard with `!text.is_char_boundary(offset)`, showing the risk is known elsewhere in the same codebase.
- Impact: a single mid-character cursor offset from the editor (or a future caller) would panic the editor instead of being clamped.
- Suggested fix: clamp every offset to a char boundary at the top of these functions, the same way `src/copilot.rs:747` already does.

### Diff view drops the body of any line starting with a multi-byte character

- Location: `src/diff_view.rs:140`
- Evidence: `let body: SharedString = l.text.get(1..).unwrap_or("").to_string().into();` strips the leading `+`, `-`, or space mark by byte index 1, but `get(1..)` returns `None` when byte 1 is inside a multi-byte first character, so the whole line renders empty.
- Impact: diff lines starting with a non-ASCII character display blank. Cosmetic only, no crash or data loss.
- Suggested fix: strip the first character by `char` boundary instead of by byte index.

### Live Share defaults to internet exposure with no per-guest approval and no revocation

- Location: `src/collab/mod.rs:166` (`internet: true`), `src/collab/net.rs:216`, `src/collab/mod.rs:540-567` (`Hello` to `Welcome`)
- Evidence: hosting defaults to opening UPnP port forwarding and a public relay tunnel (`src/collab/reach.rs`), the listener is always `0.0.0.0`, and any `Hello` with the right session secret is admitted immediately with a `Welcome` and the full shareable file list. There is no host accept step and no way to revoke a leaked code short of ending the session.
- Impact: security of the whole shared folder (read plus live edit of every shareable file) rests solely on the invite code never leaking. A leaked code gives immediate, unmediated access. The secret itself is sound (random 32 bytes).
- Suggested fix: default hosting to LAN only, ask the host to approve each guest, and add a revoke action that rotates the code.

### UPnP discovery trusts the first SSDP responder on the LAN

- Location: `src/collab/reach.rs:292-313` (`ssdp_find`), `src/collab/reach.rs:262-290` (`upnp_map`)
- Evidence: `ssdp_find` returns the `LOCATION` URL from the first SSDP reply, and `upnp_map` then fetches its device description and POSTs SOAP `AddPortMapping` to the control URL it names, over plain HTTP.
- Impact: a malicious LAN device can steer the editor's SOAP requests to an arbitrary LAN URL (LAN-side request forgery and interaction with non-router devices).
- Suggested fix: verify the SSDP reply belongs to a router (check the device type in the description) and only talk to the gateway address from the routing table.

### Agent flag values are passed as argv, but an agent-supplied model value could be parsed as a CLI flag

- Location: `src/agent.rs:730-742, 919-922`
- Evidence: `agent_flag_args` builds `[flag, value]` pairs (for example `--model <value>`) and `agent_start` appends them with `cmd.args(...)`, that is argv, not a shell string, so shell metacharacters are safe. But `value` comes from settings populated from the agent's own `configOptions` choices, so a value starting with `--` would be parsed by the agent CLI as its own flag.
- Impact: a narrow argument-injection surface against the agent binary's own flag parser (for example a fake "model" that is really `--some-flag`). The agent CLI would most likely reject it and fail to start rather than silently bypassing anything.
- Suggested fix: reject config values that start with `--` or insert `--` before the value arguments.

### Agent-supplied config options, slash commands, and ACP notifications are trusted verbatim, with no session check

- Location: `src/agent.rs:632-665, 1270-1295, 1116-1130, 1251-1260`
- Evidence: `agent_take_config` stores the agent's `configOptions` ids, names, and choices as-is. `available_commands_update` stores the agent's `availableCommands` as-is and they are merged with builtins in the `/` menu. `agent_on_message` and `agent_on_notification` apply `session/update` (tool calls, plan, usage, config) without ever checking a `sessionId` against `self.agent.session` (no `sessionId` is read on the notification path at all).
- Impact: a rogue agent can inject misleading model and mode choices, fake slash commands, and fake tool-call and usage displays. A mismatched-session message cannot be distinguished from a real one. Impact is display and confusion rather than new execution, since the agent already has a stdio channel.
- Suggested fix: check `sessionId` on every notification and ignore messages for other sessions, and mark agent-supplied commands as agent-provided in the menu.

### Elicitation URL from the agent is opened with no scheme validation

- Location: `src/agent_ask.rs:167-180, 344-351`
- Evidence: `agent_on_elicitation` takes `url` verbatim from `params["url"]` when `mode == "url"`, displays it, and the "Open the page" button calls `cx.open_url(u)` directly. The agent's `message` is also rendered as Markdown (`TextView::markdown`, `src/agent_ask.rs:330-335`).
- Impact: a malicious agent can present a lookalike sign-in URL. Exploitation still needs the user to click "Open the page", so this is a phishing aid, not direct execution.
- Suggested fix: allow only `https://` URLs in elicitation, and show the full URL before opening.

## Checked and clean

These areas were read and no finding was filed for them. They are listed so the next reviewer does not redo the work.

- Live Share crypto and limits: ChaCha20-Poly1305 with per-connection keys derived from both salts, host proof frame, 24 MB frame cap, replay rejection (`src/collab/net.rs:84-214`). Per-guest token buckets plus a host-wide bucket, `MAX_GUESTS = 16`, 16 MB file cap (`src/collab/mod.rs:39-52, 485-495, 579-612`).
- Live Share file filter and host-side paths: `src/collab/share.rs` refuses traversal, absolute and drive paths, denied dirs (`.git`, `.ssh`, `.aws`, `node_modules`), and credential filenames (`.env*`, `*.pem` and `*.key`, `id_*`, `.npmrc`) with Unicode case-folding for NTFS. The host side canonicalises symlinks before serving (`src/collab/mod.rs:633-641`).
- Live Share path handling on the host is strong: `safe_components` refuses `..`, empty components, absolute and drive paths, and Windows trailing-dot and space tricks (`src/collab/share.rs:25-42`).
- Git invocations use argv, never a shell, and file arguments go after `--` (`src/scm.rs:663-691`). WSL `wsl.exe` invocations use `--exec` with positional parameters (`src/remote/wsl.rs`), and the env values spliced into single quotes are constants only (`src/scm.rs:297-300`, `src/scm_incoming.rs:49-52`).
- WSL helper: stdio pipes only, no network listener, off by default (`src/remote/client.rs:282-293`). The release tarball SHA-256 is verified before install, and the protocol frame cap is 512 MB (`src/remote/proto.rs:160-167`).
- GitHub token verification: the token is sent to curl through `--config -` on stdin, never on the command line, and a token with whitespace or quotes is rejected (`src/github.rs:60-78`).
- Copilot authentication: FluxCode never touches a Copilot token or API key. Sign-in is the server's device flow (`src/copilot.rs:456-490`), credentials live in the server process, and no secret appears in process args, env, or logs.
- Secrets in logs, status bar, and crash output: a grep over `src/` found no token, key, or password printed to logs, status, or UI. The only `eprintln!` of agent output is inside `#[cfg(test)]` live tests (`src/acp.rs:1202, 1212`).
- Collab sharing explicitly refuses `.env`, `.env.*`, and `.envrc` (`src/collab/share.rs:73`).
- No settings are ever loaded from the workspace. `Settings::load` and `load_json` read only `%APPDATA%/FluxCode-GPUI/settings.json` (`src/settings.rs:21-36, 187-213`). No `.fluxcode` settings, `tasks.json`, or `launch.json` in a workspace folder is read or executed.
- Extension tasks run only on an explicit user click (`ext_run`, `src/ext/view.rs:355`). Extensions install only from the user-picked file or the built-in catalog.
- The agent process is not started on folder open (`AgentState::new`, `src/agent.rs:467`), Copilot starts lazily on the next suggestion request (`src/copilot.rs:156-160`), and the terminal is not auto-started on folder open.
- There is no extension runtime or code loading beyond manifests: manifests are JSON data only, and no `dlopen`, WASM, or JS execution path for extensions was found (`src/ext/mod.rs:1-14`). Native steps (`paper.deploy`, `bedrock.pack`, and similar) are a fixed dispatch in `src/ext/view.rs` (`ext_native`); unknown names error out.
- Bedrock pack and deploy: `deploy()` deletes and copies into `com.mojang/development_*_packs/<pack-folder-name>` where the folder name is the pack dir's `file_name()` (path components stripped), and `pack()` writes only under `<project>/dist/`. No path-traversal input was found. `src/ext/zip.rs` `read_entry()` caps inflate at `size*2+1024`.
- Terminal escape sequences: there is no real PTY or terminal emulator in this codebase. The panel is a one-shot shell whose output is plain text, and all output passes through `strip_ansi()` (`src/proc.rs:181-214`, called at `src/workbench.rs:2442`), which strips CSI sequences and OSC sequences up to BEL or ST, so title-setting (OSC 0/2) and clipboard-setting (OSC 52) sequences from program output cannot take effect.
- No in-editor HTML renderer: Flux and Markdown documents are drawn natively by GPUI (`src/fluxdoc/view.rs`). Raw HTML in Markdown is stripped by `gfm::strip_html` (`src/fluxdoc/gfm.rs`), and inline HTML is not executed. No `innerHTML`, `dangerouslySetInnerHTML`, or iframe rendering was found.
- Preview 404 page: the reflected path escapes `<` (`src/preview.rs:172`), and no tag can open without `<`, so reflected script injection there is not exploitable as written.
- No `unsafe` code in the reviewed parsing and rendering files (`src/preview.rs`, `src/preview_ui.rs`, `src/term.rs`, `src/docs.rs`, `src/fluxdoc/*.rs`, `src/settings.rs`, `src/viewer.rs`, `src/files.rs`, `src/proc.rs`, `src/ext/mod.rs`).
- WebView usage is only behind the off-by-default `embedded-preview` feature (`Cargo.toml:55-60`). The default build has no WebView in the process and opens the preview URL in the system browser (`src/preview_ui.rs:55-64`).
- Untrusted file content, general handling: large files are capped (`MAX_FILE_BYTES` 64 MB, `src/core.rs:8`; `read_head` reads only 64 KB past the limit, `src/workbench.rs:2858-2876`), binary files are detected by NUL in the first 8 KB and shown as hex (max 64 KB, `src/viewer.rs:9`, `src/core.rs:24`), invalid UTF-8 is lossily decoded rather than erroring (`String::from_utf8_lossy`, `src/core.rs:31`), and terminal and output lines are capped at 10,000 (`src/workbench.rs:2425`). Flux documents pack text over 1 MB and progressively load over 256 KB (`src/fluxdoc/view.rs:29-33`).
- `diff.rs`: the LCS table is capped at 4,000,000 cells (`MAX_CELLS`), oversized diffs fall back to a line-multiset count, and `unified` and `typing_frames` index bounds were verified safe.
- Find in files: plain substring only. No regex exists anywhere in the repo. 5,000-hit cap, 2 MB per-file cap, binary skip, symlink directories are not followed, and it runs on a background thread.
- Saves are atomic through a sibling temp file plus rename (`src/core.rs:47-59`), and `finish_save` only clears the dirty flag when the saved generation matches the current one, so a late older save cannot hide newer unsaved text (`src/workbench.rs:1110-1120`). Duplicate opens are de-duped by path (`src/workbench.rs:741-751`).
- Rename safeguards: `core::rename_path` refuses when the target exists (`src/core.rs:69-74`), and names containing `/ \ : * ? " < > |` are rejected (`src/explorer.rs:277-280`). Rename of a folder correctly rewrites open doc paths through component-aware `strip_prefix` (`src/explorer.rs:288-296`).
- SCM discard of tracked files requires a two-step confirm (`confirm_discard`, `src/scm.rs:671-676, 944`).
- CI, `pull_request_target` (pr-guard): `.github/workflows/pr-guard.yml:37-41` checks out the base SHA from main with `persist-credentials: false`, fetches the PR only as git objects, and runs only main's `pr_scan.py` as data. No PR code is built or run, despite the write-capable token.
- CI, pr-check and pr-report: `pr-check.yml` runs with `permissions: contents: read`, no secrets, and runs main's scripts from a base worktree. `pr-report.yml` runs on `workflow_run` from main, unpacks only the five named `.json` files under 5 MB, and cross-checks the artifact's PR number against `run.head_sha` before commenting.
- CI, actions pinning: all third-party actions are pinned to major versions (`actions/checkout@v4`, `actions/github-script@v7`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `taiki-e/install-action@v2`). No unpinned `@main` or `@master` and no other `pull_request_target` were found. No `GITHUB_TOKEN` misuse or secret echo was found in any workflow.
- Cargo deps, spot-checked against RustSec on 2026-11-14: `rustls 0.23.46` is the fixed version for RUSTSEC-2026-0346, 0347, 0348, and 0349 (fixed in 0.23.46). `rustls-webpki 0.103.16` is above the 0.103.13 fix for RUSTSEC-2026-0098, 0099, and 0104. `quick-xml 0.41.0` is the fixed version for RUSTSEC-2026-0194 and 0195. `chrono 0.4.45` is above the `>=0.4.20` patch for RUSTSEC-2020-0159 (patched, not affected).

## Recommended next fixes, in order

1. Verify self-update downloads by published checksum before executing them, and publish the checksum with every release.
2. Contain the agent: scope its file tools to the workspace, prompt for paths outside it, derive tool kinds on the FluxCode side, and change the default access level so writes ask first.
3. Add a workspace trust prompt. Until a folder is trusted, do not run its `git` config, its Python binary, its build wrappers, its `package.json` scripts, or its agent instruction files.
4. Store the GitHub token in the OS credential store and stop passing it to the terminal, the agent, and extension tasks.
5. Lock down Live Preview: serve only preview file types, resolve symlinks, add a per-preview token, and stop the server when the preview closes.
6. Bounds-check the SQLite viewer and cap decompression in the Office, PDF, and spreadsheet readers. Treat every file parser as untrusted input.
7. Remove the relay secret from PR builds, rotate the current secret, and stop baking it into the binary.
8. Fix the destructive-action UI: show full paths in confirms, use the trash on all platforms, guard every workspace root against whole-folder delete, and prompt before closing dirty tabs.
9. Show the exact command before a task runs for the first time, including `package.json` script bodies and project wrapper binaries.
10. Add a `deny.toml` and make the dependency check fail on the total advisory count, not only on new ones, and replace the three unmaintained crates when their parents allow it.

## What this review did not cover

- Dynamic testing. No proof-of-concept files were run and no exploit was executed. The SQLite crash, the ZIP bombs, the preview reads, and the other findings were verified by reading code only.
- Fuzzing of any parser.
- The live ACP registry contents. The code-level fact is that the SHA-256 check is optional and absent for npm and uvx installs. Whether every binary entry currently lists a SHA-256 was not checked.
- A full `cargo audit` or `cargo deny` run. The dependency findings rest on the RustSec pages read on 2026-11-14.
- `gpui-component` internals. Undo and redo live in the dependency's `EditorState`, so history growth or caps could not be verified here. In-editor Find is the dependency's built-in Search action, and whether its cursor positions are always char-boundary aligned could not be verified in the time available.
- macOS and Windows-only code paths, which were read but not executed on Linux.
- The MuaraAI relay server itself. Only the client side and the CI handling of its secret were reviewed.
- Whether the WSL helper save hang reproduces on a live WSL setup. The code path was read and the end-to-end test confirms the frame mismatch; the hang itself was not reproduced by running the editor.
