"""Integration scenarios use disposable Git remotes; no GitHub writes or credentials."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from check import WIKI_URL, check
from publish import GitHub, SYNC_BRANCH, publish
import publish as publisher


def git(path, *args):
    return subprocess.check_output(["git", "-C", str(path), *args], text=True, stderr=subprocess.PIPE).strip()


class FakeAPI:
    def __init__(self):
        self.open = False
        self.created = 0

    def reconcile_pr(self, needed, body):
        if needed and not self.open:
            self.created += 1
        self.open = needed


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.environment = patch.dict(os.environ, {"WIKI_TEST_REMOTES": "1", "GH_TOKEN": ""})
        self.environment.start()
        self.wiki_remote, self.wiki = self.repository("wiki", "master")
        (self.wiki / "Home.md").write_text(f"# Home\n\n[Guide]({WIKI_URL}/Guide)\n")
        (self.wiki / "_Sidebar.md").write_text(f"[Guide]({WIKI_URL}/Guide)\n")
        (self.wiki / "Guide.md").write_text("# Guide\n\nOriginal usage.\n")
        self.base = self.commit(self.wiki, "Initial wiki", "master")
        self.app_remote, self.app = self.repository("application", "main")
        (self.app / ".gitmodules").write_text(f'[submodule "wiki"]\n path = wiki\n url = {self.wiki_remote}\n')
        self.pin(self.base)
        self.api = FakeAPI()

    def tearDown(self):
        self.environment.stop()
        self.directory.cleanup()

    def repository(self, name, branch):
        remote = self.root / f"{name}.git"
        checkout = self.root / name
        git(self.root, "init", "--bare", f"--initial-branch={branch}", str(remote))
        git(self.root, "clone", str(remote), str(checkout))
        git(checkout, "config", "user.name", "Test")
        git(checkout, "config", "user.email", "test@example.org")
        return remote, checkout

    def commit(self, checkout, message, branch):
        git(checkout, "add", "--all")
        git(checkout, "commit", "-m", message)
        git(checkout, "push", "origin", f"HEAD:refs/heads/{branch}")
        return git(checkout, "rev-parse", "HEAD")

    def pin(self, revision):
        git(self.app, "add", ".gitmodules")
        git(self.app, "update-index", "--add", "--cacheinfo", f"160000,{revision},wiki")
        git(self.app, "commit", "-m", "Application with usage update")
        git(self.app, "push", "origin", "HEAD:main")

    def prepare(self, content="Updated usage.\n"):
        git(self.wiki, "checkout", "-B", "docs/change", self.base)
        (self.wiki / "Guide.md").write_text("# Guide\n\n" + content)
        return self.commit(self.wiki, "Prepared usage", "docs/change")

    def published(self):
        return git(self.wiki_remote, "rev-parse", "master")

    def test_preparation_stays_unpublished_and_fast_forward_needs_no_pr(self):
        target = self.prepare()
        self.assertEqual(self.published(), self.base)
        self.pin(target)
        self.assertEqual(publish(str(self.app_remote), self.api), target)
        self.assertEqual(self.published(), target)
        self.assertFalse(self.api.open)
        publish(str(self.app_remote), self.api)
        self.assertEqual(self.api.created, 0)

    def divergent(self):
        target = self.prepare()
        git(self.wiki, "checkout", "master")
        (self.wiki / "Home.md").write_text(f"# Home\n\nExisting published edit.\n[Guide]({WIKI_URL}/Guide)\n")
        self.commit(self.wiki, "Existing wiki edit", "master")
        self.pin(target)
        return target

    def test_merge_pr_retry_and_pointer_merge_do_not_loop(self):
        self.divergent()
        published = publish(str(self.app_remote), self.api)
        self.assertEqual(self.published(), published)
        self.assertEqual(self.api.created, 1)
        sync = git(self.app_remote, "rev-parse", SYNC_BRANCH)
        changed = git(self.app_remote, "diff-tree", "--no-commit-id", "--name-only", "-r", sync)
        self.assertEqual(changed, "wiki")
        publish(str(self.app_remote), self.api)
        self.assertEqual(git(self.app_remote, "rev-parse", SYNC_BRANCH), sync)
        self.assertEqual(self.api.created, 1)
        self.pin(published)
        publish(str(self.app_remote), self.api)
        self.assertEqual(self.published(), published)
        self.assertFalse(self.api.open)

    def test_conflict_leaves_published_branch_unchanged(self):
        target = self.prepare("Prepared conflicting usage.\n")
        git(self.wiki, "checkout", "master")
        (self.wiki / "Guide.md").write_text("# Guide\n\nPublished conflicting usage.\n")
        before = self.commit(self.wiki, "Published conflict", "master")
        self.pin(target)
        with self.assertRaisesRegex(RuntimeError, "merge conflict"):
            publish(str(self.app_remote), self.api)
        self.assertEqual(self.published(), before)
        self.assertFalse(self.api.open)

    def test_validation_failure_does_not_publish(self):
        target = self.prepare(f"[Missing page]({WIKI_URL}/Missing)\n")
        self.pin(target)
        with self.assertRaisesRegex(RuntimeError, "validation failed"):
            publish(str(self.app_remote), self.api)
        self.assertEqual(self.published(), self.base)

    def test_consecutive_application_merges_publish_latest_history(self):
        first = self.prepare()
        self.pin(first)
        publish(str(self.app_remote), self.api)
        git(self.wiki, "checkout", "docs/change")
        (self.wiki / "Guide.md").write_text("# Guide\n\nSecond application change.\n")
        second = self.commit(self.wiki, "Second change", "docs/change")
        self.pin(second)
        self.assertEqual(publish(str(self.app_remote), self.api), second)
        self.assertEqual(self.published(), second)
        self.assertEqual(self.api.created, 0)

    def test_recursive_checkout_fetches_prepared_revision(self):
        target = self.prepare()
        self.pin(target)
        clone = self.root / "recursive"
        git(self.root, "-c", "protocol.file.allow=always", "clone", "--recurse-submodules", str(self.app_remote), str(clone))
        self.assertEqual(git(clone / "wiki", "rev-parse", "HEAD"), target)
        self.assertEqual(check(clone), [])

    def test_retry_after_pr_api_failure_reuses_published_commit(self):
        self.divergent()
        with patch.object(self.api, "reconcile_pr", side_effect=RuntimeError("Access denied")):
            with self.assertRaisesRegex(RuntimeError, "Access denied"):
                publish(str(self.app_remote), self.api)
        before = self.published()
        publish(str(self.app_remote), self.api)
        self.assertEqual(self.published(), before)
        self.assertEqual(self.api.created, 1)

    def test_main_advancing_during_publication_retries_with_latest_pointer(self):
        first = self.prepare()
        self.pin(first)
        (self.wiki / "Guide.md").write_text("# Guide\n\nLatest main usage.\n")
        second = self.commit(self.wiki, "Next application change", "docs/change")
        original = publisher.publish_history
        advanced = False

        def advance(wiki, target, branch):
            nonlocal advanced
            result = original(wiki, target, branch)
            if not advanced:
                advanced = True
                self.pin(second)
            return result

        with patch.object(publisher, "publish_history", side_effect=advance):
            self.assertEqual(publish(str(self.app_remote), self.api), second)
        self.assertEqual(self.published(), second)
        self.assertEqual(self.api.created, 0)

    def branches(self):
        return git(self.wiki_remote, "for-each-ref", "--format=%(refname:short)", "refs/heads").split()

    def test_published_preparation_branches_are_deleted(self):
        target = self.prepare()
        git(self.wiki, "checkout", "-B", "docs/pending", self.base)
        (self.wiki / "Guide.md").write_text("# Guide\n\nNot merged yet.\n")
        self.commit(self.wiki, "Pending usage", "docs/pending")
        git(self.wiki, "push", "origin", f"{target}:refs/heads/personal")
        self.pin(target)
        publish(str(self.app_remote), self.api)
        self.assertEqual(sorted(self.branches()), ["docs/pending", "master", "personal"])
        clone = self.root / "recursive"
        git(self.root, "-c", "protocol.file.allow=always", "clone", "--recurse-submodules", str(self.app_remote), str(clone))
        self.assertEqual(git(clone / "wiki", "rev-parse", "HEAD"), target)

    def test_branch_advanced_during_publication_is_kept(self):
        target = self.prepare()
        self.pin(target)
        original = publisher.prune_branches

        def advance(wiki, published):
            (self.wiki / "Guide.md").write_text("# Guide\n\nFollow-up usage.\n")
            self.commit(self.wiki, "Follow-up", "docs/change")
            original(wiki, published)

        with patch.object(publisher, "prune_branches", side_effect=advance):
            publish(str(self.app_remote), self.api)
        self.assertIn("docs/change", self.branches())
        self.assertEqual(self.published(), target)

    def test_rejected_wiki_push_preserves_remote_edits(self):
        target = self.prepare()
        self.pin(target)
        original = publisher.git

        def reject(path, *args):
            if Path(path).name == "wiki" and args[:2] == ("push", "origin"):
                raise subprocess.CalledProcessError(1, ["git", "push"])
            return original(path, *args)

        with patch.object(publisher, "git", side_effect=reject):
            with self.assertRaises(subprocess.CalledProcessError):
                publish(str(self.app_remote), self.api)
        self.assertEqual(self.published(), self.base)
        self.assertFalse(self.api.open)


class PullRequestTests(unittest.TestCase):
    def test_create_update_and_close_existing_pr(self):
        api = GitHub("bossm8/Torqa", "unused-test-token")
        with patch.object(api, "request", return_value=[]) as request:
            api.reconcile_pr(True, "Review")
            self.assertEqual(request.call_args.args[:2], ("POST", "pulls"))
        with patch.object(api, "request", return_value=[{"number": 42}]) as request:
            api.reconcile_pr(True, "Updated review")
            self.assertEqual(request.call_args.args[:2], ("PATCH", "pulls/42"))
            self.assertEqual(request.call_args.args[2]["body"], "Updated review")
        with patch.object(api, "request", return_value=[{"number": 42}]) as request:
            api.reconcile_pr(False, "Already synchronized")
            self.assertEqual(request.call_args.args[2], {"state": "closed"})
