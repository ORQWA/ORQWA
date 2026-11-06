"""Build a standalone, unshipped building comparison using ORQWA's Blender kit.

Run with scripts/art.sh blender --background --factory-startup --python
art/studies/building_realism.py, then follow docs/building-realism-study.md.
The application assets, world generator and saved-world contracts are untouched.
"""

import json
import math
from pathlib import Path
import shutil
import sys

import bpy
import bmesh

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "art" / "buildings"))

import kinds  # noqa: E402
import kit  # noqa: E402

OUT = ROOT / "screenshots" / "building-realism" / "project"
MODELS = OUT / "assets" / "models" / "buildings"
ORIGINAL_WINDOW = kit.window


def detailed_window(mesh, facade, opening, **kwargs):
    """Keep the kit's reveals, adding slender glazing bars and room-coordinate UVs."""
    start = len(mesh.faces)
    ORIGINAL_WINDOW(mesh, facade, opening, **kwargs)
    p, o = facade.point, opening
    for index in range(start, len(mesh.faces)):
        if mesh.materials[index] == "glass":
            mesh.uvs[index] = [
                ((mesh.verts[v] - p(o.u0 + 0.07, o.z0 + 0.07)).dot(facade.along)
                 / (o.width - 0.14),
                 (mesh.verts[v].z - o.z0 - 0.07) / (o.z1 - o.z0 - 0.14))
                for v in mesh.faces[index]
            ]
    mid = (o.u0 + o.u1) / 2.0
    depth = o.depth - 0.045
    if o.width > 0.8:
        mesh.beam(p(mid, o.z0 + 0.07, depth), p(mid, o.z1 - 0.07, depth),
                  0.045, 0.04, "frame", up=facade.out)
    if o.z1 - o.z0 > 1.25:
        z = o.z0 + (o.z1 - o.z0) * 0.68
        mesh.beam(p(o.u0 + 0.07, z, depth), p(o.u1 - 0.07, z, depth),
                  0.04, 0.045, "frame", up=kit.UP)


def pipe(mesh, points):
    """Six-sided drainpipes keep the detail within a game-mesh budget."""
    for start, end in zip(points, points[1:]):
        mesh.cylinder(start, end, 0.055, 6, "metal")


def drainage(mesh, rect, eaves, pitch, overhang=0.65, verge=0.5):
    l, w = rect.length / 2.0, rect.width / 2.0
    edge = eaves - overhang * math.tan(math.radians(pitch)) + 0.10
    for side in (-1.0, 1.0):
        y = side * (w + overhang + 0.035)
        # A narrow gutter casts a separate shadow below the fascia.
        mesh.cylinder((-l - verge, y, edge), (l + verge, y, edge), 0.09, 8, "metal")
        for end in (-1.0, 1.0):
            x = end * (l - 0.15)
            wall_y = side * (w + 0.13)
            pipe(mesh, [(x, y, edge), (x, y, edge - 0.25),
                        (x, wall_y, edge - 0.65), (x, wall_y, 0.25),
                        (x, wall_y + side * 0.18, 0.13)])
            for z in (0.8, 2.8, 4.8, 6.8, 8.8, 10.8):
                if z < edge - 0.7:
                    mesh.beam((x, side * w, z), (x, wall_y, z), 0.07, 0.07, "metal")


def house():
    with (ROOT / "art/buildings/catalogue.json").open() as source:
        spec = next(s for s in json.load(source)["models"] if s["name"] == "house_gable_2_m")
    kinds.window = detailed_window
    mesh, _ = kinds.house(spec)
    kinds.window = ORIGINAL_WINDOW
    rect = kit.Rect(spec["length"], spec["width"])
    eaves = kinds.eaves_height(spec["storeys"])
    drainage(mesh, rect, eaves, spec["pitch"])
    for facade in rect.facades():
        # Small masonry courses articulate the plinth without changing the building mass.
        for z in (0.18,):
            mesh.beam(facade.point(0, z, -0.035), facade.point(facade.length, z, -0.035),
                      0.08, 0.12, "stone")
    return mesh


def facade_band(mesh, facade, height, thickness, reach, material="stone"):
    mesh.beam(facade.point(0, height, -reach / 2),
              facade.point(facade.length, height, -reach / 2),
              reach, thickness, material)


