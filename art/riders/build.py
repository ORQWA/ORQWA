"""Builds the riders on their bikes (R46, ADR 0011) and exports them for Godot.

Run in the art container:
    scripts/art.sh blender --background --factory-startup --python art/riders/build.py

Each rider starts from Blender Studio's CC0 stylized "primitive" body (art/sources/
human-base-meshes): a mannequin of parts with their origins at the joints. The script keeps the
parts at their base level (no subdivision), so they are faceted and lean; drops fingers and
toes; brings the proportions closer to natural ones (longer arms, smaller heads); dresses the
parts by material name; and fits hair, a helmet and sunglasses to the head's own shape (and a
ponytail for the female rider). It then seats the rider: the hips where the legs reach the
pedals almost straight, the torso bent forward until the hands reach the hoods of a bike sized
to fit. The bike is built around those contact points, so each rider gets their own.

`rider_<sex>.glb` holds these nodes, in Godot's axes (x right, y up, −z forward; metres, the
origin on the ground between the wheels):

- `body`: everything of the rider that does not move while riding, posed;
- `thigh_l`, `shin_l`, `foot_l` and the same with `_r`: the legs, each with its origin at its
  joint (hip, knee, ankle), thigh and shin running down their −y axis with their front to −z,
  so the app can aim them at the pedals; the foot level;
- `frame`, `wheel_front`, `wheel_rear`: the bike, the wheels with their origins at the hubs;
- `crankset` (origin at the bottom bracket, the right crank pointing forward) and `pedal_l`,
  `pedal_r` (origins at the axles).

`riders.json` beside them holds what the app needs to pedal: hips, thigh and shin lengths,
where the ankle sits over the pedal axle, and the bike's hubs, bottom bracket and cranks. The
app colours the materials by name from the palette (`rider_female`, `rider_male`, `bike`).
"""

import json
import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Quaternion, Vector
from mathutils.bvhtree import BVHTree

HERE = os.path.dirname(os.path.abspath(__file__))
SOURCE = os.path.normpath(
    os.path.join(HERE, "..", "sources", "human-base-meshes", "primitive_stylized_bodies.blend"))
OUT = os.path.normpath(os.path.join(HERE, "..", "..", "app", "assets", "models", "riders"))

# Parts too small to show at riding distance; the eyes, hidden behind sunglasses, go once the
# sunglasses are fitted to them.
DROP = ("finger", "thumb", "toe")
EYES = ("eye",)
# Parts with many small facets for their size: thinned to this share of their faces.
THIN = {"nose": 0.3, "nose_bridge": 0.45, "ear": 0.35}
# The cycling kit, by part; the rest is skin.
DRESS = {
    "pelvis": "shorts", "leg_upper": "shorts", "belly": "jersey", "chest": "jersey",
    "breast": "jersey", "shoulder": "sleeve", "arm_upper": "sleeve", "hand": "gloves",
    "foot": "shoes",
}
LEGS = ("leg_upper", "leg_lower", "foot")
LEG_NODES = {"leg_upper": "thigh", "leg_lower": "shin", "foot": "foot"}
# The stylized bodies have short arms, a short torso (female) and big heads; Torqa's riders
# have natural proportions (ADR 0011). Arms, hands (standing in for the dropped fingers) and
# feet are lengthened by these factors, the torso by metres, head and shoulder caps scaled.
PROPORTIONS = {
    "female": {"arms": 1.3, "hands": 1.35, "feet": 1.2, "torso": 0.07, "head": 0.86,
               "shoulders": 0.88},
    "male": {"arms": 1.22, "hands": 1.35, "feet": 1.2, "torso": 0.0, "head": 0.86,
             "shoulders": 0.88},
}

# The bike, in Blender's axes: x right, y forward, z up. Built with the bottom bracket at
# y = 0, then moved so the origin lies between the wheels.
WHEEL_RADIUS = 0.34
BOTTOM_BRACKET = Vector((0.0, 0.0, 0.27))
CHAINSTAY = 0.41
CRANK = 0.17
PEDAL_OFFSET = 0.12
SEAT_ANGLE = math.radians(74.0)
HEAD_ANGLE = math.radians(72.5)
FORK_RAKE = 0.045
# Axle to crown, along the steering axis.
FORK_LENGTH = 0.37
STEM = 0.1
HOODS_HALF_WIDTH = 0.2
# Saddle top to hoods top.
DROP_TO_HOODS = 0.03
# The hoods at least this far ahead of the bottom bracket, so the front wheel clears the feet.
HOODS_AHEAD = 0.5
# Bike fit: the knee's bend with the pedal at the bottom, and the foot's pitch there (toes
# down; app/scenes/rider_avatar.gd turns the feet so, ANKLE_PITCH and ANKLE_SWING).
KNEE_AT_BOTTOM = math.radians(30.0)
FOOT_AT_BOTTOM = math.radians(-18.0)
# Arm extension on the hoods: shoulder to wrist as a share of the arm, elbows a little bent.
REACH = 0.92
# The torso's angle above horizontal: upright enough to look relaxed, flat enough to reach.
TORSO_ANGLES = (math.radians(50.0), math.radians(28.0))
# The wrist sits behind and above the hood's top, the palm on it.
WRIST_ON_HOOD = Vector((0.0, -0.06, 0.045))

PREVIEW = {
    "skin": (0.74, 0.53, 0.47), "jersey": (0.94, 0.71, 0.8), "sleeve": (0.81, 0.35, 0.65),
    "shorts": (0.18, 0.24, 0.48), "gloves": (0.38, 0.26, 0.4), "shoes": (0.22, 0.35, 0.63),
    "helmet": (0.96, 0.76, 0.43), "glasses": (0.29, 0.18, 0.27), "hair": (0.38, 0.26, 0.4),
    "vents": (0.29, 0.18, 0.27),
    "frame": (0.89, 0.44, 0.37), "tyre": (0.29, 0.18, 0.27), "rim": (0.95, 0.92, 0.85),
    "metal": (0.56, 0.58, 0.61), "saddle": (0.38, 0.26, 0.4), "bar": (0.38, 0.26, 0.4),
    "socks": (0.92, 0.92, 0.9), "soles": (0.1, 0.1, 0.11),
}
# Kit painted on the lower end of a part (share of its height, in the source's standing pose):
# socks on the shins above the shoes, soles under the shoes.
LOWER_KIT = {"leg_lower": ("socks", 0.22), "foot": ("soles", 0.2)}


def material(name):
    found = bpy.data.materials.get(name)
    if found:
        return found
    created = bpy.data.materials.new(name)
    created.diffuse_color = (*PREVIEW[name], 1.0)
    return created


def update():
    bpy.context.view_layer.update()


def origin(obj):
    return obj.matrix_world.translation.copy()


def transform(obj, matrix, about):
    """Applies a 3×3 `matrix` (turn or scale) to an object about a world point; parts that hang
    from it follow."""
    pivot = Matrix.Translation(about) @ matrix.to_4x4() @ Matrix.Translation(-about)
    obj.matrix_world = pivot @ obj.matrix_world
    update()


def move(obj, offset):
    obj.matrix_world = Matrix.Translation(offset) @ obj.matrix_world
    update()


def world_vertices(obj):
    return [obj.matrix_world @ v.co for v in obj.data.vertices]


def bounds(obj):
    points = world_vertices(obj)
    low = Vector((min(p.x for p in points), min(p.y for p in points), min(p.z for p in points)))
    high = Vector((max(p.x for p in points), max(p.y for p in points), max(p.z for p in points)))
    return low, high


def middle(a, b):
    return (a + b) / 2.0


def two_bone(start, end, first, second, pole):
    """The middle joint of a limb from `start` to `end` with segments `first` and `second`,
    bent towards `pole`."""
    span = end - start
    reach = min(span.length, (first + second) * 0.999)
    along = span.normalized()
    distance = (first * first - second * second + reach * reach) / (2.0 * reach)
    out = math.sqrt(max(first * first - distance * distance, 0.0))
    bend = (pole - along * pole.dot(along)).normalized()
    return start + along * distance + bend * out


def outward(face, inside):
    """The face wound counter-clockwise seen from outside, away from the point `inside`."""
    normal = (face[1] - face[0]).cross(face[2] - face[0])
    centre = sum(face, Vector()) / len(face)
    return face if normal.dot(centre - inside) >= 0.0 else list(reversed(face))


