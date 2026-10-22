"""Bakes tileable PBR textures from procedural Blender materials (ADR 0020).

Run in the art container:
    scripts/art.sh blender --background --factory-startup --python art/textures/build.py
    scripts/art.sh blender --background --factory-startup --python art/textures/build.py \
        -- [name …] [--size 1024] [--out screenshots/textures]

Materials are procedural node trees, except rock, converted from the archived CC0 scan
(rock_scan.py, R112). To make a procedural tile seamless, the plane's UV (u, v)
is wrapped onto a torus in 4D, (cos 2πu, sin 2πu, cos 2πv, sin 2πv), and every noise and
Voronoi node samples that 4D point, so the texture has no edges to hide. Outputs, by default
into app/assets/textures: <name>_albedo.webp (sRGB), <name>_normal.webp (OpenGL tangent space,
as Godot expects) and <name>_rough.webp. Adapted from the author's earlier game project
(choppa, tools/blender/make_textures.py).

Formats (ADR 0020): colour and roughness as lossy WebP (quality 90), since Godot compresses
every texture for the GPU anyway and loses more than that; normal maps lossless, because lossy
WebP subsamples the colour channels, which in a normal map are directions.
"""

import math
import os
import sys

import bpy

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, "..", "..", "app", "assets", "textures"))
sys.path.insert(0, HERE)

from rock_scan import convert as convert_rock  # noqa: E402


