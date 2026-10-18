#!/usr/bin/env python3
"""Checks that every place naming Torqa's version names the same one, or sets a new one.

The version lives in the Cargo workspace (the About tab, the CLI and the User-Agent read it)
and in the macOS export preset (the bundle's version). A tag changes neither: v0.2.0 was
built from a version still saying 0.1.0, and its About tab showed 0.1.0.

With --tag, the tag must also be `v` followed by that version, so that a release is built only
from a commit that says what it is. With --set, both places get the given version;
`core/Cargo.lock` follows with `cargo update --workspace` (`make release-version` runs both).
"""

import argparse
import pathlib
import re
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
PRESET_VERSION = re.compile(r'^(application/(?:short_)?version)="([^"]*)"$', re.MULTILINE)
# Only the workspace's own version: dependencies and rust-version name versions too.
WORKSPACE_VERSION = re.compile(
    r'^(\[workspace\.package\]\n(?:(?!\[).*\n)*?version = )"[^"]*"', re.MULTILINE
)
# The macOS bundle's short version must be three numbers, so no pre-release suffix either.
RELEASE_VERSION = re.compile(r"\d+\.\d+\.\d+")


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


def set_version(root: pathlib.Path, version: str) -> None:
    """Writes `version` to the Cargo workspace and the macOS preset, or neither."""
    if not RELEASE_VERSION.fullmatch(version):
        raise ValueError(f'"{version}" is not a release version such as 0.3.1')
    manifest = root / "core" / "Cargo.toml"
    cargo, cargo_count = WORKSPACE_VERSION.subn(rf'\1"{version}"', manifest.read_text())
    if cargo_count != 1:
        raise ValueError("core/Cargo.toml has no [workspace.package] version")
    preset_file = root / "app" / "export_presets.cfg"
    presets, preset_count = PRESET_VERSION.subn(rf'\1="{version}"', preset_file.read_text())
    if preset_count == 0:
        raise ValueError("app/export_presets.cfg sets no application version")
    manifest.write_text(cargo)
    preset_file.write_text(presets)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--tag", help="the release tag, which must be v<version>")
    action.add_argument("--set", metavar="VERSION", help="the new version, such as 0.3.1")
    args = parser.parse_args()
    if args.set is not None:
        try:
            set_version(ROOT, args.set)
        except ValueError as error:
            print(error, file=sys.stderr)
            return 1
    found = problems(ROOT, args.tag)
    for problem in found:
        print(problem, file=sys.stderr)
    if not found:
        print(f"Version {cargo_version(ROOT)}")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