class Faces:
    """Faces with a material name each, made into an object at the end."""

    def __init__(self):
        self.faces = []

    def add(self, points, material_name, inside=None):
        points = [Vector(p) for p in points]
        self.faces.append((outward(points, inside) if inside is not None else points,
                           material_name))

    def tube(self, start, end, radius, material_name, sides=10, caps=True):
        start, end = Vector(start), Vector(end)
        axis = (end - start).normalized()
        helper = Vector((0, 0, 1)) if abs(axis.z) < 0.9 else Vector((1, 0, 0))
        side = axis.cross(helper).normalized()
        across = axis.cross(side)

        def ring(centre):
            return [centre + radius * (math.cos(2 * math.pi * k / sides) * side
                                       + math.sin(2 * math.pi * k / sides) * across)
                    for k in range(sides)]

        a, b = ring(start), ring(end)
        inside = middle(start, end)
        for k in range(sides):
            j = (k + 1) % sides
            self.add([a[k], a[j], b[j], b[k]], material_name, inside)
        if caps:
            self.add(a, material_name, inside)
            self.add(b, material_name, inside)

    def prism(self, bottom, top, material_name):
        """A solid between two outlines with the same number of corners."""
        inside = (sum(bottom, Vector()) + sum(top, Vector())) / (2 * len(bottom))
        count = len(bottom)
        for k in range(count):
            j = (k + 1) % count
            self.add([bottom[k], bottom[j], top[j], top[k]], material_name, inside)
        self.add(bottom, material_name, inside)
        self.add(top, material_name, inside)

    def box(self, centre, size, material_name):
        hx, hy, hz = (s / 2.0 for s in size)
        outline = [Vector((-hx, -hy, 0)), Vector((hx, -hy, 0)), Vector((hx, hy, 0)),
                   Vector((-hx, hy, 0))]
        self.prism([centre + p - Vector((0, 0, hz)) for p in outline],
                   [centre + p + Vector((0, 0, hz)) for p in outline], material_name)

    def to_object(self, name, at):
        """An object with its origin at `at` holding the faces."""
        mesh = bpy.data.meshes.new(name)
        vertices, indices, names = [], [], []
        for points, material_name in self.faces:
            indices.append(list(range(len(vertices), len(vertices) + len(points))))
            vertices.extend(p - at for p in points)
            if material_name not in names:
                names.append(material_name)
        mesh.from_pydata([tuple(v) for v in vertices], [], indices)
        for n in names:
            mesh.materials.append(material(n))
        for polygon, (_, material_name) in zip(mesh.polygons, self.faces):
            polygon.material_index = names.index(material_name)
        shared = bmesh.new()
        shared.from_mesh(mesh)
        bmesh.ops.remove_doubles(shared, verts=shared.verts, dist=1e-5)
        shared.to_mesh(mesh)
        shared.free()
        smooth(mesh)
        obj = bpy.data.objects.new(name, mesh)
        obj.matrix_world = Matrix.Translation(at)
        bpy.context.scene.collection.objects.link(obj)
        return obj


def smooth(mesh):
    """Smooth shading, hard only where faces meet at a sharp angle (ADR 0019)."""
    mesh.shade_smooth()
    mesh.set_sharp_from_angle(angle=math.radians(40))


# --- the body --------------------------------------------------------------------------------


def load_body(sex):
    """The parts of one body from the source, keyed by part name (`chest`, `arm_upper.L`, …)."""
    marker = f"_{sex}_primitive_stylized"
    with bpy.data.libraries.load(SOURCE, link=False) as (source, target):
        target.objects = [name for name in source.objects if marker in name]
    parts = {}
    for obj in target.objects:
        bpy.context.scene.collection.objects.link(obj)
        parts[obj.name.replace("GEO-", "").replace(marker, "")] = obj
    update()
    for part in [p for p in parts if p.startswith(DROP)]:
        bpy.data.objects.remove(parts.pop(part))
    for part, obj in parts.items():
        kind = part.split(".")[0]
        # Left and right share their meshes; each part gets its own, to be reshaped alone.
        obj.data = obj.data.copy()
        obj.modifiers.clear()
        if kind in THIN:
            thin = obj.modifiers.new("thin", "DECIMATE")
            thin.ratio = THIN[kind]
            depsgraph = bpy.context.evaluated_depsgraph_get()
            thinned = bpy.data.meshes.new_from_object(obj.evaluated_get(depsgraph))
            obj.modifiers.clear()
            obj.data = thinned
        # The bases are low-poly: one level of subdivision rounds them into realistic forms
        # (ADR 0019) before they are reshaped and posed.
        rounder = obj.modifiers.new("rounder", "SUBSURF")
        rounder.levels = 1
        depsgraph = bpy.context.evaluated_depsgraph_get()
        rounded = bpy.data.meshes.new_from_object(obj.evaluated_get(depsgraph))
        obj.modifiers.clear()
        obj.data = rounded
        obj.data.materials.clear()
        obj.data.materials.append(material(DRESS.get(kind, "skin")))
        for polygon in obj.data.polygons:
            polygon.material_index = 0
        if kind in LOWER_KIT:
            paint_lower(obj, *LOWER_KIT[kind])
        smooth(obj.data)
    return parts


def paint_lower(obj, material_name, share):
    """Gives the faces in the lowest `share` of the part's height `material_name`."""
    heights = [(obj.matrix_world @ v.co).z for v in obj.data.vertices]
    low, high = min(heights), max(heights)
    cut = low + (high - low) * share
    obj.data.materials.append(material(material_name))
    index = len(obj.data.materials) - 1
    for polygon in obj.data.polygons:
        centre = sum((heights[i] for i in polygon.vertices), 0.0) / len(polygon.vertices)
        if centre < cut:
            polygon.material_index = index


def stretch(obj, start, direction, factor):
    """Lengthens a part beyond `start` along `direction` by `factor`; parts that hang from it
    move along."""
    direction = direction.normalized()
    to_world = obj.matrix_world.copy()
    to_local = to_world.inverted()
    for vertex in obj.data.vertices:
        point = to_world @ vertex.co
        beyond = (point - start).dot(direction)
        if beyond > 0.0:
            vertex.co = to_local @ (point + direction * beyond * (factor - 1.0))
    for child in obj.children:
        beyond = (origin(child) - start).dot(direction)
        if beyond > 0.0:
            move(child, direction * beyond * (factor - 1.0))


def proportion(parts, sex):
    """Brings the standing body (facing −y) to natural proportions."""
    change = PROPORTIONS[sex]
    belly, chest = parts["belly"], parts["chest"]
    waist = origin(belly) + Vector((0, 0, 0.03))
    stretch(belly, waist, Vector((0, 0, 1)),
            1.0 + change["torso"] / (origin(chest).z - waist.z))
    for side in ("L", "R"):
        upper, lower, hand = (parts[f"{p}.{side}"] for p in ("arm_upper", "arm_lower", "hand"))
        stretch(upper, origin(upper), origin(lower) - origin(upper), change["arms"])
        forearm = origin(hand) - origin(lower)
        stretch(lower, origin(lower), forearm, change["arms"])
        stretch(hand, origin(hand), forearm, change["hands"])
        foot = parts[f"foot.{side}"]
        stretch(foot, origin(foot), Vector((0, -1, 0)), change["feet"])
        # Smaller shoulder caps (the mesh alone: the arm hanging from it stays).
        parts[f"shoulder.{side}"].data.transform(Matrix.Scale(change["shoulders"], 4))
    head = parts["head"]
    transform(head, Matrix.Scale(change["head"], 3), origin(head))


def along_part(obj, start, end):
    """Each vertex's and each face's distance along the segment from `start` towards `end`,
    as a share of its length."""
    axis = end - start
    length = axis.length
    axis /= length
    points = world_vertices(obj)
    shares = [(p - start).dot(axis) / length for p in points]
    faces = [sum(shares[v] for v in polygon.vertices) / len(polygon.vertices)
             for polygon in obj.data.polygons]
    return shares, faces


def tailor(parts):
    """Ends the shorts above the knees and the sleeves halfway down the upper arms, and trims
    the thighs at hip and knee so they do not show through when the legs bend."""
    skin = material("skin")
    for side in ("L", "R"):
        thigh, shin = parts[f"leg_upper.{side}"], parts[f"leg_lower.{side}"]
        hip, knee = origin(thigh), origin(shin)
        shares, faces = along_part(thigh, hip, knee)
        axis = knee - hip
        to_local = thigh.matrix_world.inverted()
        for vertex, share in zip(thigh.data.vertices, shares):
            trim = max(share - 1.0, 0.0) + min(share + 0.05, 0.0)
            if trim:
                vertex.co = to_local @ (thigh.matrix_world @ vertex.co - axis * trim)
        for obj, cut, faces in ((thigh, 0.8, faces),
                                (parts[f"arm_upper.{side}"], 0.55,
                                 along_part(parts[f"arm_upper.{side}"],
                                            origin(parts[f"arm_upper.{side}"]),
                                            origin(parts[f"arm_lower.{side}"]))[1])):
            obj.data.materials.append(skin)
            for polygon, share in zip(obj.data.polygons, faces):
                if share > cut:
                    polygon.material_index = 1