class Tree:
    """Builds a material's node tree in code, on the 4D torus coordinate."""

    def __init__(self, material):
        if material.node_tree is None and hasattr(material, "use_nodes"):
            material.use_nodes = True
        self.nt = material.node_tree
        self.nt.nodes.clear()
        self.n = self.nt.nodes
        self.l = self.nt.links
        self._torus()

    def node(self, kind, **props):
        nd = self.n.new(kind)
        for key, value in props.items():
            if key.startswith("in_"):
                nd.inputs[key[3:].replace("_", " ")].default_value = value
            else:
                setattr(nd, key, value)
        return nd

    def link(self, a, b):
        self.l.new(a, b)

    def _torus(self):
        """The 4D torus coordinate: self.p (vector xyz) and self.w (float)."""
        uv = self.node("ShaderNodeTexCoord")
        sep = self.node("ShaderNodeSeparateXYZ")
        self.link(uv.outputs["UV"], sep.inputs[0])

        def trig(src, op):
            m = self.node("ShaderNodeMath", operation="MULTIPLY", in_Value=0.0)
            m.inputs[1].default_value = 2 * math.pi
            self.link(src, m.inputs[0])
            t = self.node("ShaderNodeMath", operation=op)
            self.link(m.outputs[0], t.inputs[0])
            return t.outputs[0]

        cu, su = trig(sep.outputs["X"], "COSINE"), trig(sep.outputs["X"], "SINE")
        cv, sv = trig(sep.outputs["Y"], "COSINE"), trig(sep.outputs["Y"], "SINE")
        comb = self.node("ShaderNodeCombineXYZ")
        self.link(cu, comb.inputs[0])
        self.link(su, comb.inputs[1])
        self.link(cv, comb.inputs[2])
        self.p = comb.outputs[0]
        self.w = sv
        # A torus with a fatter u-circle: features come out squeezed along u, i.e. long
        # streaks running along v (fibres, grass blades).
        scaled = self.node("ShaderNodeVectorMath", operation="MULTIPLY")
        scaled.inputs[1].default_value = (5.0, 5.0, 1.0)
        self.link(comb.outputs[0], scaled.inputs[0])
        self.p_streak = scaled.outputs[0]
        # Plain u and v (0–1) too, for straight bands such as rock strata.
        self.v = sep.outputs["Y"]
        self.u = sep.outputs["X"]

    def noise(self, scale, detail=8.0, rough=0.55, distortion=0.0, offset=0.0, kind="FBM",
              streak=False):
        """4D noise; scale is in whole-tile periods (about half the features per tile)."""
        nd = self.node("ShaderNodeTexNoise", noise_dimensions="4D", noise_type=kind)
        nd.inputs["Scale"].default_value = scale
        nd.inputs["Detail"].default_value = detail
        nd.inputs["Roughness"].default_value = rough
        nd.inputs["Distortion"].default_value = distortion
        self.link(self.p_streak if streak else self.p, nd.inputs["Vector"])
        if offset:
            add = self.node("ShaderNodeMath", operation="ADD")
            add.inputs[1].default_value = offset
            self.link(self.w, add.inputs[0])
            self.link(add.outputs[0], nd.inputs["W"])
        else:
            self.link(self.w, nd.inputs["W"])
        return nd

    def voronoi(self, scale, feature="F1", metric="EUCLIDEAN", rand=1.0, streak=False):
        nd = self.node("ShaderNodeTexVoronoi", voronoi_dimensions="4D", feature=feature,
                       distance=metric)
        nd.inputs["Scale"].default_value = scale
        nd.inputs["Randomness"].default_value = rand
        self.link(self.p_streak if streak else self.p, nd.inputs["Vector"])
        self.link(self.w, nd.inputs["W"])
        return nd

    def ramp(self, src, stops):
        """A colour ramp; stops: [(position, (r, g, b)), …]."""
        r = self.node("ShaderNodeValToRGB")
        cr = r.color_ramp
        while len(cr.elements) > 1:
            cr.elements.remove(cr.elements[-1])
        cr.elements[0].position = stops[0][0]
        cr.elements[0].color = (*stops[0][1], 1.0)
        for pos, col in stops[1:]:
            e = cr.elements.new(pos)
            e.color = (*col, 1.0)
        self.link(src, r.inputs[0])
        return r.outputs[0]

    def math(self, op, a, b=None, clamp=False):
        m = self.node("ShaderNodeMath", operation=op, use_clamp=clamp)
        for idx, val in enumerate([a, b]):
            if val is None:
                continue
            if isinstance(val, (int, float)):
                m.inputs[idx].default_value = val
            else:
                self.link(val, m.inputs[idx])
        return m.outputs[0]

    def mix(self, a, b, fac, mode="MIX"):
        m = self.node("ShaderNodeMix", data_type="RGBA", blend_type=mode)
        if isinstance(fac, (int, float)):
            m.inputs["Factor"].default_value = fac
        else:
            self.link(fac, m.inputs["Factor"])
        for slot, val in (("A", a), ("B", b)):
            if isinstance(val, tuple):
                m.inputs[slot].default_value = (*val, 1.0)
            else:
                self.link(val, m.inputs[slot])
        return m.outputs["Result"]

    def cells(self, columns, rows, stagger=0.0):
        """A grid of `columns` × `rows` cells per tile (bricks, tiles, boards), every other row
        shifted by `stagger` of a cell: the position inside the cell (0–1 across, 0–1 up) and a
        random number per cell. Integer counts keep the grid tileable."""
        row = self.math("FLOOR", self.math("MULTIPLY", self.v, float(rows)))
        shift = self.math("MULTIPLY", self.math("FLOORED_MODULO", row, 2.0), stagger)
        across = self.math("ADD", self.math("MULTIPLY", self.u, float(columns)), shift)
        column = self.math("FLOORED_MODULO", self.math("FLOOR", across), float(columns))
        inside_u = self.math("FRACT", across)
        inside_v = self.math("FRACT", self.math("MULTIPLY", self.v, float(rows)))
        key = self.node("ShaderNodeCombineXYZ")
        self.link(column, key.inputs[0])
        self.link(row, key.inputs[1])
        white = self.node("ShaderNodeTexWhiteNoise", noise_dimensions="3D")
        self.link(key.outputs[0], white.inputs["Vector"])
        return inside_u, inside_v, white.outputs["Value"]

    def edge(self, inside, width):
        """1 within `width` of either edge of a cell (0–1), else 0, softened a little."""
        near = self.math("MINIMUM", inside, self.math("SUBTRACT", 1.0, inside))
        return self.math("SUBTRACT", 1.0, self.math("MULTIPLY", near, 1.0 / width, clamp=True))

    def output(self, color, rough, height, bump_strength=1.0, bump_distance=0.02):
        """Emission carries the albedo for baking; the BSDF carries bump and roughness."""
        bsdf = self.node("ShaderNodeBsdfPrincipled")
        bump = self.node("ShaderNodeBump")
        bump.inputs["Strength"].default_value = bump_strength
        bump.inputs["Distance"].default_value = bump_distance
        self.link(height, bump.inputs["Height"])
        self.link(bump.outputs["Normal"], bsdf.inputs["Normal"])
        self.link(color, bsdf.inputs["Base Color"])
        if isinstance(rough, (int, float)):
            bsdf.inputs["Roughness"].default_value = rough
        else:
            self.link(rough, bsdf.inputs["Roughness"])
        self.color, self.rough, self.bsdf = color, rough, bsdf
        out = self.node("ShaderNodeOutputMaterial")
        self.out = out
        self.link(bsdf.outputs[0], out.inputs["Surface"])


