"""Builds the props that stand by the road (R89) — guard rails, delineator posts and snow
poles — and exports them for Godot (ADR 0009, ADR 0020).

Run in the art container:
    scripts/art.sh blender --background --factory-startup --python art/props/build.py

The world (`core/torqa-world/src/props.rs`) places them beside the road ridden, each with its
back to the road: the model's +x points away from the road, its length runs along Blender +y
(Godot −z, the way the road is ridden on the right-hand side), the origin on the ground where
it stands. Guard rails are 4 m spans that the world shears to the road's grade, so posts stay
upright; `guard_rail_end` slopes down into the ground ahead (+y), `guard_rail_start` behind.
Each model is one mesh with named materials (`galvanised`, `post`, `band`, `reflector`,
`pole_red`, `pole_white`) that the app colours (`app/scenes/prop_models.gd`). Parts reach
`BURIED` below the origin, so they meet sloping ground. `models.json` lists each model's height.
Pass model names after `--` to build only those, and `--preview DIR` to render them there
instead (art/preview.py).
"""

import json
import math
import os
import sys

import bpy

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, "..", "..", "app", "assets", "models", "props"))
sys.path.insert(0, os.path.dirname(HERE))

import preview  # noqa: E402  (needs the path above)

# Preview colours in Blender only; the app replaces the materials by name.
PREVIEW = {"galvanised": (0.62, 0.64, 0.66), "post": (0.9, 0.9, 0.88), "band": (0.05, 0.05, 0.05),
           "reflector": (0.95, 0.55, 0.1), "pole_red": (0.7, 0.08, 0.06),
           "pole_white": (0.92, 0.92, 0.9)}

# How far posts and blades reach below the ground's level at the origin.
BURIED = 0.4
# A guard rail's span and its W-beam: the profile across it (metres out from the face, up),
# from the bottom edge up, as the common A-profile of European roads, its top 0.75 m high.
SPAN = 4.0
W_BEAM = [(0.0, 0.44), (-0.045, 0.47), (-0.06, 0.52), (-0.035, 0.575), (-0.01, 0.6),
          (-0.035, 0.625), (-0.06, 0.68), (-0.045, 0.73), (0.0, 0.76)]
SHEET = 0.004
# Steps along an end span as it twists down into the ground.
END_STEPS = 8


class Mesh:
    """Faces with material names (z up, metres), shaded flat or smooth per face group."""

    def __init__(self):
        self.verts = []
        self.faces = []
        self.materials = []
        self.smooth = []

    def face(self, points, material, smooth=False):
        start = len(self.verts)
        self.verts.extend(points)
        self.faces.append(list(range(start, start + len(points))))
        self.materials.append(material)
        self.smooth.append(smooth)

    def strip(self, rows, material, smooth=False, closed=False):
        """Quads between consecutive rows of points (each row the same length)."""
        for a, b in zip(rows, rows[1:]):
            count = len(a) if closed else len(a) - 1
            for i in range(count):
                j = (i + 1) % len(a)
                self.face([a[i], a[j], b[j], b[i]], material, smooth)

    def box(self, low, high, material):
        (x0, y0, z0), (x1, y1, z1) = low, high
        corners = [(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0),
                   (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)]
        for face in ((0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6),
                     (3, 0, 4, 7)):
            self.face([corners[i] for i in face], material)

    def pole(self, x, y, radius, bands, material_of, sides=10):
        """A round pole at (x, y) through the heights `bands` (bottom to top), each band
        coloured by `material_of(index)`, capped on top."""
        def ring(z):
            return [(x + radius * math.cos(2 * math.pi * i / sides),
                     y + radius * math.sin(2 * math.pi * i / sides), z) for i in range(sides)]
        for k, (z0, z1) in enumerate(zip(bands, bands[1:])):
            self.strip([ring(z0), ring(z1)], material_of(k), smooth=True, closed=True)
        self.face(ring(bands[-1]), material_of(len(bands) - 2))


def beam(mesh, rows):
    """A W-beam sheet through `rows` of profile points (front face and back), closed at its
    edges."""
    backs = [[(x + SHEET, y, z) for x, y, z in row] for row in rows]
    # The front faces the road (−x): its quads wound so their normals point there.
    mesh.strip([list(reversed(r)) for r in rows], "galvanised", smooth=True)
    mesh.strip(backs, "galvanised", smooth=True)
    for row, back in ((rows[0], backs[0]), (rows[-1], backs[-1])):
        for i in range(len(row) - 1):
            mesh.face([row[i], row[i + 1], back[i + 1], back[i]], "galvanised")


def post(mesh, y, top):
    """A guard-rail post behind the beam, with the spacer block holding the beam off it."""
    mesh.box((0.05, y - 0.03, -BURIED), (0.15, y + 0.03, top), "galvanised")
    mesh.box((SHEET, y - 0.02, 0.5), (0.05, y + 0.02, 0.7), "galvanised")


def guard_rail(spec):
    mesh = Mesh()
    rows = [[(x, y, z) for x, z in W_BEAM] for y in (-SPAN / 2, SPAN / 2)]
    beam(mesh, rows)
    post(mesh, 0.0, 0.72)
    return mesh, 0.76


