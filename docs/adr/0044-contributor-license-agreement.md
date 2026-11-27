# ADR 0044 — Contributor License Agreement

- Status: proposed
- Date: 2026-11-27

## Context

ORQWA is GPL-3.0. The maintainer wants to distribute it through app stores, whose terms add
restrictions that GPL-3.0 section 10 forbids. GPL-3.0 section 7 allows an additional permission
to cover this, but only the copyright holders can grant it, and several people have contributed
code under plain GPL-3.0 with no agreement beyond it. Every later licensing change, such as an
app store exception or a license for a trainer maker's integration, would mean finding every
contributor again, which gets harder as the project grows.

Options considered:

- **Ask for each change.** No new process, but every change waits on every contributor, and one
  who cannot be reached blocks it or forces a rewrite.
- **DCO (`Signed-off-by`).** Confirms where code comes from but grants no right to relicense.
- **CONTRIBUTING note covering only the app store exception.** Solves today's need; the next
  change starts over.
- **CLA with a license grant.** Contributors keep their copyright and grant a broad license that
  includes relicensing.
- **CLA with copyright assignment.** Gives the most control and puts off the most contributors.

## Decision

Contributors accept the [ORQWA CLA](../../CLA.md), a license grant to the maintainer based on
the Apache Individual CLA, before their first pull request is merged. It:

- covers past contributions as well, so existing contributors sign once instead of agreeing to
  each change;
- lets the maintainer relicense and add exceptions;
- commits the maintainer to keep every version available under an OSI- or FSF-approved license,
  so ORQWA cannot become closed source only;
- can be transferred to a successor maintainer or a legal entity.

CLA Assistant (cla-assistant.io) asks for signatures on pull requests; it is a hosted GitHub
app and adds nothing to the build.

## Consequences

- Existing contributors are asked to accept the CLA. Code from anyone who does not accept stays
  plain GPL-3.0 and is rewritten before any licensing change relies on it.
- First-time contributors take one extra click.
- The app store exception becomes a separate, later change to `LICENSE` once the CLA covers
  all code, and only for code ORQWA owns: third-party GPL-only dependencies would still need
  checking.
