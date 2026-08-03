import contextlib
import io
import os
import tempfile
import types
import unittest
import re
from unittest.mock import patch

from system import af


PROJECT_MD = """---
name: Autonomy Test
slug: auto-test
schema_version: 2026-07-19-v2
created_at: 2026-07-10
domain: marketing
status: active
current_phase: active
flow: open-flow
last_activity: 2026-07-10T10:00:00+00:00
deliverables:
---

# Autonomy Test
"""


class AutonomyTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = self._tmp.name
        self.projects = os.path.join(self.root, "workspace", "projects")
        self.cdir = os.path.join(self.projects, "auto-test")
        os.makedirs(os.path.join(self.cdir, "knowledge"), exist_ok=True)
        af.write(os.path.join(self.cdir, "project.md"), PROJECT_MD)
        self._root_patch = patch.object(af, "ROOT", self.root)
        self._projects_patch = patch.object(af, "PROJECTS", self.projects)
        self._root_patch.start()
        self._projects_patch.start()
        self.addCleanup(self._root_patch.stop)
        self.addCleanup(self._projects_patch.stop)
        self.addCleanup(self._tmp.cleanup)

    @property
    def run_path(self):
        return os.path.join(self.cdir, "knowledge", "autonomy", "build-widget.md")

    def quiet(self, fn, args):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return fn(args)

    def init(self, level="assisted"):
        self.quiet(af.cmd_autonomy_init, types.SimpleNamespace(
            project="auto-test", run_id="build-widget", level=level))

    def make_ready(self, **overrides):
        fm, body = af.split_fm(af.read(self.run_path), self.run_path)
        values = {
            "goal": af.yaml_quote("Build the widget"),
            "done_when": af.yaml_quote("Widget checks pass"),
            "context_sources": "[operator-brief, project.md]",
            "allowed_paths": "[build_repo/src, build_repo/tests]",
            "verification": "[pytest, independent-review]",
        }
        values.update({key: str(value) for key, value in overrides.items()})
        for key, value in values.items():
            fm = af.set_scalar(fm, key, value, self.run_path)
        af.write(self.run_path, af.join_fm(fm, body))

    def start(self, resume_reason=None, session_binding="codex:test-session"):
        self.quiet(af.cmd_autonomy_start, types.SimpleNamespace(
            project="auto-test", run_id="build-widget", resume_reason=resume_reason,
            session_binding=session_binding))

    def checkpoint(self, outcome="continue", summary="unit complete", evidence=None, spawned=0):
        self.quiet(af.cmd_autonomy_checkpoint, types.SimpleNamespace(
            project="auto-test", run_id="build-widget", outcome=outcome,
            summary=summary, evidence=evidence, subagents_spawned=spawned))

    def run_fm(self):
        return af.split_fm(af.read(self.run_path), self.run_path)[0]

    def test_init_scaffolds_one_proposed_run_file(self):
        self.init()
        self.assertTrue(os.path.isfile(self.run_path))
        fm = self.run_fm()
        self.assertEqual(af.get_scalar(fm, "status"), "proposed")
        self.assertEqual(af.get_scalar(fm, "subagents_used"), "0")
        self.assertEqual(af.get_scalar(fm, "bound_session"), "null")
        self.assertEqual(af.get_scalar(fm, "contract_sha256"), "null")
        self.assertEqual(
            af.fm_list(fm, "prohibited_effects"),
            list(af.autonomy_contract.PROHIBITED_EFFECTS),
        )
        issues = af.autonomy_issues(self.run_path, expected_project="auto-test")
        self.assertTrue(any("goal" in issue for issue in issues))
        self.assertTrue(any("context_sources" in issue for issue in issues))

    def test_doctor_schema_allows_unfinished_proposed_contract(self):
        self.init()
        issues = af.autonomy_issues(
            self.run_path, expected_project="auto-test", require_ready=False)
        self.assertEqual(issues, [])

    def test_ready_run_starts_and_logs_material_event(self):
        self.init()
        self.make_ready()
        self.start()
        fm = self.run_fm()
        self.assertEqual(af.get_scalar(fm, "status"), "running")
        self.assertNotEqual(af.get_scalar(fm, "started_at"), "null")
        self.assertEqual(af.get_scalar(fm, "bound_session"), "codex:test-session")
        self.assertRegex(af.get_scalar(fm, "contract_sha256"), r"^[0-9a-f]{64}$")
        activity = af.read(os.path.join(self.cdir, "activity.md"))
        self.assertIn("autonomy_started: build-widget running", activity)

    def test_unattended_requires_independent_reviewer(self):
        self.init(level="unattended")
        self.make_ready(reviewer_mode="same-context")
        with self.assertRaises(SystemExit):
            self.start()
        self.assertEqual(af.get_scalar(self.run_fm(), "status"), "proposed")

    def test_cursor_refuses_unattended_binding(self):
        self.init(level="unattended")
        self.make_ready()
        with self.assertRaises(SystemExit):
            self.start(session_binding="cursor:test-session")
        self.assertEqual(af.get_scalar(self.run_fm(), "status"), "proposed")

    def test_contract_drift_only_allows_budget_neutral_quarantine(self):
        self.init()
        self.make_ready()
        self.start()
        fm, body = af.split_fm(af.read(self.run_path), self.run_path)
        fm = af.set_scalar(fm, "goal", af.yaml_quote("Changed after seal"), self.run_path)
        af.write(self.run_path, af.join_fm(fm, body))
        with self.assertRaises(SystemExit):
            self.checkpoint()
        self.checkpoint(outcome="blocked", summary="quarantine drift", spawned=3)
        fm = self.run_fm()
        self.assertEqual(af.get_scalar(fm, "status"), "blocked")
        self.assertEqual(af.get_scalar(fm, "iteration"), "0")
        self.assertEqual(af.get_scalar(fm, "subagents_used"), "0")
        self.assertIn("stored=", af.get_scalar(fm, "blocked_reason"))

    def test_frozen_file_bytes_participate_in_hash(self):
        source = os.path.join(self.root, "brief.md")
        af.write(source, "approved\n")
        self.init()
        self.make_ready(
            context_sources="[brief.md, project.md]",
            frozen_context="[brief.md]",
        )
        self.start()
        af.write(source, "changed\n")
        with self.assertRaises(SystemExit):
            self.checkpoint()

    def test_one_session_cannot_own_two_running_runs(self):
        self.init()
        self.make_ready()
        self.start()
        self.quiet(
            af.cmd_autonomy_init,
            types.SimpleNamespace(
                project="auto-test", run_id="second-run", level="assisted"
            ),
        )
        second = os.path.join(
            self.cdir, "knowledge", "autonomy", "second-run.md"
        )
        fm, body = af.split_fm(af.read(second), second)
        for key, value in {
            "goal": af.yaml_quote("Second"),
            "done_when": af.yaml_quote("Second passes"),
            "context_sources": "[project.md]",
            "allowed_paths": "[build_repo/second]",
            "verification": "[pytest]",
        }.items():
            fm = af.set_scalar(fm, key, value, second)
        af.write(second, af.join_fm(fm, body))
        with self.assertRaises(SystemExit):
            self.quiet(
                af.cmd_autonomy_start,
                types.SimpleNamespace(
                    project="auto-test",
                    run_id="second-run",
                    resume_reason=None,
                    session_binding="codex:test-session",
                ),
            )

    def test_legacy_proposed_migrates_without_invented_hash(self):
        self.init()
        text = af.read(self.run_path)
        text = text.replace(
            f"schema_version: {af.AUTONOMY_SCHEMA_VERSION}",
            "schema_version: 2026-07-10",
        )
        for field in (
            "frozen_context",
            "bound_session",
            "contract_sha256",
            "prohibited_effects",
        ):
            text = re.sub(rf"^{field}:.*\n?", "", text, flags=re.M)
        af.write(self.run_path, text)
        self.quiet(
            af.cmd_autonomy_migrate,
            types.SimpleNamespace(
                project="auto-test", run_id="build-widget", quarantine=False
            ),
        )
        fm = self.run_fm()
        self.assertEqual(
            af.get_scalar(fm, "schema_version"),
            af.AUTONOMY_SCHEMA_VERSION,
        )
        self.assertEqual(af.get_scalar(fm, "contract_sha256"), "null")

    def test_checkpoint_tracks_iterations_and_subagents(self):
        self.init()
        self.make_ready()
        self.start()
        self.checkpoint(spawned=2)
        fm, body = af.split_fm(af.read(self.run_path), self.run_path)
        self.assertEqual(af.get_scalar(fm, "status"), "running")
        self.assertEqual(af.get_scalar(fm, "iteration"), "1")
        self.assertEqual(af.get_scalar(fm, "subagents_used"), "2")
        self.assertIn("| iteration 1 | continue | unit complete", body)

    def test_iteration_cap_blocks_continue(self):
        self.init()
        self.make_ready(max_iterations=1)
        self.start()
        self.checkpoint()
        fm = self.run_fm()
        self.assertEqual(af.get_scalar(fm, "status"), "blocked")
        self.assertIn("iteration budget exhausted", af.get_scalar(fm, "blocked_reason"))

    def test_subagent_cap_blocks_run(self):
        self.init()
        self.make_ready(max_subagents=1)
        self.start()
        self.checkpoint(spawned=2)
        fm = self.run_fm()
        self.assertEqual(af.get_scalar(fm, "status"), "blocked")
        self.assertIn("subagent budget exceeded", af.get_scalar(fm, "blocked_reason"))

    def test_blocked_run_requires_deliberate_resume(self):
        self.init()
        self.make_ready(max_iterations=1)
        self.start()
        self.checkpoint()
        with self.assertRaises(SystemExit):
            self.start()

        fm, body = af.split_fm(af.read(self.run_path), self.run_path)
        fm = af.set_scalar(fm, "max_iterations", "2", self.run_path)
        af.write(self.run_path, af.join_fm(fm, body))
        self.start(resume_reason="Operator narrowed the remaining unit")
        self.assertEqual(af.get_scalar(self.run_fm(), "status"), "running")

    def test_review_requires_evidence_and_human_gate(self):
        self.init()
        self.make_ready()
        self.start()
        with self.assertRaises(SystemExit):
            self.checkpoint(outcome="review")
        self.checkpoint(outcome="review", evidence="pytest passed; reviewer approved", spawned=1)
        self.assertEqual(af.get_scalar(self.run_fm(), "status"), "review")

        with self.assertRaises(SystemExit):
            self.quiet(af.cmd_autonomy_finish, types.SimpleNamespace(
                project="auto-test", run_id="build-widget", approved_by="reviewer"))
        self.quiet(af.cmd_autonomy_finish, types.SimpleNamespace(
            project="auto-test", run_id="build-widget", approved_by="operator"))
        fm = self.run_fm()
        self.assertEqual(af.get_scalar(fm, "status"), "complete")
        self.assertEqual(af.get_scalar(fm, "approved_by"), "operator")

    def test_independent_review_gate_allows_reviewer_finish(self):
        self.init(level="unattended")
        self.make_ready(completion_gate="independent-review")
        self.start()
        self.checkpoint(outcome="review", evidence="tests pass; independent reviewer approved", spawned=1)
        self.quiet(af.cmd_autonomy_finish, types.SimpleNamespace(
            project="auto-test", run_id="build-widget", approved_by="reviewer"))
        self.assertEqual(af.get_scalar(self.run_fm(), "status"), "complete")


if __name__ == "__main__":
    unittest.main()
