import contextlib
import io
import os
import tempfile
import types
import unittest
from unittest.mock import patch

from system import af


PROJECT_MD = """---
name: Automation Test
slug: automation-test
schema_version: 2026-07-19-v2
created_at: 2026-07-12
domain: project-mgmt
status: active
current_phase: active
flow: open-flow
last_activity: 2026-07-12T10:00:00+00:00
deliverables: {}
---

# Automation Test
"""


class AutomationTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = self._tmp.name
        self.projects = os.path.join(self.root, "workspace", "projects")
        self.cdir = os.path.join(self.projects, "automation-test")
        os.makedirs(self.cdir, exist_ok=True)
        af.write(os.path.join(self.cdir, "project.md"), PROJECT_MD)
        self._root_patch = patch.object(af, "ROOT", self.root)
        self._projects_patch = patch.object(af, "PROJECTS", self.projects)
        self._root_patch.start()
        self._projects_patch.start()
        self.addCleanup(self._root_patch.stop)
        self.addCleanup(self._projects_patch.stop)
        self.addCleanup(self._tmp.cleanup)

    @property
    def contract(self):
        return os.path.join(self.cdir, "automations", "email-intake", "automation.md")

    def quiet(self, fn, args):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return fn(args)

    def init(self):
        self.quiet(af.cmd_automation_init, types.SimpleNamespace(
            project="automation-test", automation_id="email-intake",
            job="Route approved emails into project intake"))

    def project_fm(self):
        return af.split_fm(af.read(os.path.join(self.cdir, "project.md")), "project.md")[0]

    def transition(self, fn, deployment=None):
        args = types.SimpleNamespace(
            project="automation-test", automation_id="email-intake", deployment=deployment)
        self.quiet(fn, args)

    def test_init_creates_optional_tracker_and_project_bundle(self):
        self.init()
        self.assertTrue(os.path.isfile(self.contract))
        fm = self.project_fm()
        self.assertEqual(af.mapping_rows(fm, "automations"), ["email-intake"])
        self.assertEqual(
            af.mapping_row_get(fm, "automations", "email-intake", "file"),
            "automations/email-intake/automation.md",
        )
        self.assertEqual(
            af.mapping_row_get(fm, "automations", "email-intake", "status"), "proposed")
        self.assertEqual(af.automation_issues(self.cdir, fm), [])

    def test_lifecycle_requires_ready_and_deployment(self):
        self.init()
        with self.assertRaises(SystemExit):
            self.transition(af.cmd_automation_activate, deployment="work-email")
        self.transition(af.cmd_automation_ready)
        with self.assertRaises(SystemExit):
            self.transition(af.cmd_automation_activate)
        self.transition(af.cmd_automation_activate, deployment="work-email")
        fm = self.project_fm()
        self.assertEqual(
            af.mapping_row_get(fm, "automations", "email-intake", "status"), "active")
        self.assertEqual(
            af.mapping_row_get(fm, "automations", "email-intake", "deployment_id"),
            "work-email",
        )

    def test_pause_resume_and_retire(self):
        self.init()
        self.transition(af.cmd_automation_ready)
        self.transition(af.cmd_automation_activate, deployment="work-email")
        self.transition(af.cmd_automation_pause)
        self.transition(af.cmd_automation_activate)
        self.transition(af.cmd_automation_retire)
        fm = self.project_fm()
        self.assertEqual(
            af.mapping_row_get(fm, "automations", "email-intake", "status"), "retired")
        with self.assertRaises(SystemExit):
            self.transition(af.cmd_automation_activate)

    def test_doctor_flags_orphan_contract_and_missing_active_deployment(self):
        orphan = os.path.join(self.cdir, "automations", "orphan")
        os.makedirs(orphan)
        af.write(os.path.join(orphan, "automation.md"), "orphan")
        issues = af.automation_issues(self.cdir)
        self.assertTrue(any("no project.md tracker row" in issue for issue in issues))

        self.init()
        fm, body = af.split_fm(af.read(os.path.join(self.cdir, "project.md")), "project.md")
        fm = af.mapping_row_set(fm, "automations", "email-intake", "status", "active")
        af.write(os.path.join(self.cdir, "project.md"), af.join_fm(fm, body))
        issues = af.automation_issues(self.cdir)
        self.assertTrue(any("active status requires deployment_id" in issue for issue in issues))

    def test_managed_run_marker_allows_project_mechanics_but_blocks_terminal_actions(self):
        args = types.SimpleNamespace(automation_cmd="init")
        af.check_mode_gate("automation", args)
        with patch.dict(os.environ, {"AGENTFRAME_MANAGED_RUN": "1"}):
            af.check_mode_gate("version", args)
            af.check_mode_gate("draft", args)
            with self.assertRaises(SystemExit):
                af.check_mode_gate("ready", args)
            with self.assertRaises(SystemExit):
                af.check_mode_gate("automation", args)
            with self.assertRaises(SystemExit):
                af.check_mode_gate("sync-harnesses", args)


if __name__ == "__main__":
    unittest.main()
