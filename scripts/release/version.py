#!/usr/bin/env python3
"""Checks that every place naming Torqa's version names the same one.

The version lives in the Cargo workspace (the About tab, the CLI and the User-Agent read it)
and in the macOS export preset (the bundle's version). A tag changes neither: v0.2.0 was
built from a version still saying 0.1.0, and its About tab showed 0.1.0.

With --tag, the tag must also be `v` followed by that version, so that a release is built only
from a commit that says what it is.
"""

import argparse
import pathlib
import re
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
PRESET_VERSION = re.compile(r'^(application/(?:short_)?version)="([^"]*)"$', re.MULTILINE)


def cargo_version(root: pathlib.Path) -> str:
    with (root / "core" / "Cargo.toml").open("rb") as manifest:
        return tomllib.load(manifest)["workspace"]["package"]["version"]


def problems(root: pathlib.Path, tag: str | None = None) -> list[str]:
    version = cargo_version(root)
    found = []
    presets = PRESET_VERSION.findall((root / "app" / "export_presets.cfg").read_text())
    # Without them, Godot would fall back to an unset project version for the macOS bundle.
    if not presets:
        found.append("app/export_presets.cfg sets no application version")
    for setting, value in presets:
        if value != version:
            found.append(
                f"app/export_presets.cfg has {setting}={value}, core/Cargo.toml has {version}"
            )
    if tag is not None and tag != f"v{version}":
        found.append(
            f"tag {tag} does not name version {version}: set the version first, "
            f"then tag v<version>"
        )
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--tag", help="the release tag, which must be v<version>")
    found = problems(ROOT, parser.parse_args().tag)
    for problem in found:
        print(problem, file=sys.stderr)
    if not found:
        print(f"Version {cargo_version(ROOT)}")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
