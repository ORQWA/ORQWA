"""A release is built only when its tag, the Cargo workspace and the macOS bundle agree; setting a
version changes both places and nothing else."""

import pathlib
import tempfile
import unittest

import version

CARGO = (
    '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "{}"\nedition = "2024"\n'
    'rust-version = "1.99"\n\n[workspace.dependencies]\nclap = {{ version = "4.6.7" }}\n\n'
    '[workspace.dependencies.serde]\nversion = "1.0.229"\n'
)
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

    def read(self):
        return (
            (self.root / "core" / "Cargo.toml").read_text(),
            (self.root / "app" / "export_presets.cfg").read_text(),
        )

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

    def test_setting_a_version_changes_only_the_workspace_and_bundle_versions(self):
        self.write("0.2.2", "0.2.2", "0.2.2")
        version.set_version(self.root, "0.3.0")
        self.assertEqual(
            self.read(), (CARGO.format("0.3.0"), PRESETS.format("0.3.0", "0.3.0"))
        )
        self.assertEqual(version.problems(self.root, "v0.3.0"), [])

    def test_setting_a_version_the_bundle_cannot_carry_changes_nothing(self):
        self.write("0.2.2", "0.2.2", "0.2.2")
        before = self.read()
        for new in ("", "0.3", "v0.3.0", "0.3.0-rc.1", "0.3.0.1", "0.3.0 "):
            with self.subTest(version=new):
                with self.assertRaises(ValueError):
                    version.set_version(self.root, new)
                self.assertEqual(self.read(), before)

    def test_setting_without_a_bundle_version_leaves_cargo_unchanged(self):
        self.write("0.2.2", "0.2.2", "0.2.2")
        (self.root / "app" / "export_presets.cfg").write_text('[preset.0]\n\nname="macOS"\n')
        before = self.read()
        with self.assertRaises(ValueError):
            version.set_version(self.root, "0.3.0")
        self.assertEqual(self.read(), before)


if __name__ == "__main__":
    unittest.main()
