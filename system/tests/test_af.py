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
        "schema_version: 2026-07-19-v2",
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


class ProjectStateContractTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = self._tmp.name
        self.cdir = os.path.join(self.root, "workspace", "projects", "minimal")
        os.makedirs(self.cdir, exist_ok=True)
        self._root_patch = patch.object(af, "ROOT", self.root)
        self._domains_patch = patch.object(
            af, "DOMAINS", os.path.join(REPO_ROOT, "library", "domains")
        )
        self._root_patch.start()
        self._domains_patch.start()
        self.addCleanup(self._root_patch.stop)
        self.addCleanup(self._domains_patch.stop)
        self.addCleanup(self._tmp.cleanup)

    def write_project(self, *, status="active", terminal=""):
        af.write(
            os.path.join(self.cdir, "project.md"),
            "---\n"
            "name: Minimal\n"
            "slug: minimal\n"
            "schema_version: 2026-07-19-v2\n"
            "created_at: 2026-07-19\n"
            "domain: project-mgmt\n"
            f"status: {status}\n"
            "current_phase: active\n"
            "flow: open-flow\n"
            "last_activity: 2026-07-19T10:00:00-07:00\n"
            f"{terminal}"
            "deliverables: {}\n"
            "---\n\n# Minimal\n",
        )

    def test_minimal_active_index_needs_no_null_placeholders(self):
        self.write_project()
        self.assertEqual(af.check_project(self.cdir), [])

    def test_complete_requires_completed_at(self):
        self.write_project(status="complete")
        self.assertTrue(any("complete requires completed_at" in i for i in af.check_project(self.cdir)))

    def test_terminal_timestamp_must_match_lifecycle(self):
        self.write_project(terminal="completed_at: 2026-07-19\n")
        self.assertTrue(any("completed_at is present but status is active" in i for i in af.check_project(self.cdir)))


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
deliverables:
  post-1:
    status: published
    file: posts/post-1/post-FINAL.md
    last_updated: 2026-06-01
  post-2:
    status: drafting
    file: posts/post-2/post-FINAL.md
    last_updated: 2026-07-01"""

ARCHIVE_FM = """---
deliverables:
  post-0:
    status: published
    file: posts/post-0/post-FINAL.md
    last_updated: 2026-05-01
---

