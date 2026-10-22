# ADR 0020 — Realistic look instead of faceted low-poly

- Status: accepted
- Date: 2026-10-18
- Supersedes: the look of [ADR 0011](0011-stylized-look.md) (faceted, flat pastel colours); its
  palette file, pipeline and review practice stay

## Context

ADR 0011 replaced the first realistic attempt (photo textures, MyWhoosh as the benchmark) with
a faceted low-poly diorama in pastels, because realism with few assets showed every gap. The
world, vegetation, buildings, clouds and riders are now all faceted. The user's verdict
(2026-10-18): ORQWA should look **as realistic as possible**.

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
  mapping. No racing UI or accessories (R44). The working guide is the `orqwa-look` skill.
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
  exist in the app's shaders; the world is still checked with `orqwa-render-review`.
- **Performance.** 60 fps on Medium on an M1 stays the bar (R43). Mesh LODs from Godot's
  import, texture sizes per preset, and shadows, AO, view distance and vegetation density
  varying by preset keep it there.
- **Order.** Ground and roads, vegetation (with a new rock recipe), buildings, light and sky,
  then riders and bikes — each a PR with renders before and after (`scripts/render-views.sh`).

## Consequences

- Most of ADR 0011's restyling is redone; the logic it kept (route snapping, road shaping,
  building kinds and placement, presets, rider IK) stays.
- Assets grow again: about 2 MB per 1024 px material in the repository; the budgets in
  `orqwa-look` keep this in check.
- More triangles and texture samples cost GPU time; Medium on an M1 has to be measured after
  every group, and presets may need to cut density.
- Riders need smooth, realistic bodies; the faceted Human Base Meshes bases are revisited
  (R46).
- Until a group is restyled it keeps its faceted look; mixed screenshots are expected in
  between.

### Default frame rate (2026-10-21, R111)

Rendering is capped at 60 fps from startup and with the frame rate limit on Auto, at every
quality preset, to avoid unnecessary power use on high-refresh displays. Explicit limits and
Unlimited still apply when chosen; V-Sync remains enabled on Auto and may limit the rate further.

### Forest density (2026-10-21, R103)

Mapped woods use a separate, jittered 4 m candidate grid near the route instead of the
7 m grid for scattered plants on open land. Beyond 300 m they thin to an equivalent 10 m
spacing instead of 20 m, retaining a visible canopy. Mapped forest takes precedence over
rocks inferred from steepness; mapped rock, water, clearings and infrastructure exclusions
still apply. Generator 2 lets riders regenerate kept worlds for this change (ADR 0025).

The Gurten hairpin at the same position: [before](../images/issues/forest-gurten-before.jpg)
and [after](../images/issues/forest-gurten-after.jpg).

### Tree quality budgets (2026-10-21, R108)

The dense generated forest is Ultra's population. Low, Medium and High upload about
12.5 %, 25 % and 50 % of its trees, sampled in Rust from their positions before copying
buffers to Godot. Higher settings keep the lower setting's trees; bushes, ferns and rocks
retain their populations. This reduces instance allocation and drawing on Low rather than
only hiding a complete dense forest. No per-frame sampling is needed.

Tree ranges are 600 / 1,200 / 2,200 / 3,240 m from Low to Ultra, with a fade margin of
15 % capped at 300 m. View distance overrides the tree range independently of density.
Presentation chunks refresh when density changes, including when a loaded course is ridden
again. The generated and stored worlds stay complete and unchanged, so neither generator
nor stored-format versions change; older kept worlds use the same rendering budgets.

The same woodland on [Low](../images/views/forest-low.jpg) and
[Ultra](../images/views/forest-ultra.jpg). These software-rendered views verify appearance;
60 fps on the reference M1 still needs measurement on its GPU.

Tree density is also an independent Graphics detail (R109): Auto follows the quality
preset; Low, Medium, High and Ultra choose the same population fractions above without
changing range, shadows, grass or other effects. The setting lives in `computer.toml` and
applies from the next ride. Resetting graphics details returns it to Auto. This enables
comparing dense and sparse trees with otherwise identical rendering settings before
retuning preset budgets; native GPU FPS remains the evidence for those changes.

### Authored tree LODs (2026-10-21, R43/R108)

Disconnected foliage cards cannot be simplified into a coherent canopy by automatic mesh
decimation alone. `art/vegetation/lods.py` authors medium and far forms for every tree,
retaining species, crown volumes, origins and material names. These rendering assets live
under `vegetation/lods/`, outside the placement catalogue. The original nearby models stay.

`TreeLodMesh` combines their vertex attributes and index levels into Godot's mesh LODs;
selection follows projected size, including viewport resolution, field of view and shadows.
No per-frame instance filtering or buffer copying is needed. Medium through Ultra use
160 m spatial batches so one close tree does not keep an entire 480 m chunk at full detail.
Low retains chunk-sized batches to protect its CPU/draw-call budget. Density and view-distance
controls retain their independent meanings. Neither generator nor stored format changes.

Evaluate this against the same native GPU, course, camera and pixel dimensions; compare Low
as well as dense forests. Smaller batches alone did not improve the tested valley scene,
so a batch-only optimization is insufficient. The base M1 target still needs its own GPU test.

### Woodland ferns (2026-10-21, R104)

Two fern forms add low undergrowth: spreading and upright crowns with arching fronds and
paired geometric leaflets, about 1,400–1,500 triangles each. They reuse the solid `frond`
material, tinted from `plants.ferns`; no texture or third-party asset is added. Shared seeds
over 24 m patches choose ferns within the existing undergrowth budget, under mapped woods
within 60 m of the route. Small-plant draw distance limits their rendering cost. Generator 3
offers regeneration; existing model names and instance buffers stay compatible with kept
worlds, so the stored format remains 1.


### Natural rocks (2026-10-22, R115)

Cliffs and boulders share a non-repeating field of irregular interrupted fractures and broad
mineral variation. Derivative normals add relief at the scale of broken faces. Poly Haven's
CC0 **Rock Boulder Dry** supplies mineral mottling, fissures and grain; a comparison on the
same boulders under identical lighting favoured its eroded faces over procedural noise.
Untouched 1K diffuse and OpenGL normal originals are archived in
`art/sources/rock-boulder-dry/`, credited in `docs/CREDITS.md`, and converted offline by
`art/textures/rock_scan.py`. The app fetches nothing for them.

The material avoids periodic strata and paving cells; irregular exposure patches break up
grass-to-rock edges. Boulders retain their model names, triangle counts and height contract.
Terrain positions, placement and buffers are unchanged, so kept worlds receive the look
without a generator or stored-format increment.

The same wooded cliff face in Bern: [before](../images/issues/rock-cliff-before.jpg),
[after](../images/issues/rock-cliff-close.jpg) and
[overview](../images/issues/rock-cliff-overview.jpg).
