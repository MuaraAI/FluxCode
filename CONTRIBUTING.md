# Contributing to FluxCode

Thanks for helping build FluxCode! FluxCode is free and open source under the
[GNU GPL v3.0 or later](LICENSE): you may use, fork, change and share it, as
long as what you distribute stays under the same license.

## How to contribute

1. Open an issue first for anything bigger than a small fix, so we can agree
   on the approach.
2. Fork the repository, create a branch, and make your change.
3. Build and test it (see the README's *Build from source*):

   ```bash
   npm install
   npx tauri build --no-bundle
   node scripts/test-app.mjs
   ```

   All checks of `scripts/test-app.mjs` must pass, and new behavior should get
   a check there.
4. Open a pull request against `main` describing what changed and why, with a
   screenshot for UI changes.

## Guidelines

- **Light first.** No new runtime dependency without a reason; lazy-load
  anything that isn't needed at startup; measure memory with the test script.
- Keep the VS Code layout and keybindings unless there's a clear improvement.
- Plain JavaScript and CSS on the frontend, Rust for anything that touches the
  machine. Colors come from theme variables (`src/theme.js`), never hard-coded.
- Write commit messages that explain *why*.

## Contributor terms

By opening a pull request you license your contribution under the
GPL-3.0-or-later (the same license as the project), and you confirm it is your
own work or properly licensed to you under GPL-compatible terms.
