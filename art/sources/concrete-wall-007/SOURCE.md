# Concrete Wall 007

- Source: https://polyhaven.com/a/concrete_wall_007
- Photography: Charlotte Baglioni; processing: Dario Barresi; tiling: Rico Cilliers (Poly Haven).
- Licence: CC0 1.0, https://polyhaven.com/license and
  https://creativecommons.org/publicdomain/zero/1.0/
- Downloaded: 2026-10-23. Untouched 1K originals, covering 2.16 × 2.16 metres.
- Diffuse: https://dl.polyhaven.org/file/ph-assets/Textures/jpg/1k/concrete_wall_007/concrete_wall_007_diff_1k.jpg
- OpenGL normal: https://dl.polyhaven.org/file/ph-assets/Textures/png/1k/concrete_wall_007/concrete_wall_007_nor_gl_1k.png
- Roughness: https://dl.polyhaven.org/file/ph-assets/Textures/jpg/1k/concrete_wall_007/concrete_wall_007_rough_1k.jpg

SHA-256 of the originals:

| File | SHA-256 |
|---|---|
| `concrete_wall_007_diff_1k.jpg` | `1a5ff668a66e147d9fb795d8407ca67a144a1ca010779a5b8386137a4917a086` |
| `concrete_wall_007_nor_gl_1k.png` | `3226418bde0ee3faad7eb8d74f376969c4855e86938330fdbdcc41e5bd53f08e` |
| `concrete_wall_007_rough_1k.jpg` | `5de46a478cbcb4cf4851be9b1dc4a58609c580e498fe758a6959119d45e7a5cb` |

Reproduce the dedicated bridge maps with:

```sh
scripts/art.sh blender --background --factory-startup --python art/textures/build.py -- bridge_concrete
```

`concrete_scan.py` writes sRGB albedo and linear roughness at quality 90, and OpenGL normals
as lossless WebP without a colour transform or channel inversion. The scan replaces a
paving recipe whose repeated joints made bridge walls look tiled. Sidewalks retain that
separate paving material. ORQWA fetches nothing for these textures at runtime.
