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
- Reuse existing own-work textures. Road vertex colours encode material, width, marking
  style and estimated lane/track flags. Geometry without an across-road coordinate, such as
  junction corners and round caps, has no painted markings.
- Add **no provider, network request, dependency or preparation download**. Decode the saved
  tiles at generation; course opening and regeneration remain offline. Course files and their
  format are unchanged. Detailed raw OSM enrichment needs a separate bounded preparation
  design and measurements; do not reinstate the unreliable public Overpass bulk download
  rejected in ADR 0008 or fetch missing surface details at ride start.

## Kept worlds and validation

Generator **9**, stored format **3**. The new vertex-colour meaning requires rebuilding
older kept worlds; the stored-world fixture records the contract. Course inputs stay valid.
Tests cover retained tags, hierarchy and paving overrides, route matching across neighbouring
ways and levels, narrow ridden cycleways, surrounding motorway widths, mesh attribute lengths,
and existing junction, terrain, urban and stored-world behaviour. Render comparisons cover
local streets, main roads and cycleways; native M1 frame time remains a manual check.

## Limits

Lane count, paint and width are inferred from hierarchy and direction. National marking
rules, surveyed lane layouts, exact surface materials, medians and motorway barriers are not
reconstructed from tags that the provider omits. Some mapped data is missing or inaccurate;
the renderer makes a plausible world without claiming a surveyed road model.
