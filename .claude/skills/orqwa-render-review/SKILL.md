---
name: orqwa-render-review
description: How to see ORQWA's 3D world and models without a GPU — render screenshots in the dev container with software Vulkan (lavapipe), at a chosen place on a route and from a chosen camera, then judge and share them. Use whenever a change affects what the world, roads, terrain, buildings, vegetation, riders, sky or lighting look like, before declaring it done.
---

# Rendering ORQWA for review

On a Windows PC with a GPU, render natively instead: `scripts/dev.sh scripts/build-gdext.sh
release windows` cross-builds the extension into `app/bin/` (ADR 0021), and the portable Godot
editor on the host runs the same scripts (`app/tools/render_views.gd` with `ROUTE`, `SHOTS`,
`OUT_DIR`) in seconds — a long route in under a minute. Rebuild the DLL after any Rust change;
only the editor runs on the host.

Godot renders in the dev container with **lavapipe** (software Vulkan) on a virtual display.
It is correct but slow: a town scene can take minutes per frame burst, so take few, chosen
shots, run them in the background, and read the PNGs with the Read tool. Output goes to
`screenshots/` (gitignored).

## Ready-made

For road hierarchy and paving (R119), generate the offline comparison meshes with
`cargo test --manifest-path core/Cargo.toml -p orqwa-world road_profiles_render_fixture -- --ignored`,
then render `app/tools/render_roads.gd` with `OUT` set to an absolute PNG path. It shares
`RoadMaterials` and the real Rust generator with the app, rather than redrawing profiles.
Review motorway/ramp shoulders and dashes, unmarked local ways, narrow ridden cycleways and
grass only in unpaved tracks. Pair the controlled scene with fixed real-route views, including
Oberalp and rain; compare against current main using the same cameras and software settings.

- `scripts/dev.sh scripts/render-views.sh` — the **standard views** (`app/tools/render_views.gd`):
  fixed shots on the fixture routes (village, climb, hairpin, lake, rain, morning, evening,
  shops, junctions, river, old town, roundabout, bridges and the Oberalp reference route) into
  `screenshots/views/`. Render them before a visual change (`OUT_DIR=…/views-before`, pass it inside
  `sh -c '…'`) and after, and compare pairs; `VIEWS="village-chase lake-drone"` limits it.
  For a prepared course, set `COURSE=core/orqwa-gd/courses/twann-chasseral.tqc` and
  `SHOTS="shortcut:4200:2:0,180,-20"` (adjust the distance/camera as needed). This opens its saved
  inputs and builds offline, rather than preparing a GPX with fresh provider requests.

- `scripts/dev.sh scripts/screenshots.sh` — the standard app screenshots (`app/tests/
  screenshots.gd`): start page tabs, course page, a ride in each camera, the settings, summary,
  history, analysis, workouts and the overlay. The rider gets 16 weeks of made-up rides first
  (`orqwa-cli sample-rides`), so the history and the Analysis tab have something to show. The
  wiki guides show some of them (assets in `docs/images/app/`, `workouts/`, `overlay/`); copy the new ones
  there when a change alters a screen.
- `scripts/dev.sh scripts/render-models.sh` — every Blender model as the world draws it:
  buildings (three variants and a close-up each) and plants (three variants) into
  `screenshots/models/<group>/`, the clouds together; `GROUPS="vegetation clouds"` or
  `MODELS="house_gable_2_m chalet_2_m"` limits it (pass them inside `sh -c '…'` so they reach
  the container). `scripts/render-riders.sh` does the same for the riders.
- With `GALLERY=docs/images/models`, both also write the small JPEGs of the gallery in
  `art/README.md`; refresh it whenever a model changes.

## A view of a place on a route

Copy a template from `templates/` into `app/tests/` (they are scratch scripts: **never commit
them**; delete them and their `.uid` files afterwards), then run a small runner script with
`scripts/dev.sh sh <runner>`:

- `templates/tmp_road.gd` — loads `core/fixtures/$ROUTE.gpx`, starts a simulated ride, jumps to
  `$DIST` metres, uses camera `$CAM` (0 chase, 1 first person, 2 drone), saves `$OUT`.
- `templates/tmp_buildings.gd` — the same, then a free camera `$HEIGHT` up, `$BACK` behind,
  `$SIDE` aside, looking `$AHEAD` and `$LOOK_SIDE`; with `NEAR_MODEL=any` it aims at the model
  instance nearest the rider from eye height instead (good for street-level detail).

Runner pattern (one `godot` per shot; each loads the route and builds the world):

