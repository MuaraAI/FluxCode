# FluxCode Agent Rules

You are working on **FluxCode**, a lightweight native code editor in Rust (`gpui-kit`, single process, ~50 ms startup, ~1.4 MB idle RAM, zero telemetry).

For full architecture and contributor runbooks, load `.agents/skills/fluxcode-dev/SKILL.md`.

## Project Architecture & Constraints

- **Entry & State**: `src/main.rs` -> `src/boot.rs` -> `src/workbench.rs` (tabs, splits, commands, keybindings).
- **UI & Theme Tokens**: All colors go through `theme::c(theme::Tok)` in `src/theme.rs` (`FG`, `FG_STRONG`, `FG_MUTED`, `ACCENT`, `BG_EDITOR`, `BG_SIDE`, `BORDER`, etc.). Never hardcode hex colors for UI chrome. Use shared primitives in `src/ui.rs` (`action_button`, `tool_button`, `meander`). Keep UI chrome still (hover/active states and theme fades only; no bounce or entrance animations).
- **Localization (`i18n`)**: Wrap user-facing strings in `tr("...")` or `trf!("...")` (`src/i18n.rs`) and always add the matching everyday-developer Indonesian translation in `src/i18n_id.rs`.
- **AI Agent Panel**: `src/agent.rs`, `src/agent/`, `src/acp.rs`, `src/copilot.rs`, `src/ws_context.rs`.
- **Docs**: `README.md` and `README.id.md` are compiled from `README.flux` (`fluxcode --flux-to-md`). Edit `README.flux`, never the generated Markdown files directly.
- **Verification**: Run `cargo test --bin fluxcode`, `cargo clippy --bin fluxcode -- -W clippy::all`, and `cargo fmt -- <changed_files>` before finishing code changes. Never leave `dbg!`, `println!`, `todo!`, or new unapproved dependencies in `Cargo.toml`.

<!-- antislop:start -->
## antislop
For UI, copy, people, mobile layout, or code comments work, read these installed skill files directly (use these paths even if a same-named global skill exists):
- Core filter, always on: `antislop`: `.agents/skills/antislop/SKILL.md`
- UI / visual: `antislop-ui`: `.agents/skills/antislop-ui/SKILL.md`
- Copy & text: `antislop-copywriting`: `.agents/skills/antislop-copywriting/SKILL.md`
- People: `antislop-human`: `.agents/skills/antislop-human/SKILL.md`
- Mobile / responsive: `antislop-layoutmobile`: `.agents/skills/antislop-layoutmobile/SKILL.md`
- Code comments: `antislop-code`: `.agents/skills/antislop-code/SKILL.md`
Before starting, follow the core's "Two Usage Modes" section in strict order: explicit session instruction first, then global preference, then ask. A session instruction always wins. For a resolved mode, say `antislop active: <mode> (session override).` or `antislop active: <mode> (global preference).` once before presenting findings or making edits, using the actual mode and source. Acknowledging the user's request without naming the source does not replace this notice.
Only an explicit choice of antislop during or after selects a session mode. A request to review, audit, or avoid file edits does not select a mode; read the global preference in that case. Another skill's mode does not select antislop's mode.
If the mode is unresolved, ask during/after and end the response; wait for the answer before any UI review, planning, or concept. For read-only tasks, put the active-mode notice only at the start of the final answer, never in progress messages. For editing tasks, announce before the first edit and omit it from the final answer.
To update antislop later: `npx antislop-ai --update`, or run `npx antislop-ai` and pick Overwrite them.
<!-- antislop:end -->

## Ponytail (Lazy Senior Dev Mode)

The best code is the code never written. Solve the whole problem with the least new code:
- Load `.agents/skills/ponytail/SKILL.md` on coding tasks, and `.agents/skills/ponytail-review/SKILL.md` or `.agents/skills/ponytail-audit/SKILL.md` when reviewing or auditing.
- Reuse existing helpers in `src/` before writing new ones; never add a crate to `Cargo.toml` for a few lines.
- No unrequested abstractions, wrappers, options, or boilerplate.
- Comment only the *why* the code cannot show, in one concise line (and follow `antislop-code`).
- Put a `#[cfg(test)]` unit test next to new non-trivial logic.
- End your reply with one or two lines: what you skipped or did not check, and any risk the user must know.
