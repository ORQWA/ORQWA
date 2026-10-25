"""Adds what only the Android build needs to its copy of the project (ADR 0024): the extension's
Android library, Android's texture formats, ORQWA's Android plugin, the Android preset and the
launcher icon's layers.
And the version of rustls-platform-verifier's Java part that matches the crate in Cargo.lock,
and ORQWA's version from Cargo.toml as the APK's: Android installs an update only over a lower
version code.
Usage: stage.py <copied app folder> <android/godot/export_preset.cfg> <core/Cargo.lock>
    <core/Cargo.toml>"""

import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

app = Path(sys.argv[1])
preset = Path(sys.argv[2]).read_text()
lock = Path(sys.argv[3]).read_text()
manifest = Path(sys.argv[4]).read_text()

verifier = re.search(
    r'name = "rustls-platform-verifier-android"\nversion = "([^"]+)"', lock
)
if verifier is None:
    sys.exit("Cargo.lock names no rustls-platform-verifier-android")
(app / "addons/orqwa_android/verifier_version.txt").write_text(verifier.group(1) + "\n")

extension = app / "orqwa.gdextension"
library = '"res://bin/android/liborqwa_gd.so"'
extension.write_text(
    extension.read_text().rstrip("\n")
    + f"\nandroid.debug.arm64 = {library}\nandroid.release.arm64 = {library}\n"
)


def set_setting(text: str, section: str, key: str, value: str) -> str:
    """Sets `key` in `[section]` of a Godot project file, adding either as needed."""
    line = f"{key}={value}"
    header = f"[{section}]"
    if header not in text:
        return text.rstrip("\n") + f"\n\n{header}\n\n{line}\n"
    start = text.index(header) + len(header)
    end = text.find("\n[", start)
    end = len(text) if end < 0 else end
    body = text[start:end]
    pattern = re.compile(rf"^{re.escape(key)}=.*$", re.M)
    body = pattern.sub(line, body) if pattern.search(body) else body.rstrip("\n") + f"\n{line}\n"
    return text[:start] + body + text[end:]


# Android lays the launcher icon's foreground over its background and crops both to a shape of the
# launcher's choosing, leaving only a circle of 66 dp of the 108 dp layers sure to show. Given the
# desktop icon whole, it cut the logo off. So the foreground is the logo without its tile, its
# enclosing circle (centre 382 367, radius 295 in icon.svg's units) inside that one, and the
# background the tile's colour.
SVG = "http://www.w3.org/2000/svg"
ET.register_namespace("", SVG)
icon = ET.parse(app / "icon.svg")
tile = icon.getroot().find(f"{{{SVG}}}rect")
if tile is None:
    sys.exit("icon.svg has no tile")
icon.getroot().remove(tile)
icon.getroot().set("viewBox", "-118 -133 1000 1000")
icon.write(app / "icon_foreground.svg")
(app / "icon_background.svg").write_text(
    f'<svg xmlns="{SVG}" viewBox="0 0 432 432">'
    f'<rect width="432" height="432" fill="{tile.get("fill")}"/></svg>\n'
)

project = app / "project.godot"
text = project.read_text()
text = set_setting(text, "rendering", "textures/vram_compression/import_etc2_astc", "true")
text = set_setting(
    text, "editor_plugins", "enabled", 'PackedStringArray("res://addons/orqwa_android/plugin.cfg")'
)
project.write_text(text)

presets = app / "export_presets.cfg"
existing = presets.read_text() if presets.exists() else ""
numbers = [int(n) for n in re.findall(r"^\[preset\.(\d+)\]", existing, re.M)]
number = str(max(numbers, default=-1) + 1)
body = "\n".join(line for line in preset.splitlines() if not line.startswith(";"))
# 0.5.0 is code 500 and 1.2.3 is 10203: each release's code above the last one's.
version = re.search(r'^\[workspace\.package\]\n(?:.*\n)*?version = "(\d+)\.(\d+)\.(\d+)"', manifest, re.M)
if version is None:
    sys.exit("core/Cargo.toml names no workspace version")
major, minor, patch = (int(n) for n in version.groups())
body = body.replace(
    "[preset.PRESET.options]\n",
    "[preset.PRESET.options]\n\n"
    f"version/code={major * 10000 + minor * 100 + patch}\n"
    f'version/name="{major}.{minor}.{patch}"\n',
)
presets.write_text(existing.rstrip("\n") + "\n\n" + body.replace("PRESET", number).strip() + "\n")
