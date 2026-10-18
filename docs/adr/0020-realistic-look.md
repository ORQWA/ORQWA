# ADR 0020 — Realistic look instead of faceted low-poly

- Status: accepted
- Date: 2026-10-18
- Supersedes: the look of [ADR 0011](0011-stylized-look.md) (faceted, flat pastel colours); its
  palette file, pipeline and review practice stay

## Context

ADR 0011 replaced the first realistic attempt (photo textures, MyWhoosh as the benchmark) with
a faceted low-poly diorama in pastels, because realism with few assets showed every gap. The
world, vegetation, buildings, clouds and riders are now all faceted. The user's verdict
(2026-10-18): Torqa should look **as realistic as possible**.

Meanwhile the author's earlier Godot game (choppa) showed a way to realistic surfaces with no
asset budget: **procedural PBR materials scripted in Blender and baked to tileable textures**
(colour, normal, roughness), detailed scripted models, and Cycles previews from the build
scripts. A before/after mock-up on the same roadside scene (2026-10-18) showed the baked grass,
gravel and asphalt reading as the real thing at road height; it also showed that textures on
faceted models only make the toy shapes stand out, and that the ported rock recipe looks like
paving.

Decisions (user, 2026-10-18):

1. **As realistic as possible**: real proportions, smooth shading, physically based materials
   and light — the look of a modern cycling simulator, as far as open, scripted assets reach.
   (A "stylized-realistic" middle way was proposed first and dropped after the mock-up.)
2. **Replace** the faceted look; no second look is kept (two looks would double every art
   task, as ADR 0011 argued).
3. First the decision, the tools and the guides; the world is restyled group by group after.

## Decision

- **Look.** Physically based rendering throughout: albedo, normal and roughness maps; metal only
  where things are metal; real reflections on water and glass. Models with real proportions and
  enough geometry for their silhouettes (eaves, window depth, leaf-card tree crowns, rounded
  rock forms); smooth shading with hard edges where objects have them. Natural colours,
  weathering where it happens in nature. Physically based sky, sun and fog, a filmic tone
  mapping. No racing UI or accessories (R44). The working guide is the `torqa-look` skill.
- **Even quality over local peaks.** What made the first attempt fail was a few good assets
  beside many poor ones. A group is restyled as a whole, and every surface in view on a ride
  must reach the same level before the next group starts.
- **Textures.** Baked from procedural Blender materials by `art/textures/build.py` (tileable
  through a 4D torus mapping) by default: no sources to track, any size, consistent style.
  Where procedural materials cannot reach realism (rock, bark and plaster are likely), **CC0
  scanned textures** (Poly Haven, ambientCG) are allowed, as ADR 0009 allows CC0: downloaded
  while authoring, the untouched originals kept under `art/sources/<name>/` with their source,
  and credited in `docs/CREDITS.md`. The app downloads nothing for them.
- **Texture files.** Committed with their Godot import settings, like the `.glb` files:
  colour and roughness as **lossy WebP (quality 90)**, normal maps as **lossless WebP**. Godot
  compresses every texture for the GPU (VRAM compression, mipmaps), which loses more than
  quality 90; lossy WebP would subsample a normal map's colour channels, which are directions.
  Measured on five baked 1024 px materials: 19 MB as PNG, about 13 MB as lossless WebP, about
  10 MB with this split (colour maps 1.1 MB in all), and the normal maps exactly as baked.
  1024 px by default, 2048 px for the road and the ground near it. No Git LFS while the
  textures stay below about 150 MB; past that, decide again. Both GPU layers, S3TC/BPTC and
  ETC2/ASTC, are imported on every editor (`import_s3tc_bptc` and `import_etc2_astc` pinned
  in `project.godot`): Godot's defaults follow the editor's own architecture, and an arm64
  editor imported only ETC2 while the Linux arm64 preset packed S3TC — the arm64 app shipped
  without a single texture and crashed on start (found by CI's smoke test, 2026-10-18).
- **Palette.** `app/assets/palette.json` stays the one place for colours that the Rust core and
  the shaders pick per instance (walls, roofs, leaves, kits); they tint neutral textures, and
  the values move from pastels to natural tones as each group is restyled.
- **Pipeline unchanged** (ADR 0009): Blender scripts are the source, exports are committed,
  Blender runs headless in the art container. New: build scripts render **Cycles previews**
  (`--preview DIR`, `art/preview.py`) with their own materials and textures, before those
  exist in the app's shaders; the world is still checked with `torqa-render-review`.
- **Performance.** 60 fps on Medium on an M1 stays the bar (R43). Mesh LODs from Godot's
  import, texture sizes per preset, and shadows, AO, view distance and vegetation density
  varying by preset keep it there.
- **Order.** Ground and roads, vegetation (with a new rock recipe), buildings, light and sky,
  then riders and bikes — each a PR with renders before and after (`scripts/render-views.sh`).

## Consequences

- Most of ADR 0011's restyling is redone; the logic it kept (route snapping, road shaping,
  building kinds and placement, presets, rider IK) stays.
- Assets grow again: about 2 MB per 1024 px material in the repository; the budgets in
  `torqa-look` keep this in check.
- More triangles and texture samples cost GPU time; Medium on an M1 has to be measured after
  every group, and presets may need to cut density.
- Riders need smooth, realistic bodies; the faceted Human Base Meshes bases are revisited
  (R46).
- Until a group is restyled it keeps its faceted look; mixed screenshots are expected in
  between.
