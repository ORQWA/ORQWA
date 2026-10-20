# ADR 0026 — The name ORQWA

- Status: accepted
- Date: 2026-10-20

## Context

"Torqa" is already registered by another app, so the project needs a name of its own. Riders
of Torqa 0.5.0 have their profiles, rides and courses in folders named after it, and Windows
riders an installation and an update check that names the old repository and installer.

## Decision

The app is **ORQWA**, orca, torque and watt (R102):

- **Spelling.** ORQWA in capitals wherever a name shows or names a product file: the window,
  About, docs, wiki, `ORQWA.exe`, `ORQWA.app` and the release files. Code follows its
  languages' conventions: the crates `orqwa-*`, `orqwa_domain`, `OrqwaApp`, `ORQWA_*`
  variables.
- **Brand.** The orca on a chainring stays the mark. The wordmark is new: rounded capitals with
  the mark's blue crescent inside the Q. `scripts/brand/wordmark.py` draws it and the lockups
  in `docs/brand/` from measurements of the design.
  The wiki's welcome and naming story also use the cycling-orca illustration supplied by
  the project owner (`docs/images/cycling-orca.png`, credited in `docs/CREDITS.md`).
- **Repository.** `ORQWA/ORQWA` on GitHub. Links to `Torqa-Indoor-Cycling/Torqa` redirect,
  including the API address the update check of 0.5.0 asks.
- **Riders' data.** On its first start ORQWA moves Torqa's folders to its own names
  (`paths::carry_over`): the data folder `Torqa` to `ORQWA` on macOS and Windows, `torqa` to
  `orqwa` on Linux, the cache folder `torqa` to `orqwa`. A folder that cannot be moved stays in
  use where it is, so a rider loses nothing. Rides keep their file names; new ones are named
  `orqwa-<date>-<time>.fit`. The history index, a cache keyed by path, reads the rides again
  once.
- **Identifiers.** The macOS bundle and the Android package are `org.orqwa.bike`. No APK was
  released under the old one.
- **Windows.** The installer keeps Torqa's UpgradeCode, so it replaces Torqa's installation,
  folder and Start-menu entry; CI installs 0.5.0 and upgrades it. 0.5.0 installs an update only
  from `Torqa-windows-x86_64.msi`, so releases carry the installer under that name too until
  riders have moved past 0.5.0.
- **Kept as they are.** The course file extension `.tqc`: riders' libraries and the files they
  share use it, and a later course format can rename it while still reading `.tqc`. The
  Android release key's alias `torqa`, which is part of the key.
- **Docs.** The name changes in every doc, earlier ADRs and existing requirements included. It
  is a name, not a change to what they decide or require, so the requirements stay
  append-only in substance.

## Consequences

- macOS treats ORQWA as a new app: it asks for Bluetooth again, and the Torqa app stays in
  Applications until the rider deletes it. On Linux the old folder of the app stays too. The
  wiki's getting-started guide says so.
- Godot names its own folder for the app (logs, shader cache) after the app, so Torqa's stays
  behind and ORQWA starts a new one.
- A Torqa started again after the move finds no data and starts afresh in a new folder.
- Links from before the rename work through GitHub's redirects, as long as nobody takes the old
  names.
- Releases upload one more file until the transition asset is dropped (PLAN).