# --- materials -------------------------------------------------------------------------------
# Colours are a starting point: tune them towards the palette as each surface is restyled.


def grass(t):
    # Clumps of slightly different greens, each made of fine blade streaks, with soil showing
    # through in places.
    patch = t.noise(3, detail=4, rough=0.6)
    clumps = t.voronoi(18, feature="F1", rand=1.0)
    blades = t.noise(14, detail=8, rough=0.75, streak=True)
    blades2 = t.noise(22, detail=8, rough=0.75, offset=9.0, streak=True)
    base = t.ramp(patch.outputs["Fac"], [
        (0.3, (0.13, 0.21, 0.05)), (0.5, (0.2, 0.3, 0.08)), (0.72, (0.33, 0.36, 0.13)),
    ])
    clump_tint = t.ramp(clumps.outputs["Color"], [(0.0, (0.75, 0.8, 0.7)),
                                                  (1.0, (1.15, 1.1, 0.95))])
    col = t.mix(base, clump_tint, 1.0, "MULTIPLY")
    streaks = t.math("MULTIPLY", blades.outputs["Fac"], blades2.outputs["Fac"])
    col = t.mix(col, t.ramp(streaks, [(0.15, (0.25, 0.25, 0.25)), (0.45, (1.0, 1.0, 1.0))]),
                0.45, "OVERLAY")
    soil = t.noise(10, detail=6, rough=0.6, offset=5.0)
    bare = t.math("GREATER_THAN",
                  t.math("ADD", soil.outputs["Fac"], t.math("MULTIPLY", streaks, -0.3)), 0.58)
    col = t.mix(col, (0.25, 0.18, 0.11), t.math("MULTIPLY", bare, 0.85))
    height = t.math("ADD", t.math("MULTIPLY", streaks, 1.2),
                    t.math("MULTIPLY", clumps.outputs["Distance"], -0.3))
    t.output(col, 0.9, height, 1.0, 0.02)


def dirt(t):
    pebbles = t.voronoi(12, feature="F1", rand=0.9)
    pebble_mask = t.math("LESS_THAN", pebbles.outputs["Distance"], 0.45)
    pebble_tone = t.ramp(pebbles.outputs["Color"], [(0.0, (0.3, 0.26, 0.22)),
                                                    (1.0, (0.6, 0.55, 0.48))])
    grain = t.noise(45, detail=10, rough=0.7)
    clump = t.noise(5, detail=6, rough=0.6, offset=4.0)
    col = t.ramp(clump.outputs["Fac"], [(0.3, (0.27, 0.19, 0.12)), (0.55, (0.42, 0.31, 0.2)),
                                        (0.8, (0.55, 0.43, 0.3))])
    col = t.mix(col, t.ramp(grain.outputs["Fac"], [(0.3, (0.3, 0.3, 0.3)), (0.7, (1, 1, 1))]),
                0.22, "OVERLAY")
    col = t.mix(col, pebble_tone, t.math("MULTIPLY", pebble_mask, 0.9))
    dome = t.math("SUBTRACT", 0.45, pebbles.outputs["Distance"])
    height = t.math("ADD", t.math("MULTIPLY", t.math("MAXIMUM", dome, 0.0), 2.5),
                    t.math("MULTIPLY", grain.outputs["Fac"], 0.3))
    t.output(col, 0.92, height, 1.0, 0.03)


