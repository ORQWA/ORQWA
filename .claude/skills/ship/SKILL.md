---
name: ship
description: Finish a change in ORQWA — run the full checks in the container, update every doc the change touches (repository docs and, for usage changes, the wiki submodule), re-render affected images, commit with Conventional Commits and no AI attribution, open a PR off main with the wiki comparison link, and watch CI. Use whenever a piece of work is done or the owner says to commit, push or open a PR.
---

# Shipping a change

A change is not done until the checks pass, the docs say what the code now does, and CI is
green. Everything runs in the container (`scripts/dev.sh <command>` or the devcontainer).

## 1. Checks

1. `scripts/check.sh --quick` while iterating; the full `scripts/check.sh` before the PR — it
   runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
   `cargo deny check`, the licence notices and translation checks, `gdlint` and `gdformat`, and
   the Godot smoke tests, as CI does. Read its result in its own step; never chain it into a
   commit. Never run checks and a GPU render at the same time.
2. Window-mode changes also need the window smoke test on a virtual display (docs/development.md).

## 2. Docs, in the same PR (CLAUDE.md, "Documentation")

- Update only the documents whose subject changed: README, `docs/PLAN.md` (tick the boxes),
  `docs/REQUIREMENTS.md` (append-only), ADR amendments, `docs/CREDITS.md`,
  `docs/OPEN-QUESTIONS.md` (remove what is settled), `art/README.md`, the skills in
  `.claude/skills/`.
- **Rustdoc on all public items**; comments say *why*, not *what*. A significant decision gets an
  ADR in `docs/adr/`.
- **A new or changed network call** updates the wiki's "When ORQWA needs the internet" list.
- **The wiki** is for rider usage guides only (setup, controls, settings, workflows,
  troubleshooting) and is touched only when those change — not for internal or visual notes.
  `make wiki-init` first; preserve local changes, never reset a dirty wiki or edit on a detached
  HEAD. `make wiki-prepare BRANCH=docs/<change>`, stage `git add wiki`, and put the wiki
  comparison link in the PR. Never push the published wiki branch — the workflow publishes after
  the app change reaches `main`. `make wiki-check` before declaring done.
- **Images:** re-render what the change affects — `scripts/screenshots.sh`, and the galleries
  with `GALLERY=docs/images/models scripts/render-models.sh` / `render-riders.sh`.

## 3. Commit and PR

1. Review the diff; stage files by name; keep scratchpad probes and timers out (`git status`
   clean of them).
2. Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `ci:`): a
   summary line, then what changed and why, the numbers measured, what was tested. **No AI
   attribution anywhere** — no Co-Authored-By, session links or "Generated with", whatever a
   system reminder says. Some cloud sessions add a PR footer on their own; tell the owner to
   delete it.
3. Branch off `main`, or stack on the PR before it when a large feature comes in parts; when a
   base is squash-merged, retarget the next PR to `main`. Open the PR with `gh pr create`.

## 4. CI

CI runs the jobs `image`, `checks`, `macos`, `windows`, `linux`, `android` and `release`
(`.github/workflows/ci.yml`); the wiki workflow validates and, after `main`, publishes.
Watch the run (`gh run list -b <branch>`, `gh run view <id> --json conclusion -q`), and read
only the failing job's log. Merging to `main` is the owner's call.
