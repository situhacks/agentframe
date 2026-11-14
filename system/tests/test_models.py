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
CARDS = ("fable", "opus")   # cards are for the big models only

# (file, id) pairs that name an old id on purpose, as a fixture for what a transcript or a refusal says.
# Any other stale id in those files still fails. The ids are split here so this file names none.
OLD_ID_FIXTURES = {
    ("system/af/tests/session_classes.rs", "claude-fable-" + "5-1"),
    ("system/af/tests/session_compact.rs", "claude-haiku-" + "4-5"),
}
NOT_CHECKED = {"CHANGELOG.md"}


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
                if (f, found) in OLD_ID_FIXTURES:
                    continue
                self.assertIn(found, models.ALL, f"{f} names {found}, which system/models.json does not hold")
        self.assertGreater(scanned, 100)
        for f, old in OLD_ID_FIXTURES:
            self.assertIn(old, (ROOT / f).read_text(encoding="utf-8"), f"{f} no longer names {old}: drop it from OLD_ID_FIXTURES")


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

    def test_only_the_big_models_have_a_short_card(self):
        folder = ROOT / "library" / "models"
        self.assertEqual(sorted(p.name for p in folder.iterdir()), sorted(f"{a}.md" for a in CARDS))
        for alias in CARDS:
            rel = f"library/models/{alias}.md"
            text = (ROOT / rel).read_text(encoding="utf-8")
            self.assertTrue(text.startswith("# Model card: Claude "), rel)
            self.assertRegex(text, r"(?m)^checked: \d{4}-\d{2}-\d{2}$")
            self.assertLessEqual(words(rel), 180, rel)


class ReviewFixTests(unittest.TestCase):
    def read(self, rel: str) -> str:
        return (ROOT / rel).read_text(encoding="utf-8")

    def test_the_two_skills_keep_their_fixed_job_models_and_link_the_rule(self):
        mail = self.read(".claude/skills/email-cleanup/SKILL.md")
        self.assertIn("Hand the browser clicking to a Sonnet subagent", mail)
        self.assertIn("library/schema/models.md", mail)
        research = self.read(".claude/skills/deep-research/SKILL.md")
        self.assertIn("sonnet; haiku for `quick` tier", research)
        self.assertIn("library/schema/models.md", research)

    def test_the_desk_agent_lists_every_picker_name(self):
        desk = self.read(".claude/agents/desk.md")
        for name in ("Fable 5.2", "Opus 5.5", "Sonnet 5.5", "Haiku 5.5"):
            self.assertIn(f"`{name}`", desk)

    def test_the_vendor_record_has_no_doubled_words(self):
        text = self.read("system/tools/agy_call.VENDOR.md")
        self.assertNotIn("fence last fence", text)

    def test_the_card_sentence_does_not_say_once(self):
        self.assertNotIn("once, at the start", self.read("library/schema/models.md"))

    def test_the_schema_index_row_for_models_has_the_link_alone_in_its_first_column(self):
        rows = [r for r in self.read("library/schema/README.md").splitlines() if "(models.md)" in r]
        self.assertEqual(len(rows), 1)
        cells = [c.strip() for c in rows[0].strip().strip("|").split("|")]
        self.assertEqual(cells[0], "[`models.md`](models.md)")
        self.assertEqual(len(cells), 3)


if __name__ == "__main__":
    unittest.main()
