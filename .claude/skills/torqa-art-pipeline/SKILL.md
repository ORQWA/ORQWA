---
name: torqa-art-pipeline
description: How Torqa makes 3D models and textures — scripted Blender (5.2 LTS, headless, in the x86-64 art container), models exported as .glb into app/assets/models with a manifest the Rust world reads and material names the Godot side styles, tileable PBR textures baked from procedural materials, quick Cycles previews (ADR 0009, ADR 0019). Use when creating or changing anything under art/, app/assets/models/, app/assets/textures/, the model loaders (app/scenes/*_models.gd) or the model shaders, and when bringing in third-party (CC0) bases such as rider bodies.
---

# Torqa's art pipeline

Models are **Blender Python scripts** (the source of truth) plus the **exported `.glb` files**
(committed, so the app builds without Blender). Look and colours follow the `torqa-look` skill.
For bpy itself, the imported skills `blender-python-scripting`, `blender-modeling-modifiers`,
`blender-animation-rigging`, `blender-scene-rendering` and `blender-geometry-nodes` are the
reference — but ignore their "MCP-first" advice: here Blender always runs **headless**, no live
session, no MCP server.

## Running Blender

Never install Blender on the host. It runs in its own container (official Linux builds are
x86-64 only; Docker emulates it on Apple Silicon — fine for scripted work, a build of all
building models takes seconds):

```sh
scripts/art.sh blender --background --factory-startup --python art/<group>/build.py
scripts/art.sh blender --background --factory-startup --python art/<group>/build.py -- <name>…
```

The repository is mounted at `/workspaces/torqa`. Write scratch files to `screenshots/`
(gitignored), never elsewhere in the tree.

## Quick previews (`art/preview.py`)

`build.py` of buildings, vegetation and clouds take `--preview DIR`: instead of exporting, they
render each model with Cycles from four views (`quarter`, `side`, `back`, `top`) into
`DIR/<name>_<view>.png`, on a meadow under a warm sun (a few seconds per view). Materials show
the script's preview colours or textures under Cycles, not the app's shaders, so new materials
can be tried before the app has them; check in the world after (`torqa-render-review`).

```sh
scripts/art.sh blender --background --factory-startup --python art/buildings/build.py     -- --preview screenshots/preview chalet_2_m
```

A new group's build script calls `preview.options()` for the names and directory and
`preview.render(obj, directory, name)` instead of exporting (`ground=False` for things in the
air).

## Textures (`art/textures/build.py`)

Tileable PBR textures baked from procedural node trees: the plane's UV is wrapped onto a torus
in 4D and every noise and Voronoi node samples that point, so nothing has an edge. Each recipe
(`rock`, `grass`, `dirt`, `sand`, `bark`, `asphalt`, …) is a function building its tree with the
`Tree` helper (`noise`, `voronoi`, `ramp`, `math`, `mix`, `output(colour, roughness,
height)`); the bake writes `<name>_albedo.webp` (sRGB), `<name>_normal.webp` (OpenGL tangent
space, as Godot expects) and `<name>_rough.webp` into `app/assets/textures/`: colour and
roughness lossy (quality 90), normal maps lossless, written through the render output with a
view transform that keeps the baked values exactly (see the script).

```sh
scripts/art.sh blender --background --factory-startup --python art/textures/build.py     -- [name …] [--size 1024] [--out screenshots/textures]
```

- Iterate with `--out screenshots/textures --size 512`, and look at a 2 × 2 tiling of the
  albedo to check for seams and repeats.
- `noise(scale, …)`: scale counts whole periods per tile; `streak=True` stretches features
  along v (fibres, blades); `offset` decorrelates two noises at the same scale.
- Commit the textures only together with the shader or model that uses them, with their `.import`
  files set to `compress/mode=2` (VRAM), `mipmaps/generate=true`, `detect_3d/compress_to=0`, and
  `compress/normal_map=1` for normal maps (Godot writes other defaults on first import: fix
  them, then re-import).
- Keep recipes neutral in hue where the app tints per instance (walls, roofs, leaves).
- Where a recipe does not look real, a CC0 scan may replace it (ADR 0019): keep the original
  under `art/sources/<name>/` with `SOURCE.md`, convert it to the same names and formats in a
  script, and credit it in `docs/CREDITS.md`.
- The `rock` recipe looks like paving; it is redone with the vegetation.

## bpy pitfalls

- Import `bpy` before `bmesh`.
- `bpy.ops.object.transform_apply` applies the location too by default; to rotate geometry of
  an object away from the origin, use `mesh.transform(matrix)` instead.
- Meshes the world reads bare (instanced by MultiMesh) lose their object's location, rotation
  and scale: bake transforms into the vertices before export.
- Never leave a file named like a standard Python module (`inspect.py`, `json.py`) beside a
  script Blender runs: it shadows the real one and Blender aborts with obscure errors.
- From Blender 5.0 new materials always have a node tree; the viewport colour
  (`diffuse_color`) and the Principled BSDF's Base Color are separate: set both, or let
  `preview.render` copy the first into the second.

## Layout of a model group (see art/buildings)

- `catalogue.json` — one entry per model: name, kind and parameters (sizes, storeys, pitch…).
- `kinds.py` — one builder per kind; returns the mesh and a dict describing its footprint.
- `kit.py` — reusable parts and the `Mesh` accumulator: faces with **material names**, texture
  coordinates in metres by default, counter-clockwise from the front (glTF convention).
- `build.py` — builds the catalogue (or the named models), exports `app/assets/models/<group>/
  <name>.glb` and writes `models.json` beside them (kind, dimensions, heights…) for the Rust
  world, which embeds it (`include_str!`) to choose and fit models.

Coordinates: metres, x along the model, y across, z up, origin at the footprint centre on the
ground; walls reach 3 m below ground for slopes. glTF/Godot turn Blender's y into −z.

## Contracts — change both sides together

- **Material names → looks.** Files carry no textures; every face has a material *name* and the
  app gives it its look — a texture from `app/assets/textures` and a palette tint
  (`app/scenes/building_models.gd` + `app/shaders/building_model.gdshader` for buildings,
  `app/scenes/vegetation_models.gd` + `vegetation.gdshader` / `foliage.gdshader` for plants).
  A new name needs an entry there.
- **Per-instance variation** comes from MultiMesh data: instance colour (e.g. walls), custom
  data (e.g. roof colour, variant 0–1). Keep it within the palette.
- **`models.json`** fields are read by `core/torqa-world` (e.g. `buildings/models.rs`); a test
  there checks every listed model has its `.glb`.

## Shading and budgets

- Realistic (ADR 0019, `torqa-look`): smooth shading with sharp edges where the object
  has them (`use_smooth` on, shade auto smooth by angle or marked sharp edges); the exporter
  then splits normals only there. Vegetation writes its own normals per corner (smooth within a
  trunk or rock, pointing out of the crown on foliage cards); buildings are boxes and keep one
  normal per face.
- UVs in metres on every surface, so tileable textures keep their real scale.
- Stay within the triangle and texture budgets of `torqa-look`; towns place thousands of
  buildings and forests thousands of trees. Print face counts when building (build.py does).
- Exports are deterministic: rebuilding unchanged scripts must give byte-identical `.glb`s.
  If a rebuild changes files you did not touch, find out why before committing.

## Riders (`art/riders`)

- Realistic bodies (Human Base Meshes) skinned to a skeleton built from the bundle's joints.
  Heat weighting fails on the detailed mesh: weight a thinned copy and hand the weights on
  (Data Transfer, nearest face interpolated). Pose by aiming pose bones at target points
  (`aim`, `turn_bone` about a pivot), never by raw Euler angles: bone roll makes their signs
  unpredictable. Turn the pelvis about the hip joints, or the fit's hips move.
- The riding pose is baked as the rest (apply the armature modifier, `pose.armature_apply`,
  skin again); the app only turns the leg bones (two-bone IK, shortest arc from rest) and moves
  the pelvis bone. Every leg bone needs a child (the foot a `toe`) so the app can read where it
  ends. Keep bone and node names stable — the app finds them by name.
- Hands: finger bones from the bundle's finger joints, curled joint by joint (`grip`); orient
  the hand by its joints (wrist, knuckles, thumb), not its skin's outline, which the spread
  thumb skews; place it by its knuckles. Spare the hands when thinning, or the fingers
  flatten into strips; bending finger vertices without bones tears the mesh.
- Kit edges: thin first, then cut planes across the limbs (`bmesh.ops.bisect_plane` on the
  faces near each limb) and choose materials by position; decimating after dressing makes
  the edges ragged. Unlinked appended objects have no computed world matrix: read `location`.
- Fit to a real rider on the hoods (a race photo, 1.76 m): sit bones 9 cm below the hip joints
  (the standing body's lowest point between the hips is the inner thighs, far lower), hoods
  8 cm below the saddle and 0.57 m ahead of the bottom bracket, the back bending mostly at the
  pelvis. Solve the arm's reach to the wrist, not the hood. Check side views against a photo.
- The bike's shaped parts are `Faces.sweep` sections along `path` curves; chainrings and
  rotors are rings (`annulus`) — solid discs read as pale plates.
- Male and female riders share node and material names; each has their own bike, sized to them.

## Third-party bases (CC0 characters and the like)

- Allowed: own work, CC0, CC-BY, CC-BY-SA (ADR 0009). Not: NC, ND, engine-locked, Mixamo.
- Check the licence at the source (the page, not a re-upload) and record it before using.
- Keep the untouched original under `art/sources/<name>/` with `LICENSE` or `SOURCE.md` (URL,
  author, licence, date); the script derives the Torqa model from it, so changes stay
  reviewable and repeatable.
- List every third-party asset in `docs/CREDITS.md`.

## Before committing

1. Build the group and check the printed face counts; iterate with `--preview DIR`.
2. Render for review in the world: `scripts/dev.sh scripts/render-models.sh` (`MODELS="a b"` or
   `GROUPS="vegetation"` for some; `scripts/render-riders.sh` for riders) — see
   `torqa-render-review`; look at the images, compare with `torqa-look`.
3. Refresh the gallery in `art/README.md`: the same scripts with `GALLERY=docs/images/models`
   (`scripts/dev.sh sh -c 'GALLERY=docs/images/models scripts/render-models.sh'`), and add or
   remove table rows for models added or taken out; update the group's section there too.
4. Commit the scripts, the `.glb`, their `.glb.import` files, `models.json` and the gallery;
   `scripts/check.sh` must pass (it runs the Rust test that every model exists).
5. Put a few review images in the PR (JPEG under `docs/images/`, linked by commit, see
   `torqa-render-review`).
