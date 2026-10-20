"""Builds the props that stand by the road (R92) — guard rails, delineator posts and snow
poles; in towns street lamps, benches, bins and bus shelters; in the countryside pasture
fences, hay bales, wayside crosses and power poles; road signs — and exports them for Godot
(ADR 0009, ADR 0020). The power lines' wires are the world's own geometry.

Run in the art container:
    scripts/art.sh blender --background --factory-startup --python art/props/build.py

The world (`core/orqwa-world/src/props.rs`) places them beside the road ridden, each with its
back to the road: the model's +x points away from the road, its length runs along Blender +y
(Godot −z, the way the road is ridden on the right-hand side), the origin on the ground where
it stands. Guard rails are 4 m spans that the world shears to the road's grade, so posts stay
upright; `guard_rail_end` slopes down into the ground ahead (+y), `guard_rail_start` behind.
Each model is one mesh with named materials (`galvanised`, `post`, `band`, `reflector`,
`pole_red`, `pole_white`, `metal_dark`, `wood`, `lamp`, `glass`, `wood_weathered`, `hay`,
`wrap`, `stone`, `sign_blue`, `sign_white`, `sign_red`, `sign_black`, `insulator`) that the
app colours
(`app/scenes/prop_models.gd`). A street lamp's arm reaches over the road (−x); a bench's and
a shelter's seats face it. Parts reach
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
           "pole_white": (0.92, 0.92, 0.9), "metal_dark": (0.18, 0.19, 0.2),
           "wood": (0.45, 0.3, 0.18), "lamp": (0.95, 0.93, 0.85), "glass": (0.7, 0.8, 0.85),
           "wood_weathered": (0.42, 0.38, 0.32), "hay": (0.75, 0.62, 0.32),
           "wrap": (0.9, 0.9, 0.88), "stone": (0.55, 0.53, 0.5),
           "sign_blue": (0.05, 0.25, 0.6), "sign_white": (0.95, 0.95, 0.95),
           "sign_red": (0.75, 0.1, 0.1), "sign_black": (0.05, 0.05, 0.05),
           "insulator": (0.6, 0.63, 0.6)}

# How far posts and blades reach below the ground's level at the origin.
BURIED = 0.4
# A guard rail's span and its W-beam: the profile across it (metres out from the face, up),
# from the bottom edge up, as the common A-profile of European roads, its top 0.75 m high.
SPAN = 4.0
W_BEAM = [(0.0, 0.44), (-0.045, 0.47), (-0.06, 0.52), (-0.035, 0.575), (-0.01, 0.6),
          (-0.035, 0.625), (-0.06, 0.68), (-0.045, 0.73), (0.0, 0.76)]
SHEET = 0.004
# A pasture fence's span along the road.
FENCE_SPAN = 3.0
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


def street_lamp(spec):
    """A street lamp: a tapered pole bending at its top into an arm over the road and a flat
    luminaire, on a small foot."""
    mesh = Mesh()
    height = 6.5
    sides = 10

    def ring(centre, radius, axis):
        # A ring round `axis` (unit, in the x–z plane) at `centre`.
        ax, az = axis
        across = (az, -ax)
        return [(centre[0] + radius * math.cos(2 * math.pi * i / sides) * across[0],
                 centre[1] + radius * math.sin(2 * math.pi * i / sides),
                 centre[2] + radius * math.cos(2 * math.pi * i / sides) * across[1])
                for i in range(sides)]

    # Up the pole, then bending over toward the road (−x) in a quarter circle.
    path = [((0.0, 0.0, -BURIED), (0.0, 1.0), 0.075), ((0.0, 0.0, 0.4), (0.0, 1.0), 0.07),
            ((0.0, 0.0, height - 0.6), (0.0, 1.0), 0.05)]
    bend = 0.6
    for k in range(1, 7):
        a = math.pi / 2 * k / 6
        centre = (-bend + bend * math.cos(a), 0.0, height - 0.6 + bend * math.sin(a))
        path.append((centre, (-math.sin(a), math.cos(a)), 0.045))
    path.append(((-1.5, 0.0, height), (-1.0, 0.0), 0.04))
    rings = [ring(c, r, axis) for c, axis, r in path]
    mesh.strip(rings, "metal_dark", smooth=True, closed=True)
    mesh.face(list(reversed(rings[0])), "metal_dark")
    # The luminaire: a flat box at the arm's end, its glass underneath.
    mesh.box((-2.05, -0.16, height - 0.1), (-1.45, 0.16, height + 0.05), "metal_dark")
    mesh.face([(-2.0, -0.12, height - 0.103), (-2.0, 0.12, height - 0.103),
               (-1.5, 0.12, height - 0.103), (-1.5, -0.12, height - 0.103)], "lamp")
    mesh.box((-0.12, -0.12, -BURIED), (0.12, 0.12, 0.35), "metal_dark")
    return mesh, height + 0.05


def bench(spec):
    """A park bench facing the road (−x): wooden slats for the seat and back on two cast
    frames."""
    mesh = Mesh()
    length = 1.8
    for y in (-length / 2 + 0.15, length / 2 - 0.15):
        # Frame: legs, a seat bearer and the back's upright, leaning back.
        mesh.box((-0.25, y - 0.03, -BURIED), (-0.2, y + 0.03, 0.43), "metal_dark")
        mesh.box((0.15, y - 0.03, -BURIED), (0.2, y + 0.03, 0.43), "metal_dark")
        mesh.box((-0.27, y - 0.03, 0.4), (0.22, y + 0.03, 0.44), "metal_dark")
        for k in range(6):
            z0, z1 = 0.44 + 0.08 * k, 0.44 + 0.08 * (k + 1)
            mesh.box((0.16 + 0.02 * k, y - 0.03, z0), (0.21 + 0.02 * k, y + 0.03, z1),
                     "metal_dark")
    # Seat slats along the bench.
    for k in range(4):
        x = -0.27 + 0.125 * k
        mesh.box((x, -length / 2, 0.44), (x + 0.1, length / 2, 0.475), "wood")
    # Back slats, leaning back with the uprights.
    for k in range(3):
        z = 0.58 + 0.12 * k
        x = 0.2 + 0.02 * ((z - 0.44) / 0.08)
        mesh.box((x, -length / 2, z), (x + 0.03, length / 2, z + 0.09), "wood")
    return mesh, 0.95


def litter_bin(spec):
    """A litter bin on a post: a round dark drum with a lighter rim."""
    mesh = Mesh()
    mesh.box((0.21, -0.03, -BURIED), (0.27, 0.03, 0.95), "metal_dark")
    # Brackets holding the drum to the post.
    for z in (0.45, 0.78):
        mesh.box((0.15, -0.02, z), (0.21, 0.02, z + 0.04), "metal_dark")
    sides = 14

    def ring(z, radius):
        return [(radius * math.cos(2 * math.pi * i / sides),
                 radius * math.sin(2 * math.pi * i / sides), z) for i in range(sides)]

    mesh.strip([ring(0.35, 0.18), ring(0.85, 0.2)], "metal_dark", smooth=True, closed=True)
    mesh.face(list(reversed(ring(0.35, 0.18))), "metal_dark")
    mesh.strip([ring(0.85, 0.2), ring(0.88, 0.21)], "galvanised", smooth=True, closed=True)
    return mesh, 0.95


def bus_shelter(spec):
    """A bus stop's shelter open to the road (−x): a glass back and ends in thin frames, a flat
    roof reaching over the front, a bench inside."""
    mesh = Mesh()
    length, back, height = 3.6, 1.5, 2.4
    for y in (-length / 2, 0.0, length / 2):
        mesh.box((back - 0.05, y - 0.03, -BURIED), (back + 0.01, y + 0.03, height), "metal_dark")
    for y in (-length / 2, length / 2):
        mesh.box((-0.03, y - 0.03, -BURIED), (0.03, y + 0.03, height), "metal_dark")
    # Glass: the back wall and the two ends, seen from both sides.
    for quad in (
        [(back - 0.02, -length / 2, 0.1), (back - 0.02, length / 2, 0.1),
         (back - 0.02, length / 2, height - 0.1), (back - 0.02, -length / 2, height - 0.1)],
        [(0.03, -length / 2, 0.1), (back - 0.05, -length / 2, 0.1),
         (back - 0.05, -length / 2, height - 0.1), (0.03, -length / 2, height - 0.1)],
        [(back - 0.05, length / 2, 0.1), (0.03, length / 2, 0.1),
         (0.03, length / 2, height - 0.1), (back - 0.05, length / 2, height - 0.1)],
    ):
        mesh.face(quad, "glass")
        mesh.face(list(reversed(quad)), "glass")
    # The roof, reaching a little beyond the front.
    mesh.box((-0.35, -length / 2 - 0.1, height), (back + 0.1, length / 2 + 0.1, height + 0.08),
             "metal_dark")
    # The bench along the back.
    mesh.box((back - 0.45, -length / 2 + 0.3, 0.42), (back - 0.08, length / 2 - 0.3, 0.46), "wood")
    for y in (-length / 2 + 0.4, length / 2 - 0.4):
        mesh.box((back - 0.3, y - 0.03, 0.0), (back - 0.25, y + 0.03, 0.42), "metal_dark")
    return mesh, height + 0.08


def fence(spec):
    """A pasture fence's span: two weathered posts at its ends and two split rails between,
    `FENCE_SPAN` long along the road."""
    mesh = Mesh()
    half = FENCE_SPAN / 2
    for y in (-half, half):
        mesh.box((-0.05, y - 0.05, -BURIED), (0.05, y + 0.05, 1.15), "wood_weathered")
    for z in (0.45, 0.9):
        mesh.box((-0.09, -half, z), (-0.05, half, z + 0.1), "wood_weathered")
    return mesh, 1.15


def hay_bale(spec):
    """A round bale of hay lying on its side, its axis along the road: straw, or wrapped in white
    plastic for silage."""
    mesh = Mesh()
    radius, width, sides = 0.75, 1.2, 18
    material = "wrap" if spec.get("wrapped") else "hay"

    # Sunk a little into the ground, as a heavy bale settles.
    centre_z = radius - 0.05

    def ring(y):
        return [(radius * math.cos(2 * math.pi * i / sides), y,
                 centre_z + radius * math.sin(2 * math.pi * i / sides)) for i in range(sides)]

    rings = [ring(-width / 2), ring(width / 2)]
    # From +y to -y: the rings run round the axis the other way to a pole's, and the strip
    # would face inwards otherwise, drawing only the bale's ends.
    mesh.strip(rings[::-1], material, smooth=True, closed=True)
    # The flat ends as fans round their centres, facing out.
    for y, ring_points, outward in ((-width / 2, rings[0], -1), (width / 2, rings[1], 1)):
        centre = (0.0, y, centre_z)
        for i in range(sides):
            a, b = ring_points[i], ring_points[(i + 1) % sides]
            mesh.face([centre, b, a] if outward > 0 else [centre, a, b], material)
    return mesh, 2 * radius - 0.05


def wayside_cross(spec):
    """A wooden wayside cross on a stone foot, with a small pitched roof over the crossing."""
    mesh = Mesh()
    mesh.box((-0.25, -0.25, -BURIED), (0.25, 0.25, 0.3), "stone")
    mesh.box((-0.06, -0.06, 0.3), (0.06, 0.06, 2.6), "wood_weathered")
    mesh.box((-0.05, -0.55, 1.85), (0.05, 0.55, 1.97), "wood_weathered")
    # The roof: two boards meeting over the top, along the arms, seen from both sides.
    for sign in (-1.0, 1.0):
        board = [(0.0, -0.65, 2.72), (0.0, 0.65, 2.72), (sign * 0.28, 0.65, 2.5),
                 (sign * 0.28, -0.65, 2.5)]
        mesh.face(board, "wood_weathered")
        mesh.face(list(reversed(board)), "wood_weathered")
    return mesh, 2.72


def sign_pole(mesh, top):
    """A round galvanised pole behind a sign's face."""
    mesh.pole(0.0, 0.05, 0.03, [-BURIED, top], lambda k: "galvanised", sides=8)


