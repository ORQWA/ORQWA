# ADR 0018 — Build commit and licence notices in the app

- Status: accepted
- Date: 2026-10-17

## Context

The settings' About tab (R83) shows the version, the commit the app is built from and the
licences. The release archives shipped no licence text at all, although the app statically
links about 220 Rust crates, mostly under MIT, BSD or Apache licences that ask for their notices
to go with every copy, plus FFmpeg (LGPL) and the Godot engine (MIT, with its own components).
The version comes from Cargo; nothing recorded the commit.

## Decision

**Commit.** `orqwa-gd`'s build script sets `ORQWA_COMMIT`: the environment variable of that
name when set, else `git rev-parse --short=7 HEAD`, else empty (About then shows the version
alone). It reruns when the variable, Git's `HEAD` or `HEAD`'s log changes. Git runs with
`safe.directory=*`, as CI's container mounts the checkout under another user. `scripts/dev.sh`
passes the host's commit in, because a worktree's Git directory lies outside the mounted
checkout. Only the leaf crate rebuilds on a new commit.

**Crate notices.** cargo-about (in the dev container, like cargo-deny) writes them from the
crates' own licence files with `core/about.toml` and `core/about.hbs`, for the extension's
dependency graph on all four release targets, without build and dev dependencies or ORQWA's
own crates. The result, `core/orqwa-app/notices/crates.txt`, is committed: release builds run
on the native CI runners, which do not have cargo-about. `scripts/notices.sh` regenerates it;
`scripts/check.sh` runs it with `--check`, so a dependency change without new notices fails CI.

**The other notices** live in `orqwa_app::about`: ORQWA's `LICENSE` and FFmpeg's LGPL 2.1
(taken from its release's `COPYING.LGPLv2.1`) embedded with `include_str!`, the models' and
data's credits, and Godot's licence and components as the running engine reports them
(`Engine.get_license_text`, `get_copyright_info`, `get_license_info`). `OrqwaCore.notices()`
hands them to the About tab, which only shows them.

## Consequences

- Every copy of the app carries the notices, without extra files beside the binary.
- Changing a dependency means running `scripts/notices.sh` and committing the result.
- The texts are as their licensors wrote them, in English; only the titles are translated.
- Builds from a source archive without Git show no commit.
- `orqwa-cli` carries no notices; it would need its own if it is distributed further.

## Amendment 2026-10-17: the version a release shows

Release v0.2.0 showed version 0.1.0: a tag sets no version, and the workspace still said
0.1.0. The version stays in the source rather than coming from the tag at build time, so a
build from a source archive, without Git, still knows it. It is set in `core/Cargo.toml` and,
for the macOS bundle, in `app/export_presets.cfg`; `scripts/release/version.py` fails
`scripts/check.sh` while they disagree, and fails a tag's CI run, and so its release, unless
the tag is `v` followed by that version. Releasing means merging the new version first, then
tagging it (docs/development.md).

## Amendment 2026-10-19: the Windows `.exe`'s version

The Windows `.exe` now carries ORQWA's icon, name and version instead of the Godot template's.
Its file and product versions take four numbers, so the preset holds the version with a fourth
0 (`0.3.0.0`); `scripts/release/version.py` checks and sets them with the macOS bundle's.

## Amendment 2026-10-24: the branch (R130)

- **What.** About names the branch with the commit; the copied version and the first log line
  name both as `main@90b74ef`. `build.rs` sets `ORQWA_BRANCH` like `ORQWA_COMMIT`: the
  variable when set, else `git rev-parse --abbrev-ref HEAD`, empty on a detached checkout.
  `ORQWA_COMMIT` is cut to seven characters, so it can be a full hash.
- **Pull requests.** Checkout builds a pull request merged with main, a commit no branch has,
  so a test build looked like no commit the rider knew. CI sets both variables for every job:
  for a pull request to its branch and last commit (`github.head_ref`,
  `github.event.pull_request.head.sha`), for a push to the branch and the commit pushed, and
  for a tag to the commit alone. A pull request's build therefore names its last commit
  although main's latest changes are in it too.
