# ADR 0027 — Urban ground, sidewalks and building access

- Status: accepted
- Date: 2026-10-21
- Requirements: R105, R106, R89, R101

## Context

Residential land used the meadow shader and commercial land used gravel, so buildings stood
in grass even in dense city centres. Streets lacked sidewalks and buildings had no paths or
forecourts connecting them to their streets. The existing map contains land use, buildings
and roads, but not every entrance or property boundary. The result must work from a prepared
course offline, without another data provider, and leave parks and suburban gardens green.

## Decision

- Generate surfaces in Rust (`orqwa-world::urban`) from the same map inputs. Index
  buildings, streets and paving spatially; sort buildings by map ID for deterministic
  geometry across runs and chunk boundaries.
- Preserve mapped meadows/parks, woods, water, rock, farmland and orchards. Meadows take
  precedence over residential land. Residential areas with building footprints covering
  over 24% of a 50 m neighbourhood get paving; suburban lawns retain grass. Commercial
  land gets mostly asphalt and industrial yards asphalt with gravel.
- Eligible urban roads have 1.8 m sidewalks and 14 cm curbs. Short pieces leave junctions
  clear; bridges, tunnels, tracks and rural stretches get no inferred urban sidewalks.
  Building access lowers the sidewalk and interrupts its curb.
- Infer a street-facing paved frontage and an access path for buildings near a paved
  street. Businesses get broader forecourts and asphalt access. Small homes can have low
  front-garden walls with a broad opening. These represent plausible surroundings, not
  surveyed entrances or cadastral boundaries.
- Cut surfaces to terrain triangles and chunk edges, avoiding flicker. Keep them out of
  buildings, carriageways, railways, water and mapped green spaces. Vegetation uses the
  same paving decisions, keeping grass out of hard surfaces.
- Bake own concrete and staggered-paver textures (albedo, normal, roughness) with
  `art/textures/build.py`, sharing asphalt with roads. The urban shader reads material
  codes from mesh colours; terrain has separate dense-city and commercial cover codes.
  Wet surfaces darken and show puddles with the existing weather settings.
- Godot stays presentation only: `UrbanSurfaces` supplies shared materials and draws the
  mesh; `WorldMeshes` converts the core's mesh arrays.
- City bench candidates recur every 120 m, alternating sidewalks and trying the other side
  or a nearby spot on the block when blocked. Reuse the wooden bench and bin models at the
  sidewalk's height. Require a continuous, nearly level surface for both; leave entrances,
  junctions and the inner walking
  strip open. A complete pair uses the terrain chunk containing its feet, avoiding coarse
  height samples across a boundary. No extra benches on rural stretches, bridges or tunnels.

## Kept worlds

Generator **5**, stored format **2**. Each chunk gains an `urban` mesh and terrain colour
codes gain urban/commercial kinds. Older kept worlds rebuild on the next ride, as required
for an incompatible layout (ADR 0025); course files and their format are unchanged. The new
stored-world fixture records the layout. The bench placement raises the generator from 4
to 5 without changing the stored format; a format-2 world offers regeneration.

## Validation and limits

Tests cover raised curbs, clear junctions/carriageways, building access and lowered curbs,
garden walls, parks in dense districts, suburban lawns, vegetation exclusion, and rural/bridge
exclusions. Bench tests cover regular seating, paired bins, sidewalk height and trying the
opposite side while keeping access clear. Review images compare the same street and overview
on the Bern fixture, with MSAA and bloom disabled in both software-rendered views to avoid
lavapipe artifacts. Native M1 performance remains a manual check (R43).

Only geometry supplied by the current tiles is authoritative. Inferred access and low walls
can differ from the real property. Pedestrian-only spaces, further street furniture and
neighbourhood-specific styles remain separate work.
