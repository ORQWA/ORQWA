# Human Base Meshes — realistic bodies

The riders (`art/riders/build.py`) are derived from this extract: rigged from the skeleton's
joints, posed on the bike, dressed and fitted with a helmet by script (ADR 0019). (Until
2026-10-18 they came from the bundle's stylized "primitive" bodies; that extract is gone.)

## What and where from

- **What:** the realistic female and male bodies (`GEO-body_female_realistic`,
  `GEO-body_male_realistic`) with their sculpted detail applied at the first of the bundle's
  three subdivision levels (42,342 vertices each), and the realistic skeleton's joints: an
  empty at the origin of each bone that marks one (hip, knee, ankle, heel, ball of the foot,
  shoulder, elbow, wrist, every finger's joints, the spine's ends, the skull), named after the
  bone. The rest of the
  skeleton and the finer sculpt levels are left out to keep the repository small (the full
  levels alone are 24 MB).
- **From:** *Human Base Meshes* asset bundle v1.4.1 by Blender Studio and contributors,
  <https://www.blender.org/download/demo-files/> (file
  `human-base-meshes-bundle-v1.4.1.zip`, 50,643,039 bytes, SHA-256
  `811f43accbb31a88266d932f8f5563b2d13586fca0ba2693aad1f5fe582b3515`, downloaded
  2026-10-05 and again 2026-10-18 from
  <https://download.blender.org/demo/asset-bundles/human-base-meshes/>).
- **Licence:** CC0 1.0 (public domain dedication), as stated on the Blender demo files page.
- **How:** `extract_realistic.py` writes `realistic_bodies.blend` from the bundle's
  `human_base_meshes_bundle.blend`.
