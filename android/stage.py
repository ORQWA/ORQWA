"""Adds what only the Android build needs to its copy of the project (ADR 0024): the extension's
Android library, Android's texture formats, Torqa's Android plugin and the Android preset.
And the version of rustls-platform-verifier's Java part that matches the crate in Cargo.lock.
Usage: stage.py <copied app folder> <android/godot/export_preset.cfg> <core/Cargo.lock>"""

import re
import sys
from pathlib import Path

app = Path(sys.argv[1])
preset = Path(sys.argv[2]).read_text()
lock = Path(sys.argv[3]).read_text()

verifier = re.search(
    r'name = "rustls-platform-verifier-android"\nversion = "([^"]+)"', lock
)
if verifier is None:
    sys.exit("Cargo.lock names no rustls-platform-verifier-android")
(app / "addons/torqa_android/verifier_version.txt").write_text(verifier.group(1) + "\n")

extension = app / "torqa.gdextension"
library = '"res://bin/android/libtorqa_gd.so"'
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


project = app / "project.godot"
text = project.read_text()
text = set_setting(text, "rendering", "textures/vram_compression/import_etc2_astc", "true")
text = set_setting(
    text, "editor_plugins", "enabled", 'PackedStringArray("res://addons/torqa_android/plugin.cfg")'
)
project.write_text(text)

presets = app / "export_presets.cfg"
existing = presets.read_text() if presets.exists() else ""
numbers = [int(n) for n in re.findall(r"^\[preset\.(\d+)\]", existing, re.M)]
number = str(max(numbers, default=-1) + 1)
body = "\n".join(line for line in preset.splitlines() if not line.startswith(";"))
presets.write_text(existing.rstrip("\n") + "\n\n" + body.replace("PRESET", number).strip() + "\n")