def sand(t):
    warp = t.noise(2, detail=3, rough=0.5)
    ripple = t.math("SINE", t.math("MULTIPLY", t.math(
        "ADD", t.u, t.math("MULTIPLY", warp.outputs["Fac"], 0.25)), 2 * math.pi * 18))
    grain = t.noise(80, detail=8, rough=0.8)
    tone = t.noise(4, detail=5, rough=0.6, offset=1.0)
    col = t.ramp(tone.outputs["Fac"], [(0.3, (0.62, 0.5, 0.34)), (0.7, (0.8, 0.68, 0.5))])
    col = t.mix(col, t.ramp(grain.outputs["Fac"], [(0.3, (0.4, 0.4, 0.4)), (0.7, (1, 1, 1))]),
                0.2, "OVERLAY")
    height = t.math("ADD", t.math("MULTIPLY", ripple, 0.25),
                    t.math("MULTIPLY", grain.outputs["Fac"], 0.2))
    t.output(col, 0.95, height, 0.6, 0.008)


def bark(t):
    # Vertical fibres from streaked noise, broken into plates.
    fibres = t.noise(6, detail=10, rough=0.65, distortion=0.3, streak=True)
    plates = t.voronoi(0.8, feature="DISTANCE_TO_EDGE", streak=True)
    col = t.ramp(fibres.outputs["Fac"], [(0.3, (0.12, 0.08, 0.05)), (0.6, (0.28, 0.19, 0.12)),
                                         (0.8, (0.38, 0.28, 0.19))])
    crack = t.math("LESS_THAN", plates.outputs["Distance"], 0.04)
    col = t.mix(col, (0.05, 0.03, 0.02), t.math("MULTIPLY", crack, 0.9))
    height = t.math("SUBTRACT", t.math("MULTIPLY", fibres.outputs["Fac"], 0.5),
                    t.math("MULTIPLY", crack, 0.7))
    t.output(col, 0.9, height, 1.0, 0.03)


def asphalt(t):
    # Fine aggregate in dark binder, worn lighter in broad patches, with a few hairline cracks.
    stones = t.voronoi(90, feature="F1", rand=1.0)
    stone = t.math("LESS_THAN", stones.outputs["Distance"], 0.35)
    stone_tone = t.ramp(stones.outputs["Color"], [(0.0, (0.18, 0.18, 0.18)),
                                                  (1.0, (0.42, 0.41, 0.4))])
    wear = t.noise(2.5, detail=5, rough=0.55, offset=2.0)
    grain = t.noise(120, detail=6, rough=0.7)
    col = t.ramp(wear.outputs["Fac"], [(0.3, (0.09, 0.09, 0.1)), (0.7, (0.2, 0.2, 0.2))])
    col = t.mix(col, stone_tone, t.math("MULTIPLY", stone, 0.6))
    col = t.mix(col, t.ramp(grain.outputs["Fac"], [(0.3, (0.5, 0.5, 0.5)), (0.7, (1, 1, 1))]),
                0.15, "OVERLAY")
    cracks = t.voronoi(1.5, feature="DISTANCE_TO_EDGE")
    crack_mask = t.noise(1.2, detail=2, rough=0.5, offset=6.0)
    crack = t.math("MULTIPLY", t.math("LESS_THAN", cracks.outputs["Distance"], 0.006),
                   t.math("GREATER_THAN", crack_mask.outputs["Fac"], 0.55))
    col = t.mix(col, (0.04, 0.04, 0.04), t.math("MULTIPLY", crack, 0.9))
    height = t.math("SUBTRACT", t.math("MULTIPLY", stone, 0.4), t.math("MULTIPLY", crack, 0.6))
    rough = t.math("SUBTRACT", 0.92, t.math("MULTIPLY", wear.outputs["Fac"], 0.15))
    t.output(col, rough, height, 0.8, 0.006)