def panel(mesh, corners, material, y):
    """A flat face in the x–z plane at `y`, seen from −y (the traffic it faces), its back
    galvanised."""
    front = [(x, y, z) for x, z in corners]
    back = [(x, y + 0.01, z) for x, z in corners]
    # Corners counter-clockwise in x–z face −y as given.
    mesh.face(front, material)
    mesh.face(list(reversed(back)), "galvanised")


def band(mesh, points, width, material, y):
    """A flat band of `width` along `points` (x, z) on a sign's face at `y`."""
    for (ax, az), (bx, bz) in zip(points, points[1:]):
        dx, dz = bx - ax, bz - az
        length = math.hypot(dx, dz) or 1.0
        nx, nz = -dz / length * width / 2, dx / length * width / 2
        quad = [(ax + nx, az + nz), (bx + nx, bz + nz), (bx - nx, bz - nz), (ax - nx, az - nz)]
        mesh.face([(x, y, z) for x, z in reversed(quad)], material)


def sign_place(spec):
    """A place-name sign: a blue panel with a white rim on a pole; the name is the app's label
    on its face. The end sign has a red bar across."""
    mesh = Mesh()
    sign_pole(mesh, 2.15)
    low, high, half = 1.65, 2.15, 0.7
    panel(mesh, [(-half, low), (half, low), (half, high), (-half, high)], "sign_white", 0.0)
    inset = 0.03
    panel(mesh, [(-half + inset, low + inset), (half - inset, low + inset),
                 (half - inset, high - inset), (-half + inset, high - inset)], "sign_blue",
          -0.005)
    if spec.get("end"):
        band(mesh, [(-half + 0.06, low + 0.06), (half - 0.06, high - 0.06)], 0.07, "sign_red",
             -0.045)
    return mesh, high


