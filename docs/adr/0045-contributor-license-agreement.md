# ADR 0045 — Contributor License Agreement

- Status: proposed
- Date: 2026-11-27

## Context

ORQWA is GPL-3.0. The maintainer wants to distribute it through app stores, whose terms add
restrictions that GPL-3.0 section 10 forbids. GPL-3.0 section 7 allows an additional permission
to cover this, but only the copyright holders can grant it, and several people have contributed
code under plain GPL-3.0 with no agreement beyond it. Every later permission, such as an app
store exception or one for a trainer maker's integration, would mean finding every contributor
again, which gets harder as the project grows.

Options considered:

- **Ask for each change.** No new process, but every change waits on every contributor, and one
  who cannot be reached blocks it or forces a rewrite.
- **DCO (`Signed-off-by`).** Confirms where code comes from but grants no right to relicense.
- **CONTRIBUTING note covering only the app store exception.** Solves today's need; the next
  change starts over.
- **CLA with a broad license grant.** Contributors keep their copyright and allow relicensing
  under any terms. More than ORQWA needs, and it puts off contributors who want their work to
  stay GPL.
- **CLA limited to the GPL.** Contributors keep their copyright and allow the maintainer to
  distribute their work under GPL-3.0 or later with additional permissions under its section 7.
- **CLA with copyright assignment.** Gives the most control and puts off the most contributors.

## Decision

Contributors accept the [ORQWA CLA](../../CLA.md), a license grant to the maintainer based on
the Apache Individual CLA, before their first pull request is merged. It:

- covers past contributions as well, so existing contributors sign once instead of agreeing to
  each change;
- lets the maintainer add, change or remove GPL section 7 permissions, such as an app store
  exception, without asking again;
- allows nothing beyond the GPL, version 3 or later: section 7 permissions only loosen its
  terms, so every version stays GPL and ORQWA cannot become closed source. A permissive license
  was ruled out because it would let anyone release a closed fork;
- leaves third-party material under its own license;
- accepts work made with AI tools when the contributor directed and reviewed it;
- can be transferred, under the same terms, only to a successor maintainer or a legal entity
  that continues the Project.

CLA Assistant (cla-assistant.io) asks for signatures on pull requests; it is a hosted GitHub
app and adds nothing to the build. Its signature store also records acceptances made by
comment. Bot accounts (`github-actions[bot]`, `devin-ai-integration[bot]`) are on its
allowlist, and so is Claude's co-author credit (`noreply@anthropic.com`) if CLA Assistant checks
co-authors, since that email has no GitHub account that could sign. The person who directed a
bot or coding agent signs for its commits.

## Consequences

- Existing contributors are asked to accept the CLA. Code from anyone who does not accept stays
  plain GPL-3.0 and is rewritten before any licensing change relies on it.
- First-time contributors sign once, with their full name.
- The app store exception becomes a separate, later change to `LICENSE` once the CLA covers
  all code, and only for code ORQWA owns. Third-party dependencies still need checking: GPL-only
  ones, and FFmpeg, which is LGPL here (ADR 0010) and whose relinking requirement is its own app
  store question. That change also updates the SPDX expression (`GPL-3.0-only` in
  `core/Cargo.toml`).
- The grant is to one person. If the maintainer can no longer act and has not transferred it,
  nobody can add or change permissions. Forming a legal entity or naming a successor avoids this.
- The CLA names no governing law. A lawyer's review of `CLA.md` comes before the first
  signature, since version 1.0 is hard to change once signed.
