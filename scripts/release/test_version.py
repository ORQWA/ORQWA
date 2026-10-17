"""A release is built only when its tag, the Cargo workspace and the macOS bundle agree."""

import pathlib
import tempfile
import unittest

import version

CARGO = '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "{}"\nedition = "2024"\n'
PRESETS = (
    '[preset.0]\n\nname="macOS"\n\n[preset.0.options]\n\n'
    'application/short_version="{}"\napplication/version="{}"\n'
    'application/min_macos_version_arm64="11.00"\n'
)


class VersionTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = pathlib.Path(temp.name)
        (self.root / "core").mkdir()
        (self.root / "app").mkdir()

    def write(self, cargo, short_version, bundle_version):
        (self.root / "core" / "Cargo.toml").write_text(CARGO.format(cargo))
        presets = PRESETS.format(short_version, bundle_version)
        (self.root / "app" / "export_presets.cfg").write_text(presets)

    def test_matching_tag_and_versions_pass(self):
        self.write("0.2.1", "0.2.1", "0.2.1")
        self.assertEqual(version.problems(self.root), [])
        self.assertEqual(version.problems(self.root, "v0.2.1"), [])

    def test_tag_naming_another_version_fails(self):
        self.write("0.1.0", "0.1.0", "0.1.0")
        for tag in ("v0.2.0", "0.1.0", "release-0.1.0", "v0.1.0-rc.1"):
            with self.subTest(tag=tag):
                found = version.problems(self.root, tag)
                self.assertEqual(len(found), 1)
                self.assertIn(tag, found[0])

    def test_bundle_version_differing_from_cargo_fails_without_a_tag(self):
        for short_version, bundle_version in (("0.1.0", "0.2.1"), ("0.2.1", "0.1.0")):
            with self.subTest(short=short_version, bundle=bundle_version):
                self.write("0.2.1", short_version, bundle_version)
                found = version.problems(self.root)
                self.assertEqual(len(found), 1)
                self.assertIn("0.1.0", found[0])

    def test_presets_without_a_version_fail(self):
        self.write("0.2.1", "0.2.1", "0.2.1")
        (self.root / "app" / "export_presets.cfg").write_text('[preset.0]\n\nname="macOS"\n')
        self.assertEqual(len(version.problems(self.root)), 1)


if __name__ == "__main__":
    unittest.main()