def measure_legs(parts):
    """Thigh and shin lengths, and the ball of the foot from the ankle in a frame facing +y."""
    hip, knee, ankle = (origin(parts[f"{p}.L"]) for p in LEGS)
    foot = [p - ankle for p in world_vertices(parts["foot.L"])]
    ahead = [-p.y for p in foot]
    heel, toe = min(ahead), max(ahead)
    ball = Vector((0.0, heel + 0.68 * (toe - heel), min(p.z for p in foot)))
    return (knee - hip).length, (ankle - knee).length, ball


def hips(parts):
    return middle(origin(parts["leg_upper.L"]), origin(parts["leg_upper.R"]))


def shoulders(parts):
    return middle(origin(parts["arm_upper.L"]), origin(parts["arm_upper.R"]))


# Hair and helmet: how far they stand off the scalp, and where the hair ends (metres from the
# eyes' height: at the back, at the sides).
HAIR = {"female": {"thickness": 0.014, "back": -0.085, "sides": -0.035},
        "male": {"thickness": 0.006, "back": -0.05, "sides": -0.005}}
# The helmet sits this far off the scalp (pressing the hair under it flat) and is this thick.
HELMET_GAP = 0.007
HELMET_SHELL = 0.02


def scalp(head, keep, rim=None):
    """The head's faces whose centre `keep` accepts, in world coordinates: points, their
    normals and the faces as lists of point indices. `rim(point)` gives the height the open
    edge is evened out to."""
    mesh = bmesh.new()
    mesh.from_mesh(head.data)
    mesh.transform(head.matrix_world)
    mesh.normal_update()
    chosen = [f for f in mesh.faces if keep(f.calc_center_median())]
    used = sorted({v.index for f in chosen for v in f.verts})
    mesh.verts.ensure_lookup_table()
    index = {v: k for k, v in enumerate(used)}
    points = [mesh.verts[v].co.copy() for v in used]
    normals = [mesh.verts[v].normal.copy() for v in used]
    faces = [[index[v.index] for v in f.verts] for f in chosen]
    mesh.free()
    if rim is not None:
        for k in edge_points(faces):
            points[k].z = rim(points[k])
    return points, normals, faces


def edge_points(faces):
    """The points on the open edge of a patch of faces."""
    count = {}
    for face in faces:
        for a, b in zip(face, face[1:] + face[:1]):
            key = (min(a, b), max(a, b))
            count[key] = count.get(key, 0) + 1
    return {k for (a, b), n in count.items() if n == 1 for k in (a, b)}


def shell(gear, patch, inner, outer, material_name, shape=None):
    """A solid shell over a patch of the scalp, from `inner` to `outer` metres off it (`outer`
    may depend on the scalp's point), closed at its edge; `shape(point)` reshapes the outer
    surface."""
    points, normals, faces = patch
    thickness = outer if callable(outer) else (lambda point: outer)
    low = [p + n * inner for p, n in zip(points, normals)]
    high = [p + n * thickness(p) for p, n in zip(points, normals)]
    if shape is not None:
        high = [shape(p) for p in high]
    count = {}
    for face in faces:
        middle_point = sum((points[k] for k in face), Vector()) / len(face)
        gear.add([high[k] for k in face], material_name, middle_point)
        gear.add([low[k] for k in reversed(face)], material_name)
        for a, b in zip(face, face[1:] + face[:1]):
            key = (min(a, b), max(a, b))
            count[key] = count.get(key, 0) + 1
    for face in faces:
        middle_point = sum((points[k] for k in face), Vector()) / len(face)
        for a, b in zip(face, face[1:] + face[:1]):
            if count[(min(a, b), max(a, b))] == 1:
                gear.add([low[a], low[b], high[b], high[a]], material_name, middle_point)


def tube_along(gear, spine, radii, material_name, sides=10):
    """A faceted tube along a spine of points with a radius at each, closed at both ends (a
    radius of 0 ends in a point)."""
    rings = []
    for k, (point, radius) in enumerate(zip(spine, radii)):
        ahead = spine[min(k + 1, len(spine) - 1)] - spine[max(k - 1, 0)]
        axis = ahead.normalized()
        side = axis.cross(Vector((0, 0, 1))).normalized() if abs(axis.z) < 0.95 else Vector((1, 0, 0))
        across = axis.cross(side)
        # Every other ring turned half a step: facets, not stripes.
        turn = math.pi / sides * (k % 2)
        rings.append([point + radius * (math.cos(2 * math.pi * j / sides + turn) * side
                                        + math.sin(2 * math.pi * j / sides + turn) * across)
                      for j in range(sides)])
    for k, (a, b) in enumerate(zip(rings, rings[1:])):
        inside = middle(spine[k], spine[k + 1])
        for j in range(sides):
            n = (j + 1) % sides
            gear.add([a[j], a[n], b[n], b[j]], material_name, inside)
    gear.add(rings[0], material_name, spine[1])
    gear.add(rings[-1], material_name, spine[-2])


