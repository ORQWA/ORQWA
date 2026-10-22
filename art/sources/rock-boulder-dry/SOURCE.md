# Rock Boulder Dry

- Source: https://polyhaven.com/a/rock_boulder_dry
- Photography: Dimitrios Savva; processing: Rico Cilliers (Poly Haven).
- Licence: CC0 1.0, https://polyhaven.com/license and
  https://creativecommons.org/publicdomain/zero/1.0/
- Downloaded: 2026-10-22. Untouched 1K originals, approximately 1.8 m tall.
- Diffuse: https://dl.polyhaven.org/file/ph-assets/Textures/jpg/1k/rock_boulder_dry/rock_boulder_dry_diff_1k.jpg
- OpenGL normal: https://dl.polyhaven.org/file/ph-assets/Textures/png/1k/rock_boulder_dry/rock_boulder_dry_nor_gl_1k.png

SHA-256 of the originals:

| File | SHA-256 |
|---|---|
| `rock_boulder_dry_diff_1k.jpg` | `20f653a42fb24f42aa40641663070d578678da49af18cc1049dbdd4ed8679f3c` |
| `rock_boulder_dry_nor_gl_1k.png` | `7ae21a2702d613d9d3eedcfb8b42d212b69302c07e2c4fd94ecaa91d52e9345b` |

`art/textures/rock_scan.py` converts these to the shared rock WebP maps. Albedo uses sRGB,
quality 90; the OpenGL normal is quantized to 8-bit lossless WebP without a colour transform
or channel inversion. Run `scripts/art.sh blender --background --factory-startup --python
art/textures/build.py -- rock` to reproduce them. ORQWA fetches nothing for them at runtime.

Comparison on identical boulders under identical lighting favoured the scan's eroded
mineral faces over procedural noise contours. The shared shader adds larger cliff fractures.
