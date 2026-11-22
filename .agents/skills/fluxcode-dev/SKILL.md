---
name: fluxcode-dev
description: >-
  FluxCode architecture, GPUI UI rules, localization (i18n), and PR verification workflow.
  Use when contributing to, building, modifying, or reviewing FluxCode Rust source code,
  UI panels, themes, translations, or agent integrations.
---

# FluxCode Contributor Skill

FluxCode is a lightweight, single-process native code editor written in Rust using `gpui-kit` (GPUI Fast retained mode), with an integrated AI agent panel, terminal, Source Control, and `.flux` document engine.

## 1. Core Architecture (`src/`)

- `main.rs`, `boot.rs`, `instance.rs`: Single-instance startup, CLI arguments (`--flux-to-md`), and window creation.
- `workbench.rs`: Main editor state, tabs, splits, command palette, and keybindings.
- `theme.rs`, `ui.rs`, `scene.rs`, `scene_art.rs`: Theme tokens (`theme::c(Tok)`), palettes (`FluxDark`, `FluxCream`, `FluxMono`, `FluxRed`, `FluxMuara`, `FluxSky`), and shared GPUI components (`action_button`, `tool_button`, `meander`).
- `agent.rs`, `agent/`: AI agent panel, sessions, tool rendering, token saver, and ACP/OpenCode/Copilot integration (`acp.rs`, `copilot.rs`, `agent_ctx.rs`, `agent_ask.rs`).
- `explorer.rs`, `files.rs`, `search.rs`, `viewer.rs`: File tree, file icons (`fluxicon.rs`), syntax highlighting (`syntect`), and read-only binary/archive/office viewers.
- `scm.rs`, `scm_incoming.rs`, `diff.rs`, `diff_view.rs`: Built-in Git source control and diff views.
- `term.rs`, `shell.rs`, `proc.rs`: Integrated terminal and Python runner (`F5`).
- `i18n.rs`, `i18n_id.rs`, `i18n/*.tsv`: Interface translations (`tr("...")` and `trf!(...)`).
- `fluxdoc/`, `whatsnew.rs`: `.flux` document parser/renderer and `changelog.flux` ("What's new" view).
- `vendor/`: Patched `gpui-fast-windows` (swap-chain and path texture memory cuts) and `gpui-component` (incremental syntax re-parsing).

## 2. Non-Negotiable Constraints

1. **Theme Tokens Only**: Never hardcode raw hex colors for UI chrome, backgrounds, borders, or text. Use `theme::c(theme::FG)`, `theme::c(theme::ACCENT)`, `theme::c(theme::BG_SIDE)`, etc. Secondary text must maintain WCAG AA (4.5:1) contrast even over translucent background images.
2. **Still Chrome**: Editor UI is looked at for hours. Limit UI motion to hover/active states and smooth theme fades (`theme::fade_from`). Never add bounce, floating, or entrance animations to editor chrome.
3. **Bilingual UI Text**: Every new user-visible string wrapped in `tr("English text")` or `trf!("...")` MUST have its everyday-developer Indonesian entry added in `src/i18n_id.rs` (keep standard English developer terms like `commit`, `push`, `branch`, `terminal`, `workspace`).
4. **Zero Bloat & Dependencies**:
   - Do not add crates to `Cargo.toml` without explicit approval (CI warns at +1 crate and fails above budgets).
   - No background polling loops when idle; FluxCode sits at ~1.4 MB idle RAM and ~0.05% idle CPU.
   - No telemetry.
5. **Documentation (`README.flux`)**: `README.md` and `README.id.md` are generated from `README.flux`. Edit `README.flux`, then regenerate with:
   - `fluxcode --flux-to-md README.flux README.md --lang en`
   - `fluxcode --flux-to-md README.flux README.id.md --lang id`

## 3. Pre-PR Verification Checklist

Run these commands before finishing any code change:

```sh
cargo test --bin fluxcode
cargo clippy --bin fluxcode -- -W clippy::all
cargo fmt -- <changed_files>
```

Ensure added lines contain:
- No `dbg!`, `println!`, `todo!`, or unhandled `unwrap()` / `expect()` in production paths.
- No em dashes (`—`) in UI copy, docs, or comments (antislop R-02).
- No generic AI comments restating the code (antislop-code); write 1-line comments explaining *why* only when non-obvious.
- Unit tests (`#[cfg(test)]`) alongside any new non-trivial logic.
