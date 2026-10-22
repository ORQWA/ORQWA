"""Converts the archived CC0 rock scan into ORQWA's shared rock maps (R115, ADR 0020)."""

from pathlib import Path

import bpy

SOURCE = Path(__file__).resolve().parent.parent / "sources" / "rock-boulder-dry"


def convert(size, out):
    """Convert sRGB albedo and OpenGL normals to the app's 8-bit WebP formats."""
    scene = bpy.context.scene
    scene.view_settings.look = "None"
    scene.render.image_settings.file_format = "WEBP"
    scene.render.image_settings.color_mode = "RGB"
    for suffix, kind, ext in [("diff", "albedo", "jpg"), ("nor_gl", "normal", "png")]:
        image = bpy.data.images.load(str(SOURCE / f"rock_boulder_dry_{suffix}_1k.{ext}"))
        if kind == "normal":
            image.colorspace_settings.name = "Non-Color"
        if tuple(image.size) != (size, size):
            image.scale(size, size)
        scene.view_settings.view_transform = "Raw" if kind == "normal" else "Standard"
        scene.render.image_settings.quality = 100 if kind == "normal" else 90
        path = Path(out).resolve() / f"rock_{kind}.webp"
        image.save_render(str(path), scene=scene)
        bpy.data.images.remove(image)
        print("converted", path, flush=True)
