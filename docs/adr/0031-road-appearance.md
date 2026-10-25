# ADR 0031 — Mapped road hierarchy, surfaces and cross-sections

- Status: accepted
- Date: 2026-10-23
- Requirements: R119, R89, R35, R101

## Context

All ridden routes were six-metre asphalt roads with edge and centre lines. Surrounding
motorways, trunk roads, primary and secondary roads shared one width; all tracks and paths
were gravel. Paved cycleways became gravel beside the route and wide carriageways when ridden.
The existing OpenFreeMap transportation tiles already contain more information, bundled in
each prepared course. Their surface field normally distinguishes only paved and unpaved;
surveyed width and lane counts are absent.

## Decision

- Retain road hierarchy, surface, one-way and ramp tags separately from the existing broad
  routing classes. Do not change route snapping priorities in this change.
- Use one Rust appearance policy and one shader for ridden and surrounding ways. Explicit
  paving overrides road class. Unknown surfaces use deterministic visual defaults; they are
  not recorded as surveyed materials. Specific material tags are accepted when supplied,
  but current tiles cannot reliably distinguish asphalt, concrete or cobbles.
- Estimate motorway carriageways at 10.5 m, one-way trunk roads at 8.5 m, two-way trunk roads
  at 8 m, primary/secondary/tertiary roads at 7/6.5/6 m, local streets at 5.5 m, service roads
  at 3.5 m, tracks at 2.8 m, cycleways at 2.5 m and other paths at 1.6 m. Ramps are 4.5 m;
  ordinary one-way streets are narrower. These are appearance defaults, not measurements.
- Motorway carriageways have shoulders and long dashed lane dividers. Through roads have
  centre and edge lines, one-lane ramps edge lines only, local roads and paths no inferred
  paint. Only unpaved tracks get a grass centre strip. Keep mapped carriageways separate:
  do not create a duplicate carriageway or median from an offset of the line.
- Match route segments spatially to nearby, aligned map segments on the same bridge/tunnel
  level. Prefer the nearest aligned way so a parallel cycleway retains its own appearance.
  Unmatched route pieces keep a six-metre asphalt fallback. Ease changes of width along the
  route. Terrain, junctions, vegetation clearance, bridge decks, sidewalks and roadside props
  follow the drawn widths. Motorways, trunk roads, ramps and unpaved ways get no inferred
  urban sidewalk.
- Suppress surrounding road geometry only when its whole cross-section is covered by an
  aligned ridden road on the same structure level. Retain the transition segment into that
  covered section; shallow branches must meet the ridden road without a grass gap. Nearby
  parallel paths keep their visible surface. Open the ridden road's shoulder at these mouths.
- Reuse existing own-work textures. Road vertex colours encode material, width, marking
  style and estimated lane/track flags. Geometry without an across-road coordinate, such as
  junction corners and round caps, has no painted markings. Shore strips appended to the
  unpaved-road mesh carry complete plain-gravel metadata too; a partial colour array would
  cause Godot to reject the combined mesh.
- Add **no provider, network request, dependency or preparation download**. Decode the saved
  tiles at generation; course opening and regeneration remain offline. Course files and their
  format are unchanged. Detailed raw OSM enrichment needs a separate bounded preparation
  design and measurements; do not reinstate the unreliable public Overpass bulk download
  rejected in ADR 0008 or fetch missing surface details at ride start.

## Kept worlds and validation

Generator **10**, stored format **3**. The new vertex-colour meaning requires rebuilding
older kept worlds; the stored-world fixture records the contract. Generator 9 introduced the
road profiles; 10 also offers regeneration for the junction and shore-material corrections.
Course inputs stay valid.
Tests cover retained tags, hierarchy and paving overrides, route matching across neighbouring
ways and levels, narrow ridden cycleways, surrounding motorway widths, mesh attribute lengths,
shallow junction continuity across chunk boundaries, nearby parallel paths, and existing
junction, terrain, urban and stored-world behaviour. Render comparisons cover
local streets, main roads and cycleways, plus the bundled Twann–Chasseral course near
Lamboing at 4.2 and 4.6 km with networking disabled. Native M1 frame time remains a manual check.

## Limits

Lane count, paint and width are inferred from hierarchy and direction. National marking
rules, surveyed lane layouts, exact surface materials, medians and motorway barriers are not
reconstructed from tags that the provider omits. Some mapped data is missing or inaccurate;
the renderer makes a plausible world without claiming a surveyed road model.

## Amendment — local traffic lanes (2026-10-23, R120)

The avatar rides halfway into the local half of the carriageway. A spatial width index reads
the flat road mesh between its bevels, interpolates width changes and distinguishes stacked
roads by elevation. This follows the road actually drawn, including kept worlds and a road
ridden again in the opposite direction. Ghosts stay nearer the centre on that same side;
on very narrow paths their translucent models may overlap.

Country-border lookup retains the country at each route point, including short stretches and
repeat crossings. ISO country/territory defaults determine the traffic side, independently of
language and steering-wheel position. Border tiles used on GPX preparation are recorded with
the other course inputs. Course opening reads the existing cache offline; unavailable or
unknown countries keep right. Bundled, simplified outlines distinguish Hong Kong, Macau and
the US Virgin Islands from their parent countries, whose traffic side differs (ISC data,
credited in `docs/CREDITS.md`); these need no downloads. Road-specific exceptions are not
inferred. An older course lacking border tiles uses its saved country when it knows exactly
one; otherwise unknown stretches keep right until the border inputs are available.

Only avatar coordinates change. The original centre-line route still drives recording,
physics, distance, video synchronization and the minimap. Geometry and its stored contract
are unchanged, so generator 10 / format 3 remain valid and no world regeneration is needed.

## Amendment — surfaces from map detail (2026-10-25, ADR 0033)

The tiles still tell only paved and unpaved. Where a course was prepared with a region's map
detail ([ADR 0033](0033-map-detail.md)), each road takes the surface of the mapped way along
it: the same kind of road, within 4 m, aligned (cosine at least 0.9), at most of three points
along each segment, and only a surface the tile's paved or unpaved agrees with — a paved
sidewalk beside an asphalt street, or a dirt surface on a paved road, changes nothing. A
road is split at its own points where the surface changes; pieces under 5 m join their
neighbour, so junctions leave no slivers. The detail is applied after the route is matched,
which still reads the roads as the tiles have them. The rule above stands: the detail comes
from the cache and the course file, never from a download on preparation, opening, ride start
or regeneration. Courses without detail come out as before, so the generator stays.
