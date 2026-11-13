# Contributing

FluxCode is open source under the [GNU GPL v3.0 or later](LICENSE): you may use,
study, fork, change and share it, as long as what you distribute stays under the
same license. Issues and pull requests are open to everyone.

## Issues

[Open an issue](https://github.com/MuaraAI/FluxCode/issues/new/choose) for a bug or an
idea. The forms ask for what is needed to act on it. For bugs, that includes the build
ID: Help > About FluxCode copies it.

A bot checks each new issue within a minute. It lists earlier issues that look like the
same problem (open, or closed as fixed) and tells you when a newer build exists. It
only comments and labels; people decide what happens next.

## Pull requests

1. **Big change? Open an issue first.** A new feature, a new dependency or a change to
   how something works is easier to agree on before the code exists. Typos, small
   fixes and tests can go straight to a pull request.
2. **Fork, branch from `main`, change, push, open a pull request.** Fill in the
   template: what changed, why, and how you tested it.
3. **Build and test locally** (see [Build from source](README.md#build-from-source)):

   ```sh
   cargo test --bin fluxcode
   cargo clippy --bin fluxcode -- -W clippy::all
   cargo fmt -- <the files you changed>
   ```

4. **Wait for the checks**, fix what they report, and push again: every push is
   checked again and the comments update in place.
5. **A maintainer reviews it.** Nothing is merged without a person reading it,
   however green the checks are.

Your first pull request waits for a maintainer to press *Approve and run* before the
build and the measurements start, because they run your code. The safety scan below
runs straight away, because it only reads the diff.

## What the checks look at

Three automatic checks run on every pull request. Each leaves one comment that it keeps
up to date, and one status at the bottom of the pull request.

### 1. Safety scan (`Safety scan`)

Runs first, from `main`'s copy of [`.github/scripts/pr_scan.py`](.github/scripts/pr_scan.py),
and only **reads** the diff: nothing in the pull request is built or run. It looks at
every added line and every added file for:

| Blocks the pull request | Examples |
| --- | --- |
| Malware patterns | download piped into a shell, reverse shells, crypto miners, process injection, key logging, browser/Discord credential theft, exfiltration webhooks, destructive commands, turning off antivirus |
| Leaked secrets | private keys, GitHub/AWS/Google/Slack/OpenAI/Anthropic tokens |
| Hidden code | Trojan Source (invisible and right-to-left characters), look-alike Cyrillic/Greek letters, long encoded blobs, minified lines, URLs to raw IP addresses |
| Committed executables | `.exe`, `.dll`, `.so`, archives, anything that starts like a program |
| Supply chain | dependencies from outside crates.io, a changed checksum in `Cargo.lock` |
| CI and build files from outside contributors | `.github/`, `build.rs`, `packaging/`, `relay/`, `.cargo/` |

| Asks a maintainer to read it (`needs-maintainer-review`) | |
| --- | --- |
| New processes, network connections, `unsafe`, FFI, recursive deletes, new web addresses, secret-looking environment variables, and any change to sensitive paths (dependencies, the updater, live share, `tools/`, `extensions/`) | |

A maintainer who has read a blocked finding and found it harmless can add the
`security-reviewed` label. Critical findings (malware patterns, secrets, executables)
cannot be cleared that way: remove them.

### 2. Quality, performance and dependencies (`PR verdict`)

[`pr-check.yml`](.github/workflows/pr-check.yml) builds the pull request and `main`
on the same runner and compares them. [`pr-report.yml`](.github/workflows/pr-report.yml)
then judges the numbers with `main`'s limits from
[`.github/pr-check.json`](.github/pr-check.json), so a pull request cannot loosen its own
limits.

**Performance**: both sides are built the same way and run in turn, 40 times each:

| Measure | Warns above | Fails above |
| --- | --- | --- |
| Executable size | +3% | +10% |
| Time to start and exit, README to Markdown, a 3 MB document to Markdown (median) | +10% | +25% |
| Peak memory of the same runs | +15% | +40% |
| New crates in `Cargo.lock` | 1 | 15 |

Opening the window cannot be measured on a runner without a GPU;
[`tools/bench`](tools/bench) measures that on a PC, and a maintainer runs it when a
change could affect it.

**Code quality**: only the lines you add count, so older code does not fail your
pull request:

- Clippy `correctness`, `suspicious` and `perf` lints, compiler errors, and a leftover
  `dbg!` fail the check.
- Other clippy lints, code `cargo fmt` would change, `unwrap()`/`expect()`,
  `todo!`, `println!`, `thread::sleep`, `block_on`, `#[allow]`, very long lines and
  new files over 3,000 lines are listed as notes.

**Dependencies**: [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) checks the
RustSec advisory database. An advisory that `main` does not already have fails the check.

### 3. Build (`Build`)

[`build.yml`](.github/workflows/build.yml) builds Windows, Linux and macOS, runs the
unit tests, installs, starts and uninstalls FluxCode on the runners.

## What makes a pull request easy to merge

- One change per pull request. A fix and a refactor are two pull requests.
- Light first: no new dependency without a reason (say it in the pull request),
  nothing running in the background that the user did not ask for, and no telemetry.
- Tests for what you fix or add, next to the code (`#[cfg(test)]`).
- User-visible text in English and Indonesian ([`src/i18n_id.rs`](src/i18n_id.rs)).
- The README is made from [`README.flux`](README.flux): edit that, then run
  `fluxcode --flux-to-md README.flux README.md --lang en` and
  `fluxcode --flux-to-md README.flux README.id.md --lang id`.
- Add what changed to the top of [CHANGELOG.md](CHANGELOG.md) when it matters to users.
- Commit messages say what changed and why, in plain words.

## For maintainers

- Merging into `main` publishes a `BuildDev-...` release, and installed copies offer it.
- Read every line marked 👀 before merging, whatever the checks say. The scan catches
  known patterns; a careful attacker writes new ones.
- `security-reviewed` clears *high* findings for the current pull request. Remove the
  label if new commits arrive that you have not read.
- Limits live in [`.github/pr-check.json`](.github/pr-check.json); changing them is a
  pull request like any other.