def gravel(t):
    # Rounded stones: each Voronoi cell one stone of its own tone, shaded darker towards its
    # rim, so it reads as a pebble rather than a tile; smaller stones show between the big ones,
    # fine grit in the deepest gaps. (Voronoi cells come about 2π times the scale per tile on the
    # torus: 9 gives ~55 stones across.)
    big = t.voronoi(9, feature="F1", rand=1.0)
    big_edge = t.voronoi(9, feature="DISTANCE_TO_EDGE", rand=1.0)
    small = t.voronoi(24, feature="F1", rand=1.0)
    small_edge = t.voronoi(24, feature="DISTANCE_TO_EDGE", rand=1.0)
    grit = t.noise(160, detail=4, rough=0.7)
    # Roundness 0 at a stone's rim to 1 in its middle; big stones are smaller than their cells.
    big_round = t.math("MULTIPLY", t.math("SUBTRACT", big_edge.outputs["Distance"], 0.025), 14.0,
                       clamp=True)
    small_round = t.math("MULTIPLY", t.math("SUBTRACT", small_edge.outputs["Distance"], 0.01), 25.0,
                         clamp=True)
    big_on = t.math("GREATER_THAN", big_round, 0.0)
    big_tone = t.ramp(big.outputs["Color"], [(0.0, (0.36, 0.33, 0.29)), (0.5, (0.54, 0.51, 0.46)),
                                             (1.0, (0.7, 0.66, 0.6))])
    small_tone = t.ramp(small.outputs["Color"], [(0.0, (0.33, 0.3, 0.27)),
                                                 (1.0, (0.56, 0.52, 0.47))])
    grit_tone = t.ramp(grit.outputs["Fac"], [(0.3, (0.2, 0.18, 0.16)), (0.7, (0.34, 0.31, 0.27))])
    smalls = t.mix(grit_tone, small_tone, t.math("POWER", small_round, 0.5))
    smalls = t.mix(smalls, (0.0, 0.0, 0.0), t.math("MULTIPLY", t.math("SUBTRACT", 1.0, small_round), 0.25), "MULTIPLY")
    bigs = t.mix(t.mix(big_tone, (0.25, 0.23, 0.2), 0.0), (1, 1, 1), 0.0)
    bigs = t.mix(bigs, (0.0, 0.0, 0.0), t.math("MULTIPLY", t.math("SUBTRACT", 1.0, big_round), 0.35), "MIX")
    col = t.mix(smalls, bigs, big_on)
    height = t.math("ADD", t.math("MULTIPLY", t.math("POWER", big_round, 0.5), 1.0),
                    t.math("MULTIPLY", t.math("POWER", small_round, 0.5), 0.35))
    rough = t.math("SUBTRACT", 0.95, t.math("MULTIPLY", big_on, 0.1))
    t.output(col, rough, height, 1.0, 0.025)


