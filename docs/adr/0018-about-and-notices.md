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

**Commit.** `torqa-gd`'s build script sets `TORQA_COMMIT`: the environment variable of that
name when set, else `git rev-parse --short=7 HEAD`, else empty (About then shows the version
alone). It reruns when the variable, Git's `HEAD` or `HEAD`'s log changes. Git runs with
`safe.directory=*`, as CI's container mounts the checkout under another user. `scripts/dev.sh`
passes the host's commit in, because a worktree's Git directory lies outside the mounted
checkout. Only the leaf crate rebuilds on a new commit.

**Crate notices.** cargo-about (in the dev container, like cargo-deny) writes them from the
crates' own licence files with `core/about.toml` and `core/about.hbs`, for the extension's
dependency graph on all four release targets, without build and dev dependencies or Torqa's
own crates. The result, `core/torqa-app/notices/crates.txt`, is committed: release builds run
on the native CI runners, which do not have cargo-about. `scripts/notices.sh` regenerates it;
`scripts/check.sh` runs it with `--check`, so a dependency change without new notices fails CI.

**The other notices** live in `torqa_app::about`: Torqa's `LICENSE` and FFmpeg's LGPL 2.1
(taken from its release's `COPYING.LGPLv2.1`) embedded with `include_str!`, the models' and
data's credits, and Godot's licence and components as the running engine reports them
(`Engine.get_license_text`, `get_copyright_info`, `get_license_info`). `TorqaCore.notices()`
hands them to the About tab, which only shows them.

## Consequences

- Every copy of the app carries the notices, without extra files beside the binary.
- Changing a dependency means running `scripts/notices.sh` and committing the result.
- The texts are as their licensors wrote them, in English; only the titles are translated.
- Builds from a source archive without Git show no commit.
- `torqa-cli` carries no notices; it would need its own if it is distributed further.

## Amendment 2026-10-17: the version a release shows

Release v0.2.0 showed version 0.1.0: a tag sets no version, and the workspace still said
0.1.0. The version stays in the source rather than coming from the tag at build time, so a
build from a source archive, without Git, still knows it. It is set in `core/Cargo.toml` and,
for the macOS bundle, in `app/export_presets.cfg`; `scripts/release/version.py` fails
`scripts/check.sh` while they disagree, and fails a tag's CI run, and so its release, unless
the tag is `v` followed by that version. Releasing means merging the new version first, then
tagging it (docs/development.md).

## Amendment 2026-10-19: the Windows `.exe`'s version

The Windows `.exe` now carries Torqa's icon, name and version instead of the Godot template's.
Its file and product versions take four numbers, so the preset holds the version with a fourth
0 (`0.3.0.0`); `scripts/release/version.py` checks and sets them with the macOS bundle's.