def headgear(parts, sex):
    """Hair, helmet and sunglasses on the upright head facing +y, fitted to its shape and
    parented to it."""
    head = parts["head"]
    low, high = bounds(head)
    centre = middle(low, high)
    half_y = (high.y - low.y) / 2.0
    eyes_l, eyes_r = origin(parts["eye.L"]), origin(parts["eye.R"])
    eyes = middle(eyes_l, eyes_r).z
    hair = HAIR[sex]

    def ahead(point):
        """Front (+1) to back (−1) of the head."""
        return max(-1.0, min(1.0, (point.y - centre.y) / half_y))

    def face(point):
        return ahead(point) > 0.25 and point.z < eyes + 0.065

    def hairline(point):
        back = max(-ahead(point), 0.0)
        return eyes + hair["sides"] + (hair["back"] - hair["sides"]) * back

    def helmet_rim(point):
        # Above the brow in front, above the ears at the sides, lower at the back.
        return eyes + 0.012 + 0.025 * ahead(point)

    def hair_thickness(point):
        # Full where it shows below the helmet, flat under it.
        return hair["thickness"] if point.z < helmet_rim(point) + 0.01 else HELMET_GAP * 0.5

    gear = Faces()
    shell(gear, scalp(head, lambda c: c.z > hairline(c) and not face(c)),
          0.0, hair_thickness, "hair")

    def streamline(point):
        # Drawn out to a point at the back: a road helmet, not a bowl.
        back = max(-ahead(point), 0.0)
        return point + Vector((0.0, -0.035 * back ** 3, 0.004))

    before = len(gear.faces)
    shell(gear, scalp(head, lambda c: c.z > helmet_rim(c), helmet_rim),
          HELMET_GAP, HELMET_GAP + HELMET_SHELL, "helmet", streamline)
    # Vents: long slots from front to back, laid on the helmet's top.
    helmet = [points for points, _ in gear.faces[before:]]
    corners = [p for face in helmet for p in face]
    polygons, start = [], 0
    for face in helmet:
        polygons.append(list(range(start, start + len(face))))
        start += len(face)
    top = BVHTree.FromPolygons(corners, polygons)
    half_x = (high.x - low.x) / 2.0
    for across in (-0.52, -0.18, 0.18, 0.52):
        length = 0.85 - abs(across) * 0.5
        rows = []
        for k in range(7):
            along = 0.55 - (0.55 + length) * k / 6
            edge = []
            for side in (-0.1, 0.1):
                x = centre.x + (across + side) * half_x
                y = centre.y + along * half_y
                hit = top.ray_cast(Vector((x, y, high.z + 0.5)), Vector((0, 0, -1)))[0]
                edge.append(hit + Vector((0, 0, 0.0015)) if hit is not None else None)
            rows.append(edge)
        for (a, b), (c, d) in zip(rows, rows[1:]):
            if None not in (a, b, c, d):
                gear.add([a, b, d, c], "vents", middle(a, d) - Vector((0, 0, 0.02)))

    # Sunglasses on the face: lenses over the eyes following its curve, a bridge over the nose
    # and arms back over the ears.
    surface = [head] + [parts[p] for p in ("nose", "nose_bridge")]
    points, polygons = [], []
    for obj in surface:
        base = len(points)
        points.extend(world_vertices(obj))
        polygons.extend([[base + v for v in p.vertices] for p in obj.data.polygons])
    tree = BVHTree.FromPolygons(points, polygons)

    def on_face(x, z, stand_off=0.01):
        hit = tree.ray_cast(Vector((x, centre.y + 0.5, z)), Vector((0, -1, 0)))[0]
        return hit + Vector((0.0, stand_off, 0.0))

    tops = []
    for eye in (eyes_l, eyes_r):
        out = 1.0 if eye.x > centre.x else -1.0
        columns = [eye.x - out * 0.026, eye.x, eye.x + out * 0.032]
        rows = [eye.z - 0.02, eye.z, eye.z + 0.016]
        grid = [[on_face(x, z) for x in columns] for z in rows]
        # The outer lower corner rounded off.
        grid[0][2] = on_face(columns[2], rows[0] + 0.008)
        inside = Vector((eye.x, centre.y, eye.z))
        for r in range(2):
            for c in range(2):
                gear.add([grid[r][c], grid[r][c + 1], grid[r + 1][c + 1], grid[r + 1][c]],
                         "glasses", inside)
        tops.append((grid[2][0], grid[2][2]))
        ear = origin(parts[f"ear.{'L' if eye is eyes_l else 'R'}"]) + Vector((0, 0, 0.02))
        temple = Vector((ear.x + out * 0.006, middle(grid[2][2], ear).y, eye.z + 0.012))
        hit = tree.ray_cast(temple + Vector((out * 0.3, 0, 0)), Vector((-out, 0, 0)))[0]
        if hit is not None:
            temple = hit + Vector((out * 0.006, 0, 0))
        for a, b in ((grid[2][2] - Vector((0, 0, 0.004)), temple), (temple, ear)):
            gear.tube(a, b, 0.0035, "glasses", sides=4)
    (left_inner, _), (right_inner, _) = tops
    gear.add([left_inner - Vector((0, 0, 0.007)), right_inner - Vector((0, 0, 0.007)),
              right_inner, left_inner], "glasses",
             Vector((centre.x, centre.y, eyes)))

    if sex == "female":
        # A ponytail from under the back of the helmet, with a hair tie in the kit's colour.
        nape = tree.ray_cast(Vector((centre.x, centre.y - 0.5, eyes - 0.005)),
                             Vector((0, 1, 0)))[0]
        spine = [nape + Vector(offset) for offset in (
            (0, 0.01, 0), (0, -0.03, -0.012), (0, -0.065, -0.05), (0, -0.085, -0.105),
            (0, -0.088, -0.16), (0, -0.08, -0.21), (0, -0.07, -0.25))]
        tube_along(gear, spine, (0.022, 0.026, 0.03, 0.028, 0.022, 0.014, 0.0), "hair")
        tie = spine[0] + (spine[1] - spine[0]) * 0.55
        gear.tube(tie - (spine[1] - spine[0]).normalized() * 0.007,
                  tie + (spine[1] - spine[0]).normalized() * 0.007, 0.027, "sleeve")

    obj = gear.to_object("headgear", origin(head))
    obj.parent = head
    obj.matrix_parent_inverse = head.matrix_world.inverted()
    for part in [p for p in parts if p.startswith(EYES)]:
        bpy.data.objects.remove(parts.pop(part))
    update()


def pelvis_bottom(parts):
    """The lowest point of the pelvis between the sit bones: where the saddle meets it."""
    centre = hips(parts)
    points = [p for p in world_vertices(parts["pelvis"]) if abs(p.x - centre.x) < 0.07]
    return min(points, key=lambda p: p.z)


def hoods_for(parts, saddle_top, arm):
    """Where the hoods go for the body as posed: the arms at `REACH`, the hoods
    `DROP_TO_HOODS` below the saddle. None if the arms cannot reach down that far."""
    shoulder = origin(parts["arm_upper.R"])
    wrist_z = saddle_top - DROP_TO_HOODS + WRIST_ON_HOOD.z
    across = HOODS_HALF_WIDTH - shoulder.x
    down = shoulder.z - wrist_z
    ahead = (REACH * arm) ** 2 - across ** 2 - down ** 2
    if ahead < 0.0:
        return None
    return Vector((0.0, shoulder.y + math.sqrt(ahead) - WRIST_ON_HOOD.y,
                   saddle_top - DROP_TO_HOODS))


def lean(parts, arm):
    """Bends pelvis, belly and chest forward until the hoods end up far enough ahead (or the
    torso is as flat as it goes), then lifts the head to look down the road. Returns the
    saddle's top, the hoods and the torso's angle above horizontal."""
    pivot = hips(parts)
    total = 0.0
    step = math.radians(1.0)
    while True:
        torso = shoulders(parts) - hips(parts)
        angle = math.atan2(torso.z, torso.y)
        saddle_top = pelvis_bottom(parts).z
        hoods = hoods_for(parts, saddle_top, arm)
        ready = hoods is not None and hoods.y >= HOODS_AHEAD
        if (angle <= TORSO_ANGLES[0] and ready) or angle <= TORSO_ANGLES[1]:
            break
        # Turning about −x tips the top towards +y, forward.
        transform(parts["pelvis"], Matrix.Rotation(-0.4 * step, 3, "X"), pivot)
        transform(parts["belly"], Matrix.Rotation(-0.3 * step, 3, "X"), origin(parts["belly"]))
        transform(parts["chest"], Matrix.Rotation(-0.3 * step, 3, "X"), origin(parts["chest"]))
        total += step
    if hoods is None:
        raise RuntimeError("the arms cannot reach the hoods")
    transform(parts["neck"], Matrix.Rotation(0.4 * total, 3, "X"), origin(parts["neck"]))
    transform(parts["head"], Matrix.Rotation(0.45 * total, 3, "X"), origin(parts["head"]))
    return saddle_top, hoods, math.degrees(angle)


def reach_hoods(parts, hoods):
    """Puts the hands on the hoods, elbows bent out and down."""
    for side, sign in (("L", -1.0), ("R", 1.0)):
        upper, lower, hand = (parts[f"{p}.{side}"] for p in ("arm_upper", "arm_lower", "hand"))
        wrist = Vector((sign * HOODS_HALF_WIDTH, hoods.y, hoods.z)) + WRIST_ON_HOOD
        first = (origin(lower) - origin(upper)).length
        second = (origin(hand) - origin(lower)).length
        pole = Vector((sign * 0.5, -0.3, -1.0))
        # The shoulder turns half the way with the arm, as a deltoid does; left in place it
        # sticks up like a pad.
        shoulder = origin(upper)
        elbow = two_bone(shoulder, wrist, first, second, pole)
        turn = (origin(lower) - shoulder).rotation_difference(elbow - shoulder)
        deltoid = parts[f"shoulder.{side}"]
        transform(deltoid, Quaternion().slerp(turn, 0.5).to_matrix(), origin(deltoid))
        shoulder = origin(upper)
        elbow = two_bone(shoulder, wrist, first, second, pole)
        turn = (origin(lower) - shoulder).rotation_difference(elbow - shoulder)
        transform(upper, turn.to_matrix(), shoulder)
        start = origin(lower)
        turn = (origin(hand) - start).rotation_difference(wrist - start)
        transform(lower, turn.to_matrix(), start)
        # The hand bends down over the hood.
        transform(hand, Matrix.Rotation(math.radians(-25.0), 3, "X"), origin(hand))


def baked(obj):
    """A copy of the object's mesh in world coordinates."""
    mesh = obj.data.copy()
    mesh.transform(obj.matrix_world)
    if obj.matrix_world.determinant() < 0.0:
        mesh.flip_normals()
    return mesh


def leg_nodes(parts):
    """Each leg part as its own node: origin at its joint, thigh and shin running down −z with
    their front to +y (Godot: down −y, front −z), the foot level."""
    nodes = []
    for side in ("L", "R"):
        chain = [parts[f"{p}.{side}"] for p in LEGS]
        joints = [origin(obj) for obj in chain]
        for k, (part, obj) in enumerate(zip(LEGS, chain)):
            mesh = baked(obj)
            mesh.transform(Matrix.Translation(-joints[k]))
            if k + 1 < len(chain):
                along = (joints[k + 1] - joints[k]).normalized()
                mesh.transform(along.rotation_difference(Vector((0, 0, -1))).to_matrix().to_4x4())
            mesh.name = f"{LEG_NODES[part]}_{side.lower()}"
            node = bpy.data.objects.new(mesh.name, mesh)
            node.matrix_world = Matrix.Translation(joints[k])
            bpy.context.scene.collection.objects.link(node)
            nodes.append(node)
    for obj in [parts[f"{p}.{s}"] for p in LEGS for s in ("L", "R")]:
        bpy.data.objects.remove(obj)
    update()
    return nodes


