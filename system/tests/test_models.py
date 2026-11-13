"""Contract tests for system/models.json, the one place that names the model ids."""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

import bills_watch as bw  # noqa: E402
import loop_sweep as ls  # noqa: E402
import loop_watch as lw  # noqa: E402
import models  # noqa: E402

FILE = ROOT / "system" / "models.json"


class ModelsTests(unittest.TestCase):
    def test_the_file_names_four_models_and_the_module_reads_two(self):
        ids = json.loads(FILE.read_text(encoding="utf-8"))
        self.assertEqual(set(ids), {"haiku", "sonnet", "opus", "fable"})
        self.assertTrue(all(isinstance(v, str) and v.startswith("claude-") for v in ids.values()))
        self.assertEqual(models.HAIKU, ids["haiku"])
        self.assertEqual(models.SONNET, ids["sonnet"])

    def test_the_runners_take_their_ids_from_it(self):
        self.assertEqual(bw.MODEL, models.HAIKU)
        self.assertEqual(lw.MODEL, models.HAIKU)
        self.assertEqual(ls.MODEL, models.SONNET)

    def test_the_weekly_consolidation_reads_sonnet_from_the_file_and_names_no_id(self):
        text = (ROOT / "system" / "tools" / "consolidate-weekly.ps1").read_text(encoding="utf-8")
        self.assertIn("models.json", text)
        self.assertIn("ConvertFrom-Json).sonnet", text)
        self.assertNotIn("claude-sonnet", text)

    def test_no_runner_hardcodes_a_haiku_or_sonnet_id(self):
        for name in ("bills_watch.py", "loop_watch.py", "loop_sweep.py"):
            text = (ROOT / "system" / "tools" / name).read_text(encoding="utf-8")
            self.assertNotRegex(text, r"claude-(haiku|sonnet)-\d", name)


if __name__ == "__main__":
    unittest.main()
