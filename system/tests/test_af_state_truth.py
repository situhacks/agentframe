"""Group 1: the tracker must not assert something the world contradicts.

Covers BB-2026-08-17-01 (a mis-addressed --file builds a doubled tree that every check
reports as clean) and BB-2026-08-07-01 / BB-2026-08-25-02 (publishing off-button, and
reaching a manifest ingredient at all).
"""
import contextlib
import io
import os
import tempfile
import types
import unittest
from unittest.mock import patch

from system import af


class TempProject(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.projects = os.path.join(self._tmp.name, "workspace", "projects")
        p = patch.object(af, "PROJECTS", self.projects)
        p.start()
        self.addCleanup(p.stop)
        self.addCleanup(self._tmp.cleanup)
        self.cdir = os.path.join(self.projects, "campaign")

    def write_project(self, rows):
        os.makedirs(self.cdir, exist_ok=True)
        af.write(os.path.join(self.cdir, "project.md"),
                 "---\nname: campaign\nslug: campaign\ndomain: marketing\nstatus: active\n"
                 "last_activity: 2026-08-27T10:00:00-07:00\n"
                 "deliverables:\n" + rows + "---\n")

    def fm(self):
        return af.split_fm(af.read(os.path.join(self.cdir, "project.md")), "project.md")[0]


class RepoRootPathGuard(TempProject):
    """A --file value that re-enters the project from the repo root."""

    def setUp(self):
        super().setUp()
        self.write_project("  brief:\n    status: not_started\n    file: brief/brief-v1.md\n"
                           "    last_updated: 2026-08-27\n")

    def test_draft_refuses_repo_root_relative(self):
        args = types.SimpleNamespace(
            project="campaign", deliverable="brief", artifact=None,
            file="workspace/projects/campaign/brief/brief-v1.md")
        err = io.StringIO()
        with contextlib.redirect_stderr(err), self.assertRaises(SystemExit):
            af.cmd_draft(args)
        self.assertIn("repo-root-relative", err.getvalue())

    def test_adopt_refuses_repo_root_relative(self):
        args = types.SimpleNamespace(
            project="campaign", deliverable="brief", workstream=None,
            file="workspace/projects/campaign/brief/brief-v1.md")
        err = io.StringIO()
        with contextlib.redirect_stderr(err), self.assertRaises(SystemExit):
            af.cmd_adopt(args)
        self.assertIn("PROJECT-relative", err.getvalue())

    def test_ordinary_project_relative_path_still_accepted(self):
        rel, target = af.safe_project_rel(self.cdir, "brief/brief-v1.md")
        self.assertEqual(rel, "brief/brief-v1.md")
        self.assertTrue(target.endswith(os.path.join("campaign", "brief", "brief-v1.md")))

    def test_a_folder_literally_named_workspace_is_still_refused(self):
        # Deliberate: the guard is a blunt prefix rule, and a deliverable folder called
        # "workspace" is far less likely than the mistake it prevents.
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            af.safe_project_rel(self.cdir, "workspace/notes-v1.md")


class PublishDrift(TempProject):
    """A row recording published-and-not-ready at the same time."""

    def test_url_below_ready_is_an_issue(self):
        self.write_project("  essay:\n    status: drafting\n    file: essay/essay-v1.md\n"
                           "    last_updated: 2026-08-27\n"
                           '    substack_url: "https://example.com/p/x"\n')
        issues = af.state_truth_issues(self.cdir, self.fm())
        self.assertEqual(len(issues), 1, issues)
        self.assertIn("substack_url", issues[0])
        self.assertIn("published and not", issues[0])

    def test_draft_id_below_ready_is_an_issue(self):
        self.write_project("  essay:\n    status: drafting\n    file: essay/essay-v1.md\n"
                           "    last_updated: 2026-08-27\n    substack_draft: 210153401\n")
        self.assertEqual(len(af.state_truth_issues(self.cdir, self.fm())), 1)

    def test_published_row_with_url_is_clean(self):
        self.write_project("  essay:\n    status: published\n    file: essay/essay-v1.md\n"
                           "    last_updated: 2026-08-27\n"
                           '    substack_url: "https://example.com/p/x"\n')
        self.assertEqual(af.state_truth_issues(self.cdir, self.fm()), [])

    def test_ready_row_with_url_is_clean(self):
        self.write_project("  essay:\n    status: ready\n    file: essay/essay-v1.md\n"
                           "    last_updated: 2026-08-27\n"
                           '    posted_url: "https://example.com/p/x"\n')
        self.assertEqual(af.state_truth_issues(self.cdir, self.fm()), [])

    def test_null_url_does_not_fire(self):
        self.write_project("  essay:\n    status: drafting\n    file: essay/essay-v1.md\n"
                           "    last_updated: 2026-08-27\n    substack_url: null\n")
        self.assertEqual(af.state_truth_issues(self.cdir, self.fm()), [])

    def test_matches_by_shape_not_by_vendor(self):
        # A channel nobody has integrated yet is still covered.
        self.write_project("  essay:\n    status: drafting\n    file: essay/essay-v1.md\n"
                           "    last_updated: 2026-08-27\n"
                           '    mastodon_url: "https://example.social/@x/1"\n')
        self.assertEqual(len(af.state_truth_issues(self.cdir, self.fm())), 1)


class EmptyHeadNotes(TempProject):
    """The shape a mis-addressed draft leaves behind."""

    def _head(self, body):
        self.write_project("  brief:\n    status: drafting\n    file: brief/brief-v1.md\n"
                           "    last_updated: 2026-08-27\n")
        os.makedirs(os.path.join(self.cdir, "brief"), exist_ok=True)
        af.write(os.path.join(self.cdir, "brief", "brief-v1.md"),
                 "---\nstatus: drafting\nlast_updated: 2026-08-27\n---\n" + body)

    def test_empty_body_is_a_note(self):
        self._head("\n")
        notes = af.empty_head_notes(self.cdir)
        self.assertEqual(len(notes), 1, notes)
        self.assertIn("empty head", notes[0])

    def test_written_body_is_quiet(self):
        self._head("\n# Brief\n\nReal content.\n")
        self.assertEqual(af.empty_head_notes(self.cdir), [])

    def test_note_never_becomes_an_issue(self):
        self._head("\n")
        self.assertEqual(af.state_truth_issues(self.cdir, self.fm()), [])


class ReadyArtifact(unittest.TestCase):
    """BB-2026-08-25-02: reaching a nested manifest ingredient."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.projects = os.path.join(self._tmp.name, "workspace", "projects")
        p = patch.object(af, "PROJECTS", self.projects)
        p.start()
        self.addCleanup(p.stop)
        self.addCleanup(self._tmp.cleanup)
        self.cdir = os.path.join(self.projects, "campaign")
        self.post_dir = os.path.join(self.cdir, "posts", "post-1.5-vitamix")
        os.makedirs(self.post_dir, exist_ok=True)
        af.write(os.path.join(self.cdir, "project.md"),
                 "---\nname: campaign\nslug: campaign\ndomain: marketing\nstatus: active\n"
                 "last_activity: 2026-08-27T10:00:00-07:00\n"
                 "post_manifest:\n  ingredients: [body-copy]\n"
                 "deliverables:\n  post-1-5:\n    status: drafting\n"
                 "    file: posts/post-1.5-vitamix/post-FINAL.md\n    last_updated: 2026-08-27\n---\n")
        af.write(os.path.join(self.post_dir, "post-FINAL.md"),
                 "---\nstatus: drafting\nlast_updated: 2026-08-27\n---\n\n# Post\n")
        af.write(os.path.join(self.post_dir, "body-copy-v2.md"),
                 "---\nstatus: drafting\nlast_updated: 2026-08-27\n---\n\nBody text.\n")

    def test_artifact_flag_readies_the_ingredient_head(self):
        args = types.SimpleNamespace(project="campaign", deliverable="post-1-5",
                                     artifact="body-copy", allow_missing_exports=False)
        with contextlib.redirect_stdout(io.StringIO()):
            af.cmd_ready(args)
        head = af.split_fm(af.read(os.path.join(self.post_dir, "body-copy-v2.md")))[0]
        self.assertEqual(af.get_scalar(head, "status"), "ready")

    def test_readying_the_last_ingredient_promotes_the_parent(self):
        args = types.SimpleNamespace(project="campaign", deliverable="post-1-5",
                                     artifact="body-copy", allow_missing_exports=False)
        with contextlib.redirect_stdout(io.StringIO()):
            af.cmd_ready(args)
        cfm = af.split_fm(af.read(os.path.join(self.cdir, "project.md")), "project.md")[0]
        self.assertEqual(af.row_get(cfm, "post-1-5", "status"), "ready")

    def test_blocked_parent_names_the_exact_next_command(self):
        args = types.SimpleNamespace(project="campaign", deliverable="post-1-5",
                                     artifact=None, allow_missing_exports=False)
        err = io.StringIO()
        with contextlib.redirect_stderr(err), self.assertRaises(SystemExit):
            af.cmd_ready(args)
        msg = err.getvalue()
        self.assertIn("post-1-5 --artifact body-copy", msg)  # the tracker slug, not the folder name
        self.assertIn("Still drafting", msg)


if __name__ == "__main__":
    unittest.main()
