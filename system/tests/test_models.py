"""Contract tests for system/models.json, the one place that names the model ids."""

from __future__ import annotations

import functools
import json
import re
import subprocess
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
ID = re.compile(r"claude-(?:fable|opus|sonnet|haiku)-\d+-\d+")
ALIASES = ("fable", "opus", "sonnet", "haiku")

# Files that name an old id on purpose, as a fixture for what a transcript or a refusal says.
OLD_ID_FIXTURES = {
    "system/af/tests/session_classes.rs",
    "system/af/tests/session_compact.rs",
}
NOT_CHECKED = {"CHANGELOG.md"} | OLD_ID_FIXTURES


@functools.lru_cache(maxsize=1)
def tracked() -> list[str]:
    out = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return [f for f in out.splitlines() if (ROOT / f).is_file()]


def words(rel: str) -> int:
    return len((ROOT / rel).read_text(encoding="utf-8").split())


class ModelsTests(unittest.TestCase):
    def test_the_file_names_four_models_and_the_module_reads_all_four(self):
        ids = json.loads(FILE.read_text(encoding="utf-8"))
        self.assertEqual(set(ids), set(ALIASES))
        self.assertTrue(all(isinstance(v, str) and ID.fullmatch(v) for v in ids.values()))
        self.assertEqual(models.FABLE, ids["fable"])
        self.assertEqual(models.OPUS, ids["opus"])
        self.assertEqual(models.SONNET, ids["sonnet"])
        self.assertEqual(models.HAIKU, ids["haiku"])
        self.assertEqual(models.ALL, (ids["fable"], ids["opus"], ids["sonnet"], ids["haiku"]))

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

    def test_no_python_tool_but_models_py_holds_a_claude_model_id(self):
        tools = [f for f in tracked() if f.startswith("system/tools/") and f.endswith(".py") and f != "system/tools/models.py"]
        self.assertGreater(len(tools), 5)
        for f in tools:
            self.assertNotRegex((ROOT / f).read_text(encoding="utf-8"), ID, f)

    def test_every_claude_model_id_in_a_tracked_file_is_one_the_module_holds(self):
        scanned = 0
        for f in tracked():
            if f in NOT_CHECKED or not (f.startswith(("system/", "library/", ".claude/")) or ("/" not in f and f.endswith(".md"))):
                continue
            try:
                text = (ROOT / f).read_text(encoding="utf-8")
            except (UnicodeDecodeError, OSError):
                continue
            scanned += 1
            for found in ID.findall(text):
                self.assertIn(found, models.ALL, f"{f} names {found}, which system/models.json does not hold")
        self.assertGreater(scanned, 100)
        for f in OLD_ID_FIXTURES:
            self.assertTrue((ROOT / f).is_file(), f"{f} is exempt but gone: drop it from OLD_ID_FIXTURES")


class ModelGuidanceTests(unittest.TestCase):
    def test_the_model_file_holds_every_id_and_alias_a_date_and_stays_short(self):
        rel = "library/schema/models.md"
        text = (ROOT / rel).read_text(encoding="utf-8")
        for model_id in models.ALL:
            self.assertIn(model_id, text)
        for alias in ALIASES:
            self.assertIn(f"`{alias}`", text)
        self.assertRegex(text, r"(?m)^checked: \d{4}-\d{2}-\d{2}$")
        self.assertLessEqual(words(rel), 700)
        self.assertIn("(../models/)", text, "it links the cards")

    def test_agents_md_links_the_model_file_and_lost_the_old_sentence(self):
        text = (ROOT / "AGENTS.md").read_text(encoding="utf-8")
        self.assertIn("(library/schema/models.md)", text)
        self.assertNotIn("Propose Opus for deep writing", text)
        self.assertNotIn("Fable when he asks", text)

    def test_the_brief_and_registry_schemas_point_to_the_model_file(self):
        for rel in ("library/schema/handoff.md", "library/schema/sessions-registry.md"):
            text = (ROOT / rel).read_text(encoding="utf-8")
            self.assertIn("(models.md)", text, rel)
        self.assertNotIn("(`opus` or `sonnet`)", (ROOT / "library/schema/handoff.md").read_text(encoding="utf-8"))

    def test_the_builder_agent_runs_on_sonnet(self):
        text = (ROOT / ".claude/agents/builder.md").read_text(encoding="utf-8")
        head = text.split("---")[1]
        self.assertRegex(head, r"(?m)^model: sonnet\s*$")
        self.assertIn("stronger model", text)

    def test_each_alias_has_one_short_card(self):
        folder = ROOT / "library" / "models"
        self.assertEqual(sorted(p.name for p in folder.iterdir()), sorted(f"{a}.md" for a in ALIASES))
        for alias in ALIASES:
            rel = f"library/models/{alias}.md"
            text = (ROOT / rel).read_text(encoding="utf-8")
            self.assertTrue(text.startswith("# Model card: Claude "), rel)
            self.assertRegex(text, r"(?m)^checked: \d{4}-\d{2}-\d{2}$")
            self.assertLessEqual(words(rel), 180, rel)


if __name__ == "__main__":
    unittest.main()