def join(objects, name):
    """One object from several, in world coordinates, with one slot per material name."""
    joined = bmesh.new()
    names = []
    for obj in objects:
        mesh = baked(obj)
        slots = []
        for slot in mesh.materials:
            if slot.name not in names:
                names.append(slot.name)
            slots.append(names.index(slot.name))
        before = len(joined.faces)
        joined.from_mesh(mesh)
        joined.faces.ensure_lookup_table()
        for k in range(before, len(joined.faces)):
            joined.faces[k].material_index = slots[joined.faces[k].material_index]
        bpy.data.meshes.remove(mesh)
    data = bpy.data.meshes.new(name)
    joined.to_mesh(data)
    joined.free()
    for n in names:
        data.materials.append(material(n))
    smooth(data)
    for obj in objects:
        bpy.data.objects.remove(obj)
    result = bpy.data.objects.new(name, data)
    bpy.context.scene.collection.objects.link(result)
    return result


# --- the bike --------------------------------------------------------------------------------


def ring_of(hub, radius, x, count, phase=0.0):
    """`count` points round the wheel's axis at `radius`, `x` across the bike."""
    return [hub + Vector((x, radius * math.cos(phase + 2 * math.pi * k / count),
                          radius * math.sin(phase + 2 * math.pi * k / count)))
            for k in range(count)]


def disc(faces, hub, radius, x, thickness, material_name, count=32, teeth=0.0):
    """A thin disc round the wheel's axis (rotor, cog, chainring, pulley), with `teeth` deep
    teeth on every other corner where set."""
    outline = []
    for k in range(count):
        r = radius - (teeth if k % 2 else 0.0)
        a = 2 * math.pi * k / count
        outline.append(Vector((0.0, r * math.cos(a), r * math.sin(a))))
    faces.prism([hub + p + Vector((x - thickness / 2, 0, 0)) for p in outline],
                [hub + p + Vector((x + thickness / 2, 0, 0)) for p in outline], material_name)


def wheel(name, hub, cassette=False):
    """A wheel in the y–z plane: a round tyre on a deep-section rim, flanged hub, 20 spokes, a
    disc rotor on the left and, at the back, the cassette on the right — all turning with it."""
    faces = Faces()
    segments = 48
    # The tyre: a tube of 12 sides bent round the wheel, so it is round and shades smooth.
    tube_radius = 0.014
    centre = WHEEL_RADIUS - tube_radius
    sides = 12
    for k in range(segments):
        a0, a1 = 2 * math.pi * k / segments, 2 * math.pi * (k + 1) / segments
        for j in range(sides):
            b0, b1 = 2 * math.pi * j / sides, 2 * math.pi * (j + 1) / sides

            def at(a, b):
                r = centre + tube_radius * math.cos(b)
                return hub + Vector((tube_radius * math.sin(b), r * math.cos(a), r * math.sin(a)))

            inside = hub + Vector((0.0, centre * math.cos((a0 + a1) / 2),
                                   centre * math.sin((a0 + a1) / 2)))
            faces.add([at(a0, b0), at(a1, b0), at(a1, b1), at(a0, b1)], "tyre", inside)
    # A deep rim, a little narrower than the tyre and tapering to the spoke bed.
    outer, inner = WHEEL_RADIUS - 0.024, WHEEL_RADIUS - 0.07
    for k in range(segments):
        a0, a1 = 2 * math.pi * k / segments, 2 * math.pi * (k + 1) / segments

        def rim(radius, a, x):
            return hub + Vector((x, radius * math.cos(a), radius * math.sin(a)))

        inside = rim((outer + inner) / 2, (a0 + a1) / 2, 0.0)
        for x_out, x_in in ((-0.011, -0.008), (0.011, 0.008)):
            faces.add([rim(outer, a0, x_out), rim(outer, a1, x_out), rim(inner, a1, x_in),
                       rim(inner, a0, x_in)], "rim", inside)
        faces.add([rim(inner, a0, -0.008), rim(inner, a1, -0.008), rim(inner, a1, 0.008),
                   rim(inner, a0, 0.008)], "rim", inside)
        faces.add([rim(outer, a0, -0.011), rim(outer, a1, -0.011), rim(outer, a1, 0.011),
                   rim(outer, a0, 0.011)], "rim", inside)
    # Spokes from the hub's flanges, laced to alternate sides.
    for k in range(20):
        angle = 2 * math.pi * k / 20
        flange = 0.025 if k % 2 else -0.025
        start = hub + Vector((flange, 0.03 * math.cos(angle), 0.03 * math.sin(angle)))
        end = hub + Vector((0.0, inner * math.cos(angle), inner * math.sin(angle)))
        faces.tube(start, end, 0.0018, "metal", sides=4, caps=False)
    faces.tube(hub - Vector((0.05, 0, 0)), hub + Vector((0.05, 0, 0)), 0.012, "metal")
    for x in (-0.025, 0.025):
        disc(faces, hub, 0.032, x, 0.004, "metal", count=16)
    disc(faces, hub, 0.08, -0.042, 0.002, "metal", count=32)
    if cassette:
        for k in range(11):
            disc(faces, hub, 0.058 - k * 0.0032, 0.03 + k * 0.0038, 0.0018, "metal",
                 count=24, teeth=0.003)
    return faces.to_object(name, hub)


def saddle_outline(back, nose, z):
    """The saddle's outline at height `z`: wide at the back, narrow at the nose."""
    shape = [(-0.07, 0.0), (0.07, 0.0), (0.06, 0.06), (0.025, 0.17), (0.02, 0.26),
             (-0.02, 0.26), (-0.025, 0.17), (-0.06, 0.06)]
    return [Vector((x, back + y / 0.26 * (nose - back), z)) for x, y in shape]


