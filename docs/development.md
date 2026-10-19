# Development

How to build, check and change Torqa. Contributor conventions are in [CLAUDE.md](../CLAUDE.md),
architecture decisions in [adr/](adr/).

## Setup

All tooling runs in a container — nothing is installed on your machine.

1. Install [Docker](https://www.docker.com/) and [VS Code](https://code.visualstudio.com/) with the
   [Dev Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) extension.
2. Open this folder in VS Code and choose **Reopen in Container**.

Without VS Code, `scripts/dev.sh <command>` runs any command in the same container; on Windows,
from Git Bash. The checkout uses LF line endings everywhere (`.gitattributes`): the container
runs the scripts and the Dockerfile from the checkout, and `sh` rejects CRLF. A clone made
before that setting may hold CRLF files; `git ls-files --eol | grep w/crlf` lists them, and
`git add --renormalize .` after a clean `git checkout` puts them right.

The containers run as root. With rootful Docker on Linux, a WSL clone included, what they write
into the checkout would stay owned by root, so `scripts/dev.sh` and `scripts/art.sh` hand those
files back to you afterwards (Docker Desktop and rootless setups need nothing and get nothing).
The VS Code devcontainer does not; after working in it on such a host, one `scripts/dev.sh true`
does the same.

## Tasks

| Task | Command (inside the container) |
|---|---|
| All checks (fmt, clippy, tests, cargo-deny, licence notices, translations, gdlint, gdformat, Godot smoke tests), as CI runs them | `scripts/check.sh` |
| Only the checks the branch's changes since main can affect (committed or not), for quick turns while working; the full run before a PR | `scripts/check.sh --quick` |
| Build the GDExtension into `app/bin/` | `scripts/build-gdext.sh [debug\|release]` |
| Render screenshots of the start page, a ride, the summary and the analysis (software Vulkan, with 16 weeks of made-up rides) into `screenshots/` | `scripts/screenshots.sh` |
| Render only the English/German error-details views, without preparing a route | `SCREENSHOT_ERRORS_ONLY=1 scripts/screenshots.sh` |
| Render the screenshots in another window size, e.g. the smallest the layouts are made for (R94) | `SCREENSHOT_SIZE=1024x640 scripts/screenshots.sh` |
| Render the standard views of the 3D world (before and after a visual change) into `screenshots/views/` | `scripts/render-views.sh` |
| Render the 3D models (buildings, plants, clouds) and the riders for review; with `GALLERY=docs/images/models`, also the galleries in `art/README.md` | `scripts/render-models.sh`, `scripts/render-riders.sh` |
| Regenerate the boot splash PNG after a logo change | `scripts/render-splash.sh` |
| Refresh the translation template after text changes | `python3 scripts/i18n/extract.py` |
| Refresh the crates' licence notices the About tab shows, after changing dependencies ([ADR 0018](adr/0018-about-and-notices.md)) | `scripts/notices.sh` |
| Run the CLI with the fake trainer | `cargo run --manifest-path core/Cargo.toml -p torqa-cli -- ride --fake` |

The 3D models are Blender scripts in `art/` (see [art/README.md](../art/README.md) for them and a
gallery of every model); Blender runs in its own container on the host:
`scripts/art.sh blender --background --factory-startup --python art/<group>/build.py`.

## Builds

Tagged builds of the app and CLI are available on the
[Releases page](https://github.com/Torqa-Indoor-Cycling/Torqa/releases), without signing in.
Between releases, download artifacts from the
[CI runs on main](https://github.com/Torqa-Indoor-Cycling/Torqa/actions/workflows/ci.yml?query=branch%3Amain)
while signed in to GitHub. Both use the same archives:

The `macos` CI job builds `Torqa-macos-arm64.zip` and `torqa-cli-macos-arm64.tar.gz`. To test
real trainers on macOS, use the CLI built by CI: see the [command-line guide](https://github.com/Torqa-Indoor-Cycling/Torqa/wiki/Command-line-tool).

Windows builds come from the `windows` CI job: `Torqa-windows-x86_64.zip` (`Torqa.exe` with
`torqa_gd.dll` beside it; keep both in one folder) and `torqa-cli-windows-x86_64.zip`, both under
the run's artifacts. Like the macOS app it is built natively on the runner (MSVC, with MSYS2's
`sh` and `make` for FFmpeg).
FFmpeg's libraries come from `scripts/build-ffmpeg.sh` (a pinned, verified release; ADR 0010):
built into the dev image, and once per platform in CI.

For a quick look on a Windows PC's GPU while developing, the dev container cross-builds the
extension too ([ADR 0021](adr/0021-windows-extension-in-the-container.md)):

```sh
scripts/dev.sh scripts/build-gdext.sh release windows
```

It puts `torqa_gd.dll` (MinGW, linked statically) into `app/bin/`, where the portable Godot
editor on the host loads it to render the views in seconds instead of lavapipe's minutes. It is
for development only; the released Windows app is CI's.

Linux builds come from the `linux` CI job, for x86_64 and arm64: `Torqa-linux-<arch>.tar.gz`
(a `Torqa` folder with `Torqa.<arch>` and `libtorqa_gd.so`; keep both together) and
`torqa-cli-linux-<arch>.tar.gz`. They are built on Ubuntu 22.04 so they run on any distribution
with glibc 2.35 or newer (Ubuntu 22.04+, Debian 12+, Fedora 36+), not only on ones as new as
the dev container ([ADR 0012](adr/0012-linux-builds.md)). CI starts the exported app
headless to check that it loads.

Docker on macOS cannot access Bluetooth or the GPU, so macOS builds are produced by GitHub Actions.
To test 3D rendering or a real trainer, run the built app (or the portable Godot editor) natively.

Every build names the commit it comes from on the settings' About tab and in its first log
line (`Torqa 0.2.1 (0ad9af1)`), as Git reports it while building, or the `TORQA_COMMIT`
variable when set ([ADR 0018](adr/0018-about-and-notices.md)). `scripts/dev.sh` sets it from
the host: a worktree's Git directory lies outside the checkout the container sees.

### Publishing a release

A tag does not set the version. The About tab, the first log line, the CLI and the User-Agent
read `version` in `core/Cargo.toml`; the macOS bundle reads `application/short_version` and
`application/version` in `app/export_presets.cfg`, the Windows `.exe` `application/file_version`
and `application/product_version` there, with a fourth number 0 (`0.3.1.0`). First set the new
version in all those places and update `core/Cargo.lock` to match:
`make release-version VERSION=0.3.1` does both in the container, refusing anything but three
numbers (the macOS bundle's version allows no more). Commit that, re-render the About
screenshot with `scripts/screenshots.sh` (it shows the version and the commit) and merge both.
`scripts/check.sh` fails while the versions disagree (`scripts/release/version.py`).

Then push the tag `v<version>` on the merged commit, for example `git tag v0.2.1` then
`git push origin v0.2.1`. Every tag triggers CI, but only a tag naming its commit's version is
released: the `version` job fails any other, such as `v0.2.0` on a commit saying 0.2.1, and
the release needs it. Branch and PR builds only produce workflow artifacts.
The tag's commit must contain this workflow. A tag push runs the container checks and the
native macOS, Windows and both Linux builds. After all succeed, the `release` job verifies
all eight nonempty app/CLI archives, creates a draft with generated release notes, uploads
the archives and publishes it. Only that job has `contents: write`, using the automatic
`GITHUB_TOKEN`; no release secret is needed.

Build jobs upload each archive as a single file (`archive: false`). The release download
uses `skip-decompress: true` to keep those files intact: without it the download action
extracts ZIPs, leaving the publisher without the macOS and Windows release archives.

If a build or check fails, fix it, set a new version and tag the corrected commit with it.
If publication fails, rerun the failed job from the tag's CI run: an unfinished draft is
resumed and its assets replaced before publication. A published release is left unchanged on
reruns. Tag runs are not cancelled by a later push. Check the CI run and the release's assets
before announcing it; merging the workflow PR alone does not create a release.

A workflow fix does not change an existing tag's CI run. For a failure caused by the workflow
itself, merge the fix with a new version and tag that rather than rerunning the old
workflow.

## More

- Contributor conventions: [CLAUDE.md](../CLAUDE.md)
- Architecture decisions: [adr/](adr/)
- Roadmap: [PLAN.md](PLAN.md); requirements: [REQUIREMENTS.md](REQUIREMENTS.md)
- Third-party assets and their licences: [CREDITS.md](CREDITS.md)
- Translating the app: [translating.md](translating.md)

## Usage wiki

Rider guides live in the [GitHub wiki](https://github.com/Torqa-Indoor-Cycling/Torqa/wiki), checked out as
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
   GitHub page names. Use absolute `https://github.com/Torqa-Indoor-Cycling/Torqa/wiki/<page>` links, and
   `https://raw.githubusercontent.com/Torqa-Indoor-Cycling/Torqa/main/docs/images/<file>` for shared images.
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
