# Building detail

R233 integrates the building study into the production renderer. **Ultra** selects detailed
fitted models and adds nearby façade geometry; Low, Medium and High retain the ordinary
geometry. Scanned plaster and roof materials are shared across presets and preserve the
existing regional palette hues. All resources are bundled and work offline.

## Rendering and kept worlds

`art/buildings/build.py -- --detail` exports a second appearance of every catalogue model
under `app/assets/models/buildings/detail/`, preserving the existing names, dimensions and
placement manifest. These variants add glazing bars and main-roof drainage where appropriate.
Window panes carry normalized UVs plus stable seeds in the second UV channel; the opaque
glass shader suggests curtains, blinds and dark rooms while retaining sky reflections.
Tall French blocks keep their existing balcony/dormer geometry budget and use the interior
shader without extra glazing bars.

`orqwa_world::building_details` reconstructs raised frames, sills, glazing bars, glass and
trim from existing saved shell panels. Plastered/timber window grids, shopfronts, office
ribbons and individual rectangular windows are supported. Unsupported, narrow and
non-rectangular panels keep their existing appearance. Mapped shopfronts suppress the
ordinary ground-floor window detail behind them. Decorations follow each façade's orientation
and are grouped on a 64 m grid for culling.

Only an Ultra upload derives those extra meshes. Fitted models switch between ordinary and
detailed appearances at **160 m**, with a **20 m** fade; shell additions use the same close
range. The range remains fixed when View distance changes. Switching to a lighter preset
refreshes uploaded chunks and releases the detailed model cache. The offline graphics
benchmark uses the same quality-dependent model upload path.

The generated world, stored layout, original GLBs, model names and instance buffers stay
unchanged. Existing kept courses receive the appearance through presentation; changing
quality requires no regeneration. Winter snow conditions also reach the new trim and roof
drainage materials. These are plausible scenery details rather than surveyed façades.

## Materials and review

`art/textures/building_scans.py` converts the archived CC0 plaster and roof scans into
1024 px albedo, OpenGL normal and roughness maps. Albedo is neutralized and normalized in
linear space so wall and roof tints remain meaningful. Colour/roughness use quality-90 WebP;
normal maps are lossless. Godot imports use VRAM compression, mipmaps and normal-map settings.
See [credits](CREDITS.md) and the originals' `SOURCE.md` files for provenance and checksums.

The [initial house](images/building-realism/house-comparison.jpg) and
[town study](images/building-realism/town-comparison.jpg) preserve the design review.
Production Medium/Ultra views and full-course comparisons are recorded with the PR.
Software renders establish appearance and geometry budgets, not native frame rate. Native
Medium/M1 and dense-street Ultra performance remain explicit acceptance work in the roadmap.

## Reproduce

Run all authoring, building and checks in the existing containers:

```sh
scripts/art.sh blender --background --factory-startup --python art/textures/building_scans.py
scripts/art.sh blender --background --factory-startup --python art/buildings/build.py -- --detail
scripts/dev.sh cargo test --manifest-path core/Cargo.toml -p orqwa-world \
  building_detail_render_fixture -- --ignored
scripts/dev.sh sh -c '
  scripts/build-gdext.sh debug &&
  godot --headless --path app --editor --import &&
  timeout 300 xvfb-run -a -s "-screen 0 1280x720x24" \
  godot --path app --rendering-driver vulkan --audio-driver Dummy \
    --resolution 1280x720 -s res://tools/render_building_details.gd
'
```

The controlled views use production assets, materials and Rust-derived shell details, with
matching cameras and light. Refresh the ordinary model gallery using `GROUPS=buildings` and
`GALLERY=docs/images/models` with `scripts/render-models.sh`; `BUILDING_DETAIL=1` selects the
nearby Ultra variant. `RENDER_FRAMES` can shorten static captures with wind disabled.
Pair the fixtures with actual course views, including a town, a regional house, distance
transitions, Low and Winter. The application smoke test covers preset gating, View distance
overrides, loaded-course refresh and original instance-data retention.
