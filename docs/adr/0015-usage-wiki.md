# ADR 0015 — Usage wiki published after application merge

- Status: accepted
- Date: 2026-10-13

## Context

Rider documentation should be browsable as a GitHub wiki while agents can edit and review it
alongside application changes. Publishing guides before their code reaches `main` would
describe behavior that readers cannot yet use. The wiki has a separate Git repository and
does not share the application's PR lifecycle.

## Decision

- Check out the GitHub wiki as the `wiki/` submodule. Keep developer guides, requirements,
  roadmap, ADRs, credits and shared images in the application repository.
- Prepare usage edits on wiki branches and include their commit as the application's
  submodule pointer. Review both changes in the application PR.
- Publish the pinned wiki history after the application change reaches `main`, using the
  automatic GitHub Actions token by default. A custom automation token is optional.
- Preserve existing published edits with a merge; stop on conflicts rather than overwrite.
  When the published commit differs from the pointer, open one reconciliation PR containing
  only the pointer update. Never automatically merge that PR.
- Require every AI agent to update affected guides, navigation and images in the same
  session as the relevant feature change, addition or removal.

## Consequences

- Wiki preparation and publication require separate Git commits and pushes. Remote `docs/*`
  preparation branches must remain available until publication; the publisher then deletes
  those whose commits are all published, which keeps every pinned revision reachable.
- Publication is not atomic with the application merge. Failures are visible in the Wiki
  workflow, and manual retries read current application `main` without reverting wiki history.
- Recursive cloning retrieves the reviewed documentation revision. The published wiki may
  temporarily contain additional edits until its reconciliation PR merges.
- Shared image URLs follow application `main`. Documentation checks verify those assets
  against the corresponding application checkout before publication.

Setup and recovery are in [Development](../development.md#usage-wiki).