def plaster(t):
    # Rendered wall, light and neutral (the app tints it per building): a fine trowelled grain,
    # soft large blotches where it weathered, faint streaks running down.
    grain = t.noise(60, detail=8, rough=0.7)
    trowel = t.noise(8, detail=6, rough=0.6, distortion=0.5, offset=2.0)
    blotch = t.noise(1.5, detail=4, rough=0.55, offset=6.0)
    streak = t.noise(3, detail=6, rough=0.6, offset=9.0, streak=True)
    tone = t.math("ADD", t.math("MULTIPLY", blotch.outputs["Fac"], 0.5),
                  t.math("MULTIPLY", streak.outputs["Fac"], 0.3))
    col = t.ramp(tone, [(0.3, (0.7, 0.69, 0.67)), (0.6, (0.82, 0.81, 0.79)), (0.8, (0.86, 0.85, 0.83))])
    col = t.mix(col, t.ramp(grain.outputs["Fac"], [(0.35, (0.6, 0.6, 0.6)), (0.65, (1, 1, 1))]),
                0.2, "OVERLAY")
    height = t.math("ADD", t.math("MULTIPLY", grain.outputs["Fac"], 0.3),
                    t.math("MULTIPLY", trowel.outputs["Fac"], 0.5))
    t.output(col, 0.9, height, 0.6, 0.01)


def roof_tiles(t):
    # Clay tiles in rows (UV: u along the eaves, v up the slope), 8 across and 6 up per tile:
    # each tile curved, its lower edge casting a shadow on the row below, tones varying, light
    # and neutral for the app's roof colour.
    inside_u, inside_v, pick = t.cells(8, 6, stagger=0.5)
    curve = t.math("SINE", t.math("MULTIPLY", inside_u, math.pi))
    overlap = t.math("POWER", inside_v, 3.0)
    gap = t.edge(inside_u, 0.06)
    tone = t.ramp(pick, [(0.0, (0.62, 0.6, 0.58)), (0.5, (0.78, 0.76, 0.74)), (1.0, (0.9, 0.88, 0.86))])
    dirt = t.noise(4, detail=5, rough=0.6, offset=3.0)
    col = t.mix(tone, t.ramp(dirt.outputs["Fac"], [(0.3, (0.55, 0.55, 0.55)), (0.7, (1, 1, 1))]),
                0.35, "MULTIPLY")
    shade = t.math("ADD", t.math("MULTIPLY", curve, 0.25), 0.75)
    col = t.mix(col, (0.0, 0.0, 0.0), t.math("SUBTRACT", 1.0, shade), "MIX")
    col = t.mix(col, (0.12, 0.1, 0.09), t.math("MULTIPLY", t.math("SUBTRACT", 1.0, overlap), 0.0))
    col = t.mix(col, (0.1, 0.09, 0.08), t.math("MULTIPLY", gap, 0.6))
    height = t.math("ADD", t.math("MULTIPLY", curve, 0.6), t.math("MULTIPLY", overlap, 0.6))
    height = t.math("SUBTRACT", height, t.math("MULTIPLY", gap, 0.4))
    t.output(col, 0.75, height, 1.0, 0.03)


def planks(t):
    # Vertical boards (UV: u along the wall, v up), 10 per tile: grain running up each board,
    # boards of slightly different tones, dark joints.
    inside_u, _, pick = t.cells(10, 1)
    grain = t.noise(10, detail=10, rough=0.65, distortion=0.4, streak=True)
    knots = t.voronoi(3, feature="F1", rand=1.0)
    knot = t.math("LESS_THAN", knots.outputs["Distance"], 0.12)
    tone = t.ramp(pick, [(0.0, (0.3, 0.2, 0.13)), (1.0, (0.45, 0.32, 0.21))])
    col = t.mix(tone, t.ramp(grain.outputs["Fac"], [(0.3, (0.55, 0.55, 0.55)), (0.7, (1.1, 1.1, 1.1))]),
                0.6, "MULTIPLY")
    col = t.mix(col, (0.12, 0.08, 0.05), t.math("MULTIPLY", knot, 0.6))
    joint = t.edge(inside_u, 0.04)
    col = t.mix(col, (0.05, 0.035, 0.025), t.math("MULTIPLY", joint, 0.85))
    height = t.math("SUBTRACT", t.math("MULTIPLY", grain.outputs["Fac"], 0.3),
                    t.math("MULTIPLY", joint, 0.8))
    t.output(col, 0.85, height, 1.0, 0.02)