def guard_rail_end(spec):
    """A span twisting down into the ground and flaring away from the road at its far end:
    +y for `end`, −y for `start` (mirrored)."""
    sign = 1.0 if spec["toward"] == "end" else -1.0
    mesh = Mesh()
    rows = []
    for k in range(END_STEPS + 1):
        t = k / END_STEPS
        y = sign * (-SPAN / 2 + SPAN * t)
        drop = 0.8 * t * t
        flare = 0.35 * t * t
        rows.append([(x + flare, y, z - drop) for x, z in W_BEAM])
    if sign < 0:
        # Mirrored along y: keep the faces wound outward.
        rows = [[(x, y, z) for x, y, z in row] for row in reversed(rows)]
    beam(mesh, rows)
    post(mesh, sign * -SPAN / 4, 0.62)
    return mesh, 0.76


def delineator(spec):
    """A white delineator post, its narrow face toward the road, sloping top, black band
    with a reflector facing the traffic."""
    mesh = Mesh()
    height = 1.0
    front, back, depth = 0.06, 0.03, 0.08
    section = [(-0.0, -front), (-0.0, front), (depth, back), (depth, -back)]

    def ring(z, tilt=0.0):
        return [(x, y, z + tilt * (depth - x) / depth) for x, y in section]

    levels = [-BURIED, 0.72, 0.92]
    rings = [ring(z) for z in levels] + [ring(height - 0.04, 0.04)]
    materials = ["post", "band", "post"]
    for k in range(3):
        mesh.strip([rings[k], rings[k + 1]], materials[k], closed=True)
    mesh.face(list(reversed(rings[-1])), "post")
    # The reflector, just proud of the band on the road side.
    mesh.face([(-0.003, -0.035, 0.76), (-0.003, -0.035, 0.88), (-0.003, 0.035, 0.88),
               (-0.003, 0.035, 0.76)], "reflector")
    return mesh, height


def snow_pole(spec):
    """A slim snow pole, red and white in half-metre bands, its top banded red."""
    mesh = Mesh()
    height = 2.5
    bands = [-BURIED] + [0.5 * k for k in range(1, 6)]
    mesh.pole(0.0, 0.0, 0.02, bands, lambda k: "pole_red" if k % 2 == 0 else "pole_white")
    return mesh, height


KINDS = {"guard_rail": guard_rail, "guard_rail_end": guard_rail_end, "delineator": delineator,
         "snow_pole": snow_pole}
CATALOGUE = [
    {"name": "guard_rail", "kind": "guard_rail"},
    {"name": "guard_rail_end", "kind": "guard_rail_end", "toward": "end"},
    {"name": "guard_rail_start", "kind": "guard_rail_end", "toward": "start"},
    {"name": "delineator", "kind": "delineator"},
    {"name": "snow_pole", "kind": "snow_pole"},
]


def material(name):
    found = bpy.data.materials.get(name)
    if found:
        return found
    created = bpy.data.materials.new(name)
    created.diffuse_color = (*PREVIEW[name], 1.0)
    return created


def to_object(mesh, name):
    data = bpy.data.meshes.new(name)
    data.from_pydata(mesh.verts, [], mesh.faces)
    names = sorted(set(mesh.materials))
    for n in names:
        data.materials.append(material(n))
    index = {n: i for i, n in enumerate(names)}
    data.polygons.foreach_set("material_index", [index[m] for m in mesh.materials])
    data.polygons.foreach_set("use_smooth", mesh.smooth)
    # Points shared by faces of a part (a pole's ring, a beam's rows) are merged, so smooth
    # faces shade smooth across them.
    import bmesh
    shape = bmesh.new()
    shape.from_mesh(data)
    bmesh.ops.remove_doubles(shape, verts=shape.verts, dist=1e-5)
    shape.to_mesh(data)
    shape.free()
    data.update()
    obj = bpy.data.objects.new(name, data)
    bpy.context.scene.collection.objects.link(obj)
    return obj


def export(obj, path):
    for other in bpy.context.scene.objects:
        other.select_set(False)
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_materials="EXPORT",
        export_yup=True,
    )


def main():
    wanted, preview_dir = preview.options()
    os.makedirs(OUT, exist_ok=True)
    manifest = {}
    for spec in CATALOGUE:
        if wanted and spec["name"] not in wanted:
            continue
        bpy.ops.wm.read_factory_settings(use_empty=True)
        mesh, height = KINDS[spec["kind"]](spec)
        obj = to_object(mesh, spec["name"])
        if preview_dir:
            preview.render(obj, preview_dir, spec["name"])
            continue
        export(obj, os.path.join(OUT, spec["name"] + ".glb"))
        manifest[spec["name"]] = {"kind": spec["kind"], "height": round(height, 2)}
        triangles = sum(len(face) - 2 for face in mesh.faces)
        print(f"built {spec['name']}: {triangles} triangles, {height:.2f} m")
    if preview_dir:
        return
    path = os.path.join(OUT, "models.json")
    if wanted and os.path.exists(path):
        with open(path, encoding="utf-8") as file:
            manifest = {**json.load(file)["models"], **manifest}
    with open(path, "w", encoding="utf-8") as file:
        json.dump({"models": dict(sorted(manifest.items()))}, file, indent=1)
        file.write("\n")


main()
