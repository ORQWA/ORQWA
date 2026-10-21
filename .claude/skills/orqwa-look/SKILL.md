---
name: orqwa-look
description: ORQWA's art direction — as realistic as possible (since 2026-10-18, ADR 0020; replaces the faceted pastel low-poly look) — real proportions, smooth shading, physically based materials and light, textures baked from procedural Blender materials or CC0 scans. Use for any visual work on the 3D world or riders — shaders in app/shaders, colours and geometry from core/orqwa-world, Blender models and textures in art/, lighting, sky, weather, quality presets — and whenever judging a screenshot or render.
---

# ORQWA's look

ORQWA's 3D world and riders are to look **as realistic as possible**: the real place — its
terrain, roads and buildings — with real proportions, smooth shading and physically based
materials and light, like a modern cycling simulator, as far as open, scripted assets reach.
The decision (user, 2026-10-18) replaces the faceted pastel diorama of ADR 0011 completely; no
second look is kept. The ADR is
`docs/adr/0020-realistic-look.md`; it wins over this file if they disagree.

**State (2026-10-18).** Ground and roads, vegetation, buildings, light, sky, water and rain are
restyled; riders are realistic bodies, rigged and posed on the bike, in cycling kit.
`docs/PLAN.md`, Phase 9, lists what is left. Restyle a group as a whole, never half of it. **Even quality beats local peaks**: the
first realistic attempt failed because a few good assets stood beside many poor ones, so every
surface in view on a ride must reach the same level before the next group starts.

## What it means

- **Real forms.** Natural proportions and silhouettes; detail in geometry where it changes the
  silhouette at riding distance (eaves, balconies, window depth, crowns, trunks), in textures
  where it does not (tiles, wood grain, bark, gravel, cracks).
- **Smooth shading.** Smooth normals with hard edges where the object has them (auto-smooth by
  angle, or marked sharp edges). No faceted look.
- **PBR materials.** Albedo, normal and roughness maps. Natural surfaces rough (0.7–0.95);
  metal only where things are metal; water and glass with real reflections, not mirrors.
- **Textures.** Baked from procedural Blender materials first (`art/textures/build.py`,
  skill `orqwa-art-pipeline`), tileable, at a scale that reads right (a roof tile ~0.3 m, a
  brick ~0.25 m). Where a procedural material does not look real (rock, bark and plaster are
  likely), use a **CC0 scan** (Poly Haven, ambientCG; original under `art/sources/`, credited,
  ADR 0020). Judge both side by side in the same light before choosing.
- **Weathering as in nature.** Where water runs and feet and tyres go (darker wall feet,
  streaks under sills, worn wheel tracks, moss on north faces, dust on verges). Variation from
  per-instance colours and texture offsets so repeated models differ, and from blending
  materials (grass into dirt, asphalt into gravel) instead of hard edges.