TRIANGLE = (0.9, 1.45)  # side, height of its base above the ground


def warning_face(mesh):
    """A warning triangle, point up: a red border round a white field."""
    side, base = TRIANGLE
    top = base + side * math.sqrt(3) / 2
    sign_pole(mesh, top - 0.1)
    panel(mesh, [(-side / 2, base), (side / 2, base), (0.0, top)], "sign_red", 0.0)
    inner = side - 0.2
    inner_base = base + 0.1 / math.sqrt(3) * 1.7
    panel(mesh, [(-inner / 2, inner_base), (inner / 2, inner_base),
                 (0.0, inner_base + inner * math.sqrt(3) / 2)], "sign_white", -0.005)
    return base, top


def sign_bend(spec):
    """A warning of a bend: a black arrow rising and curving the way the road turns."""
    mesh = Mesh()
    base, top = warning_face(mesh)
    turn = 1.0 if spec["turn"] == "right" else -1.0
    points = [(0.0, base + 0.15), (0.0, base + 0.3)]
    for k in range(1, 7):
        a = math.pi / 2 * k / 6
        points.append((turn * (0.12 - 0.12 * math.cos(a)), base + 0.3 + 0.12 * math.sin(a)))
    band(mesh, points, 0.045, "sign_black", -0.01)
    tip_x, tip_z = points[-1]
    head = [(tip_x + turn * 0.08, tip_z), (tip_x, tip_z + 0.06), (tip_x, tip_z - 0.06)]
    if turn > 0:
        head = [head[0], head[2], head[1]]
    mesh.face([(x, -0.01, z) for x, z in head], "sign_black")
    return mesh, top


