"""A release is built only when its tag, the Cargo workspace and the macOS and Windows presets
agree; setting a version changes all of them and nothing else."""

import pathlib
import tempfile
import unittest

import version

CARGO = (
    '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "{}"\nedition = "2024"\n'
    'rust-version = "1.99"\n\n[workspace.dependencies]\nclap = {{ version = "4.6.7" }}\n\n'
    '[workspace.dependencies.serde]\nversion = "1.0.229"\n'
)
MACOS = (
    '[preset.0]\n\nname="macOS"\n\n[preset.0.options]\n\n'
    'application/short_version="{short}"\napplication/version="{bundle}"\n'
    'application/min_macos_version_arm64="11.00"\n'
)
WINDOWS = (
    '\n[preset.1]\n\nname="Windows Desktop"\n\n[preset.1.options]\n\n'
    'application/modify_resources=true\napplication/file_version="{file}"\n'
    'application/product_version="{product}"\napplication/product_name="Torqa"\n'
)
SETTINGS = {
    "short": "application/short_version",
    "bundle": "application/version",
    "file": "application/file_version",
    "product": "application/product_version",
}


def versions(release, **changed):
    """Each preset setting naming `release` as its system writes it, but for those `changed`."""
    windows = f"{release}.0"
    return {"short": release, "bundle": release, "file": windows, "product": windows} | changed


def presets(release, **changed):
    named = versions(release, **changed)
    return MACOS.format(**named) + WINDOWS.format(**named)


class VersionTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = pathlib.Path(temp.name)
        (self.root / "core").mkdir()
        (self.root / "app").mkdir()

    def write(self, cargo, preset_text):
        (self.root / "core" / "Cargo.toml").write_text(CARGO.format(cargo))
        (self.root / "app" / "export_presets.cfg").write_text(preset_text)

    def read(self):
        return (
            (self.root / "core" / "Cargo.toml").read_text(),
            (self.root / "app" / "export_presets.cfg").read_text(),
        )

    def test_matching_tag_and_versions_pass(self):
        self.write("0.2.1", presets("0.2.1"))
        self.assertEqual(version.problems(self.root), [])
        self.assertEqual(version.problems(self.root, "v0.2.1"), [])

    def test_tag_naming_another_version_fails(self):
        self.write("0.1.0", presets("0.1.0"))
        for tag in ("v0.2.0", "0.1.0", "release-0.1.0", "v0.1.0-rc.1"):
            with self.subTest(tag=tag):
                found = version.problems(self.root, tag)
                self.assertEqual(len(found), 1)
                self.assertIn(tag, found[0])

    def test_a_preset_version_differing_from_cargo_fails_without_a_tag(self):
        for setting, value in (
            ("short", "0.1.0"),
            ("bundle", "0.1.0"),
            ("file", "0.1.0.0"),
            ("product", "0.1.0.0"),
            # Windows needs the fourth number.
            ("file", "0.2.1"),
            ("product", "0.2.1.1"),
        ):
            with self.subTest(setting=setting, value=value):
                self.write("0.2.1", presets("0.2.1", **{setting: value}))
                found = version.problems(self.root)
                self.assertEqual(len(found), 1)
                self.assertIn(f"{SETTINGS[setting]}={value},", found[0])

    def test_presets_without_a_systems_version_fail(self):
        for system, preset_text in (("macOS", WINDOWS), ("Windows", MACOS)):
            with self.subTest(missing=system):
                self.write("0.2.1", preset_text.format(**versions("0.2.1")))
                found = version.problems(self.root)
                self.assertEqual(len(found), 1)
                self.assertIn(system, found[0])

    def test_setting_a_version_changes_only_the_workspace_and_preset_versions(self):
        self.write("0.2.2", presets("0.2.2"))
        version.set_version(self.root, "0.3.0")
        self.assertEqual(self.read(), (CARGO.format("0.3.0"), presets("0.3.0")))
        self.assertIn('application/file_version="0.3.0.0"', self.read()[1])
        self.assertEqual(version.problems(self.root, "v0.3.0"), [])

    def test_setting_a_version_the_presets_cannot_carry_changes_nothing(self):
        self.write("0.2.2", presets("0.2.2"))
        before = self.read()
        for new in ("", "0.3", "v0.3.0", "0.3.0-rc.1", "0.3.0.1", "0.3.0 "):
            with self.subTest(version=new):
                with self.assertRaises(ValueError):
                    version.set_version(self.root, new)
                self.assertEqual(self.read(), before)

    def test_setting_without_a_systems_version_changes_nothing(self):
        for system, preset_text in (("macOS", WINDOWS), ("Windows", MACOS)):
            with self.subTest(missing=system):
                self.write("0.2.2", preset_text.format(**versions("0.2.2")))
                before = self.read()
                with self.assertRaises(ValueError):
                    version.set_version(self.root, "0.3.0")
                self.assertEqual(self.read(), before)


if __name__ == "__main__":
    unittest.main()
