# ADR 0025 — Kept worlds: a course's 3D world kept on the computer that built it

- Status: accepted
- Date: 2026-10-20

## Context

**Ride** on a course page waits while the course's 3D world is built (#237): format 1 course
files hold the inputs only, and every ride builds the world from them (ADR 0007). Measured on an
M5 Pro (18 cores, in the dev container, optimised build), the world alone:

| Course | Length | Chunks | Built in | Stored | Deflated | Read back |
|---|---|---|---|---|---|---|
| Twann – Chasseral | 20 km | 273 | 2.2 s | 238 MB | 103 MB | 0.05 s |
| Bielersee | 56 km | 793 | 10 s | 1.3 GB | 529 MB | 0.26 s |
| Weissenstein – Balmberg | 103 km | 1343 | 18–24 s | 2.7 GB | 1.1 GB | 0.46 s |
| Alpenbrevet Gold | 211 km | 2663 | 25–34 s | 2.8 GB | 1.2 GB | 0.60 s |

A built world is 10–40 times its course file, far beyond the "few hundred MB for long routes"
ADR 0007 expected, and slower computers build for longer. Options considered:

- **Build while the course's page is open** (ADR 0007's amendment of 2026-10-15): small, but
  hides only as much of the wait as the rider spends on the page, and builds for every course
  merely looked at.
- **Ride at once and stream the world in along the route**: no wait at all, on every computer,
  but `orqwa_world::generate` is one job today, and the parts built whole (roads, portals,
  bridges) would still come first.
- **Store the world in the course file** (format 2): shared files ten times larger, and a
  shared world goes stale with each new generator.
- **Keep the world on the computer, opt-in**: no wait from the second ride on (the first, for a
  course prepared from GPX); costs disk only for riders who ask for it.

## Decision

A setting of the computer (`keep_worlds` in `computer.toml`, R78), **off until turned on**,
keeps the 3D world of a library course once it is built (R101):

- **Where.** `<cache>/worlds/<fingerprint>.world`: in the cache directory, never in the course
  file nor the data directory, which may sync (R30). Course files and their format stay as they
  are.
- **Across updates.** A kept world stays; two numbers at the head of every world tell what
  became of it. Releases mostly leave the generator alone (four of the five before this one
  did), so dropping every world with each release would make riders wait for nothing.
  - `orqwa_world::stored::FORMAT`: what a kept world needs to be **read and drawn**. It is
    raised when the stored layout changes (a test reads a sample stored in
    `core/orqwa-world/tests/data/stored-world-<FORMAT>.bin` and fails until it is) or what the
    app makes of the data (a model renamed or removed, another instance buffer layout). A world
    of another format is not read: **Ride** builds and keeps it again. As a guard for a change
    nobody raised the format for, the app leaves out models it no longer has.
  - `orqwa_world::GENERATOR`: what worlds **look like**. It is raised when a change makes the
    same course come out differently. A world of an older generator is ridden as it is, and its
    course page offers **Regenerate world**, which builds and keeps it with the current one. A
    change nobody raised it for only goes unoffered.
  Both are raised by hand, by the change that needs it (CLAUDE.md). A developer changing the
  generator turns the setting off while working on it, or rides worlds kept before the change.
- **Which course.** The fingerprint hashes the course file's track and data entries by the
  checksums and sizes in its zip directory, nothing unpacked; the manifest and the map picture
  are left out, so renaming a course or finding its countries keeps its world.
- **Format.** One zip per world: `whole` (road, structures, railways, horizon) and
  `chunks/<n>`, each deflated at level 1 through the `zip` crate the course files use, so no
  new dependency. Inside, `orqwa_world::stored` writes the types field after field,
  little-endian, after a magic, `FORMAT` and `GENERATOR`. Level 1 shrinks a world 2.4 times;
  reading inflates the chunks on all cores.
- **When.** Kept after a GPX is prepared into a course (the world is built for it anyway),
  after a library course's world is built for a ride, and at once when the setting is turned on
  with a course loaded. Writing runs in the background, about 8 s for a 100 km course.
- **Read.** **Ride** looks for the course's world before building it; a missing, damaged or
  unreadable one is built (and kept) again, which the rider sees only as the usual wait.
- **Gone.** Deleting a course, or its disappearing from the library (also deleted on another
  computer), deletes its world when the worlds are tidied, at start and after keeping one, as
  are worlds this ORQWA cannot read; turning the setting off deletes all.

## Consequences

- A ride on a kept course starts within a second or two instead of after tens of seconds.
- Riders who keep worlds give about 10 MB per kilometre of route; the settings show the total.
  There is no cap: the rider chooses which courses stay in the library.
- Updates cost no waits unless they change the format; when they change the look, the rider
  chooses which courses to build again and when. Nothing is built in the background for courses
  not ridden.
- Kept worlds are only as current as the two numbers: a format change nobody raised `FORMAT`
  for may misdraw a kept world (models it names that are gone are left out), and
  **Regenerate world** is offered only once `GENERATOR` is raised.
- The first ride of a course imported as a file still waits, as does every ride for riders who
  keep no worlds; streaming the world in remains possible later (#237).
- The world in memory is as large as the stored one before deflating (the app holds 2.7 GB for
  a 100 km course), unchanged by this decision.
