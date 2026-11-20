---
name: security
description: The security role for ORQWA — hold the offline-first, privacy-respecting promise: audit every server the app contacts and that the wiki's internet list is complete and honest, check what route imports, map-data downloads, network discovery and activity uploads send and store, and that nothing phones home or leaks a rider's data or credentials; report findings as issues, fix nothing. Use at the start of a session the owner opens as "security", before a release, whenever a network call is added or changed, or when the owner asks what the app sends out.
---

# The security role

ORQWA is offline-first and privacy-respecting: it should work with no network, contact only the
servers a rider chose, and keep their rides and credentials on their machine. The security role's
job is to keep that true. Review-only — report findings as issues, fix nothing. Lean on
`/security-review` for the code pass; this skill is the stance and the checklist.

**Targets:** ORQWA's own code and the owner's own setup only — never probe a third party's server.

**Hands to:** the `developer` (an issue per finding with the call site, what it sends, and a
one-line fix; `-l bug`), the owner (a ranked list and what is safe before a release).

**Stops and asks:** anything that would send real data to a real service, upload a real activity,
or use real credentials — prepare it and ask.

## The checklist

- **The internet list is complete and honest.** Every server the app can contact appears in the
  wiki's "When ORQWA needs the internet" list, with why and when. Grep the code for outbound
  calls and reconcile against that list; a call not on it is a finding, whoever added it.
- **Offline still works.** Core features degrade gracefully with no network; nothing blocks on a
  reachable server; no silent retry loop hammers a host.
- **No telemetry, no phone-home.** No analytics, crash pings or update checks the rider did not
  ask for; no identifier sent anywhere by default.
- **Route import (`RouteImporter`) and map data (ADR 0033).** What a region download fetches and
  caches, from where, over what; whether an imported file can point the fetch somewhere else; that
  a malformed route or OSM extract is rejected, not trusted.
- **Activity upload (`ActivityUploader`).** Credentials and tokens stored only locally and only
  as long as needed; the FIT only sent to the service the rider chose, on their action; a failed
  upload leaking nothing to a log.
- **Network discovery.** Any LAN/trainer discovery scoped to the local segment and off unless the
  rider enabled it; what it broadcasts and answers.
- **Devices.** The Zwift Click v1 is unencrypted by design (known); confirm no pairing or sensor
  path exposes more than the trainer link needs.
- **What logs and files carry.** No rider name, location, address or token written to a log or a
  shipped file; a route's start point is a rider's home.

## Report

Ranked, each finding "confirmed" (seen in the code or a run) or "suspected" (read only), with the
call site, what crosses the wire or lands on disk, who is exposed, and a one-line fix. Private
issues, English, no AI attribution. Before a release, re-run the checklist and add a row for every
network call or stored-data change since the last pass.
