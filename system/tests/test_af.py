import contextlib
import datetime
import io
import os
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

from system import af


def days_ago(n):
    return (datetime.date.today() - datetime.timedelta(days=n)).isoformat()


def make_project(root, slug, *, status="active", created_at, last_activity,
                 last_consolidated=None, decision_log_lines=0):
    cdir = os.path.join(root, "workspace", "projects", slug)
    os.makedirs(os.path.join(cdir, "knowledge"), exist_ok=True)
    fm = [
        f"name: {slug}",
        f"slug: {slug}",
        "schema_version: 2026-04-23",
        f"created_at: {created_at}",
        "domain: marketing",
        f"status: {status}",
        "current_phase: active",
        "flow: open-flow",
        f"last_activity: {last_activity}T10:00:00+00:00",
        f"last_consolidated: {last_consolidated or 'null'}",
    ]
    af.write(os.path.join(cdir, "project.md"), "---\n" + "\n".join(fm) + "\n---\n")
    if decision_log_lines:
        af.write(os.path.join(cdir, "knowledge", "decision-log.md"),
                 "\n".join(f"- decision {i}" for i in range(decision_log_lines)))
    return cdir


class NewProjectDefaultTests(unittest.TestCase):
    def test_cli_defaults_to_neutral_project_management_open_flow(self):
        with patch.object(af, "cmd_new_project") as command, \
             patch.object(af, "check_mode_gate"), \
             patch.object(sys, "argv", ["af", "new-project", "anything"]):
            af.main()

        args = command.call_args.args[0]
        self.assertEqual(args.domain, "project-mgmt")
        self.assertEqual(args.flow, "open-flow")


class DreamNoteTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = self._tmp.name
        self._patch = patch.object(af, "ROOT", self.root)
        self._patch.start()
        self.addCleanup(self._patch.stop)
        self.addCleanup(self._tmp.cleanup)

    def test_fresh_active_project_gets_no_note(self):
        cdir = make_project(self.root, "fresh", created_at=days_ago(3), last_activity=days_ago(1))
        self.assertIsNone(af.dream_note(cdir))

    def test_old_consolidation_on_active_project_fires(self):
        cdir = make_project(self.root, "longrun", created_at=days_ago(90),
                            last_activity=days_ago(2), last_consolidated=days_ago(45))
        note = af.dream_note(cdir)
        self.assertIsNotNone(note)
        self.assertIn("45d since last consolidation", note)

    def test_old_consolidation_on_idle_project_stays_quiet(self):
        cdir = make_project(self.root, "idle", created_at=days_ago(90),
                            last_activity=days_ago(30), last_consolidated=days_ago(45))
        self.assertIsNone(af.dream_note(cdir))

    def test_bloated_log_fires_regardless_of_age(self):
        cdir = make_project(self.root, "bloated", created_at=days_ago(3),
                            last_activity=days_ago(1), decision_log_lines=350)
        note = af.dream_note(cdir)
        self.assertIsNotNone(note)
        self.assertIn("knowledge/decision-log.md 350 lines (cap 300)", note)

    def test_non_active_project_never_fires(self):
        cdir = make_project(self.root, "done", status="complete", created_at=days_ago(90),
                            last_activity=days_ago(1), decision_log_lines=350)
        self.assertIsNone(af.dream_note(cdir))

    def test_null_last_consolidated_falls_back_to_created_at(self):
        cdir = make_project(self.root, "neverdreamed", created_at=days_ago(60),
                            last_activity=days_ago(1))
        note = af.dream_note(cdir)
        self.assertIsNotNone(note)
        self.assertIn("60d since last consolidation", note)

    def test_bloated_project_md_fires(self):
        cdir = make_project(self.root, "trackerheavy", created_at=days_ago(3),
                            last_activity=days_ago(1))
        path = os.path.join(cdir, "project.md")
        af.write(path, af.read(path) + "\n".join(f"- plan line {i}" for i in range(300)))
        note = af.dream_note(cdir)
        self.assertIsNotNone(note)
        self.assertIn("project.md", note)


REPO_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

PROJECT_FM = """name: arch
slug: arch
domain: marketing
status: active
posts_published: {declared}
deliverables:
  post-1:
    status: delivered
    file: posts/post-1/post-FINAL.md
    last_updated: 2026-06-01
  post-2:
    status: drafting
    file: posts/post-2/post-FINAL.md
    last_updated: 2026-07-01"""

ARCHIVE_FM = """---
deliverables:
  post-0:
    status: delivered
    file: posts/post-0/post-FINAL.md
    last_updated: 2026-05-01
---

> Rows archived from project.md by knowledge_consolidation passes.
"""


