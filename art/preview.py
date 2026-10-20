"""Quick previews of freshly built models, rendered by Blender's Cycles in the art container.

A preview shows a model straight from its build script, with the script's own materials and
any baked textures under Cycles, without exporting it, building the extension or changing the
app: the place to try shapes and materials before they exist in the app's shaders. How the
world draws a model is still checked with `scripts/render-models.sh` (skill
orqwa-render-review); colours and textures in the app may differ. Adapted from the author's earlier game project (choppa, tools/blender).

A build script calls `options()` for the model names and the preview directory, and
`render()` instead of exporting when a directory was given:

    scripts/art.sh blender --background --factory-startup --python art/<group>/build.py \
        -- --preview screenshots/preview [name …]
"""

import json
import math
import os
import sys

import bpy
from mathutils import Vector

PALETTE = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "app",
                                        "assets", "palette.json"))

# Directions the camera looks from, towards the model's centre (x along the model, y across,
# z up). The three-quarter view comes first: it shows the most at once.
VIEWS = {
    "quarter": (1.0, -1.3, 0.75),
    "side": (0.0, -1.0, 0.15),
    "back": (-1.0, 1.3, 0.75),
    "top": (0.25, -0.45, 1.6),
    # For models facing +y, such as the riders on their bikes.
    "right": (1.0, 0.0, 0.15),
    "front": (0.6, 1.0, 0.4),
}


def options(argv=None):
    """The model names and the preview directory (None: export) from the arguments after `--`."""
    argv = sys.argv if argv is None else argv
    args = argv[argv.index("--") + 1:] if "--" in argv else []
    directory = None
    if "--preview" in args:
        at = args.index("--preview")
        directory = args[at + 1]
        del args[at:at + 2]
    return args, directory


def colour(path):
    """A palette colour (`"sky.horizon"`, the first of a list) as linear RGB for Blender."""
    with open(PALETTE, encoding="utf-8") as file:
        value = json.load(file)
    for key in path.split("."):
        value = value[key]
    if isinstance(value, list):
        value = value[0]
    srgb = [int(value[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    return tuple(c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in srgb)


def _show_viewport_colour(material):
    """Gives an untextured material its viewport colour, which build scripts set as the
    preview colour, so Cycles shows it (the exported files are not affected)."""
    if material.node_tree is None and hasattr(material, "use_nodes"):
        material.use_nodes = True
    principled = material.node_tree.nodes.get("Principled BSDF")
    if principled is not None and not principled.inputs["Base Color"].is_linked:
        principled.inputs["Base Color"].default_value = material.diffuse_color


def _setup(scene, ground_z, size):
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 32
    scene.cycles.use_denoising = True
    scene.render.resolution_x = 900
    scene.render.resolution_y = 600
    # The app tone-maps linearly; Standard keeps colours closest to what it shows.
    scene.view_settings.view_transform = "Standard"
    world = bpy.data.worlds.new("preview_sky")
    world.use_nodes = True
    background = world.node_tree.nodes["Background"]
    background.inputs["Color"].default_value = (*colour("sky.horizon"), 1.0)
    background.inputs["Strength"].default_value = 0.6
    scene.world = world
    sun = bpy.data.objects.new("preview_sun", bpy.data.lights.new("preview_sun", "SUN"))
    sun.data.energy = 3.5
    sun.data.color = colour("light.sun")
    sun.data.angle = math.radians(2.0)
    # Warm key light from the upper left of the quarter view, as in the world.
    sun.rotation_euler = (math.radians(50), 0.0, math.radians(-30))
    scene.collection.objects.link(sun)
    if ground_z is not None:
        mesh = bpy.data.meshes.new("preview_ground")
        half = size * 4
        mesh.from_pydata([(-half, -half, ground_z), (half, -half, ground_z),
                          (half, half, ground_z), (-half, half, ground_z)], [], [(0, 1, 2, 3)])
        material = bpy.data.materials.new("preview_ground")
        material.use_nodes = True
        material.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (
            *colour("ground.meadow"), 1.0)
        mesh.materials.append(material)
        scene.collection.objects.link(bpy.data.objects.new("preview_ground", mesh))
    camera = bpy.data.objects.new("preview_camera", bpy.data.cameras.new("preview_camera"))
    camera.data.lens = 50
    scene.collection.objects.link(camera)
    scene.camera = camera
    return camera


def render(objects, directory, name, views=None, ground=True):
    """Renders `objects` (one or a list) from `views` into `<directory>/<name>_<view>.png`.

    The camera frames the objects' bounding sphere; with `ground`, they stand on a meadow at
    their lowest point (z = 0 for models whose walls reach below the ground).
    """
    objects = objects if isinstance(objects, (list, tuple)) else [objects]
    scene = bpy.context.scene
    for obj in objects:
        for material in obj.data.materials:
            if material is not None:
                _show_viewport_colour(material)
    bpy.context.view_layer.update()
    corners = [obj.matrix_world @ Vector(corner) for obj in objects for corner in obj.bound_box]
    low = Vector([min(c[i] for c in corners) for i in range(3)])
    high = Vector([max(c[i] for c in corners) for i in range(3)])
    ground_z = (0.0 if low.z < 0 else low.z) if ground else None
    if ground_z is not None:
        # Walls reaching below the ground for slopes stay out of the framing.
        low.z = ground_z
    centre = (low + high) / 2
    radius = max((high - low).length / 2, 0.1)
    camera = _setup(scene, ground_z, radius)
    aspect = scene.render.resolution_y / scene.render.resolution_x
    half_fov = math.atan(camera.data.sensor_width / 2 / camera.data.lens * aspect)
    distance = radius / math.sin(half_fov) * 1.05
    camera.data.clip_end = distance * 10
    os.makedirs(directory, exist_ok=True)
    for view in views or VIEWS:
        direction = Vector(VIEWS[view]).normalized()
        camera.location = centre + direction * distance
        camera.rotation_euler = (-direction).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.abspath(os.path.join(directory, f"{name}_{view}.png"))
        bpy.ops.render.render(write_still=True)
        print("rendered", scene.render.filepath, flush=True)
