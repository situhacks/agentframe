import datetime
import os
import tempfile
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


if __name__ == "__main__":
    unittest.main()