```sh
set -e
cd /workspaces/orqwa
scripts/build-gdext.sh debug >/dev/null
godot --headless --path app --import >/dev/null 2>&1 || true
shot() {
  name=$1; shift
  env "$@" OUT=$PWD/screenshots/q/$name.png timeout 1500 \
    xvfb-run -a -s "-screen 0 1600x900x24" godot --path app --rendering-driver vulkan \
    --resolution 1280x720 -s res://tests/tmp_road.gd 2>&1 | grep -E "ERROR|SCRIPT" | head -5
  echo "done $name"
}
shot hairpin ROUTE=gurtenstrasse DIST=1240 CAM=2
```

Fixtures: `gurtenstrasse` (Wabern village, a hillside climb with a hairpin), `kirchenfeldbruecke`
(Bern, a bridge, blocks and halls), `bielersee` (lakeside road, vineyards, rock cuttings), and
`oberalp`, the reference route (#167): 33.6 km of a planner's file from Andermatt over the
Oberalp pass to Disentis, with hairpins, three road tunnels, the rack railway alongside, a lake
the planner drew the road through, the summit tunnel and two villages. Render its views after
any change to routes, roads, tunnels or railways; `orqwa-cli route core/fixtures/oberalp.gpx`
prints its figures, and `cargo test -p orqwa-app -- --ignored reference_route` checks them
against the cached tiles. Map and terrain tiles are cached in the container's volumes after
the first run.

For rock changes (R115), render `rock-cliff-overview` and `rock-cliff-close` (the cameras
find a broad steep face on the Bern fixture), plus `rock_block` and `rock_pair` in the model
gallery. Grain alone is insufficient at distance. Refresh the linked wiki images too.

## Pitfalls

For bridge geometry (R121), include a camera below a non-ridden road bridge: top and side
views cannot reveal a missing underside. Check its downward-facing deck across chunk joins,
and retain a view from above to check the road surface. Motorway/local-road decks and
unpaved track/path decks share this geometry; ridden-road and railway structures use another
builder.

For the bridge restyle (R122), compare `oberalp-railway-chase` with `oberalp-bridge-close`,
`oberalp-bridge-under` and `oberalp-bridge-side`, plus Bern's `bridge-side`. Include a tunnel
view when changing the shared structure material. Check low kerbs, open metal railings,
plausible deck widths, capped supports and closed ends without stretching the texture on
vertical faces. `surface_smoke.gd` uses software Vulkan to verify the four ribbon materials
stay opaque above and below; shader flags alone are insufficient evidence.

Bridge ownership needs a slightly offset GPX/map centre line in regression tests: full
cross-section coverage can leave a second mapped deck below the ridden bridge. Preserve
independent parallel and crossing bridges. For Alpenbrevet, the Lukmanier approach has
bridges and tunnels around 34.7–36.8 km from the original course start; use saved course
inputs and check the bridge from the side as well as the riding camera.

For a complete documentation refresh, follow
[`docs/development.md#refreshing-screenshots`](../../../docs/development.md#refreshing-screenshots):
isolate sample data, capture device/error/planned-workout fixtures and licensed local video,
review contact sheets, then export to stable wiki image paths. `SCREENSHOT_DIR` chooses the app
capture folder, and `OUT_DIR` also works for model and rider runners. The app and route wrappers
set `ORQWA_RENDER_SOFTWARE=1` for FXAA without MSAA or glow under lavapipe. Standard route views
wait for all chunks while hidden, pause the rider and include city streets, benches and ferns.

- A script error makes Godot wait forever for `world_ready`: always use `timeout`, and grep the
  output for `SCRIPT ERROR` / `SHADER ERROR` — a scene that fails to compile renders nothing.
- Warnings are errors in this project: untyped `Variant` arguments fail to parse; give values a
  typed local first.
- Free-camera shots can end up inside a hillside; prefer `NEAR_MODEL` or the drone camera.
- Very dense scenes are slow under lavapipe; that says little about a real GPU. For frame rate,
  ask the user to check on their M1.

## Sharing results

Look at every image before reporting. For a PR, convert a few to JPEG on the host
(`sips -s format jpeg -s formatOptions 78 in.png --out docs/images/<topic>/name.jpg`, about
150 KB each), commit them, and reference them in the PR body by the commit that adds them:
`https://raw.githubusercontent.com/ORQWA/ORQWA/<commit>/docs/images/<topic>/name.jpg`. A
commit link keeps working after the branch is deleted and after docs refreshes remove images no
doc uses (#79); a branch link does not.