def iron_balcony(mesh, facade, opening):
    """Shallow stone balconies with open railings, suitable for a narrow town street."""
    p, o = facade.point, opening
    u0, u1, floor, depth = o.u0 - 0.17, o.u1 + 0.17, o.z0, -0.52
    mesh.beam(p(u0, floor - 0.08, depth / 2), p(u1, floor - 0.08, depth / 2),
              -depth, 0.13, "stone")
    mesh.beam(p(u0, floor + 1.0, depth), p(u1, floor + 1.0, depth),
              0.045, 0.05, "metal")
    mesh.beam(p(u0, floor + 0.15, depth), p(u1, floor + 0.15, depth),
              0.03, 0.035, "metal")
    count = math.ceil((u1 - u0) / 0.17)
    for k in range(count + 1):
        u = u0 + k * (u1 - u0) / count
        mesh.beam(p(u, floor + 0.12, depth), p(u, floor + 1.0, depth),
                  0.025, 0.025, "metal")
    for u in (u0, u1):
        mesh.beam(p(u, floor + 1.0, 0.0), p(u, floor + 1.0, depth),
                  0.045, 0.045, "metal")


def town(detailed):
    rect = kit.Rect(14.0, 10.0)
    mesh = kit.Mesh()
    eaves, pitch = 12.4, 28.0
    for side, facade in enumerate(rect.facades()):
        if not detailed:
            kit.wall(mesh, facade, -1.0, eaves, "shell_wall")
            continue
        front = side == 0
        openings, doors = [], []
        for level in range(4):
            floor = 0.22 + 3.0 * level
            centres = kinds.columns(facade.length, 2.8, 1.35)
            for column, u in enumerate(centres):
                if front and level == 0:
                    if column == len(centres) // 2:
                        doors.append(kit.Opening(u - 0.6, u + 0.6, 0.06, 2.8, depth=0.32))
                    else:
                        openings.append(kit.Opening(u - 1.0, u + 1.0, 0.42, 2.75, depth=0.25))
                else:
                    tall = front and level in (1, 2) and column % 2 == 0
                    openings.append(kit.Opening(u - 0.59, u + 0.59,
                                                floor + (0.12 if tall else 0.82),
                                                floor + 2.52, depth=0.24))
        kit.wall(mesh, facade, -1.0, eaves, "plaster", openings + doors)
        for o in openings:
            detailed_window(mesh, facade, o, shutters=None)
            if front and o.z0 > 3.0 and o.z1 - o.z0 > 2.0:
                iron_balcony(mesh, facade, o)
            if o.z0 > 3.0:
                facade_band_part(mesh, facade, o.u0 - 0.11, o.u1 + 0.11, o.z1 + 0.13)
        for o in doors:
            kit.door(mesh, facade, o, leaf="wood_dark")
            mesh.beam(facade.point(o.u1 - 0.14, 1.0, o.depth - 0.05),
                      facade.point(o.u1 - 0.14, 1.4, o.depth - 0.05),
                      0.035, 0.035, "metal")
        for height, thickness, reach in ((0.22, 0.14, 0.08), (3.18, 0.18, 0.13),
                                          (eaves - 0.22, 0.14, 0.16), (eaves - 0.04, 0.10, 0.23)):
            facade_band(mesh, facade, height, thickness, reach)
        # A pale stone surround anchors the shop floor to the pavement.
        if front:
            for u in (0.18, facade.length - 0.18):
                mesh.beam(facade.point(u, 0.0, -0.05), facade.point(u, 3.15, -0.05),
                          0.30, 0.13, "stone", up=facade.out)
    kit.gable_roof(mesh, rect, eaves, pitch, 0.40, 0.25,
                   gable="plaster" if detailed else "shell_wall", roof="slate")
    if detailed:
        drainage(mesh, rect, eaves, pitch, 0.40, 0.25)
        front = rect.facades()[0]
        # Two restrained shop awnings, with a separate valance and narrow support arms.
        for u0, u1 in ((0.7, 5.6), (8.4, 13.3)):
            p = front.point
            mesh.facing([p(u0, 2.85, -1.0), p(u1, 2.85, -1.0),
                         p(u1, 3.17, -0.03), p(u0, 3.17, -0.03)],
                        kit.UP + front.out, "shutter")
            mesh.facing([p(u0, 2.68, -1.0), p(u1, 2.68, -1.0),
                         p(u1, 2.85, -1.0), p(u0, 2.85, -1.0)], front.out, "shutter")
            for u in (u0 + 0.12, u1 - 0.12):
                mesh.beam(p(u, 2.7, 0.0), p(u, 2.84, -0.94), 0.035, 0.04, "metal")
        kit.chimney(mesh, -4.5, 1.8, 14.0, 15.7, size=0.65)
    return mesh


