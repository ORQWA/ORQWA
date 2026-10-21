# Development

How to build, check and change ORQWA. Contributor conventions are in [CLAUDE.md](../CLAUDE.md),
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
| Redraw the wordmark and the lockups in `docs/brand/` after changing their geometry ([ADR 0026](adr/0026-the-name-orqwa.md)) | `python3 scripts/brand/wordmark.py` |
| Regenerate the boot splash PNG after a logo change | `scripts/render-splash.sh` |
| Refresh the translation template after text changes | `python3 scripts/i18n/extract.py` |
| Refresh the crates' licence notices the About tab shows, after changing dependencies ([ADR 0018](adr/0018-about-and-notices.md)) | `scripts/notices.sh` |
| Run the CLI with the fake trainer | `cargo run --manifest-path core/Cargo.toml -p orqwa-cli -- ride --fake` |

The 3D models are Blender scripts in `art/` (see [art/README.md](../art/README.md) for them and a
gallery of every model); Blender runs in its own container on the host:
`scripts/art.sh blender --background --factory-startup --python art/<group>/build.py`.

Worlds can be kept across updates (R101, [ADR 0025](adr/0025-kept-worlds.md)): a change that
makes courses come out differently raises `orqwa_world::GENERATOR`, so their pages offer to
regenerate them; one that kept worlds could no longer be read or drawn with raises
`orqwa_world::stored::FORMAT` and writes the stored sample again
(`ORQWA_WRITE_STORED_WORLD=1 cargo test -p orqwa-world stored`). With **Keep courses ready to
ride** on, a library course rides the world kept before your change: turn it off while working
on the generator. Renders and screenshots prepare GPX routes, which always build.

Urban surfaces and benches (R105–R106, [ADR 0027](adr/0027-urban-surfaces.md)) use generator 5
and stored world format 2: chunks include an `urban` mesh and its material codes. The format-2
sample is committed alongside format 1. Cached format-1 worlds rebuild on their next ride; course
files keep their existing format. Format-2 worlds from generator 4 offer regeneration to
add the more regular city benches.

## Refreshing screenshots

Tree rendering uses the original close models plus authored medium/far forms from
`art/vegetation/lods.py`; rebuild those in the art container after a tree shape change.
They preserve placement and stored worlds. Compare the same native GPU, camera and pixel
dimensions before and after, with Low included; see the [tree LOD measurements](benchmarks/tree-lods.md).