def bike(saddle, hoods):
    """Frame, wheels, crankset and pedals around the rider's contact points: the saddle's top
    middle and the hoods (both in the y–z plane). Returns the objects and the hubs."""
    seat_axis = Vector((0.0, -math.cos(SEAT_ANGLE), math.sin(SEAT_ANGLE)))
    steer = Vector((0.0, math.cos(HEAD_ANGLE), -math.sin(HEAD_ANGLE)))  # down the steerer
    frame = Faces()

    # Bars: the tops a little below the hoods' top, a level stem back to the steerer.
    bar = Vector((0.0, hoods.y - 0.08, hoods.z - 0.01))
    steerer_top = bar + Vector((0.0, -STEM, 0.01))
    head_top = steerer_top + steer * 0.03
    crown_z = WHEEL_RADIUS + FORK_LENGTH * math.sin(HEAD_ANGLE)
    head_length = max((head_top.z - crown_z) / math.sin(HEAD_ANGLE), 0.08)
    head_bottom = head_top + steer * head_length
    # The steering axis meets hub height; the hub sits the rake ahead of it.
    to_hub = (head_bottom.z - WHEEL_RADIUS) / math.sin(HEAD_ANGLE)
    front_hub = head_bottom + steer * to_hub + Vector((0.0, FORK_RAKE / math.sin(HEAD_ANGLE), 0.0))
    front_hub.z = WHEEL_RADIUS
    rear_hub = Vector((0.0, -math.sqrt(CHAINSTAY ** 2 - (WHEEL_RADIUS - BOTTOM_BRACKET.z) ** 2),
                       WHEEL_RADIUS))

    # The top tube slopes a little; the seat tube ends well below the saddle.
    cluster_z = min(head_top.z - 0.03, saddle.z - 0.16)
    cluster = BOTTOM_BRACKET + seat_axis * ((cluster_z - BOTTOM_BRACKET.z) / seat_axis.z)
    post_top = BOTTOM_BRACKET + seat_axis * ((saddle.z - 0.05 - BOTTOM_BRACKET.z) / seat_axis.z)

    frame.tube(BOTTOM_BRACKET + Vector((-0.04, 0, 0)), BOTTOM_BRACKET + Vector((0.04, 0, 0)),
               0.025, "frame")
    frame.tube(BOTTOM_BRACKET, cluster + seat_axis * 0.02, 0.019, "frame")
    frame.tube(BOTTOM_BRACKET, head_bottom - steer * 0.02, 0.024, "frame")
    frame.tube(cluster, head_top + steer * 0.02, 0.018, "frame")
    frame.tube(head_top - steer * 0.01, head_bottom, 0.025, "frame")
    for sign in (-1.0, 1.0):
        side = Vector((sign, 0.0, 0.0))
        frame.tube(BOTTOM_BRACKET + side * 0.03, rear_hub + side * 0.06, 0.011, "frame")
        frame.tube(cluster + side * 0.015, rear_hub + side * 0.06, 0.01, "frame")
        # Fork blades: down the steering axis, then bent forward to the hub.
        bend = head_bottom + steer * (to_hub * 0.55) + side * 0.05
        frame.tube(head_bottom + side * 0.03, bend, 0.013, "frame")
        frame.tube(bend, front_hub + side * 0.05, 0.011, "frame")
    frame.tube(cluster, post_top, 0.012, "metal")
    frame.tube(post_top, Vector((0.0, saddle.y, saddle.z - 0.03)), 0.01, "metal")
    frame.tube(steerer_top, head_top, 0.017, "metal")
    frame.tube(steerer_top, bar, 0.014, "bar")
    frame.tube(bar + Vector((-HOODS_HALF_WIDTH, 0, 0)), bar + Vector((HOODS_HALF_WIDTH, 0, 0)),
               0.013, "bar")
    for sign in (-1.0, 1.0):
        corner = bar + Vector((sign * HOODS_HALF_WIDTH, 0.0, 0.0))
        hood = Vector((corner.x, hoods.y, hoods.z - 0.03))
        bend = hood + Vector((0.0, 0.03, -0.08))
        drop = hood + Vector((0.0, 0.0, -0.13))
        frame.tube(corner, hood, 0.013, "bar")
        frame.tube(hood, bend, 0.013, "bar")
        frame.tube(bend, drop, 0.013, "bar")
        frame.tube(drop, drop + Vector((0.0, -0.09, 0.0)), 0.013, "bar")
        # The brake hood, where the hand rests, rising to a horn at its front, and the lever
        # blade curving down in front of the drop.
        frame.box(hood + Vector((0.0, 0.0, 0.014)), (0.032, 0.075, 0.042), "bar")
        frame.box(hood + Vector((0.0, 0.035, 0.032)), (0.026, 0.02, 0.022), "bar")
        lever_top = hood + Vector((0.0, 0.045, 0.0))
        lever_mid = hood + Vector((0.0, 0.07, -0.07))
        frame.tube(lever_top, lever_mid, 0.006, "bar", sides=6)
        frame.tube(lever_mid, lever_mid + Vector((0.0, -0.01, -0.06)), 0.006, "bar", sides=6)
    back = saddle.y - 0.11
    frame.prism(saddle_outline(back, back + 0.26, saddle.z - 0.04),
                saddle_outline(back, back + 0.26, saddle.z), "saddle")
    # The saddle's rails, clamped by the seat post.
    for sign in (-1.0, 1.0):
        frame.tube(Vector((sign * 0.03, back + 0.03, saddle.z - 0.04)),
                   Vector((sign * 0.02, back + 0.2, saddle.z - 0.04)), 0.0035, "metal", sides=6)
    # A bottle in its cage on the down tube.
    down = (head_bottom - BOTTOM_BRACKET).normalized()
    lift = Vector((0.0, 0.0, 1.0)) - down * down.z
    lift.normalize()
    cage_start = BOTTOM_BRACKET + down * 0.12 + lift * 0.045
    frame.tube(cage_start, cage_start + down * 0.2, 0.034, "frame", sides=12)
    frame.tube(cage_start + down * 0.2, cage_start + down * 0.235, 0.014, "bar", sides=8)
    for sign in (-1.0, 1.0):
        rail = Vector((sign * 0.025, 0.0, 0.0))
        frame.tube(cage_start - lift * 0.03 + rail, cage_start + down * 0.17 - lift * 0.03 + rail,
                   0.003, "metal", sides=4)
    # Disc brake calipers on the fork's left blade and the left chainstay.
    frame.box(front_hub + Vector((-0.045, -0.04, 0.05)), (0.025, 0.05, 0.035), "metal")
    frame.box(rear_hub + Vector((-0.045, 0.03, 0.05)), (0.025, 0.05, 0.035), "metal")
    # The rear derailleur under the right dropout, its cage with two pulleys.
    mech = rear_hub + Vector((0.065, 0.0, -0.035))
    frame.box(mech, (0.02, 0.045, 0.04), "bar")
    upper_pulley = rear_hub + Vector((0.06, 0.0, -0.065))
    lower_pulley = rear_hub + Vector((0.06, 0.02, -0.12))
    frame.box(middle(upper_pulley, lower_pulley) + Vector((0.008, 0, 0)), (0.006, 0.035, 0.08),
              "metal")
    disc(frame, upper_pulley - Vector((0.06, 0, 0)), 0.016, 0.06, 0.006, "bar", count=12)
    disc(frame, lower_pulley - Vector((0.06, 0, 0)), 0.016, 0.06, 0.006, "bar", count=12)
    # The chain: over the big ring and the cassette, back under through the pulleys.
    ring_x = 0.048
    chain = [
        BOTTOM_BRACKET + Vector((ring_x, 0.0, 0.106)),
        rear_hub + Vector((ring_x, 0.0, 0.052)),
    ]
    frame.tube(chain[0], chain[1], 0.004, "bar", sides=4)
    frame.tube(rear_hub + Vector((ring_x, -0.05, 0.0)), upper_pulley + Vector((-0.012, -0.012, 0)),
               0.004, "bar", sides=4)
    frame.tube(lower_pulley + Vector((-0.012, 0.012, -0.012)),
               BOTTOM_BRACKET + Vector((ring_x, 0.0, -0.106)), 0.004, "bar", sides=4)

    crankset = Faces()
    bb = BOTTOM_BRACKET
    # Two toothed chainrings (50 and 34) on a four-armed spider.
    disc(crankset, bb, 0.106, 0.048, 0.004, "metal", count=100, teeth=0.004)
    disc(crankset, bb, 0.072, 0.041, 0.004, "metal", count=68, teeth=0.004)
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        crankset.tube(bb + Vector((0.045, 0.0, 0.0)),
                      bb + Vector((0.045, 0.07 * math.cos(a), 0.07 * math.sin(a))), 0.008, "bar",
                      sides=6)
    crankset.tube(bb + Vector((-0.07, 0, 0)), bb + Vector((0.07, 0, 0)), 0.012, "metal")
    for sign in (-1.0, 1.0):
        # The right crank forward, the left one back: the app turns them from there. Each arm
        # tapers from the spindle to the pedal.
        root = bb + Vector((sign * 0.06, 0, 0))
        end = bb + Vector((sign * (PEDAL_OFFSET - 0.04), sign * CRANK, 0.0))
        crankset.tube(root, middle(root, end), 0.015, "metal", sides=8)
        crankset.tube(middle(root, end), end, 0.011, "metal", sides=8)
    pedals = []
    for name, sign in (("pedal_l", -1.0), ("pedal_r", 1.0)):
        pedal = Faces()
        axle = bb + Vector((sign * PEDAL_OFFSET, sign * CRANK, 0.0))
        # A road pedal: a flat body wider at its front, on a short axle.
        pedal.prism(
            [axle + Vector((x, y, -0.008)) for x, y in ((-0.03, -0.035), (0.03, -0.035),
                                                         (0.04, 0.03), (-0.04, 0.03))],
            [axle + Vector((x, y, 0.008)) for x, y in ((-0.03, -0.035), (0.03, -0.035),
                                                        (0.04, 0.03), (-0.04, 0.03))], "bar")
        pedal.tube(axle - Vector((sign * 0.05, 0, 0)), axle, 0.005, "metal", sides=6)
        pedals.append(pedal.to_object(name, axle))
    objects = [frame.to_object("frame", Vector()), wheel("wheel_rear", rear_hub, cassette=True),
               wheel("wheel_front", front_hub), crankset.to_object("crankset", bb), *pedals]
    return objects, rear_hub, front_hub


# --- export ----------------------------------------------------------------------------------


def godot(v):
    return [round(v.x, 4), round(v.z, 4), round(-v.y, 4)]


def triangles(objects):
    return sum(len(p.vertices) - 2 for o in objects for p in o.data.polygons)