def facade_band_part(mesh, facade, u0, u1, height):
    mesh.beam(facade.point(u0, height, -0.04), facade.point(u1, height, -0.04),
              0.14, 0.10, "stone")


def export(mesh, name):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    data = bpy.data.meshes.new(name)
    data.from_pydata([tuple(v) for v in mesh.verts], [], mesh.faces)
    names = sorted(set(mesh.materials))
    for material in names:
        data.materials.append(bpy.data.materials.new(material))
    index = {name: i for i, name in enumerate(names)}
    data.polygons.foreach_set("material_index", [index[m] for m in mesh.materials])
    data.polygons.foreach_set("use_smooth", mesh.smooth)
    layer = data.uv_layers.new(name="UVMap")
    layer.data.foreach_set("uv", [v for face in mesh.uvs for uv in face for v in uv])
    shared = bmesh.new()
    shared.from_mesh(data)
    bmesh.ops.remove_doubles(shared, verts=shared.verts, dist=1e-4)
    shared.to_mesh(data)
    shared.free()
    data.update()
    data.calc_loop_triangles()
    triangles = len(data.loop_triangles)
    obj = bpy.data.objects.new(name, data)
    bpy.context.scene.collection.objects.link(obj)
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.export_scene.gltf(filepath=str(MODELS / (name + ".glb")), export_format="GLB",
                             use_selection=True, export_apply=True, export_materials="EXPORT")
    print(f"STUDY {name}: {triangles} triangles")
    return triangles


def shader():
    source = (ROOT / "app/shaders/building_model.gdshader").read_text()
    source = source.replace("uniform bool textured = false;",
                            "uniform bool textured = false;\n"
                            "uniform bool study_scan = false;\n"
                            "uniform sampler2D roughness_texture : filter_linear_mipmap_anisotropic, repeat_enable;")
    old = "\t\troughness = 0.04;\n\t\tspecular = 0.5;"
    new = """
        // Room colours sit behind the reflection; curtains and blinds vary by window.
        float seed = hash(floor(world_position.xz * 0.2) + floor(world_position.y / 3.0));
        vec2 pane = clamp(vec2(UV.x, 1.0 - UV.y), vec2(0.0), vec2(1.0));
        float curtains = 1.0 - smoothstep(0.12, 0.20 + seed * 0.14, min(pane.x, 1.0 - pane.x));
        float blinds = smoothstep(0.68 + seed * 0.30, 0.70 + seed * 0.30, pane.y);
        float pleat = 0.82 + 0.18 * sin(pane.x * 95.0);
        vec3 room = mix(vec3(0.018, 0.024, 0.029), vec3(0.075, 0.060, 0.042), seed);
        vec3 fabric = vec3(0.23, 0.21, 0.17) * pleat;
        tint = mix(room, fabric, max(curtains * step(0.22, seed), blinds));
        roughness = 0.13;
        specular = 0.5;"""
    assert source.count(old) == 1
    source = source.replace(old, new)
    marker = "\tfloat snow = winter_cover(world_position, winter_normal);"
    detail = """
    if (textured) {
        vec2 grain = UV / tile;
        float fine = texture(normal_texture, grain).g;
        roughness = study_scan ? texture(roughness_texture, grain).r
            : clamp(roughness - 0.12 + fine * 0.18, 0.45, 0.96);
        if (study_scan && pattern == TILES) {
            albedo = texture(albedo_texture, grain).rgb
                * mix(vec3(1.0), to_linear(roof_tint) * 1.75, 0.65);
        }
        float patch = sin(UV.x * 0.61 + sin(UV.y * 0.28)) * sin(UV.y * 0.73);
        albedo *= 0.96 + patch * 0.035;
    }
"""
    assert source.count(marker) == 1
    (OUT / "shaders/study.gdshader").write_text(source.replace(marker, detail + marker))
    shell = (ROOT / "app/shaders/building.gdshader").read_text()
    shell = shell.replace("uniform float storey = 3.0;", "uniform float storey = 3.0;\nuniform vec3 study_tint : source_color;")
    # Blender/glTF flips V; Rust-built shells use physical height directly in UV.y.
    shell = shell.replace("int style = int(round(COLOR.a * 11.0));\n\tvec2 uv = UV;",
                          "vec2 uv = vec2(UV.x, 1.0 - UV.y);\n"
                          "\tint style = uv.y < 3.0 && winter_normal.z > 0.9 ? SHOP : PLASTER;")
    shell = shell.replace("to_linear(COLOR.rgb)", "study_tint")
    (OUT / "shaders/shell.gdshader").write_text(shell)


