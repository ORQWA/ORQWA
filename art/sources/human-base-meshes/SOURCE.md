# Human Base Meshes — stylized primitive bodies

- **What:** the stylized "primitive" female and male bodies (100 parts: pelvis, torso, limbs,
  hands, feet, head and face parts, each with its origin at its joint), unchanged.
- **From:** *Human Base Meshes* asset bundle v1.4.1 by Blender Studio and contributors,
  <https://www.blender.org/download/demo-files/> (file
  `human-base-meshes-bundle-v1.4.1.zip`, 50,643,039 bytes, SHA-256
  `811f43accbb31a88266d932f8f5563b2d13586fca0ba2693aad1f5fe582b3515`, downloaded
  2026-10-05 from <https://download.blender.org/demo/asset-bundles/human-base-meshes/>).
- **Licence:** CC0 1.0 (public domain dedication), as stated on the Blender demo files page.
- **How:** `extract.py` copies the objects named `*_primitive_stylized*` from the bundle's
  `human_base_meshes_bundle.blend` into `primitive_stylized_bodies.blend`; nothing else is kept.

The riders (`art/riders/build.py`) are derived from this file: posed on the bike, faceted at
their base level, dressed and fitted with a helmet by script (ADR 0011).

## Realistic bodies (for the rigged riders)

- **What:** the realistic female and male bodies (`GEO-body_female_realistic`,
  `GEO-body_male_realistic`) with their sculpted detail applied at the first of the bundle's
  three subdivision levels (42,342 vertices each), and the realistic skeleton's joints: an
  empty at the origin of each bone that marks one (hip, knee, ankle, heel, ball of the foot,
  shoulder, elbow, wrist, the spine's ends, the skull), named after the bone. The rest of the
  skeleton and the finer sculpt levels are left out to keep the repository small (the full
  levels alone are 24 MB).
- **From:** the same bundle and download as above.
- **Licence:** CC0 1.0, as above.
- **How:** `extract_realistic.py` writes `realistic_bodies.blend` from the bundle's
  `human_base_meshes_bundle.blend`.