> Rows archived from project.md by knowledge_consolidation passes.
"""


class ArchivedRowDerivedTotalTests(unittest.TestCase):
    """Marketing receipts derive published totals across tracker + archive."""

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

    def total(self):
        return self.rules._published_posts(af.make_ctx(), PROJECT_FM) + self.rules._archived_published_posts(
            af.make_ctx(), self.cdir
        )

    def test_derived_total_includes_archive(self):
        af.write(os.path.join(self.cdir, "knowledge", "_archive", "deliverables-archive.md"), ARCHIVE_FM)
        self.assertEqual(self.total(), 2)

    def test_derived_total_without_archive_counts_tracker_only(self):
        self.assertEqual(self.total(), 1)


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


class ReadyExportGateTests(unittest.TestCase):
    """af ready refuses exportable deliverables whose exports[] are empty or dangling."""

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

    def run_ready(self, slug, allow_missing_exports=False):
        args = types.SimpleNamespace(project="carousel-proj", deliverable=slug,
                                     allow_missing_exports=allow_missing_exports)
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            af.cmd_ready(args)

    def deliverable_status(self, rel):
        dfm, _ = af.split_fm(af.read(os.path.join(self.cdir, rel)), rel)
        return af.get_scalar(dfm, "status")

    def test_ready_refuses_image_prompts_with_no_exports(self):
        rel = self.make_deliverable("post-1-carousel", "image-prompts-v1.md")
        with self.assertRaises(SystemExit):
            self.run_ready("post-1-carousel")
        self.assertEqual(self.deliverable_status(rel), "drafting")

    def test_ready_refuses_dangling_exports_path(self):
        rel = self.make_deliverable("post-1-carousel", "image-prompts-v1.md",
                                    "exports:\n  - media/missing.png")
        with self.assertRaises(SystemExit):
            self.run_ready("post-1-carousel")
        self.assertEqual(self.deliverable_status(rel), "drafting")

    def test_ready_accepts_image_prompts_with_filed_exports(self):
        rel = self.make_deliverable("post-1-carousel", "image-prompts-v1.md",
                                    "exports:\n  - media/final.pdf")
        os.makedirs(os.path.join(self.cdir, "post-1-carousel", "media"))
        af.write(os.path.join(self.cdir, "post-1-carousel", "media", "final.pdf"), "x")
        self.run_ready("post-1-carousel")
        self.assertEqual(self.deliverable_status(rel), "ready")

    def test_override_marks_ready_and_activity(self):
        rel = self.make_deliverable("post-1-carousel", "image-prompts-v1.md")
        self.run_ready("post-1-carousel", allow_missing_exports=True)
        self.assertEqual(self.deliverable_status(rel), "ready")
        self.assertIn("WITHOUT EXPORTS", af.read(os.path.join(self.cdir, "activity.md")))

    def test_non_exportable_deliverable_becomes_ready_without_exports(self):
        rel = self.make_deliverable("post-1-body", "body-copy-v1.md")
        self.run_ready("post-1-body")
        self.assertEqual(self.deliverable_status(rel), "ready")

    def test_direct_path_synchronizes_matching_tracker_row(self):
        rel = self.make_deliverable("post-1-body", "body-copy-v1.md")
        self.run_ready(rel)
        cfm, _ = af.split_fm(af.read(os.path.join(self.cdir, "project.md")))
        self.assertEqual(af.row_get(cfm, "post-1-body", "status"), "ready")


VERSION_PROJECT_FM = """---
name: Version Project
slug: version-project
schema_version: 2026-07-19-v2
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
        self.make_state("brief", rel, status="ready")
        self.make_artifact(rel, status="ready")

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
        self.make_state("post-8", rel, status="ready")
        self.make_artifact(rel, status="ready")
        self.make_artifact("posts/post-8/body-copy-v1.md", status="ready")

        self.run_version("post-8", artifact="body-copy")

        fm, _ = af.split_fm(af.read(os.path.join(self.cdir, "project.md")))
        self.assertEqual(af.row_get(fm, "post-8", "file"), rel)
        self.assertEqual(af.row_get(fm, "post-8", "status"), "drafting")

    def test_versioning_ready_head_records_source_status_without_unlock_ceremony(self):
        rel = "brief/brief-v1.md"
        self.make_state("brief", rel, status="ready")
        self.make_artifact(rel, status="ready")

        self.run_version("brief")

        self.assertTrue(os.path.isfile(os.path.join(self.cdir, "brief", "brief-v2.md")))
        activity = af.read(os.path.join(self.cdir, "activity.md"))
        self.assertIn("artifact_versioned: brief", activity)
        self.assertIn("source_status=ready", activity)
        self.assertNotIn("unlock_version", activity)

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


class MarketingReadyAssemblyTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.projects = os.path.join(self._tmp.name, "workspace", "projects")
        self._patch_projects = patch.object(af, "PROJECTS", self.projects)
        self._patch_projects.start()
        self.addCleanup(self._patch_projects.stop)
        self.addCleanup(self._tmp.cleanup)
        self.cdir = os.path.join(self.projects, "campaign")
        post_dir = os.path.join(self.cdir, "posts", "post-1")
        os.makedirs(os.path.join(post_dir, "media"), exist_ok=True)
        af.write(
            os.path.join(self.cdir, "project.md"),
            "---\nname: campaign\nslug: campaign\ndomain: marketing\nstatus: active\n"
            "last_activity: 2026-07-19T10:00:00-07:00\n"
            "post_manifest:\n  ingredients: [body-copy, image-prompts]\n"
            "deliverables:\n  post-1:\n    status: drafting\n"
            "    file: posts/post-1/post-FINAL.md\n    last_updated: 2026-07-19\n---\n",
        )
        af.write(
            os.path.join(post_dir, "post-FINAL.md"),
            "---\nstatus: drafting\nlast_updated: 2026-07-19\n---\n\n# Post\n",
        )
        af.write(
            os.path.join(post_dir, "body-copy-v1.md"),
            "---\nstatus: ready\nlast_updated: 2026-07-19\n---\n\nBody\n",
        )
        af.write(os.path.join(post_dir, "media", "final.png"), "png")
        af.write(
            os.path.join(post_dir, "image-prompts-v1.md"),
            "---\nstatus: drafting\nlast_updated: 2026-07-19\n"
            "exports:\n  - media/final.png\n---\n\nPrompts\n",
        )

    def test_last_ready_ingredient_marks_assembly_and_tracker_ready(self):
        args = types.SimpleNamespace(
            project="campaign", deliverable="posts/post-1/image-prompts-v1.md",
            allow_missing_exports=False,
        )
        with contextlib.redirect_stdout(io.StringIO()):
            af.cmd_ready(args)

        cfm, _ = af.split_fm(af.read(os.path.join(self.cdir, "project.md")))
        pfm, pbody = af.split_fm(
            af.read(os.path.join(self.cdir, "posts/post-1/post-FINAL.md"))
        )
        self.assertEqual(af.row_get(cfm, "post-1", "status"), "ready")
        self.assertEqual(af.get_scalar(pfm, "status"), "ready")
        self.assertIn("Image Prompts (ready from image-prompts-v1.md)", pbody)

    def test_post_assembly_cannot_be_ready_before_its_ingredients(self):
        args = types.SimpleNamespace(
            project="campaign", deliverable="post-1", allow_missing_exports=False,
        )
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            af.cmd_ready(args)

        cfm, _ = af.split_fm(af.read(os.path.join(self.cdir, "project.md")))
        self.assertEqual(af.row_get(cfm, "post-1", "status"), "drafting")


class PublishCommandTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.projects = os.path.join(self._tmp.name, "workspace", "projects")
        self._patch_projects = patch.object(af, "PROJECTS", self.projects)
        self._patch_projects.start()
        self.addCleanup(self._patch_projects.stop)
        self.addCleanup(self._tmp.cleanup)

    def make_project(self, slug, domain, row, rel, status="ready"):
        cdir = os.path.join(self.projects, slug)
        os.makedirs(os.path.dirname(os.path.join(cdir, rel)), exist_ok=True)
        af.write(
            os.path.join(cdir, "project.md"),
            "---\n"
            f"name: {slug}\nslug: {slug}\nschema_version: {af.PROJECT_SCHEMA_VERSION}\n"
            f"created_at: 2026-07-19\ndomain: {domain}\nstatus: active\n"
            "current_phase: active\nflow: open-flow\nlast_activity: 2026-07-19T10:00:00-07:00\n"
            f"deliverables:\n  {row}:\n    status: {status}\n    file: {rel}\n"
            "    last_updated: 2026-07-19\n---\n",
        )
        af.write(
            os.path.join(cdir, rel),
            f"---\nstatus: {status}\nlast_updated: 2026-07-19\n---\n\nbody\n",
        )
        return cdir

    def args(self, project, deliverable, url=None):
        return types.SimpleNamespace(
            project=project, deliverable=deliverable, url=url,
            posted_at=None, platform=None, media=[],
        )

    def test_generic_publish_moves_ready_artifact_and_tracker_to_published(self):
        cdir = self.make_project("generic", "project-mgmt", "brief", "brief/brief-v1.md")
        with contextlib.redirect_stdout(io.StringIO()):
            af.cmd_publish(self.args("generic", "brief"))

        cfm, _ = af.split_fm(af.read(os.path.join(cdir, "project.md")))
        dfm, _ = af.split_fm(af.read(os.path.join(cdir, "brief/brief-v1.md")))
        self.assertEqual(af.row_get(cfm, "brief", "status"), "published")
        self.assertEqual(af.get_scalar(dfm, "status"), "published")
        self.assertIsNone(af.get_scalar(dfm, "published_url"))
        self.assertIn("publish: brief published", af.read(os.path.join(cdir, "activity.md")))

    def test_generic_publish_records_url_only_when_supplied(self):
        cdir = self.make_project("generic", "project-mgmt", "brief", "brief/brief-v1.md")
        with contextlib.redirect_stdout(io.StringIO()):
            af.cmd_publish(self.args("generic", "brief", "https://example.com/brief"))

        dfm, _ = af.split_fm(af.read(os.path.join(cdir, "brief/brief-v1.md")))
        self.assertEqual(af.get_scalar(dfm, "published_url"), "https://example.com/brief")

    def test_generic_publish_refuses_drafting_artifact(self):
        cdir = self.make_project(
            "generic", "project-mgmt", "brief", "brief/brief-v1.md", status="drafting"
        )
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            af.cmd_publish(self.args("generic", "brief"))
        cfm, _ = af.split_fm(af.read(os.path.join(cdir, "project.md")))
        self.assertEqual(af.row_get(cfm, "brief", "status"), "drafting")

    def test_marketing_publish_records_receipt_and_adds_optional_ship_date(self):
        cdir = self.make_project(
            "campaign", "marketing", "post-1", "posts/post-1/post-FINAL.md"
        )
        args = self.args("campaign", "post-1", "https://example.com/post-1")
        with contextlib.redirect_stdout(io.StringIO()):
            af.cmd_publish(args)

        cfm, _ = af.split_fm(af.read(os.path.join(cdir, "project.md")))
        pfm, _ = af.split_fm(af.read(os.path.join(cdir, "posts/post-1/post-FINAL.md")))
        self.assertEqual(af.row_get(cfm, "post-1", "status"), "published")
        self.assertEqual(af.get_scalar(pfm, "status"), "published")
        self.assertEqual(af.get_scalar(cfm, "shipped_at"), af.today())
        self.assertIn("url: https://example.com/post-1", pfm)

    def test_non_post_marketing_deliverable_uses_generic_publish(self):
        cdir = self.make_project(
            "campaign", "marketing", "essay", "essay/substack-essay-v1.md"
        )
        with contextlib.redirect_stdout(io.StringIO()):
            af.cmd_publish(self.args("campaign", "essay"))

        cfm, _ = af.split_fm(af.read(os.path.join(cdir, "project.md")))
        self.assertEqual(af.row_get(cfm, "essay", "status"), "published")

    def test_marketing_post_path_keeps_receipt_aware_publish(self):
        rel = "posts/post-1/post-FINAL.md"
        cdir = self.make_project("campaign", "marketing", "post-1", rel)
        with contextlib.redirect_stdout(io.StringIO()):
            af.cmd_publish(self.args("campaign", rel, "https://example.com/by-path"))

        cfm, _ = af.split_fm(af.read(os.path.join(cdir, "project.md")))
        pfm, _ = af.split_fm(af.read(os.path.join(cdir, rel)))
        self.assertEqual(af.row_get(cfm, "post-1", "status"), "published")
        self.assertIn("url: https://example.com/by-path", pfm)


class LifecycleCliParserTests(unittest.TestCase):
    def test_ready_is_the_quality_gate_command(self):
        with patch.object(af, "cmd_ready") as command, \
             patch.object(af, "check_mode_gate"), \
             patch.object(sys, "argv", ["af", "ready", "project", "brief"]):
            af.main()
        self.assertEqual(command.call_args.args[0].deliverable, "brief")

    def test_retired_lock_command_is_not_parsed(self):
        with patch.object(sys, "argv", ["af", "lock", "project", "brief"]), \
             contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            af.main()


if __name__ == "__main__":
    unittest.main()
