# Contributing to FluxCode

Thanks for helping build FluxCode! FluxCode is **source-available**, not open
source: you may fork and build it **only to prepare contributions** to
[MuaraAI/FluxCode](https://github.com/MuaraAI/FluxCode). Publishing your own
copy, fork or rebrand is not allowed. The full terms are in [LICENSE](LICENSE).

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

By opening a pull request you agree to section 4 of the [LICENSE](LICENSE):
you grant MuaraAI a perpetual license to use and relicense your contribution,
and you confirm it is your own work or properly licensed to you.
