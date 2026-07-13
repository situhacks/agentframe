import datetime as dt
import json
import os
import tempfile
import unittest
from pathlib import Path

from system.server.lib.surface import automations


PROJECT = """---
name: Demo Project
slug: demo
schema_version: 2026-04-23
created_at: 2026-07-12
domain: project-mgmt
status: active
current_phase: active
flow: open-flow
last_activity: 2026-07-12T10:00:00-07:00
deliverables: {{}}
automations:
  email-intake:
    status: {status}
    file: automations/email-intake/automation.md
    deployment_id: {deployment}
    last_updated: 2026-07-12T10:00:00-07:00
    job: Route approved emails
---
# Demo
"""


class SurfaceAutomationTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.project = self.root / "workspace" / "projects" / "demo"
        self.project.mkdir(parents=True)
        self.local = self.root / "system" / "daemon" / "local"
        self.local.mkdir(parents=True)
        self.queue = self.root / "queue"
        for name in ("inbox", "processing", "archive", "outbox", "logs"):
            (self.queue / name).mkdir(parents=True)
        self.now = dt.datetime(2026, 7, 12, 15, 0, tzinfo=dt.timezone(dt.timedelta(hours=-7)))
        self.addCleanup(self._tmp.cleanup)

    def write_project(self, status="active", deployment="work-email"):
        (self.project / "project.md").write_text(
            PROJECT.format(status=status, deployment=deployment), encoding="utf-8")

    def write_runtime(self, heartbeat=None, include=True, current=None):
        registry = {
            "schema_version": 1,
            "poll_seconds": 5,
            "automations": ([{
                "id": "work-email",
                "label": "Work email",
                "project": "demo",
                "automation_id": "email-intake",
                "queue_root": str(self.queue),
                "enabled": True,
            }] if include else []),
        }
        (self.local / "registry.json").write_text(json.dumps(registry), encoding="utf-8")
        (self.local / "status.json").write_text(json.dumps({
            "schema_version": 1,
            "state": "busy" if current else "idle",
            "heartbeat_at": heartbeat or self.now.isoformat(),
            "current": current,
        }), encoding="utf-8")

    def test_declared_ready_without_registry_surfaces_not_deployed(self):
        self.write_project(status="ready", deployment="null")
        model = automations.build_model(self.root, self.now)
        row = model["rows"][0]
        self.assertEqual(row["runtime_state"], "not-deployed")
        self.assertEqual(row["issues"], ["ready-not-deployed"])

    def test_active_runtime_joins_queue_today_and_current_task(self):
        self.write_project()
        self.write_runtime(current={"deployment_id": "work-email", "task_id": "task-2"})
        (self.queue / "inbox" / "task-3.task.json").write_text("{}", encoding="utf-8")
        receipt = self.queue / "outbox" / "task-1.result.json"
        receipt.write_text(json.dumps({
            "schema_version": 1, "task_id": "task-1", "status": "done",
            "summary": "completed", "outputs": [], "operator_action": None,
        }), encoding="utf-8")
        timestamp = self.now.timestamp()
        os.utime(receipt, (timestamp, timestamp))
        model = automations.build_model(self.root, self.now)
        row = model["rows"][0]
        self.assertEqual(row["runtime_state"], "busy")
        self.assertEqual(row["queued"], 1)
        self.assertEqual(row["current_task"], "task-2")
        self.assertEqual(row["today"]["done"], 1)
        self.assertEqual(row["issues"], [])

    def test_stale_heartbeat_surfaces_active_offline(self):
        self.write_project()
        stale = (self.now - dt.timedelta(minutes=10)).isoformat()
        self.write_runtime(heartbeat=stale)
        row = automations.build_model(self.root, self.now)["rows"][0]
        self.assertEqual(row["runtime_state"], "offline")
        self.assertEqual(row["issues"], ["active-offline"])

    def test_registry_without_project_declaration_is_runtime_orphan(self):
        self.write_runtime()
        row = automations.build_model(self.root, self.now)["rows"][0]
        self.assertEqual(row["desired_status"], "undeclared")
        self.assertEqual(row["issues"], ["runtime-orphan"])

    def test_paused_automation_with_queued_task_is_attention(self):
        self.write_project(status="paused")
        self.write_runtime()
        (self.queue / "inbox" / "waiting.task.json").write_text("{}", encoding="utf-8")
        row = automations.build_model(self.root, self.now)["rows"][0]
        self.assertEqual(row["issues"], ["paused-with-queue"])


if __name__ == "__main__":
    unittest.main()