class ArchivedRowCounterTests(unittest.TestCase):
    """Marketing rules count delivered posts across tracker + dream-pass archive."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = self._tmp.name
        self._patch = patch.object(af, "ROOT", self.root)
        self._patch.start()
        self.addCleanup(self._patch.stop)
        self.addCleanup(self._tmp.cleanup)
        self.rules = af.load_rules(os.path.join(REPO_ROOT, "library", "domains", "marketing"))
        self.cdir = os.path.join(self.root, "workspace", "projects", "arch")
        os.makedirs(os.path.join(self.cdir, "knowledge", "_archive"))

    def test_counter_reconciles_across_tracker_and_archive(self):
        af.write(os.path.join(self.cdir, "knowledge", "_archive", "deliverables-archive.md"), ARCHIVE_FM)
        issues = self.rules.check(af.make_ctx(), self.cdir, PROJECT_FM.format(declared=2))
        self.assertEqual(issues, [])

    def test_counter_mismatch_still_caught_with_archive(self):
        af.write(os.path.join(self.cdir, "knowledge", "_archive", "deliverables-archive.md"), ARCHIVE_FM)
        issues = self.rules.check(af.make_ctx(), self.cdir, PROJECT_FM.format(declared=1))
        self.assertEqual(len(issues), 1)
        self.assertIn("posts_published=1 but 2 post rows are delivered", issues[0])

    def test_no_archive_file_counts_tracker_only(self):
        issues = self.rules.check(af.make_ctx(), self.cdir, PROJECT_FM.format(declared=1))
        self.assertEqual(issues, [])


LOCK_PROJECT_FM = """name: carousel-proj
slug: carousel-proj
domain: marketing
status: active
last_activity: 2026-07-01T10:00:00+00:00
deliverables:
  {slug}:
    status: drafting
    file: {file}
    last_updated: 2026-07-01"""


class LockExportGateTests(unittest.TestCase):
    """af lock refuses exportable deliverables whose exports[] are empty or dangling."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = self._tmp.name
        projects = os.path.join(self.root, "workspace", "projects")
        self._patch = patch.object(af, "PROJECTS", projects)
        self._patch.start()
        self.addCleanup(self._patch.stop)
        self.addCleanup(self._tmp.cleanup)
        self.cdir = os.path.join(projects, "carousel-proj")
        os.makedirs(self.cdir)

    def make_deliverable(self, slug, fname, extra_fm=""):
        rel = f"{slug}/{fname}"
        af.write(os.path.join(self.cdir, "project.md"),
                 "---\n" + LOCK_PROJECT_FM.format(slug=slug, file=rel) + "\n---\n")
        os.makedirs(os.path.join(self.cdir, slug), exist_ok=True)
        dfm = "status: drafting\nlast_updated: 2026-07-01"
        if extra_fm:
            dfm += "\n" + extra_fm
        af.write(os.path.join(self.cdir, rel), f"---\n{dfm}\n---\n\nbody\n")
        return rel

    def run_lock(self, slug, allow_missing_exports=False):
        args = types.SimpleNamespace(project="carousel-proj", deliverable=slug,
                                     allow_missing_exports=allow_missing_exports)
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            af.cmd_lock(args)

    def deliverable_status(self, rel):
        dfm, _ = af.split_fm(af.read(os.path.join(self.cdir, rel)), rel)
        return af.get_scalar(dfm, "status")

    def test_lock_refuses_image_prompts_with_no_exports(self):
        rel = self.make_deliverable("post-1-carousel", "image-prompts-v1.md")
        with self.assertRaises(SystemExit):
            self.run_lock("post-1-carousel")
        self.assertEqual(self.deliverable_status(rel), "drafting")

    def test_lock_refuses_dangling_exports_path(self):
        rel = self.make_deliverable("post-1-carousel", "image-prompts-v1.md",
                                    "exports:\n  - media/missing.png")
        with self.assertRaises(SystemExit):
            self.run_lock("post-1-carousel")
        self.assertEqual(self.deliverable_status(rel), "drafting")

    def test_lock_locks_image_prompts_with_filed_exports(self):
        rel = self.make_deliverable("post-1-carousel", "image-prompts-v1.md",
                                    "exports:\n  - media/final.pdf")
        os.makedirs(os.path.join(self.cdir, "post-1-carousel", "media"))
        af.write(os.path.join(self.cdir, "post-1-carousel", "media", "final.pdf"), "x")
        self.run_lock("post-1-carousel")
        self.assertEqual(self.deliverable_status(rel), "locked")

    def test_override_locks_and_marks_activity(self):
        rel = self.make_deliverable("post-1-carousel", "image-prompts-v1.md")
        self.run_lock("post-1-carousel", allow_missing_exports=True)
        self.assertEqual(self.deliverable_status(rel), "locked")
        self.assertIn("WITHOUT EXPORTS", af.read(os.path.join(self.cdir, "activity.md")))

    def test_non_exportable_deliverable_locks_without_exports(self):
        rel = self.make_deliverable("post-1-body", "body-copy-v1.md")
        self.run_lock("post-1-body")
        self.assertEqual(self.deliverable_status(rel), "locked")


if __name__ == "__main__":
    unittest.main()
