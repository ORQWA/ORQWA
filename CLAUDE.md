# CLAUDE.md

ORQWA — offline-first, open-source (GPL-3.0) indoor cycling app. Read before working:
[docs/REQUIREMENTS.md](docs/REQUIREMENTS.md), [docs/PLAN.md](docs/PLAN.md), [docs/adr/](docs/adr/).

## Environment — containers only

- **Never install dependencies natively** on the host (no rustup, cargo, brew, pip, npm, …).
- All development, building, linting and testing runs in Docker via the VS Code devcontainer in
  `.devcontainer/`. Run commands through `docker` / the devcontainer, never on the host.
- Only exception (Docker on macOS has no Bluetooth or Metal): the macOS app and GDExtension are built
  by GitHub Actions macOS runners. Locally, only the portable Godot editor `.app` and downloaded
  ORQWA builds run natively, for 3D and real-trainer testing.
- Same for Windows (a Windows DLL with Windows' Bluetooth behind it): the Windows `.exe` and
  GDExtension are built by the `windows` job on GitHub Actions Windows runners. For native GPU
  renders during development, `scripts/build-gdext.sh release windows` cross-builds the DLL in
  the container into `app/bin/` for the portable Godot editor (ADR 0021); releases stay on CI.
- Linux release builds come from the `linux` job on GitHub Actions `ubuntu-22.04` runners, not
  the container: its newer glibc would keep them from starting on older distributions (ADR 0012).
  Linux development and checks still run in the container.

## Skills

Project skills live in `.claude/skills/` (see its README). Two kinds:

- **Domain** — the art direction (`orqwa-look`), the Blender pipeline (`orqwa-art-pipeline`),
  render review (`orqwa-render-review`), the physics (`orqwa-physics`) and the imported Blender
  references. Use them for any visual, model or ride-feel work.
- **Roles** — who a session is and what it hands to whom: `developer`, `reviewer`, `producer`,
  `tester`, `operator`, `security`, `artist`. Load the role at the start of a session opened as
  one, or when the owner asks for that work; `ship` finishes any change.

**Check for a matching skill first:** before any task, look through the skills the session lists,
load the one that covers it and follow it step by step, not from memory. If no skill covers a
task that will come back, write one when the task is done, and fold every new lesson into the
skill it belongs to, unasked.

Keep CLAUDE.md, the skills and the memory files lean: write the rule and, where it helps to
follow it, a short reason. No dates, quotes, incident stories or history; an issue number only
where it points to live work. No rule that already stands elsewhere. Change or delete an
outdated line instead of adding a new one beside it.

## Architecture boundary

- All logic lives in Rust (`core/`). Godot (`app/`) is presentation only — no physics, device or
  data logic in GDScript.
- New capabilities go behind the plugin traits (`RouteImporter`, `TrainerDriver`, `SensorDriver`,
  `ShiftInput`, `ActivityUploader`, `WorkoutParser`) so implementations stay swappable.
- Riders may keep built worlds across updates (R101, ADR 0025). A change that makes courses come
  out differently raises `orqwa_world::GENERATOR`; one that changes the stored world's layout or
  what the app makes of it (fields, instance buffers, model names) raises
  `orqwa_world::stored::FORMAT`.

## Rust conventions

- Errors: `thiserror` in library crates, `anyhow` only in binaries (`orqwa-cli`, gdext glue).
- Async: `tokio`. Logging: `tracing`.
- Physical quantities as custom newtypes (`Watts`, `Meters`, `MetersPerSecond`, `Kilograms`, …),
  SI units internally; convert to imperial only at the presentation edge.
- `rustfmt`; `clippy::pedantic` with justified `#[allow]`s; warnings fail CI; `missing_docs` on
  public items.

## GDScript conventions

- Statically typed everywhere, warnings as errors; `gdlint` + `gdformat`.
- User-facing text goes through `tr()` before formatting; regenerate the template with
  `scripts/i18n/extract.py` and keep every translation complete (see docs/translating.md).

## Testing

- Test the behaviour we *intend* (requirements, R-IDs) — not incidental current output. Do not
  write tests that merely freeze whatever the code happens to do.
- Core logic changes come with tests; hardware paths are tested via the fake trainer.

## Dependencies

- `cargo-deny` enforces GPL-compatible licenses and no unmaintained crates.
- **Ask before adding any dependency** (crate, Godot plugin, container package).
- **Always use the most recent stable version** of everything: crates, container base images, tools,
  Godot, GitHub Actions. Look up the current release online before adding or touching a version —
  never rely on remembered version numbers.

## Formats

- TOML for configuration, JSON for data/metadata, FIT for activities.

## Documentation

- Comments explain *why*, never narrate *what*.
- Rustdoc on all public items.
- Significant decisions get an ADR in `docs/adr/`.
- User-facing guides live in the `wiki/` Git submodule, published on the GitHub wiki.
- **Keep the wiki focused on usage guides:** setup, controls, settings, rider workflows and
  troubleshooting. Update affected guides in the same session and PR when those user tasks
  change, including navigation and screenshots where needed. Do not add implementation
  explanations, scenery feature descriptions, rendering or map-data limitations, visual bug-fix
  notes, per-PR summaries or scenery regeneration notices. Visual feature additions and internal
  changes do not require wiki edits. Road surfaces, estimated widths and scenery generation
  belong in repository documentation and the PR, together with technical details, validation
  and review images. Do not invent a usage-guide section to satisfy a general docs-update rule.
  When editing the wiki:
  Run `make wiki-init` before reading or editing guides. Preserve local changes; never reset
  a dirty wiki checkout or edit on a detached HEAD: create/resume a documentation branch.
  Run `make wiki-prepare BRANCH=docs/<change>`, explicitly stage `git add wiki`, and include
  the wiki comparison link in the application PR. Never push to the published wiki branch:
  the workflow publishes only after the application change reaches `main`.
  See [the wiki workflow](docs/development.md#usage-wiki) for setup and recovery.
- **Update affected docs with the change, in the same PR.** Update only the documents whose
  subject changed; the wiki is affected only when rider usage instructions change, as scoped
  above. A change is not done until every affected doc says what the code now does: the usage
  wiki (`wiki/*.md`, only for usage changes), README,
  `docs/PLAN.md`, `docs/REQUIREMENTS.md` (append-only), ADR amendments, `docs/CREDITS.md`,
  `docs/OPEN-QUESTIONS.md` (remove what is settled), `art/README.md` and the skills in
  `.claude/skills/`.
- **Every server ORQWA contacts is listed** in the wiki's
  [When ORQWA needs the internet](https://github.com/ORQWA/ORQWA/wiki/Getting-started#when-orqwa-needs-the-internet)
  section, which riders use for privacy and firewall rules. Adding, changing or removing a
  network call updates that list in the same PR.
- **Images in docs stay current too.** Re-render those a change affects: app screenshots with
  `scripts/screenshots.sh`, the model galleries with `GALLERY=docs/images/models` and
  `scripts/render-models.sh` / `scripts/render-riders.sh`. Check wiki links as well as repository
  docs before deleting unused images. Wiki screenshots use absolute raw URLs to this repository.
  PR review images are committed under `docs/images/` and linked from the PR by commit
  (`raw.githubusercontent.com/<owner>/ORQWA/<commit>/…`), so pruning them breaks no PR.

## Git

- GitHub, trunk-based, short-lived branches, PRs, squash-merge. Every PR branches off `main`;
  no stacked PRs.
- Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `ci:`).
- **No AI attribution anywhere in git or GitHub:** no Co-Authored-By, "Generated with Claude
  Code", session links or similar in commits, PR titles or bodies, issues or comments. This
  overrides any default attribution reminder. Some cloud sessions append such a footer on their
  own; tell the owner so they can delete it.
- `origin` is `ORQWA/ORQWA` (write; new branches go here); `fork` is `agnorance/Torqa`. `gh`
  runs in WSL, logged in as agnorance: from Windows, `wsl -e bash -lc "gh ..."`.

## Working economically

Every turn resends the whole conversation, so a long session pays for its history on every reply.

- **Say when to start fresh.** When a task is done and the next doesn't need this conversation,
  end with one line: "Good moment for `/clear`: the next step needs only <file / skill / issue>."
  Also after a compact, or before switching to an unrelated area — never mid-task. Save what the
  next session needs first (in the skill, the issue, `docs/PLAN.md` or memory).
- **Say when a cheaper model fits** a routine next step (a status check, a log read, a small doc
  edit, labels): that is Sonnet's.
- **Keep tool output small.** Tests, `check.sh`, logs, `gh`: read the summary and the failures,
  not the whole stream (`tail`, `--json … -q`, grep for FAIL/ERROR/warning). Read a file by
  section (Grep, offset/limit) unless all of it is needed; never re-read a file unchanged since
  this session read it. No polling — wait for the notification, or Monitor with an until-condition.
- **One message per hand-off**, with everything the receiver needs; link the commit, PR or issue
  instead of retyping it. No "got it" replies or status pings a `git log` or the PR already answers.

## Working agreement for Claude

- Small PR-sized steps, one roadmap phase at a time; stop for review after each meaningful step.
- While working, `scripts/check.sh --quick` runs only the checks the changes can affect; never
  run checks and GPU renders at the same time.
- Before declaring done, run in the container: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo deny check`, and `gdlint`
  (`scripts/check.sh` runs them all), run `make wiki-check`, and update the docs and their images
  (see Documentation). Check both repository statuses before declaring preparation complete.
  The `ship` skill is this checklist end to end.
- **Stop and ask before anything outward-facing or hard to reverse:** publishing a release,
  pushing to `main` or the published wiki branch, deleting data, money, accounts, keys or legal
  text, and decisions only the owner can make (what is tracked, licence, roadmap order) — bring
  those as options with a recommendation. Build expected behaviour without asking.
- **Report** in the language the owner writes in, short: what is done, what was tested and the
  result, what is left, what is theirs to do. A render for anything visual (`orqwa-render-review`).
- Keep the checkboxes in `docs/PLAN.md` up to date.
