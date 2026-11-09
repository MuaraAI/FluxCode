# Contributing

FluxCode is open source under the [GNU GPL v3.0 or later](LICENSE): you may use,
study, fork, change and share it, as long as what you distribute stays under the
same license.

## Issues: open to everyone

[Open an issue](https://github.com/MuaraAI/FluxCode/issues/new/choose) for a bug or an
idea. The forms ask for what is needed to act on it. For bugs, that includes the build
ID: Help > About FluxCode copies it.

A bot checks each new issue within a minute. It lists earlier issues that look like the
same problem (open, or closed as fixed) and tells you when a newer build exists. It
only comments and labels; people decide what happens next.

## Pull requests: MuaraAI members only

The code is written by members of the [MuaraAI](https://github.com/MuaraAI)
organization. Pull requests from anyone else are closed automatically with a pointer to
Issues. A clear issue gets a fix into the next build faster than an outside pull request
would.

## For members

- Branch from `main`; open a pull request. CI builds Windows and Linux and runs the
  unit tests (`cargo test --bin fluxcode`).
- Merging into `main` publishes a `BuildDev-...` release, and installed copies offer it.
  Add what changed to the top of [CHANGELOG.md](CHANGELOG.md) when it matters to users.
- Light first: no new dependency without a reason, nothing running in the background
  that the user did not ask for, and no telemetry.
- Commit messages say what changed and why, in plain words.