def sign_grade(spec):
    """A warning of a steep grade: a black slope; the per cent is the app's label above it."""
    mesh = Mesh()
    base, top = warning_face(mesh)
    slope = [(-0.24, base + 0.13), (0.24, base + 0.13), (0.24, base + 0.26)]
    mesh.face([(x, -0.01, z) for x, z in reversed(slope)], "sign_black")
    return mesh, top


def power_pole(spec):
    """A wooden power pole: a tapered trunk, a crossarm across the line (along x: away from the
    road and toward it) and three insulators where the wires are held (`WIRES` in
    core/orqwa-world/src/props.rs)."""
    mesh = Mesh()
    sides = 10

    def ring(x, z, radius):
        return [(x + radius * math.cos(2 * math.pi * i / sides),
                 radius * math.sin(2 * math.pi * i / sides), z) for i in range(sides)]

    mesh.strip([ring(0.0, -BURIED, 0.13), ring(0.0, 8.25, 0.09)], "wood_weathered",
               smooth=True, closed=True)
    mesh.face(ring(0.0, 8.25, 0.09), "wood_weathered")
    mesh.box((-0.85, -0.05, 7.55), (0.85, 0.05, 7.67), "wood_weathered")
    for x, z in ((-0.7, 7.67), (0.7, 7.67), (0.0, 8.25)):
        mesh.strip([ring(x, z, 0.04), ring(x, z + 0.1, 0.03)], "insulator", smooth=True,
                   closed=True)
        mesh.face(list(reversed(ring(x, z + 0.1, 0.03))), "insulator")
    return mesh, 8.35


KINDS = {"power_pole": power_pole, "sign_place": sign_place, "sign_bend": sign_bend, "sign_grade": sign_grade,
         "fence": fence, "hay_bale": hay_bale, "wayside_cross": wayside_cross,
         "street_lamp": street_lamp, "bench": bench, "bin": litter_bin,
         "bus_shelter": bus_shelter, "guard_rail": guard_rail, "guard_rail_end": guard_rail_end, "delineator": delineator,
         "snow_pole": snow_pole}
CATALOGUE = [
    {"name": "guard_rail", "kind": "guard_rail"},
    {"name": "guard_rail_end", "kind": "guard_rail_end", "toward": "end"},
    {"name": "guard_rail_start", "kind": "guard_rail_end", "toward": "start"},
    {"name": "delineator", "kind": "delineator"},
    {"name": "snow_pole", "kind": "snow_pole"},
    {"name": "street_lamp", "kind": "street_lamp"},
    {"name": "bench", "kind": "bench"},
    {"name": "bin", "kind": "bin"},
    {"name": "bus_shelter", "kind": "bus_shelter"},
    {"name": "fence", "kind": "fence"},
    {"name": "hay_bale", "kind": "hay_bale"},
    {"name": "hay_bale_wrapped", "kind": "hay_bale", "wrapped": True},
    {"name": "wayside_cross", "kind": "wayside_cross"},
    {"name": "sign_place", "kind": "sign_place"},
    {"name": "sign_place_end", "kind": "sign_place", "end": True},
    {"name": "sign_bend_left", "kind": "sign_bend", "turn": "left"},
    {"name": "sign_bend_right", "kind": "sign_bend", "turn": "right"},
    {"name": "sign_grade", "kind": "sign_grade"},
    {"name": "power_pole", "kind": "power_pole"},
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
