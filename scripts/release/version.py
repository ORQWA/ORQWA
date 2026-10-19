#!/usr/bin/env python3
"""Checks that every place naming Torqa's version names the same one, or sets a new one.

The version lives in the Cargo workspace (the About tab, the CLI and the User-Agent read it)
and in the macOS and Windows export presets (the bundle's version, the .exe's file and product
versions). A tag changes none of them: v0.2.0 was built from a version still saying 0.1.0, and
its About tab showed 0.1.0.

With --tag, the tag must also be `v` followed by that version, so that a release is built only
from a commit that says what it is. With --set, every place gets the given version;
`core/Cargo.lock` follows with `cargo update --workspace` (`make release-version` runs both).
"""

import argparse
import pathlib
import re
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
# Each preset's version settings, and how they write the version: Windows takes four numbers.
PRESET_VERSIONS = (
    ("macOS", re.compile(r'^(application/(?:short_)?version)="([^"]*)"$', re.MULTILINE), "{}"),
    (
        "Windows",
        re.compile(r'^(application/(?:file|product)_version)="([^"]*)"$', re.MULTILINE),
        "{}.0",
    ),
)
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
    presets = (root / "app" / "export_presets.cfg").read_text()
    for system, setting_pattern, form in PRESET_VERSIONS:
        expected = form.format(version)
        settings = setting_pattern.findall(presets)
        # Without them, Godot would fall back to an unset project version (1.0.0 or 1.0.0.0).
        if not settings:
            found.append(f"app/export_presets.cfg sets no {system} version")
        for setting, value in settings:
            if value != expected:
                found.append(
                    f"app/export_presets.cfg has {setting}={value}, not {expected} for "
                    f"core/Cargo.toml's {version}"
                )
    if tag is not None and tag != f"v{version}":
        found.append(
            f"tag {tag} does not name version {version}: set the version first, "
            f"then tag v<version>"
        )
    return found


def set_version(root: pathlib.Path, version: str) -> None:
    """Writes `version` to the Cargo workspace and the macOS and Windows presets, or nowhere."""
    if not RELEASE_VERSION.fullmatch(version):
        raise ValueError(f'"{version}" is not a release version such as 0.3.1')
    manifest = root / "core" / "Cargo.toml"
    cargo, cargo_count = WORKSPACE_VERSION.subn(rf'\1"{version}"', manifest.read_text())
    if cargo_count != 1:
        raise ValueError("core/Cargo.toml has no [workspace.package] version")
    preset_file = root / "app" / "export_presets.cfg"
    presets = preset_file.read_text()
    for system, setting_pattern, form in PRESET_VERSIONS:
        presets, preset_count = setting_pattern.subn(rf'\1="{form.format(version)}"', presets)
        if preset_count == 0:
            raise ValueError(f"app/export_presets.cfg sets no {system} version")
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
