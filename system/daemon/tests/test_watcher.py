import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

from system.daemon import watcher


class WatcherTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.workspace = self.root / "AgentFrame"
        (self.workspace / "system" / "daemon").mkdir(parents=True)
        (self.workspace / "AGENTS.md").write_text("# AgentFrame - Operator Mode\n", encoding="utf-8")
        (self.workspace / "system" / "daemon" / "kickoff-prompt.md").write_text(
            "Task {task_file}; result {result_file}; project {project}; automation {automation_id}",
            encoding="utf-8",
        )
        self.fake_body = Path(__file__).with_name("fake_body.py").resolve()
        self.registry_path = self.root / "registry.json"
        self.queues = [self.root / "queue-a", self.root / "queue-b"]
        self.write_registry(timeout=3)
        self.registry = watcher.load_registry(self.registry_path)
        watcher.ensure_queues(self.registry)
        self.addCleanup(self._tmp.cleanup)

    def write_registry(self, timeout=3):
        payload = {
            "schema_version": 1,
            "workspace_root": str(self.workspace),
            "poll_seconds": 1,
            "body_profiles": {
                "fake": {
                    "executable": sys.executable,
                    "args": [str(self.fake_body), "--task-file", "{task_file}",
                             "--result-file", "{result_file}"],
                    "timeout_seconds": timeout,
                }
            },
            "automations": [
                {"id": "deploy-a", "label": "A", "project": "demo",
                 "automation_id": "auto-a", "queue_root": str(self.queues[0]),
                 "body_profile": "fake", "enabled": True},
                {"id": "deploy-b", "label": "B", "project": "demo",
                 "automation_id": "auto-b", "queue_root": str(self.queues[1]),
                 "body_profile": "fake", "enabled": True},
            ],
        }
        self.registry_path.write_text(json.dumps(payload), encoding="utf-8")

    def task(self, queue_index, task_id, requested_at, mode="done"):
        path = self.queues[queue_index] / "inbox" / f"{task_id}.task.json"
        path.write_text(json.dumps({
            "schema_version": 1,
            "id": task_id,
            "requested_at": requested_at,
            "task": "Run the fake task",
            "mode": mode,
        }), encoding="utf-8")
        return path

    def run_once(self):
        kickoff = (self.workspace / "system" / "daemon" / "kickoff-prompt.md").read_text()
        return watcher.run_once(self.registry, kickoff, watcher.now_iso())

    def receipt(self, queue_index, task_id):
        path = self.queues[queue_index] / "outbox" / f"{task_id}.result.json"
        return json.loads(path.read_text(encoding="utf-8"))

    def test_oldest_task_across_queues_runs_first_and_sets_managed_env(self):
        self.task(0, "newer", "2026-07-12T11:00:00-07:00")
        self.task(1, "older", "2026-07-12T10:00:00-07:00")
        result = self.run_once()
        self.assertEqual(result["task_id"], "older")
        self.assertEqual(result["status"], "done")
        self.assertTrue((self.queues[1] / "archive" / "older.task.json").is_file())
        self.assertTrue((self.queues[0] / "inbox" / "newer.task.json").is_file())

    def test_blocked_receipt_is_preserved_as_semantic_outcome(self):
        self.task(0, "needs-human", "2026-07-12T10:00:00-07:00", mode="blocked")
        result = self.run_once()
        self.assertEqual(result["status"], "blocked")
        self.assertEqual(self.receipt(0, "needs-human")["operator_action"], "review")

    def test_missing_receipt_synthesizes_failed(self):
        self.task(0, "missing", "2026-07-12T10:00:00-07:00", mode="no-receipt")
        result = self.run_once()
        self.assertEqual(result["status"], "failed")
        self.assertIn("valid receipt", self.receipt(0, "missing")["summary"])

    def test_timeout_terminates_body_and_synthesizes_failed(self):
        self.write_registry(timeout=1)
        self.registry = watcher.load_registry(self.registry_path)
        watcher.ensure_queues(self.registry)
        self.task(0, "slow", "2026-07-12T10:00:00-07:00", mode="timeout")
        result = self.run_once()
        self.assertEqual(result["status"], "failed")
        self.assertIn("timeout", self.receipt(0, "slow")["summary"])

    def test_restart_fails_stranded_task_closed_without_replay(self):
        source = self.task(0, "stranded", "2026-07-12T10:00:00-07:00")
        processing = self.queues[0] / "processing" / source.name
        os.replace(source, processing)
        watcher.recover_stranded(self.registry)
        self.assertFalse(processing.exists())
        receipt = self.receipt(0, "stranded")
        self.assertEqual(receipt["status"], "failed")
        self.assertIn("not replayed", receipt["summary"])

    def test_invalid_task_is_visible_as_failed_receipt(self):
        path = self.queues[0] / "inbox" / "broken.task.json"
        path.write_text('{"schema_version": 1, "id": "broken"}', encoding="utf-8")
        result = self.run_once()
        self.assertEqual(result["status"], "failed")
        self.assertIn("Invalid task file", self.receipt(0, "broken.task")["summary"])

    def test_restart_replaces_invalid_partial_receipt_with_failed(self):
        source = self.task(0, "partial", "2026-07-12T10:00:00-07:00")
        processing = self.queues[0] / "processing" / source.name
        os.replace(source, processing)
        (self.queues[0] / "outbox" / "partial.result.json").write_text("{", encoding="utf-8")
        watcher.recover_stranded(self.registry)
        receipt = self.receipt(0, "partial")
        self.assertEqual(receipt["status"], "failed")
        self.assertIn("not replayed", receipt["summary"])


if __name__ == "__main__":
    unittest.main()
