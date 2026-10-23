"""Convert the archived CC0 cast-concrete scan for bridge structures (R122)."""

from pathlib import Path

import bpy

SOURCE = Path(__file__).resolve().parent.parent / "sources" / "concrete-wall-007"


def convert(size, out):
    """Preserve photographed albedo, OpenGL normals and linear roughness as WebP."""
    scene = bpy.context.scene
    scene.view_settings.look = "None"
    scene.render.image_settings.file_format = "WEBP"
    scene.render.image_settings.color_mode = "RGB"
    maps = [("diff", "albedo", "jpg"), ("nor_gl", "normal", "png"), ("rough", "rough", "jpg")]
    for suffix, kind, ext in maps:
        image = bpy.data.images.load(str(SOURCE / f"concrete_wall_007_{suffix}_1k.{ext}"))
        if kind != "albedo":
            image.colorspace_settings.name = "Non-Color"
        if tuple(image.size) != (size, size):
            image.scale(size, size)
        scene.view_settings.view_transform = "Standard" if kind == "albedo" else "Raw"
        scene.render.image_settings.quality = 100 if kind == "normal" else 90
        path = Path(out).resolve() / f"bridge_concrete_{kind}.webp"
        image.save_render(str(path), scene=scene)
        bpy.data.images.remove(image)
        print("converted", path, flush=True)
