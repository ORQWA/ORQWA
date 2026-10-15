# Development

How to build, check and change Torqa. Contributor conventions are in [CLAUDE.md](../CLAUDE.md),
architecture decisions in [adr/](adr/).

## Setup

All tooling runs in a container — nothing is installed on your machine.

1. Install [Docker](https://www.docker.com/) and [VS Code](https://code.visualstudio.com/) with the
   [Dev Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) extension.
2. Open this folder in VS Code and choose **Reopen in Container**.

Without VS Code, `scripts/dev.sh <command>` runs any command in the same container.

## Tasks

| Task | Command (inside the container) |
|---|---|
| All checks (fmt, clippy, tests, cargo-deny, translations, gdlint, gdformat, Godot smoke tests) | `scripts/check.sh` |
| Build the GDExtension into `app/bin/` | `scripts/build-gdext.sh [debug\|release]` |
| Render screenshots of the start page, a ride, the summary and the analysis (software Vulkan, with 16 weeks of made-up rides) into `screenshots/` | `scripts/screenshots.sh` |
| Render only the English/German error-details views, without preparing a route | `SCREENSHOT_ERRORS_ONLY=1 scripts/screenshots.sh` |
| Render the standard views of the 3D world (before and after a visual change) into `screenshots/views/` | `scripts/render-views.sh` |
| Render the 3D models (buildings, plants, clouds) and the riders for review; with `GALLERY=docs/images/models`, also the galleries in `art/README.md` | `scripts/render-models.sh`, `scripts/render-riders.sh` |
| Regenerate the boot splash PNG after a logo change | `scripts/render-splash.sh` |
| Refresh the translation template after text changes | `python3 scripts/i18n/extract.py` |
| Run the CLI with the fake trainer | `cargo run --manifest-path core/Cargo.toml -p torqa-cli -- ride --fake` |

The 3D models are Blender scripts in `art/` (see [art/README.md](../art/README.md) for them and a
gallery of every model); Blender runs in its own container on the host:
`scripts/art.sh blender --background --factory-startup --python art/<group>/build.py`.

## Builds

Release builds of the app and the CLI are artifacts of the
[CI runs](https://github.com/bossm8/Torqa/actions/workflows/ci.yml?query=branch%3Amain) (download
them while signed in to GitHub). There are no tagged releases yet.

The `macos` CI job builds `Torqa-macos-arm64.zip` and `torqa-cli-macos-arm64.tar.gz`. To test
real trainers on macOS, use the CLI built by CI: see the [command-line guide](https://github.com/bossm8/Torqa/wiki/Command-line-tool).

Windows builds come from the `windows` CI job: `Torqa-windows-x86_64.zip` (`Torqa.exe` with
`torqa_gd.dll` beside it; keep both in one folder) and `torqa-cli-windows-x86_64.zip`, both under
the run's artifacts. Like the macOS app it is built natively on the runner (MSVC, with MSYS2's
`sh` and `make` for FFmpeg).
FFmpeg's libraries come from `scripts/build-ffmpeg.sh` (a pinned, verified release; ADR 0010):
built into the dev image, and once per platform in CI.

Linux builds come from the `linux` CI job, for x86_64 and arm64: `Torqa-linux-<arch>.tar.gz`
(a `Torqa` folder with `Torqa.<arch>` and `libtorqa_gd.so`; keep both together) and
`torqa-cli-linux-<arch>.tar.gz`. They are built on Ubuntu 22.04 so they run on any distribution
with glibc 2.35 or newer (Ubuntu 22.04+, Debian 12+, Fedora 36+), not only on ones as new as
the dev container ([ADR 0012](adr/0012-linux-builds.md)). CI starts the exported app
headless to check that it loads.

Docker on macOS cannot access Bluetooth or the GPU, so macOS builds are produced by GitHub Actions.
To test 3D rendering or a real trainer, run the built app (or the portable Godot editor) natively.

## More

- Contributor conventions: [CLAUDE.md](../CLAUDE.md)
- Architecture decisions: [adr/](adr/)
- Roadmap: [PLAN.md](PLAN.md); requirements: [REQUIREMENTS.md](REQUIREMENTS.md)
- Third-party assets and their licences: [CREDITS.md](CREDITS.md)
- Translating the app: [translating.md](translating.md)

## Usage wiki

Rider guides live in the [GitHub wiki](https://github.com/bossm8/Torqa/wiki), checked out as
the `wiki/` submodule ([ADR 0015](adr/0015-usage-wiki.md)). Clone with
`git clone --recurse-submodules`, or run `make wiki-init`
in an existing checkout. Initialization leaves an existing wiki checkout untouched; after
pulling application changes, run `git submodule update --init -- wiki` only when the wiki is
clean and its local work has been saved.

Git operations use your host's credential helper. Documentation validation and tests run in
the dev container; inside an already running container, use `python3 scripts/wiki/check.py`
and `python3 -m unittest discover -s scripts/wiki -p 'test_*.py'` directly.

1. Initialize the wiki and create a branch before editing:
   `make wiki-init`, then `git -C wiki switch -c docs/my-change`.
   Resume an existing branch with `git -C wiki switch docs/my-change`.
2. Update the relevant guides, `Home.md`, `_Sidebar.md` and screenshots. Wiki filenames are
   GitHub page names. Use absolute `https://github.com/bossm8/Torqa/wiki/<page>` links, and
   `https://raw.githubusercontent.com/bossm8/Torqa/main/docs/images/<file>` for shared images.
   Check wiki references before deleting an image from the application repository.
3. Run `make wiki-prepare BRANCH=docs/my-change`. It validates, commits wiki changes and
   pushes only the preparation branch; it never updates published pages. Name it `docs/<change>`
   and keep it until publication: the publisher then deletes it.
4. Explicitly stage `git add wiki` in the application PR and include the comparison link
   printed by preparation. Wiki commits are separate from application commits; review both.
5. After merge into application `main`, the **Wiki** workflow publishes the pinned wiki
   revision. Existing published edits are preserved by merging. If this creates a different
   commit, automation opens or updates `docs: sync published wiki revision`, changing only
   the submodule pointer. Review and merge that PR normally; it does not republish changes.

### GitHub setup

The wiki must be enabled and have an initial page before its Git repository can be cloned.
Publishing uses GitHub's automatic token with `contents: write` and `pull-requests: write`;
no custom secret is required. In repository **Settings → Actions → General → Workflow
permissions**, enable **Allow GitHub Actions to create and approve pull requests**. Automation
never approves or merges its own PRs. PR checks created with the automatic token may require
a maintainer to approve their workflow runs.

Optionally set the repository Actions secret `WIKI_AUTOMATION_TOKEN` to a token that can push
the wiki and application automation branch and create PRs, if checks should start without
that approval step. Enter it directly in GitHub's settings; never put credentials in files,
commit messages, URLs, logs or chat. The workflow uses this secret if present, otherwise its
automatic token.

### Recovery

The publisher validates documentation before pushing and never force-pushes the published
wiki branch. On conflicts, preserve both histories: fetch the wiki, merge its published branch
into the preparation branch, resolve the pages, prepare again and submit the new pointer in
an application PR. The published branch is currently `master`; tooling discovers its default
branch rather than assuming it is named `main`.

For transient failures or a failed PR creation after successful publication, rerun **Wiki**
using **Run workflow**. Each attempt reads the latest application `main`, publishes only its
pinned history, and reuses one pointer PR. A retry creates no changes when both repositories
are synchronized. The only branch automation may replace with a lease is its own
`automation/wiki-sync` branch; it never replaces user preparation branches. After publishing,
it deletes `docs/*` wiki branches whose commits are all published, so their commits stay
reachable from the published branch; a branch that gained commits meanwhile is kept.