- **Natural colours.** Close to real albedos (grass about 0.1–0.2, asphalt 0.05–0.1, fresh
  snow 0.8); no pure black or white, no oversaturation. The baked grass texture is about three
  times too bright for that, so `terrain.gdshader` and `street.gdshader` darken it to a deep
  green (the grass tufts' palette colours match); judge grass on a GPU render, as AgX
  compresses sunlit colours and small tint changes barely show.
- **Light.** Physically based sky and sun, soft shadows, ambient occlusion in creases, aerial
  perspective (distant terrain fading into blue-grey haze); a filmic tone mapping (AgX or
  Filmic) instead of the linear one the pastels needed, set when light and sky are restyled.
- **Unchanged from before:** no racing UI or accessories (banners, arches, crowds) (R44/R89).

## Palette

The source of truth for colours picked in code is still `app/assets/palette.json`:
`core/orqwa-world/src/palette.rs` reads it for vertex and instance colours
(`palette::srgb("ground.meadow", alpha)`), `app/scenes/palette.gd` for shader uniforms
(`Palette.color("sky.top")`). Add a colour there and use it by name; never scatter hex values
in code. Its values are the old pastels until their group is restyled: move them to natural
tones with that group. Textures carry their own colour; instance colours tint them (multiply
a neutral albedo) for walls, roofs, leaves and kits. To take colours from a reference image,
cluster them with `scripts/palette.py` beside this file (k-means in the art container's
Blender Python; its header shows how to run it).

## Budgets

- **Triangles:** tree 1–4k with LODs (leaf cards for crowns), bush 300–1.5k, rock 200–1k,
  house 1.5–5k, church 4–10k, rider about 37–39k, bike about 10k, props up to about 300 (a rail span 90). Godot generates LODs on
  import; towns place thousands of buildings and forests thousands of trees, so measure.
- **Textures:** 1024 px per tileable material, 2048 for the road and the ground near it (lower
  on Low); colour and roughness as lossy WebP (quality 90), normal maps lossless WebP, about
  2 MB per material in the repository; VRAM-compressed with mipmaps on import. Share materials
  between models rather than baking per model. No Git LFS below about 150 MB of textures.
- **Frame time:** 60 fps on Medium on an M1 (R43). Check `scripts/render-views.sh` timing and a
  ride on the reference route after every group.
- **Forest density (R103):** mapped woods have their own 4 m candidate grid within 300 m
  of the route and thin to an equivalent 10 m spacing beyond it. Keep wooded slopes wooded
  and clearings clear; scattered plants on open land use a separate 7 m grid. Check the
  canopy from road height and the drone view when changing placement.
- **Tree quality (R108):** the generated forest is Ultra's full population. Upload stable
  position-based subsets before GPU allocation: Low 12.5 %, Medium 25 %, High 50 %.
  Tree ranges are 600 / 1,200 / 2,200 / 3,240 m, with a 15 % fade capped at 300 m.
  View distance overrides range; Tree density overrides the population (R109), with Auto
  following the preset. Keep stored worlds complete
  so quality changes need no regeneration. Check Low as well as Ultra from the same camera.
- **Tree LODs (R43/R108):** keep original nearby meshes and authored medium/far crowns from
  `art/vegetation/lods.py`. Preserve species, crown volumes, materials and every chosen tree.
  Godot selects by projected size for camera and shadow geometry; Medium–Ultra use 160 m
  batches, Low keeps 480 m batches. Review forest coverage from chase and drone cameras,
  check switching while moving, and compare native FPS with Low included.
- **Woodland floor (R104):** spreading and upright ferns grow in patches within 60 m of
  the route, sharing the shrub budget. Keep their arching fronds and paired leaflets legible
  at road height; use `plants.ferns`, the shared `frond` material and the short plant range.

## How to get it

Urban surfaces (R105, ADR 0027) distinguish dense-city paving, commercial/industrial asphalt
and suburban lawns. Mapped parks and other natural land take precedence. Review sidewalk
curbs and access openings from road height, plus an overview for garden/park preservation.
Building paths, forecourts and low walls are inferred; keep their appearance plausible
without claiming exact mapped property boundaries. Shared concrete/paver textures are baked
by the art pipeline. Urban material codes are part of the stored world's format contract.
City benches and paired bins (R106) stand at the raised sidewalk's height, near its outer
edge. Keep entrances and the inner walking strip clear; review their ground contact close up.

- **Models:** scripted Blender (skill `orqwa-art-pipeline`), smooth shading with sharp edges,
  UVs in metres so tileable textures keep their scale. Iterate with `--preview DIR`.
- **Ground and roads (Rust-built meshes):** smooth vertex normals, UVs in world metres, land
  cover blended in the shader from texture layers (grass, dirt, rock, sand, snow) by weights in
  vertex data; steep slopes to rock, triplanar where UVs stretch.
- **Shaders:** Godot spatial shaders sampling the baked maps; `NORMAL_MAP` for normal maps;
  instance colour multiplies a neutral albedo; distance fades fine detail to avoid shimmer.
- **Light and sky:** one `DirectionalLight3D` (`light.sun`), soft shadows, SSAO from Medium up,
  a physical sky, fog by time of day and weather.
- **Kept worlds** (R101): a change to what `core/orqwa-world` builds raises
  `orqwa_world::GENERATOR`, so courses whose worlds are kept offer to regenerate them; shader-
  or model-only changes need nothing. Changing what the shaders read from world data (vertex
  colours, instance buffers) raises `orqwa_world::stored::FORMAT`.

## Reviewing a render

Check, in order: would it pass for a photo at a glance; quality even across the frame (no poor
surface beside a good one); silhouettes and proportions believable from the road; materials read as what
they are (plaster, wood, stone, asphalt) at riding distance and do not shimmer far away; texture
scale right next to the rider; colours natural and harmonious side by side; no visible tiling
repeats over large areas; light warm, shadows soft, AO in creases; distant terrain in haze;
frame time within budget. Quick model checks: `--preview DIR` in the art container; the world:
the `orqwa-render-review` skill.
