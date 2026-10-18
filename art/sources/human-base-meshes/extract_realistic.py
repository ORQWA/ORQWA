"""Extracts the realistic female and male bodies and the skeleton's joints from Blender Studio's
Human Base Meshes bundle into realistic_bodies.blend, for the rigged riders. See SOURCE.md.
To keep the repository small, the bodies keep their sculpted detail applied at the first
subdivision level (the bundle's three levels are 24 MB), and the skeleton keeps only its joints:
an empty at each listed bone's origin, named after the bone, instead of its mesh.

Run in the art container on the unpacked official bundle:
    scripts/art.sh blender --background --factory-startup --python \
        art/sources/human-base-meshes/extract_realistic.py -- <path to human_base_meshes_bundle.blend>
"""

import os
import sys

import bpy

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "realistic_bodies.blend")
BODIES = ("GEO-body_female_realistic", "GEO-body_male_realistic")
# Skeleton bones whose origins are joints: hip, knee (patella), ankle (talus), heel and ball of
# the foot, shoulder (humerus), elbow (radius), wrist (hand centre), the spine's ends, the skull.
JOINTS = ("hip", "leg_femur", "leg_patella", "foot_talus", "foot_calcaneus", "foot_center",
          "arm_humerus", "arm_radius", "hand_center", "shoulder_scapula", "spine_lumbar_l5",
          "spine_thoracic_t1", "spine_cervical_c1", "skull", "sacral", "ripcage")

bundle = sys.argv[sys.argv.index("--") + 1]
bpy.ops.wm.read_factory_settings(use_empty=True)
with bpy.data.libraries.load(bundle, link=False) as (source, target):
    target.objects = [
        name for name in source.objects
        if name in BODIES or (name.startswith("GEO-skeletion.")
                              and name.split(".")[1] in JOINTS)
    ]
for obj in target.objects:
    bpy.context.scene.collection.objects.link(obj)
bpy.context.view_layer.update()
kept = 0
for obj in list(target.objects):
    if obj.name in BODIES:
        for modifier in list(obj.modifiers):
            if modifier.type == "MULTIRES":
                modifier.levels = 1
                depsgraph = bpy.context.evaluated_depsgraph_get()
                detailed = bpy.data.meshes.new_from_object(obj.evaluated_get(depsgraph))
                obj.modifiers.clear()
                obj.data = detailed
        obj.parent = None
        kept += 1
    else:
        # The joint's place in the world, then an empty of the same name in place of the bone.
        name, where = obj.name, obj.matrix_world.copy()
        bpy.data.objects.remove(obj)
        joint = bpy.data.objects.new(name, None)
        joint.matrix_world = where
        bpy.context.scene.collection.objects.link(joint)
        kept += 1
for mesh in [m for m in bpy.data.meshes if m.users == 0]:
    bpy.data.meshes.remove(mesh)
print(f"extracted {kept} objects")
bpy.ops.wm.save_as_mainfile(filepath=OUT, compress=True)
