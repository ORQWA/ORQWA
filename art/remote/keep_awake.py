"""Makes the silent clip that keeps a phone's screen on while it shows the remote's page.

Run in the art container:
    scripts/art.sh blender --background --factory-startup --python art/remote/keep_awake.py

Over plain http the page has no wake lock, so it plays this clip unseen (ADR 0019). A black
picture and a silent sound, both needed: Safari keeps the screen on only for a video with sound
that is not muted, Firefox for one with a sound track, muted or not, Chrome for any video big
enough on screen. H.264 and AAC in MP4, the one
format both play on every phone. Tiny, as it is embedded in the page: 64 px, a few frames a
second, mono.
"""

import os
import shutil
import tempfile

import bpy

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(
    os.path.join(HERE, "..", "..", "core", "orqwa-remote", "src", "keep-awake.mp4"))

SIZE = 64
FPS = 5
# Long enough for the page to go back to the start a second before its end (keepAwake).
SECONDS = 4

scene = bpy.context.scene
scene.render.resolution_x = SIZE
scene.render.resolution_y = SIZE
scene.render.resolution_percentage = 100
scene.render.fps = FPS
scene.render.fps_base = 1.0
scene.frame_start = 1
scene.frame_end = FPS * SECONDS

# The sequencer's black strip is the picture: no camera, nothing to render on a GPU.
editor = scene.sequence_editor_create()
black = editor.strips.new_effect(
    name="black", type="COLOR", channel=1, frame_start=1, length=FPS * SECONDS)
black.color = (0.0, 0.0, 0.0)
scene.render.use_sequencer = True
scene.render.use_compositing = False
# No render date or other stamp in the file's metadata, so a rebuild gives the same bytes.
for name in dir(scene.render):
    if name.startswith("use_stamp"):
        setattr(scene.render, name, False)

settings = scene.render.image_settings
if hasattr(settings, "media_type"):
    settings.media_type = "VIDEO"
settings.file_format = "FFMPEG"
ffmpeg = scene.render.ffmpeg
ffmpeg.format = "MPEG4"
ffmpeg.codec = "H264"
ffmpeg.constant_rate_factor = "MEDIUM"
ffmpeg.gopsize = FPS * SECONDS
# The scene has no sound, so the mixdown is silence: the track only has to be there.
ffmpeg.audio_codec = "AAC"
ffmpeg.audio_channels = "MONO"
ffmpeg.audio_mixrate = 22050
ffmpeg.audio_bitrate = 32

# Blender names a video by its frames; render into a scratch folder and take the one file.
with tempfile.TemporaryDirectory() as scratch:
    scene.render.filepath = os.path.join(scratch, "clip")
    bpy.ops.render.render(animation=True)
    (rendered,) = os.listdir(scratch)
    shutil.copyfile(os.path.join(scratch, rendered), OUT)

print(f"wrote {OUT} ({os.path.getsize(OUT)} bytes)")
