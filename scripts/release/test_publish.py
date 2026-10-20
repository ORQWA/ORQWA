"""Exercise release publication without contacting GitHub or publishing a tag."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("publish.sh").resolve()
ASSETS = (
    "Torqa-macos-arm64.zip",
    "orqwa-cli-macos-arm64.tar.gz",
    "Torqa-windows-x86_64.zip",
    "Torqa-windows-x86_64.msi",
    "orqwa-cli-windows-x86_64.zip",
    "Torqa-linux-x86_64.tar.gz",
    "orqwa-cli-linux-x86_64.tar.gz",
    "Torqa-linux-arm64.tar.gz",
    "orqwa-cli-linux-arm64.tar.gz",
    "Torqa-android-arm64.apk",
)


class PublishTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "dist").mkdir()
        for name in ASSETS:
            (self.root / "dist" / name).write_bytes(b"archive")
        self.log = self.root / "calls.jsonl"
        self.env = {
            **os.environ,
            "PATH": f"{self.root}:{os.environ['PATH']}",
            "RELEASE_TAG": "releases/v0.1.0",
            "GH_REPO": "example/Torqa",
            "GH_TEST_LOG": str(self.log),
            "GH_TEST_STATE": "missing",
            "GH_TEST_FAIL": "",
        }
        gh = self.root / "gh"
        gh.write_text('''#!/usr/bin/env python3
import json
import os
import sys

args = sys.argv[1:]
with open(os.environ["GH_TEST_LOG"], "a") as log:
    log.write(json.dumps(args) + "\\n")
if args[1] == os.environ["GH_TEST_FAIL"]:
    sys.exit(1)
if args[1] == "view":
    state = os.environ["GH_TEST_STATE"]
    if state == "missing":
        sys.exit(1)
    print("true" if state == "draft" else "false")
''')
        gh.chmod(0o755)

    def publish(self):
        return subprocess.run(
            ["sh", str(SCRIPT)], cwd=self.root, env=self.env,
            capture_output=True, text=True, check=False,
        )

    def calls(self):
        if not self.log.exists():
            return []
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def test_new_release_uploads_all_archives_before_publishing(self):
        result = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        self.assertEqual([call[1] for call in calls], ["view", "create", "upload", "edit"])
        self.assertIn("--verify-tag", calls[1])
        self.assertIn("--generate-notes", calls[1])
        self.assertIn("--draft", calls[1])
        self.assertEqual(calls[2][3:-1], [f"dist/{name}" for name in ASSETS])
        self.assertEqual(calls[2][-1], "--clobber")
        self.assertEqual(calls[3][-1], "--draft=false")
        for call in calls:
            self.assertEqual(call[2], self.env["RELEASE_TAG"])

    def test_each_missing_or_empty_archive_blocks_all_github_changes(self):
        for name in ASSETS:
            asset = self.root / "dist" / name
            for empty in (False, True):
                with self.subTest(name=name, empty=empty):
                    asset.write_bytes(b"") if empty else asset.unlink()
                    result = self.publish()
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn(name, result.stderr)
                    self.assertEqual(self.calls(), [])
                    asset.write_bytes(b"archive")

    def test_retry_completes_existing_draft(self):
        self.env["GH_TEST_STATE"] = "draft"
        self.assertEqual(self.publish().returncode, 0)
        self.assertEqual([call[1] for call in self.calls()], ["view", "upload", "edit"])

    def test_published_release_is_preserved_on_retry(self):
        self.env["GH_TEST_STATE"] = "published"
        self.assertEqual(self.publish().returncode, 0)
        self.assertEqual([call[1] for call in self.calls()], ["view"])

    def test_failed_upload_leaves_release_unpublished(self):
        self.env["GH_TEST_FAIL"] = "upload"
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertEqual([call[1] for call in self.calls()], ["view", "create", "upload"])

    def test_failed_creation_does_not_upload_or_publish(self):
        self.env["GH_TEST_FAIL"] = "create"
        self.assertNotEqual(self.publish().returncode, 0)
        self.assertEqual([call[1] for call in self.calls()], ["view", "create"])

    def test_missing_tag_or_repository_blocks_publication(self):
        for key in ("RELEASE_TAG", "GH_REPO"):
            with self.subTest(key=key):
                value = self.env.pop(key)
                self.assertNotEqual(self.publish().returncode, 0)
                self.assertEqual(self.calls(), [])
                self.env[key] = value


if __name__ == "__main__":
    unittest.main()