def masonry(t):
    # Stone blocks in courses (UV in metres along and up), 4 across and 5 up per tile, staggered:
    # each block its own tone with a rough face, mortar joints recessed.
    inside_u, inside_v, pick = t.cells(4, 5, stagger=0.5)
    face = t.noise(12, detail=8, rough=0.65)
    stain = t.noise(2, detail=4, rough=0.55, offset=4.0)
    tone = t.ramp(pick, [(0.0, (0.5, 0.48, 0.45)), (0.5, (0.62, 0.6, 0.56)), (1.0, (0.72, 0.7, 0.66))])
    col = t.mix(tone, t.ramp(face.outputs["Fac"], [(0.35, (0.6, 0.6, 0.6)), (0.65, (1.05, 1.05, 1.05))]),
                0.5, "MULTIPLY")
    col = t.mix(col, t.ramp(stain.outputs["Fac"], [(0.3, (0.7, 0.7, 0.7)), (0.7, (1, 1, 1))]), 0.5,
                "MULTIPLY")
    mortar = t.math("MAXIMUM", t.edge(inside_u, 0.035), t.edge(inside_v, 0.05))
    col = t.mix(col, (0.55, 0.53, 0.5), mortar)
    height = t.math("SUBTRACT", t.math("MULTIPLY", face.outputs["Fac"], 0.4),
                    t.math("MULTIPLY", mortar, 0.8))
    t.output(col, 0.9, height, 1.0, 0.03)


def slate(t):
    # Small grey slates in overlapping rows, 10 across and 8 up per tile.
    inside_u, inside_v, pick = t.cells(10, 8, stagger=0.5)
    grain = t.noise(30, detail=6, rough=0.6, streak=True)
    tone = t.ramp(pick, [(0.0, (0.2, 0.21, 0.23)), (1.0, (0.32, 0.33, 0.36))])
    col = t.mix(tone, t.ramp(grain.outputs["Fac"], [(0.35, (0.8, 0.8, 0.8)), (0.65, (1.1, 1.1, 1.1))]),
                0.5, "MULTIPLY")
    gap = t.math("MAXIMUM", t.edge(inside_u, 0.04), t.edge(inside_v, 0.03))
    col = t.mix(col, (0.06, 0.06, 0.07), t.math("MULTIPLY", gap, 0.8))
    height = t.math("SUBTRACT", t.math("MULTIPLY", t.math("POWER", inside_v, 2.0), 0.5),
                    t.math("MULTIPLY", gap, 0.4))
    t.output(col, 0.6, height, 1.0, 0.02)


def concrete(t):
    # Fine aggregate and weathered slabs; 3 m per tile in the app, with narrow recessed joints.
    u, v, pick = t.cells(4, 6)
    grain = t.noise(110, detail=6, rough=0.65)
    weather = t.noise(2, detail=5, rough=0.6, offset=3.0)
    tone = t.ramp(pick, [(0.0, (0.28, 0.29, 0.28)), (1.0, (0.38, 0.39, 0.37))])
    col = t.mix(tone, t.ramp(weather.outputs["Fac"], [(0.2, (0.65, 0.65, 0.62)), (0.8, (1.0, 1.0, 0.98))]), 0.35, "MULTIPLY")
    joint = t.math("MAXIMUM", t.edge(u, 0.012), t.edge(v, 0.012))
    col = t.mix(col, (0.09, 0.1, 0.09), joint)
    height = t.math("SUBTRACT", t.math("MULTIPLY", grain.outputs["Fac"], 0.18), t.math("MULTIPLY", joint, 0.7))
    t.output(col, 0.88, height, 0.65, 0.006)