The current [screenshot gallery](screenshots.md) links the app screens and standard route
views; [Art](../art/README.md#gallery) contains the model galleries. Run captures separately
from checks. For a documentation refresh, isolate the generated sample rides from your own
profile by setting `XDG_DATA_HOME` inside the container:

```sh
scripts/dev.sh sh -c 'XDG_DATA_HOME=$PWD/screenshots/refresh-data SCREENSHOT_DIR=$PWD/screenshots/refresh/app scripts/screenshots.sh'
scripts/dev.sh sh -c 'XDG_DATA_HOME=$PWD/screenshots/refresh-data OUT_DIR=$PWD/screenshots/refresh/views scripts/render-views.sh'
scripts/dev.sh sh -c 'OUT_DIR=$PWD/screenshots/refresh/models GALLERY=$PWD/screenshots/refresh/gallery scripts/render-models.sh'
scripts/dev.sh sh -c 'OUT_DIR=$PWD/screenshots/refresh/riders GALLERY=$PWD/screenshots/refresh/gallery scripts/render-riders.sh'
```

The software-rendering runners select FXAA instead of MSAA and disable glow, which otherwise
produces bright halos under lavapipe. They wait for route preparation and streamed chunks;
the route views pause the rider at fixed positions. The graphics settings in the app are
unchanged. Both app and route runners check their capture script compiles before rendering.

Additional UI fixtures use the same real controls without hardware or account credentials:

For a graphics/settings change, capture only the settings with
`SCREENSHOT_SETTINGS_ONLY=1 scripts/screenshots.sh` in the container; this avoids building
a course or populating sample rides. `SCREENSHOT_LOCALE=de` selects German for the same run.

```sh
scripts/dev.sh sh -c 'XDG_DATA_HOME=$PWD/screenshots/refresh-data SCREENSHOT_DIR=$PWD/screenshots/refresh/app timeout 240 xvfb-run -a godot --path app --resolution 1600x900 -s res://tools/capture_details.gd'
```

`capture_details.gd` captures device readings, connection failures, planned workouts and
expanded error details in English and German. Capture `app/video-course.png` from a real
ride using locally supplied licensed footage and its Incyclist XML/GPX. Keep that source
outside app assets and preserve its attribution in [Credits](CREDITS.md) and the displayed
documentation. The exporter expects this video image alongside the standard app captures.

Review every capture before copying it into documentation. `contact_sheets.gd` recursively
builds sheets and a filename index using image I/O only:

```sh
scripts/dev.sh sh -c 'INPUT_DIR=$PWD/screenshots/refresh/app CONTACT_DIR=$PWD/screenshots/review/app godot --headless --path app -s res://tools/contact_sheets.gd'
scripts/dev.sh sh -c 'REFRESH_ROOT=$PWD/screenshots/refresh godot --headless --path app -s res://tools/export_screenshots.gd'
```

Repeat the review for `views`, `models` and `riders`; open full-size images where a thumbnail
cannot establish quality. The exporter replaces the stable app/wiki image paths and writes
the route gallery to `docs/images/views/`. Copy the reviewed model JPEGs from the scratch
gallery into `docs/images/models/`. Historical comparison baselines stay with their original
reviews. Phone-remote browser screenshots require a separate browser capture; these helpers
do not replace them.

## Checking music controls

Music control (R26, #260) uses `osascript` on macOS, Windows `SendInput` through a
PowerShell helper, and `playerctl` on Linux. No new library dependency is needed. macOS
checks bundle IDs and compiles only the running player's command: compiling a `tell`
statement for an absent Spotify can otherwise fail before the Apple Music fallback runs.
Even a literal bundle ID can fail compilation when its app is absent, so running checks
resolve IDs through a variable and treat a missing application as stopped.
Windows sends a virtual media key down and up; `SendKeys([char]179)` would type a character.

The macOS and Windows CI jobs compile and run the tests in
`core/orqwa-app/src/media/commands.rs` directly with `rustc --test` before building the app.
This is the production module, which needs only Rust's standard library; native script
checks need not build the rest of the app or FFmpeg first. The macOS test executes all three
commands with nonexistent player IDs and expects the no-player
runtime error, catching premature dictionary lookup. Windows compiles the real helper and
checks the `INPUT` size and the key-down/key-up events without injecting keys into the
runner's desktop. Container tests check command mappings; none of these confirms playback.

For native playback checks, use a downloaded app build, start a playlist in the player,
then focus ORQWA during a ride and press **M** twice, **.**, and **,**. Confirm pause/resume,
next and previous in the player. On macOS try Apple Music with Spotify absent, Spotify
alone, both running (Spotify takes priority), and neither running; verify the first
permission prompt and the error if permission is denied. On Windows use Spotify or a
browser tab that registers a system media session; successful input injection does not
tell whether a player handled it. On Linux use an MPRIS player with `playerctl` installed,
then try no player and a missing `playerctl`. For a regression report, record the app build,
OS, player and results. Run these manual checks separately from container checks.

For #260, the rider reported working music control on macOS with Music/Spotify open, on
Windows, and on Ubuntu with VLC after installing `playerctl` and loading media
(2026-10-21). These are manual playback reports; exact app/OS versions and every
permission/error-path case were not specified. The rider guide now explains installation
and how to distinguish a missing helper, no visible player and an empty VLC playlist.

## Builds

Tagged builds of the app and CLI are available on the
[Releases page](https://github.com/ORQWA/ORQWA/releases), without signing in.
Between releases, download artifacts from the
[CI runs on main](https://github.com/ORQWA/ORQWA/actions/workflows/ci.yml?query=branch%3Amain)
while signed in to GitHub. Both use the same archives:

The `macos` CI job builds `ORQWA-macos-arm64.zip` and `orqwa-cli-macos-arm64.tar.gz`. To test
real trainers on macOS, use the CLI built by CI: see the [command-line guide](https://github.com/ORQWA/ORQWA/wiki/Command-line-tool).

Windows builds come from the `windows` CI job: `ORQWA-windows-x86_64.zip` (`ORQWA.exe` with
`orqwa_gd.dll` beside it; keep both in one folder), the installer `ORQWA-windows-x86_64.msi`
and `orqwa-cli-windows-x86_64.zip`, all under the run's artifacts. The installer is built from
`scripts/release/ORQWA.wxs` with the WiX Toolset ([ADR 0023](adr/0023-windows-installer.md)),
and the job installs and uninstalls it once to check it. Like the macOS app it is built natively on the runner (MSVC, with MSYS2's
`sh` and `make` for FFmpeg).
FFmpeg's libraries come from `scripts/build-ffmpeg.sh` (a pinned, verified release; ADR 0010):
built into the dev image, and once per platform in CI.

For a quick look on a Windows PC's GPU while developing, the dev container cross-builds the
extension too ([ADR 0021](adr/0021-windows-extension-in-the-container.md)):

```sh
scripts/dev.sh scripts/build-gdext.sh release windows
```

It puts `orqwa_gd.dll` (MinGW, linked statically) into `app/bin/`, where the portable Godot
editor on the host loads it to render the views in seconds instead of lavapipe's minutes. It is
for development only; the released Windows app is CI's.

Linux builds come from the `linux` CI job, for x86_64 and arm64: `ORQWA-linux-<arch>.tar.gz`
(a `ORQWA` folder with `ORQWA.<arch>` and `liborqwa_gd.so`; keep both together) and
`orqwa-cli-linux-<arch>.tar.gz`. They are built on Ubuntu 22.04 so they run on any distribution
with glibc 2.35 or newer (Ubuntu 22.04+, Debian 12+, Fedora 36+), not only on ones as new as
the dev container ([ADR 0012](adr/0012-linux-builds.md)). CI starts the exported app
headless to check that it loads.

Docker on macOS cannot access Bluetooth or the GPU, so macOS builds are produced by GitHub Actions.
To test 3D rendering or a real trainer, run the built app (or the portable Godot editor) natively.

Every build names the commit it comes from on the settings' About tab and in its first log
line (`ORQWA 0.2.1 (0ad9af1)`), as Git reports it while building, or the `ORQWA_COMMIT`
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
native macOS, Windows and both Linux builds, and the Android build. After all succeed, the
`release` job verifies every nonempty app/CLI archive (the Android APK among them), creates a
draft with generated release notes, uploads the archives and publishes it. Only that job has
`contents: write`, using the automatic `GITHUB_TOKEN`.

The APK of a tag is signed with ORQWA's release key, from the repository secrets
`ANDROID_KEYSTORE` (the keystore, base64) and `ANDROID_KEYSTORE_PASSWORD` (alias `torqa`); the
build fails without them. Every Android update must carry the same key, so its owner keeps a
copy outside GitHub. Other builds are signed with a debug key.

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

### Android

The APK comes from a container of its own ([ADR 0024](adr/0024-android.md)), built on the dev
image only when needed. `app/` is never changed: the build works on a copy.

```sh
scripts/android.sh scripts/build-android.sh release   # → dist/ORQWA-android-arm64.apk
```

Install it on a tablet with adb over USB or wireless debugging (`adb install -r …`) and read
ORQWA's log with `adb logcat -s ORQWA godot`. Local builds are signed with a debug key made once
in the `orqwa-gradle` volume; `ORQWA_RELEASE_KEYSTORE` and `ORQWA_RELEASE_KEYSTORE_PASSWORD`
sign with a release key instead, as CI does for releases. The APK's version is ORQWA's
(`version/code` 500 for 0.5.0). [android/README.md](../android/README.md) lists
the parts and how to remove Android support again.

## More

- Contributor conventions: [CLAUDE.md](../CLAUDE.md)
- Architecture decisions: [adr/](adr/)
- Roadmap: [PLAN.md](PLAN.md); requirements: [REQUIREMENTS.md](REQUIREMENTS.md)
- Third-party assets and their licences: [CREDITS.md](CREDITS.md)
- Translating the app: [translating.md](translating.md)

## Usage wiki

Rider guides live in the [GitHub wiki](https://github.com/ORQWA/ORQWA/wiki), checked out as
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
   GitHub page names. Use absolute `https://github.com/ORQWA/ORQWA/wiki/<page>` links, and
   `https://raw.githubusercontent.com/ORQWA/ORQWA/main/docs/images/<file>` for shared images.
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
