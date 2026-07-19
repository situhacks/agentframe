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

    def test_null_last_consolidated_waits_for_first_consolidation(self):
        cdir = make_project(self.root, "neverdreamed", created_at=days_ago(60),
                            last_activity=days_ago(1))
        self.assertIsNone(af.dream_note(cdir))

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


VERSION_PROJECT_FM = """---
name: Version Project
slug: version-project
schema_version: 2026-04-23
created_at: 2026-07-01
domain: marketing
status: active
current_phase: active
flow: open-flow
last_activity: 2026-07-01T10:00:00+00:00
deliverables:
  {row}:
    status: {status}
    file: {file}
    last_updated: 2026-07-01
---
"""


class VersionCommandCharacterizationTests(unittest.TestCase):
    """Pin the pre-repair af version contract, including the Post 8 failure."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.projects = os.path.join(self._tmp.name, "workspace", "projects")
        self._patch_projects = patch.object(af, "PROJECTS", self.projects)
        self._patch_pipeline = patch.object(af, "PIPELINE", os.path.join(self._tmp.name, "workspace", "pipeline"))
        self._patch_projects.start()
        self._patch_pipeline.start()
        self.addCleanup(self._patch_projects.stop)
        self.addCleanup(self._patch_pipeline.stop)
        self.addCleanup(self._tmp.cleanup)
        self.cdir = os.path.join(self.projects, "version-project")
        os.makedirs(self.cdir)

    def make_state(self, row, rel, *, status="drafting"):
        af.write(
            os.path.join(self.cdir, "project.md"),
            VERSION_PROJECT_FM.format(row=row, status=status, file=rel),
        )

    def make_artifact(self, rel, *, status="drafting", body="body\n"):
        path = os.path.join(self.cdir, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        af.write(
            path,
            f"---\nstatus: {status}\nlast_updated: 2026-07-01\n---\n\n{body}",
        )
        return path

    def run_version(self, row, artifact=None):
        args = types.SimpleNamespace(project="version-project", deliverable=row, artifact=artifact)
        with contextlib.redirect_stdout(io.StringIO()):
            af.cmd_version(args)

    def tracker_file(self, row):
        fm, _ = af.split_fm(af.read(os.path.join(self.cdir, "project.md")), "project.md")
        return af.row_get(fm, row, "file")

    def test_tracker_owned_head_versions_and_moves_pointer(self):
        rel = "brief/brief-v1.md"
        self.make_state("brief", rel)
        source = self.make_artifact(rel, body="source snapshot\n")
        before = af.read(source)

        self.run_version("brief")

        new_rel = "brief/brief-v2.md"
        self.assertEqual(self.tracker_file("brief"), new_rel)
        self.assertTrue(os.path.isfile(os.path.join(self.cdir, new_rel)))
        self.assertEqual(af.read(source), before)

    def test_tracker_owned_new_head_resets_drafting_fields(self):
        rel = "brief/brief-v1.md"
        self.make_state("brief", rel, status="locked")
        self.make_artifact(rel, status="locked")

        self.run_version("brief")

        new_fm, _ = af.split_fm(af.read(os.path.join(self.cdir, "brief", "brief-v2.md")))
        self.assertEqual(af.get_scalar(new_fm, "status"), "drafting")
        self.assertEqual(af.get_scalar(new_fm, "last_updated"), af.today())

    def test_post_final_pointer_versions_named_nested_body_copy(self):
        rel = "posts/post-8/post-FINAL.md"
        self.make_state("post-8", rel)
        self.make_artifact(rel)
        source = self.make_artifact("posts/post-8/body-copy-v1.md", body="ingredient\n")
        before = af.read(source)

        self.run_version("post-8", artifact="body-copy")

        self.assertTrue(os.path.exists(os.path.join(self.cdir, "posts", "post-8", "body-copy-v2.md")))
        self.assertEqual(af.read(source), before)
        self.assertEqual(self.tracker_file("post-8"), rel)

    def test_nested_artifact_name_is_exact(self):
        rel = "posts/post-8/post-FINAL.md"
        self.make_state("post-8", rel)
        self.make_artifact(rel)
        self.make_artifact("posts/post-8/body-copy-notes-v1.md")
        stderr = io.StringIO()

        with contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit):
            self.run_version("post-8", artifact="body-copy")

        self.assertIn("artifact 'body-copy' has no versioned head", stderr.getvalue())

    def test_nested_artifact_updates_parent_status_but_not_pointer(self):
        rel = "posts/post-8/post-FINAL.md"
        self.make_state("post-8", rel, status="locked")
        self.make_artifact(rel, status="locked")
        self.make_artifact("posts/post-8/body-copy-v1.md", status="locked")

        self.run_version("post-8", artifact="body-copy")

        fm, _ = af.split_fm(af.read(os.path.join(self.cdir, "project.md")))
        self.assertEqual(af.row_get(fm, "post-8", "file"), rel)
        self.assertEqual(af.row_get(fm, "post-8", "status"), "drafting")

    def test_versioning_locked_head_records_explicit_unlock_version_event(self):
        rel = "brief/brief-v1.md"
        self.make_state("brief", rel, status="locked")
        self.make_artifact(rel, status="locked")

        self.run_version("brief")

        self.assertTrue(os.path.isfile(os.path.join(self.cdir, "brief", "brief-v2.md")))
        activity = af.read(os.path.join(self.cdir, "activity.md"))
        self.assertIn("unlock_version: brief", activity)
        self.assertIn("source_status=locked", activity)
        self.assertNotIn("artifact_versioned", activity)

    def test_routine_version_appends_artifact_versioned_pulse(self):
        rel = "brief/brief-v1.md"
        self.make_state("brief", rel)
        self.make_artifact(rel)

        self.run_version("brief")

        activity = af.read(os.path.join(self.cdir, "activity.md"))
        self.assertIn("artifact_versioned: brief v1 -> v2; brief/brief-v2.md", activity)
        self.assertNotIn("unlock_version", activity)
        self.assertTrue(af.ACTIVITY_LINE_RE.match(activity.strip().splitlines()[-1]))

    def test_nested_version_pulse_uses_artifact_label(self):
        rel = "posts/post-8/post-FINAL.md"
        self.make_state("post-8", rel)
        self.make_artifact(rel)
        self.make_artifact("posts/post-8/body-copy-v1.md")

        self.run_version("post-8", artifact="body-copy")

        activity = af.read(os.path.join(self.cdir, "activity.md"))
        self.assertIn("artifact_versioned: body-copy v1 -> v2; posts/post-8/body-copy-v2.md", activity)

    def test_failed_version_appends_no_activity(self):
        rel = "posts/post-8/post-FINAL.md"
        self.make_state("post-8", rel)
        self.make_artifact(rel)
        stderr = io.StringIO()

        with contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit):
            self.run_version("post-8", artifact="body-copy")

        self.assertFalse(os.path.isfile(os.path.join(self.cdir, "activity.md")))

    def test_version_receipt_states_copy_forward_contract(self):
        rel = "brief/brief-v1.md"
        self.make_state("brief", rel)
        self.make_artifact(rel)
        out = io.StringIO()
        args = types.SimpleNamespace(project="version-project", deliverable="brief", artifact=None)

        with contextlib.redirect_stdout(out):
            af.cmd_version(args)

        self.assertIn("already contains", out.getvalue())
        self.assertIn("surgical", out.getvalue())

    def test_cli_parser_accepts_nested_artifact_address(self):
        with patch.object(af, "cmd_version") as command, \
             patch.object(af, "check_mode_gate"), \
             patch.object(sys, "argv", ["af", "version", "version-project", "post-8", "--artifact", "body-copy"]):
            af.main()

        args = command.call_args.args[0]
        self.assertEqual(args.artifact, "body-copy")


class DraftCommandTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.projects = os.path.join(self._tmp.name, "workspace", "projects")
        self._patch_projects = patch.object(af, "PROJECTS", self.projects)
        self._patch_pipeline = patch.object(af, "PIPELINE", os.path.join(self._tmp.name, "workspace", "pipeline"))
        self._patch_projects.start()
        self._patch_pipeline.start()
        self.addCleanup(self._patch_projects.stop)
        self.addCleanup(self._patch_pipeline.stop)
        self.addCleanup(self._tmp.cleanup)
        self.cdir = os.path.join(self.projects, "version-project")
        os.makedirs(self.cdir)

    def make_state(self, row, rel, *, status="drafting"):
        af.write(
            os.path.join(self.cdir, "project.md"),
            VERSION_PROJECT_FM.format(row=row, status=status, file=rel),
        )

    def make_artifact(self, rel, *, status="drafting", body="body\n"):
        path = os.path.join(self.cdir, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        af.write(
            path,
            f"---\nstatus: {status}\nlast_updated: 2026-07-01\n---\n\n{body}",
        )
        return path

    def tracker_file(self, row):
        fm, _ = af.split_fm(af.read(os.path.join(self.cdir, "project.md")), "project.md")
        return af.row_get(fm, row, "file")

    def run_draft(self, row, *, artifact=None, file=None):
        args = types.SimpleNamespace(
            project="version-project", deliverable=row, artifact=artifact, file=file
        )
        with contextlib.redirect_stdout(io.StringIO()):
            af.cmd_draft(args)

    def test_tracker_owned_draft_creates_v1_and_moves_pointer(self):
        self.make_state("brief", "brief/placeholder.md", status="not_started")

        self.run_draft("brief", file="brief/brief-v1.md")

        path = os.path.join(self.cdir, "brief", "brief-v1.md")
        self.assertTrue(os.path.isfile(path))
        self.assertEqual(self.tracker_file("brief"), "brief/brief-v1.md")
        fm, _ = af.split_fm(af.read(path))
        self.assertEqual(af.get_scalar(fm, "status"), "drafting")

    def test_tracker_owned_draft_refuses_non_v1_path(self):
        self.make_state("brief", "brief/placeholder.md", status="not_started")
        stderr = io.StringIO()

        with contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit):
            self.run_draft("brief", file="brief/brief.md")

        self.assertIn("must name a canonical -v1.md", stderr.getvalue())

    def test_marketing_artifact_draft_creates_assembly_record_and_preserves_parent_address(self):
        parent = "phase-3-production/posts/post-8/post-FINAL.md"
        self.make_state("post-8", parent, status="not_started")

        self.run_draft("post-8", artifact="body-copy")

        ingredient = os.path.join(self.cdir, "phase-3-production", "posts", "post-8", "body-copy-v1.md")
        assembly = os.path.join(self.cdir, "phase-3-production", "posts", "post-8", "post-FINAL.md")
        self.assertTrue(os.path.isfile(ingredient))
        self.assertTrue(os.path.isfile(assembly))
        self.assertEqual(self.tracker_file("post-8"), parent)

    def test_artifact_draft_refuses_existing_chain(self):
        parent = "posts/post-8/post-FINAL.md"
        self.make_state("post-8", parent, status="drafting")
        self.make_artifact(parent)
        self.make_artifact("posts/post-8/body-copy-v1.md")
        stderr = io.StringIO()

        with contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit):
            self.run_draft("post-8", artifact="body-copy")

        self.assertIn("already has a version chain", stderr.getvalue())

    def test_tracker_owned_draft_appends_artifact_drafted_pulse(self):
        self.make_state("brief", "brief/placeholder.md", status="not_started")

        self.run_draft("brief", file="brief/brief-v1.md")

        activity = af.read(os.path.join(self.cdir, "activity.md"))
        self.assertIn("artifact_drafted: brief created; brief/brief-v1.md", activity)
        self.assertTrue(af.ACTIVITY_LINE_RE.match(activity.strip().splitlines()[-1]))

    def test_nested_artifact_draft_appends_artifact_drafted_pulse(self):
        parent = "phase-3-production/posts/post-8/post-FINAL.md"
        self.make_state("post-8", parent, status="not_started")

        self.run_draft("post-8", artifact="body-copy")

        activity = af.read(os.path.join(self.cdir, "activity.md"))
        self.assertIn(
            "artifact_drafted: body-copy created; phase-3-production/posts/post-8/body-copy-v1.md",
            activity,
        )

    def test_failed_draft_appends_no_activity(self):
        self.make_state("brief", "brief/placeholder.md", status="not_started")
        stderr = io.StringIO()

        with contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit):
            self.run_draft("brief", file="brief/brief.md")

        self.assertFalse(os.path.isfile(os.path.join(self.cdir, "activity.md")))

    def test_cli_parser_requires_one_draft_address(self):
        with patch.object(af, "cmd_draft") as command, \
             patch.object(af, "check_mode_gate"), \
             patch.object(sys, "argv", ["af", "draft", "version-project", "post-8", "--artifact", "body-copy"]):
            af.main()

        args = command.call_args.args[0]
        self.assertEqual(args.artifact, "body-copy")
        self.assertIsNone(args.file)

    def test_adopt_creates_row_from_empty_deliverables_map(self):
        state = VERSION_PROJECT_FM.format(
            row="brief", status="drafting", file="brief/placeholder.md"
        ).replace(
            "deliverables:\n  brief:\n    status: drafting\n    file: brief/placeholder.md\n    last_updated: 2026-07-01",
            "deliverables: {}",
        )
        af.write(os.path.join(self.cdir, "project.md"), state)
        self.make_artifact("brief/brief-v1.md")
        args = types.SimpleNamespace(
            project="version-project", deliverable="brief", file="brief/brief-v1.md",
            workstream=None, export=None, notes="existing renderer output",
        )

        with contextlib.redirect_stdout(io.StringIO()):
            af.cmd_adopt(args)

        fm, _ = af.split_fm(af.read(os.path.join(self.cdir, "project.md")))
        self.assertEqual(af.all_rows(fm), ["brief"])
        self.assertEqual(af.row_get(fm, "brief", "file"), "brief/brief-v1.md")
        self.assertEqual(af.row_get(fm, "brief", "status"), "drafting")

    def test_adopt_never_overwrites_existing_artifact(self):
        self.make_state("brief", "brief/current-v1.md", status="drafting")
        self.make_artifact("brief/current-v1.md", body="keep\n")
        self.make_artifact("brief/replacement-v1.md", body="replacement\n")
        args = types.SimpleNamespace(
            project="version-project", deliverable="brief", file="brief/replacement-v1.md",
            workstream=None, export=None, notes=None,
        )

        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            af.cmd_adopt(args)

        self.assertIn("keep", af.read(os.path.join(self.cdir, "brief/current-v1.md")))


if __name__ == "__main__":
    unittest.main()