def paving(t):
    # Staggered rectangular pavers, 25 by 50 cm when the tile spans 2 m.
    u, v, pick = t.cells(8, 4, stagger=0.5)
    grain = t.noise(65, detail=6, rough=0.7)
    weather = t.noise(3, detail=5, rough=0.6, offset=8.0)
    tone = t.ramp(pick, [(0.0, (0.22, 0.23, 0.22)), (0.5, (0.31, 0.3, 0.27)), (1.0, (0.39, 0.39, 0.36))])
    col = t.mix(tone, t.ramp(weather.outputs["Fac"], [(0.2, (0.7, 0.72, 0.66)), (0.8, (1.0, 1.0, 1.0))]), 0.3, "MULTIPLY")
    joint = t.math("MAXIMUM", t.edge(u, 0.035), t.edge(v, 0.02))
    col = t.mix(col, (0.065, 0.07, 0.055), joint)
    height = t.math("SUBTRACT", t.math("MULTIPLY", grain.outputs["Fac"], 0.2), t.math("MULTIPLY", joint, 0.8))
    t.output(col, 0.9, height, 0.8, 0.012)


MATERIALS = {
    "rock": None, "grass": grass, "dirt": dirt, "sand": sand, "bark": bark, "asphalt": asphalt,
    "gravel": gravel, "plaster": plaster, "roof_tiles": roof_tiles, "planks": planks,
    "masonry": masonry, "slate": slate, "concrete": concrete, "paving": paving,
}


# --- baking ----------------------------------------------------------------------------------

QUALITY = {"albedo": 90, "normal": 100, "rough": 90}  # 100: lossless


def setup_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 1
    # A margin would bleed the opposite edge's pixels in; the plane fills the whole image.
    scene.render.bake.margin = 0
    scene.view_settings.look = "None"
    scene.render.image_settings.file_format = "WEBP"
    scene.render.image_settings.color_mode = "RGB"
    bpy.ops.mesh.primitive_plane_add(size=2.0)
    return bpy.context.active_object


def bake(obj, name, recipe, size, out):
    mat = bpy.data.materials.new(name)
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    t = Tree(mat)
    recipe(t)
    for kind in ("albedo", "normal", "rough"):
        img = bpy.data.images.new(f"{name}_{kind}", size, size, float_buffer=False)
        if kind != "albedo":
            img.colorspace_settings.name = "Non-Color"
        tex = t.node("ShaderNodeTexImage", image=img)
        t.nt.nodes.active = tex
        if kind == "normal":
            t.link(t.bsdf.outputs[0], t.out.inputs["Surface"])
            bpy.context.scene.render.bake.normal_space = "TANGENT"
            bpy.ops.object.bake(type="NORMAL")
        else:
            # Colour and roughness go through an emission shader for exact values.
            em = t.node("ShaderNodeEmission")
            source = t.color if kind == "albedo" else t.rough
            if isinstance(source, (int, float)):
                em.inputs["Color"].default_value = (source, source, source, 1.0)
            else:
                t.link(source, em.inputs["Color"])
            t.link(em.outputs[0], t.out.inputs["Surface"])
            bpy.ops.object.bake(type="EMIT")
        path = os.path.abspath(os.path.join(out, f"{name}_{kind}.webp"))
        scene = bpy.context.scene
        # The render output applies a view transform: Standard turns the sRGB albedo back into
        # the same sRGB bytes, Raw leaves the non-colour maps alone. (Changing the image's own
        # colour space instead would regenerate it and lose the bake.)
        scene.view_settings.view_transform = "Standard" if kind == "albedo" else "Raw"
        scene.render.image_settings.quality = QUALITY[kind]
        img.save_render(path, scene=scene)
        print("baked", path, flush=True)


def main():
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    size, out = 1024, OUT
    if "--size" in args:
        i = args.index("--size")
        size = int(args[i + 1])
        del args[i:i + 2]
    if "--out" in args:
        i = args.index("--out")
        out = args[i + 1]
        del args[i:i + 2]
    os.makedirs(out, exist_ok=True)
    obj = setup_scene()
    for name in args or list(MATERIALS):
        if name == "rock":
            convert_rock(size, out)
        else:
            bake(obj, name, MATERIALS[name], size, out)


main()
