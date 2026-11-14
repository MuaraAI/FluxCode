# Security policy

## Supported versions

FluxCode is in development and has no version numbers. Each build is named
after the moment it was made, like `BuildDev-2026-11-13-23-10-32`. Only the
latest build from `main` is supported. Older builds are not patched, so
update to the newest build from the
[Releases](https://github.com/MuaraAI/FluxCode/releases) page first and check
whether the problem is still there.

## Reporting a vulnerability

Do not open a public issue for a security problem, and do not post a proof of
concept in public.

Report it privately through GitHub Security Advisories. On this repository,
open the Security tab and choose Report a vulnerability. If that is not
available to you, contact a maintainer on GitHub
([@MyKineID](https://github.com/MyKineID) or
[@Curzyori](https://github.com/Curzyori)) and say it is a private security
report.

Include what you can of the following:

- The build ID: Help > About FluxCode copies it
- The operating system and how FluxCode was installed
- Steps to reproduce the problem
- What an attacker could do with it, in plain words

## What happens next

Maintainers will confirm they received the report, reproduce it, and work on
a fix on `main`. The fix ships in the next BuildDev build, and the changelog
notes it. If you want credit for the report, say so, and it will be added
with the fix.

There is no bug bounty program. Do not test a vulnerability against other
people's machines, accounts, or data.

## Scope

Reports in scope cover the FluxCode app, its updater, the live share relay
code in `relay/`, and the build and packaging scripts. Dependency problems are
already checked with cargo-deny against the RustSec advisory database, as
described in [CONTRIBUTING.md](CONTRIBUTING.md), but a report about one is
still welcome.
