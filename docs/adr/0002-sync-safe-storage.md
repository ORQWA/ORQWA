# ADR 0002 — Sync-safe storage

- Status: accepted
- Date: 2026-10-02

## Context

The data directory is configurable and may live in a file-synced folder such as Nextcloud (R30).
SQLite databases inside synced folders are prone to corruption and sync conflicts, because sync
clients copy files while they are being written and cannot merge concurrent changes.

## Decision

- **Files are the source of truth**: each ride is one FIT file plus one JSON metadata file; routes
  and generated worlds are stored as files too. Files are written atomically (write temp, rename).
- A **SQLite index** (history, PRs, search) is a rebuildable cache stored **outside** the data
  directory, in the platform's local cache location. If it is missing or stale, it is rebuilt by
  scanning the data directory.
- Per-profile data lives in per-profile subdirectories to avoid write contention between users.

## Consequences

- Syncing between machines works with any file sync tool; at worst a conflict produces a duplicate
  file, never a corrupt database.
- Index rebuilds cost a scan of the data directory at startup when changes are detected.

## Amendment 2026-10-16: settings per rider and per computer (#257, R78)

`settings.toml` in the data directory held every setting, shared by all riders and, with the
directory synced, by all computers: the last to save won, so one machine's graphics quality
overwrote another's. Now:

- **The computer's own** settings — graphics quality (R43) and full screen (#218) — live in
  `computer.toml` in the cache directory (`torqa_storage::computer`), which is this computer's
  and never synced. Clearing the cache loses them, which costs one choice; the alternative, a
  platform configuration directory, would be a third location to explain.
- **The rider's own** overlay place and size (R55) live with the rider, in
  `profiles/<id>/overlay.toml`, like the HUD layout: they follow the rider between computers.
- `settings.toml` keeps what is shared by design: the active rider, the remembered devices,
  the shifter's buttons, the ride bar and the Courses view.

Values from before are carried over once at start (`profiles::carry_over`): the overlay's
place to every rider without their own, the quality and full screen to the computer where the
app starts first, unless it has chosen already; then they are removed from `settings.toml`,
so a second computer sharing the data starts from its own defaults rather than the first's.