def convert_scans():
    """Use archived originals offline; colour/roughness are Q90 and normals lossless."""
    scene = bpy.context.scene
    scene.view_settings.look = "None"
    scene.render.image_settings.file_format = "WEBP"
    scene.render.image_settings.color_mode = "RGB"
    for source, target in (("roof_tiles", "scan_roof"), ("painted_plaster_wall", "scan_plaster")):
        directory = ROOT / "art/sources" / source.replace("_", "-")
        for suffix, kind, ext in (("diff", "albedo", "jpg"), ("nor_gl", "normal", "exr"),
                                   ("rough", "rough", "exr")):
            image = bpy.data.images.load(str(directory / f"{source}_{suffix}_1k.{ext}"))
            if kind != "albedo":
                image.colorspace_settings.name = "Non-Color"
            scene.view_settings.view_transform = "Standard" if kind == "albedo" else "Raw"
            scene.render.image_settings.quality = 100 if kind == "normal" else 90
            image.save_render(str(OUT / "assets/textures" / f"{target}_{kind}.webp"), scene=scene)
            bpy.data.images.remove(image)
            settings = ("[remap]\nimporter=\"texture\"\ntype=\"CompressedTexture2D\"\n"
                        "[params]\ncompress/mode=2\nmipmaps/generate=true\n"
                        f"compress/normal_map={1 if kind == 'normal' else 0}\n")
            (OUT / "assets/textures" / f"{target}_{kind}.webp.import").write_text(settings)


def main():
    MODELS.mkdir(parents=True, exist_ok=True)
    for directory in ("shaders", "scenes", "assets/textures"):
        (OUT / directory).mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / "app/assets/models/buildings/house_gable_2_m.glb", MODELS / "house_before.glb")
    for name in ("palette.gd", "building_models.gd"):
        shutil.copy2(ROOT / "app/scenes" / name, OUT / "scenes" / name)
    shutil.copy2(ROOT / "app/assets/palette.json", OUT / "assets/palette.json")
    for name in ("building_model.gdshader", "winter.gdshaderinc"):
        shutil.copy2(ROOT / "app/shaders" / name, OUT / "shaders" / name)
    materials = ("plaster", "masonry", "planks", "roof_tiles", "slate", "gravel", "rock",
                 "asphalt", "paving", "concrete", "grass")
    for material in materials:
        for suffix in ("albedo", "normal", "rough"):
            texture = ROOT / f"app/assets/textures/{material}_{suffix}.webp"
            if texture.exists():
                shutil.copy2(texture, OUT / "assets/textures" / texture.name)
                settings = texture.with_suffix(texture.suffix + ".import")
                if settings.exists():
                    shutil.copy2(settings, OUT / "assets/textures" / settings.name)
    shader()
    convert_scans()
    shutil.copy2(ROOT / "art/studies/render_building_realism.gd", OUT / "render.gd")
    (OUT / "project.godot").write_text('''config_version=5
[application]
config/name="ORQWA building study"
[display]
window/size/viewport_width=1400
window/size/viewport_height=1000
[rendering]
renderer/rendering_method="forward_plus"
textures/default_filters/use_nearest_mipmap_filter=false
textures/vram_compression/import_s3tc_bptc=true
textures/vram_compression/import_etc2_astc=true
''')
    counts = {"house_after": export(house(), "house_after"),
              "town_before": export(town(False), "town_before"),
              "town_after": export(town(True), "town_after")}
    (OUT.parent / "triangles.json").write_text(json.dumps(counts, indent=2) + "\n")


if __name__ == "__main__":
    main()
