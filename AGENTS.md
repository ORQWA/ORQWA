# Agent instructions

Read and follow [CLAUDE.md](CLAUDE.md) before working. Its architecture, container-only tooling,
testing and documentation rules apply to every AI agent.

Always work in a separate Git worktree and task branch, including an independent wiki checkout.
Do not implement changes in the user's existing workspace or mix unrelated tasks into a PR.

Application usage documentation is maintained in the `wiki/` Git submodule. Keep it focused on
usage guides: setup, controls, settings, rider workflows and troubleshooting. Update affected
guides, navigation and screenshots in the same session when those user tasks change.
Do not add implementation explanations, visual bug-fix notes, per-PR summaries or scenery
regeneration notices. Pure visual or internal fixes do not require wiki changes. Keep technical
details, validation and review images in the application repository and PR instead.
Keep developer documentation, requirements, roadmap, ADRs and credits current too.

- Run `make wiki-init` before reading/editing the wiki. Preserve all local changes.
- Create or resume a wiki documentation branch before editing; do not work on a detached HEAD.
- Run `make wiki-prepare BRANCH=docs/<change>` to validate, commit and push the wiki branch.
  Explicitly stage the new pointer with `git add wiki` and include the wiki comparison link
  in the application PR. Do not stage unrelated application changes.
- Never push directly to the published wiki branch. GitHub Actions publishes the prepared
  revision after the application PR merges into `main`; any pointer reconciliation gets a PR.
- If checks, access or publication fail, preserve prepared work and report the remaining action.
  Do not claim publication succeeded without checking the workflow result.

Setup and conflict recovery: [Usage wiki](docs/development.md#usage-wiki).