def build(sex):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    parts = load_body(sex)
    proportion(parts, sex)
    tailor(parts)
    thigh, shin, ball = measure_legs(parts)
    arm = ((origin(parts["arm_lower.R"]) - origin(parts["arm_upper.R"])).length
           + (origin(parts["hand.R"]) - origin(parts["arm_lower.R"])).length)

    # The hips on the seat tube's line, where the knee bends `KNEE_AT_BOTTOM` with the pedal at
    # the bottom and the ball of the foot on it.
    ankle_over_pedal = Vector((0.0, -ball.y, 0.012 - ball.z))
    ankle = (BOTTOM_BRACKET + Vector((0.0, 0.0, -CRANK))
             + Matrix.Rotation(FOOT_AT_BOTTOM, 3, "X") @ ankle_over_pedal)
    seat_axis = Vector((0.0, -math.cos(SEAT_ANGLE), math.sin(SEAT_ANGLE)))
    offset = BOTTOM_BRACKET - ankle
    leg = math.sqrt(thigh ** 2 + shin ** 2 + 2.0 * thigh * shin * math.cos(KNEE_AT_BOTTOM))
    along = -seat_axis.dot(offset) + math.sqrt(
        seat_axis.dot(offset) ** 2 - offset.length_squared + leg * leg)

    pelvis = parts["pelvis"]
    transform(pelvis, Matrix.Rotation(math.pi, 3, "Z"), origin(pelvis))
    move(pelvis, BOTTOM_BRACKET + seat_axis * along - hips(parts))
    for obj in [parts[f"{p}.{s}"] for p in LEGS for s in ("L", "R")]:
        world = obj.matrix_world.copy()
        obj.parent = None
        obj.matrix_world = world
    update()
    headgear(parts, sex)
    saddle_top, hoods, torso_angle = lean(parts, arm)
    reach_hoods(parts, hoods)
    saddle = Vector((0.0, pelvis_bottom(parts).y, saddle_top))
    bike_objects, rear_hub, front_hub = bike(saddle, hoods)

    # The origin between the wheels.
    shift = Vector((0.0, -(rear_hub.y + front_hub.y) / 2.0, 0.0))
    for obj in list(bpy.context.scene.objects):
        if obj.parent is None:
            move(obj, shift)
    legs = leg_nodes(parts)
    body = join([o for o in bpy.context.scene.objects
                 if o.type == "MESH" and o not in legs and o not in bike_objects], "body")

    os.makedirs(OUT, exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, f"rider_{sex}.glb"),
                              export_format="GLB", export_yup=True, export_apply=True)
    print(f"built rider_{sex}: {triangles([body, *legs])} rider and {triangles(bike_objects)} "
          f"bike triangles; torso {torso_angle:.0f}° above horizontal; saddle "
          f"{saddle_top:.3f} m high, hoods {hoods.y - saddle.y:.3f} m ahead of it and "
          f"{hoods.y:.3f} m ahead of the bottom bracket; wheelbase "
          f"{front_hub.y - rear_hub.y:.3f} m")
    by_name = {o.name: o for o in legs}
    return {
        "hips": [godot(origin(by_name["thigh_l"])), godot(origin(by_name["thigh_r"]))],
        "thigh": round(thigh, 4),
        "shin": round(shin, 4),
        "ankle_over_pedal": godot(ankle_over_pedal),
        "wheel_radius": WHEEL_RADIUS,
        "wheel_rear": godot(rear_hub + shift),
        "wheel_front": godot(front_hub + shift),
        "bottom_bracket": godot(BOTTOM_BRACKET + shift),
        # Where the hands rest on the hoods: the rider stands up on the pedals about them.
        "hands": godot(hoods + shift),
        "crank": CRANK,
        "pedal_offset": PEDAL_OFFSET,
    }


# --- realistic, rigged bodies (RIDERS=rigged; work in progress) ------------------------------

REALISTIC_SOURCE = os.path.normpath(
    os.path.join(HERE, "..", "sources", "human-base-meshes", "realistic_bodies.blend"))
# The skeleton the joints come from is this tall; each body's joints are scaled to its height.
SKELETON_HEIGHT = 1.762
SKELETON_X = -7.411
# Heat weighting fails on the detailed bodies: a copy thinned to this share is weighted and its
# weights handed on.
WEIGHT_COPY = 0.25


def load_realistic(sex):
    """The realistic body standing at the origin facing +y, and its joints in the same frame."""
    with bpy.data.libraries.load(REALISTIC_SOURCE, link=False) as (source, target):
        target.objects = list(source.objects)
    objects = {o.name: o for o in target.objects}
    body = objects[f"GEO-body_{sex}_realistic"]
    bpy.context.scene.collection.objects.link(body)
    update()
    corners = [body.matrix_world @ Vector(c) for c in body.bound_box]
    low = Vector([min(c[i] for c in corners) for i in range(3)])
    high = Vector([max(c[i] for c in corners) for i in range(3)])
    height = high.z - low.z
    scale = height / SKELETON_HEIGHT
    # The source faces −y: turn it round so it faces the bike's forward, +y.
    turn = Matrix.Rotation(math.pi, 4, "Z")
    place = turn @ Matrix.Translation(Vector((-(low.x + high.x) / 2, 0.0, -low.z)))
    body.data.transform(place @ body.matrix_world)
    body.matrix_world = Matrix.Identity(4)
    joints = {}
    for name, obj in objects.items():
        if name.startswith("GEO-skeletion."):
            # Appended but not linked, an object's world matrix is not computed; the joints
            # have no parents, so their location is where they are.
            p = obj.location
            joints[name.split(".", 1)[1]] = turn @ Vector(
                ((p.x - SKELETON_X) * scale, p.y * scale, p.z * scale))
    return body, joints, scale


def realistic_skeleton(body, joints, scale):
    """An armature from the joints, the body skinned to it. Bone names are the ones the app
    poses: pelvis, spine, chest, neck, head, shoulder/upper_arm/forearm/hand and
    thigh/shin/foot, each with .L and .R."""
    armature = bpy.data.armatures.new("rig")
    rig = bpy.data.objects.new("rig", armature)
    bpy.context.scene.collection.objects.link(rig)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode="EDIT")
    bones = armature.edit_bones

    def bone(name, head, tail, parent=None, connect=False):
        made = bones.new(name)
        made.head, made.tail = head, tail
        if parent:
            made.parent = bones[parent]
            made.use_connect = connect

    hip = joints["hip"]
    middle_back = joints["spine_lumbar_l5"].lerp(joints["spine_thoracic_t1"], 0.5)
    bone("pelvis", hip, hip + Vector((0.0, 0.0, 0.1)))
    bone("spine", joints["spine_lumbar_l5"], middle_back, "pelvis")
    bone("chest", middle_back, joints["spine_thoracic_t1"], "spine", True)
    bone("neck", joints["spine_thoracic_t1"], joints["spine_cervical_c1"], "chest", True)
    bone("head", joints["spine_cervical_c1"],
         joints["spine_cervical_c1"] + Vector((0.0, 0.0, 0.2 * scale)), "neck", True)
    for side in ("L", "R"):
        # Behind the kneecap, where the knee turns.
        knee = joints[f"leg_patella.{side}"] + Vector((0.0, -0.03 * scale, 0.0))
        bone(f"thigh.{side}", joints[f"leg_femur.{side}"], knee, "pelvis")
        bone(f"shin.{side}", knee, joints[f"foot_talus.{side}"], f"thigh.{side}", True)
        bone(f"foot.{side}", joints[f"foot_talus.{side}"], joints[f"foot_center.{side}"],
             f"shin.{side}", True)
        towards = (joints[f"arm_humerus.{side}"] - joints["spine_thoracic_t1"]).normalized()
        bone(f"shoulder.{side}", joints["spine_thoracic_t1"] + towards * 0.03,
             joints[f"arm_humerus.{side}"], "chest")
        bone(f"upper_arm.{side}", joints[f"arm_humerus.{side}"], joints[f"arm_radius.{side}"],
             f"shoulder.{side}", True)
        bone(f"forearm.{side}", joints[f"arm_radius.{side}"], joints[f"hand_center.{side}"],
             f"upper_arm.{side}", True)
        along = (joints[f"hand_center.{side}"] - joints[f"arm_radius.{side}"]).normalized()
        bone(f"hand.{side}", joints[f"hand_center.{side}"],
             joints[f"hand_center.{side}"] + along * 0.09 * scale, f"forearm.{side}", True)
    bpy.ops.object.mode_set(mode="OBJECT")

    coarse = body.copy()
    coarse.data = body.data.copy()
    bpy.context.scene.collection.objects.link(coarse)
    thin = coarse.modifiers.new("thin", "DECIMATE")
    thin.ratio = WEIGHT_COPY
    bpy.context.view_layer.objects.active = coarse
    bpy.ops.object.modifier_apply(modifier="thin")
    for obj in bpy.context.selected_objects:
        obj.select_set(False)
    coarse.select_set(True)
    rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.parent_set(type="ARMATURE_AUTO")
    for group in coarse.vertex_groups:
        body.vertex_groups.new(name=group.name)
    transfer = body.modifiers.new("weights", "DATA_TRANSFER")
    transfer.object = coarse
    transfer.use_vert_data = True
    transfer.data_types_verts = {"VGROUP_WEIGHTS"}
    transfer.vert_mapping = "POLYINTERP_NEAREST"
    transfer.layers_vgroup_select_src = "ALL"
    transfer.layers_vgroup_select_dst = "NAME"
    bpy.context.view_layer.objects.active = body
    bpy.ops.object.modifier_apply(modifier="weights")
    bpy.data.objects.remove(coarse)
    skin = body.modifiers.new("rig", "ARMATURE")
    skin.object = rig
    body.parent = rig
    return rig


