# ADR 0033 — Map detail from Geofabrik region downloads

- Status: accepted (steps 1 and 2 of 4: the reader and the extractor; surfaces in prepared courses)
- Date: 2026-10-25
- Requirements: R119, R126, R3, R32, R101

## Context

The OpenFreeMap tiles (ADR 0008) follow the `OpenMapTiles` schema, which reduces OpenStreetMap's
`surface` tag to `paved` or `unpaved` and leaves out `junction=roundabout`. The 1,147 zoom-14
tiles of thirteen prepared courses held no other surface value. Cobbled streets are drawn as
asphalt with lane markings (ADR 0031), and which way round a roundabout goes falls back on the
order its pieces are drawn in (R126), which is wrong where the tiles merge a ring into a longer
line or reverse it (Erlach, Bielersee course, about 20.3 km in).

The data is in OpenStreetMap. Getting it per course was weighed against each server's policy:

- **OpenStreetMap's API** is for editing: "not for read-only purposes or projects".
- **Public Overpass** lists "an app for more than just OSM mappers relying on the public
  instances as backend" as problematic; ADR 0008 measured its failures for bulk downloads.
- **An extract or Overpass server of ORQWA's own** needs a machine for the planet file
  (95.5 GB in October 2026) or an always-on database, and hosting.
- **Geofabrik's region downloads** (`download.geofabrik.de`) are free, updated daily, and cut
  into countries and, for large ones, their parts (Bavaria 856 MB of Germany's 4.9 GB). A rider
  can download the regions they ride in, once.

## Decision

- **Opt-in and manual.** Off by default; a rider turns it on in Settings → Courses and chooses
  regions from Geofabrik's tree (countries and their parts; no continents). ORQWA downloads a
  region only when asked: never on import, course preparation, opening, ride start or world
  regeneration, so R119's rule stands.
- **The download stays in the cache** (`osm/geofabrik/`), listed with its size to update or
  delete, so a later extractor reads it again without another download.
- **Own reader** (`orqwa-osm::detail`) on `prost` and `flate2`, already in use under
  `mvt-reader` and `zip`: no new crates. It declares only the fields it reads, so a pass over
  the ways skips the nodes unread, and unpacks the blobs on every core.
- **Only what the tiles lack is kept**, in `osm/detail-v1/<region>-<date of the data>/12/<x>/<y>.json`:
  - surfaces the tiles cannot tell — concrete, paving, cobbles, dirt, grass — of roads the
    tiles keep (the same class and bicycle rules), with their kind and line;
  - roundabouts (`junction=roundabout|circular`), drawn the way traffic goes round.
  A way is whole in every zoom-12 cell it passes; its id keeps it once across cells and
  regions. The date makes each path stand for one content, as course files expect (ADR 0007).
  `detail-v1` is raised when an extract's content changes.
- **Only courses prepared afterwards use it.** They carry the cells they used, as they carry
  their tiles, and name the extracts in their manifest; opening a course uses exactly those
  (ADR 0007, amendment of 2026-10-25). Video courses ride their video and use none.
- **A region is ready** when its record, `osm/geofabrik/<id>.json` (`id`, `name`, `extract`,
  `data_time`), names an extract that is there; installing a newer download of it removes the
  older extract.
- **Roads take the surfaces** of the mapped ways along them after the route is matched (ADR
  0031, amendment of 2026-10-25): route matching sees the tiles' roads unchanged.
- Four steps, each a PR: the reader and extractor; surfaces in prepared courses; the setting,
  downloads and their list; roundabouts.

## Measurements

`orqwa-cli detail <download> <out>` in the dev container (18 cores):

| Region | Download | Time | Peak memory | Extract | Cells | Surfaces | Roundabout ways |
|---|---|---|---|---|---|---|---|
| Switzerland | 549 MB | 7 s | 78 MB | 36.7 MB | 998 | 83,443 | 5,406 |
| Germany | 4.87 GB | 72 s | 807 MB | 520 MB | 10,010 | 2,325,003 | 37,823 |

Of Switzerland's surfaces, 41,503 are dirt, 15,215 grass, 11,944 concrete, 9,252 paving and
5,529 cobbles; Bern's old town has 51 cobbled ways. Most are paths and tracks: a cell holds
37–52 KB on average, so a 100 km course carries a few MB of it before its file is packed.
Memory grows with the region's matching ways; Germany, among the largest downloads, stays
under 1 GB. Coordinates stored as differences would make extracts smaller if that matters.

## Consequences

- A new server, `download.geofabrik.de`, joins the wiki's list when downloads arrive (step 3).
- Downloads take room: 0.5–5 GB for a country, beside an extract of tens to hundreds of MB.
- The data stays © OpenStreetMap contributors (ODbL 1.0); the cells a course carries share it
  under the same licence, credited as its tiles are.