def aim(rig, name, target):
    """Turns pose bone `name` (armature space) so its tail points at `target`, keeping its roll."""
    bone = rig.pose.bones[name]
    head = bone.head.copy()
    y = (target - head).normalized()
    x = bone.matrix.col[0].to_3d()
    x = (x - y * x.dot(y)).normalized()
    z = x.cross(y)
    bone.matrix = Matrix((
        (x.x, y.x, z.x, head.x),
        (x.y, y.y, z.y, head.y),
        (x.z, y.z, z.z, head.z),
        (0.0, 0.0, 0.0, 1.0),
    ))
    update()


def turn_bone(rig, name, angle, axis="X"):
    """Turns pose bone `name` about its head by `angle` round the armature's `axis`."""
    bone = rig.pose.bones[name]
    head = bone.head.copy()
    bone.matrix = (Matrix.Translation(head) @ Matrix.Rotation(angle, 4, axis)
                   @ Matrix.Translation(-head) @ bone.matrix)
    update()


def build_realistic(sex):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    body, joints, scale = load_realistic(sex)
    bpy.context.view_layer.objects.active = body
    thigh = (joints["leg_patella.R"] - joints["leg_femur.R"]).length
    shin = (joints["foot_talus.R"] - joints["leg_patella.R"]).length
    ball = joints["foot_center.R"] - joints["foot_talus.R"]
    # As for the primitive bodies: the hips on the seat tube's line, the knee bending
    # `KNEE_AT_BOTTOM` with the pedal at the bottom and the ball of the foot on it.
    ankle_over_pedal = Vector((0.0, -ball.y, 0.012 - ball.z))
    ankle = (BOTTOM_BRACKET + Vector((0.0, 0.0, -CRANK))
             + Matrix.Rotation(FOOT_AT_BOTTOM, 3, "X") @ ankle_over_pedal)
    seat_axis = Vector((0.0, -math.cos(SEAT_ANGLE), math.sin(SEAT_ANGLE)))
    offset = BOTTOM_BRACKET - ankle
    leg = math.sqrt(thigh ** 2 + shin ** 2 + 2.0 * thigh * shin * math.cos(KNEE_AT_BOTTOM))
    along = -seat_axis.dot(offset) + math.sqrt(
        seat_axis.dot(offset) ** 2 - offset.length_squared + leg * leg)
    hips_middle = (joints["leg_femur.L"] + joints["leg_femur.R"]) / 2
    move_by = BOTTOM_BRACKET + seat_axis * along - hips_middle
    body.data.transform(Matrix.Translation(move_by))
    for name in joints:
        joints[name] = joints[name] + move_by
    centre = hips_middle + move_by
    seat = [body.matrix_world @ v.co for v in body.data.vertices
            if abs(v.co.x) < 0.07 and (v.co.yz - centre.yz).length < 0.2]
    saddle_top = min(seat, key=lambda p: p.z).z
    rig = realistic_skeleton(body, joints, scale)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode="POSE")

    upper = (joints["arm_radius.R"] - joints["arm_humerus.R"]).length
    lower = (joints["hand_center.R"] - joints["arm_radius.R"]).length
    arm = upper + lower

    def shoulders():
        return (rig.pose.bones["upper_arm.L"].head + rig.pose.bones["upper_arm.R"].head) / 2

    # Bend the back until the hands reach hoods far enough ahead, as `lean` does.
    total = 0.0
    step = math.radians(1.0)
    while True:
        torso = shoulders() - centre
        angle = math.atan2(torso.z, torso.y)
        shoulder = rig.pose.bones["upper_arm.R"].head
        hoods_z = saddle_top - DROP_TO_HOODS
        across = HOODS_HALF_WIDTH - abs(shoulder.x)
        reach = (arm * REACH) ** 2 - (shoulder.z - hoods_z) ** 2 - across ** 2
        hoods = Vector((0.0, shoulder.y + math.sqrt(reach), hoods_z)) if reach > 0 else None
        ready = hoods is not None and hoods.y >= HOODS_AHEAD
        if (angle <= TORSO_ANGLES[0] and ready) or angle <= TORSO_ANGLES[1]:
            break
        turn_bone(rig, "pelvis", -0.4 * step)
        turn_bone(rig, "spine", -0.3 * step)
        turn_bone(rig, "chest", -0.3 * step)
        total += step
    if hoods is None:
        raise RuntimeError("the arms cannot reach the hoods")
    turn_bone(rig, "neck", 0.45 * total)
    turn_bone(rig, "head", 0.5 * total)

    for side, sign in (("L", -1.0), ("R", 1.0)):
        wrist = Vector((sign * HOODS_HALF_WIDTH, hoods.y, hoods.z)) + WRIST_ON_HOOD
        shoulder = rig.pose.bones[f"upper_arm.{side}"].head
        elbow = two_bone(shoulder, wrist, upper, lower, Vector((sign * 0.5, -0.3, -1.0)))
        aim(rig, f"upper_arm.{side}", elbow)
        aim(rig, f"forearm.{side}", wrist)
        aim(rig, f"hand.{side}", wrist + Vector((0.0, 0.08, -0.04)))
        # The legs at crank angle 0: the right pedal forward, the left one back.
        pedal = BOTTOM_BRACKET + Vector((sign * PEDAL_OFFSET, sign * CRANK, 0.0))
        foot_turn = Matrix.Rotation(FOOT_AT_BOTTOM * 0.5, 3, "X")
        ankle_at = pedal + foot_turn @ ankle_over_pedal
        hip = rig.pose.bones[f"thigh.{side}"].head
        knee = two_bone(hip, ankle_at, thigh, shin, Vector((0.0, 1.0, 0.0)))
        aim(rig, f"thigh.{side}", knee)
        aim(rig, f"shin.{side}", ankle_at)
        aim(rig, f"foot.{side}", ankle_at + foot_turn @ Vector((0.0, ball.length, -0.02)))
    bpy.ops.object.mode_set(mode="OBJECT")

    saddle = Vector((0.0, centre.y - 0.02, saddle_top))
    bike_objects, rear_hub, front_hub = bike(saddle, hoods)
    shift = Vector((0.0, -(rear_hub.y + front_hub.y) / 2.0, 0.0))
    for obj in list(bpy.context.scene.objects):
        if obj.parent is None:
            move(obj, shift)
    print(f"built realistic rider_{sex}: torso {math.degrees(angle):.0f}° above horizontal, "
          f"{len(body.data.polygons)} faces")
    preview_dir = os.environ.get("PREVIEW")
    if preview_dir:
        import preview  # noqa: E402  (art/preview.py; on the path below)
        preview.render([body, *bike_objects], preview_dir, f"realistic_{sex}",
                       views=("side", "quarter"))


def main():
    if os.environ.get("RIDERS") == "rigged":
        sys.path.insert(0, os.path.dirname(HERE))
        for sex in ("female", "male"):
            build_realistic(sex)
        return
    manifest = {sex: build(sex) for sex in ("female", "male")}
    with open(os.path.join(OUT, "riders.json"), "w", encoding="utf-8") as file:
        json.dump(manifest, file, indent=1)
        file.write("\n")


main()
