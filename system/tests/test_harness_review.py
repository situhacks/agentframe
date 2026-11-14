"""Tests for the harness review: a read-only report of what looks unused or stale.

Every test builds its own temporary git repository; none runs against the real one. `af` is faked
the way the other tool tests fake it, through the entry point's `run_af`.
"""

from __future__ import annotations

import contextlib
import datetime as dt
import io
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

import harness_review as hr  # noqa: E402

TODAY = dt.date(2026, 11, 14)
HEAD = "| name | role | session_id | model | started | last_seen | status | handoff |\r\n|---|---|---|---|---|---|---|---|\r\n"


def ago(days: int) -> str:
    return (TODAY - dt.timedelta(days=days)).isoformat()


def row(name, role="builder", status="idle"):
    return f"| {name} | {role} | 0a1b2c3d | opus | 2026-10-01T09:00 | 2026-10-02T09:00 | {status} | system/sessions/{name}/handoff.md |\r\n"


class FakeAf:
    """`af session tidy` with canned output, and `af open add` writing the loop line into open.md."""

    def __init__(self, root: Path, tidy="", tidy_code=0, add_code=0):
        self.root, self.tidy, self.tidy_code, self.add_code, self.calls = root, tidy, tidy_code, add_code, []

    def __call__(self, args, root):
        self.calls.append(list(args))
        if args[:2] == ["session", "tidy"]:
            return self.tidy_code, self.tidy, ""
        if args[:2] == ["open", "add"]:
            if self.add_code:
                return self.add_code, "", "refused"
            frm = args[args.index("--from") + 1]
            page = self.root / "workspace/life/open.md"
            page.parent.mkdir(parents=True, exist_ok=True)
            old = page.read_text(encoding="utf-8") if page.exists() else "# Open loops\n\n"
            page.write_text(f"{old}- [ ] {TODAY} · {args[2]} · nudge {TODAY} · from: {frm}\n", encoding="utf-8")
            return 0, "", ""
        return 1, "", "unknown"

    def adds(self):
        return [c for c in self.calls if c[:2] == ["open", "add"]]


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.root = self.base / "repo"
        self.root.mkdir()
        self.git("init", "-b", "master")
        self.put("AGENTS.md", "main router words\n")
        self.put("README.md", "readme\n")
        self.put(".gitignore", "system/logs/\nsystem/sessions/\nworkspace/\n")
        self.fake = FakeAf(self.root)
        self.scratch = self.base / "scratch" / "af-*"

    def git(self, *args, cwd=None):
        r = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@example.test", *args], cwd=cwd or self.root,
                           capture_output=True, text=True, encoding="utf-8")
        self.assertEqual(r.returncode, 0, r.stderr)
        return r.stdout

    def put(self, rel, text, tracked=True):
        p = self.root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8", newline="")
        return p

    def commit(self, msg="c"):
        self.git("add", "-A")
        self.git("commit", "-m", msg, "--allow-empty")

    def registry(self, *rows):
        self.put("system/sessions/registry.md", HEAD + "".join(rows))

    def run_review(self, *argv, today=TODAY, fake=None):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = hr.main(["--root", str(self.root), *argv], today=today, run_af=fake or self.fake,
                           scratch_glob=str(self.scratch))
        self.out, self.err = out.getvalue(), err.getvalue()
        return code

    def report(self, day=TODAY) -> str:
        return (self.root / f"system/logs/harness-review-{day.isoformat()}.md").read_text(encoding="utf-8")

    def section(self, text, n) -> str:
        m = re.search(rf"## {n}\. .*?(?=\n## \d+\. |\Z)", text, re.S)
        self.assertTrue(m, f"no section {n}")
        return m.group(0)


class GitTests(Base):
    def test_a_merged_branch_and_a_clean_merged_worktree_are_listed_and_an_unmerged_branch_is_not_a_candidate(self):
        self.commit("first")
        self.git("branch", "merged-one")
        self.git("checkout", "-b", "unmerged-two")
        self.put("x.md", "x\n")
        self.commit("work")
        self.git("checkout", "master")
        self.git("worktree", "add", str(self.base / "wt-clean"), "-b", "merged-wt")
        self.git("worktree", "add", str(self.base / "wt-dirty"), "-b", "merged-dirty")
        (self.base / "wt-dirty" / "edit.txt").write_text("unsaved\n", encoding="utf-8")
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 3)
        head, _, rest = sec.partition("**Not merged")
        self.assertIn("merged-one", head)
        self.assertNotIn("unmerged-two", head)
        self.assertIn("unmerged-two (last commit", rest)
        wt_part = head.split("**Worktrees")[1]
        self.assertIn("wt-clean", wt_part)
        self.assertNotIn("wt-dirty", wt_part, "a worktree with changes is not listed")
        self.assertNotRegex(head.split("**Worktrees")[0].replace("merged-one", ""), r"- master\b")


class SkillToolProcessTests(Base):
    def test_a_skill_with_no_mention_outside_its_folder_is_listed(self):
        self.put(".claude/skills/orphan-skill/SKILL.md", "---\nname: orphan-skill\n---\nBody of the skill here.\n")
        self.put(".claude/skills/used-skill/SKILL.md", "---\nname: used-skill\n---\nBody.\n")
        self.put(".claude/skills/README.md", "| orphan-skill | used-skill |\n")
        self.put("README.md", "orphan-skill is only in the top readme\n")
        self.put("AGENTS.md", "Load used-skill when asked.\n")
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 5)
        listed = sec.split("| Skill")[0]
        self.assertIn("orphan-skill", listed)
        self.assertNotIn("used-skill", listed)
        self.assertRegex(sec, r"\| orphan-skill \| 5 \|", "body words, frontmatter left out")

    def test_a_tool_named_only_by_itself_and_its_own_test_is_listed(self):
        self.put("system/tools/lonely_tool.py", "# lonely_tool.py\n")
        self.put("system/tests/test_lonely_tool.py", "import lonely_tool\n")
        self.put("system/tools/used-tool.ps1", "# used\n")
        self.put("system/tools/other_tool.py", "print(1)\n")
        self.put("library/process/route.md", "Run `other_tool.py` and then used-tool.ps1.\n")
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 6)
        self.assertIn("lonely_tool.py", sec)
        self.assertNotIn("other_tool.py", sec)
        self.assertNotIn("used-tool.ps1", sec)

    def test_a_process_file_named_only_by_the_catalog_is_listed(self):
        self.put("library/process/README.md", "| lone-process.md | x |\n| shared-process.md | y |\n")
        self.put("library/process/lone-process.md", "text\n")
        self.put("library/process/shared-process.md", "text\n")
        self.put("AGENTS.md", "See library/process/shared-process.md.\n")
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 7)
        self.assertIn("lone-process.md", sec)
        self.assertNotIn("shared-process.md", sec)


class SessionTests(Base):
    def test_old_ended_rows_and_folders_without_a_row_are_listed(self):
        self.registry(row("system/old", status=f"ended {ago(20)}T10:00 exit"), row("system/fresh", status=f"ended {ago(3)}T10:00 exit"),
                      row("system/gone"))
        for n in ("system/old", "system/fresh"):
            self.put(f"system/sessions/{n}/handoff.md", "page\n")
        self.put("system/sessions/system/stray/brief.md", "brief\n")
        self.put("system/sessions/af-main/handoff.md", "main page\n")
        self.put("system/sessions/system/empty/notes.txt", "no brief or page\n")
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 4)
        old, _, rest = sec.partition("**A folder")
        self.assertIn("system/old", old)
        self.assertNotIn("system/fresh", old)
        orphans, _, missing = rest.partition("**A registry row")
        self.assertIn("system/stray", orphans)
        self.assertIn("af-main", orphans, "a top-level name with a page and no row")
        self.assertNotIn("system/old", orphans)
        self.assertNotIn("system/empty", orphans, "no brief or page")
        self.assertIn("system/gone", missing)


class WordsTests(Base):
    def test_the_table_counts_a_chats_brief_and_page_and_flags_a_total_over_3500(self):
        self.put("AGENTS.md", "word " * 100)
        self.put("AGENTS.builder.md", "word " * 200)
        self.put("workspace/life/life.md", "word " * 50)
        self.registry(row("system/big"), row("system/small"), row("af-main", role="main"), row("system/done", status=f"ended {ago(1)}T10:00 exit"))
        self.put("system/sessions/system/big/brief.md", "word " * 1000)
        self.put("system/sessions/system/big/handoff.md", "word " * 2300)
        self.put("system/sessions/system/small/brief.md", "word " * 10)
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 1)
        self.assertIn("| AGENTS.md | 100 |", sec)
        self.assertIn("| AGENTS.operator.md | 0 |", sec, "a missing file counts 0")
        self.assertRegex(sec, r"\| system/big \| builder \| 300 \| 1000 \| 2300 \| 3600 \| over 3500 \|")
        self.assertRegex(sec, r"\| system/small \| builder \| 300 \| 10 \| 0 \| 310 \|  \|")
        self.assertRegex(sec, r"\| af-main \| main \| 150 \| 0 \| 0 \| 150 \|", "main adds life.md and no role router")
        self.assertNotIn("system/done", sec.split("| Chat")[1], "an ended row loads nothing")
        self.assertIn("(1)", sec.splitlines()[0], "one chat flagged")


class DueTests(Base):
    def test_not_due_within_14_days_and_force_writes_anyway(self):
        self.commit()
        self.assertEqual(self.run_review(today=TODAY), 0, self.err)
        first = self.report()
        self.assertIn("wrote", self.out)
        code = self.run_review(today=TODAY + dt.timedelta(days=13))
        self.assertEqual(code, 0)
        self.assertIn("not due", self.out)
        self.assertEqual(len(self.out.strip().splitlines()), 1)
        self.assertFalse((self.root / f"system/logs/harness-review-{TODAY + dt.timedelta(days=13)}.md").exists())
        code = self.run_review("--force", today=TODAY + dt.timedelta(days=13))
        self.assertEqual(code, 0)
        self.assertTrue((self.root / f"system/logs/harness-review-{TODAY + dt.timedelta(days=13)}.md").exists())
        self.assertEqual(self.run_review(today=TODAY + dt.timedelta(days=26)), 0)
        self.assertIn("not due", self.out, "the forced report is the last one: 13 days on")
        self.assertEqual(self.run_review(today=TODAY + dt.timedelta(days=27)), 0)
        self.assertIn("wrote", self.out, "14 days on it is due again")
        self.assertIn("Harness review", first.splitlines()[0])

    def test_out_writes_the_report_where_asked(self):
        self.commit()
        target = self.base / "elsewhere" / "r.md"
        self.assertEqual(self.run_review("--out", str(target)), 0, self.err)
        self.assertTrue(target.is_file())


class BacklogScratchModelsTests(Base):
    def test_a_backlog_row_surfaced_40_days_ago_is_listed_and_one_surfaced_5_days_ago_is_not(self):
        self.put("system/backlog.md", "# Backlog\r\n\r\n"
                 f"## BB-{ago(40)}-01 · old friction\r\nSurfaced: main, {ago(40)}, a log\r\nFix: x\r\n\r\n"
                 f"## BB-{ago(5)}-01 · new friction\r\nSurfaced: main, {ago(5)}, a log\r\nFix: y\r\n\r\n"
                 f"## BB-{ago(2)}-01 · dated by surfaced\r\nSurfaced: main, {ago(60)}, an old find\r\nFix: z\r\n")
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 8)
        self.assertIn("3 rows; 2 surfaced", sec)
        self.assertIn("old friction", sec)
        self.assertNotIn("new friction", sec)
        self.assertIn("dated by surfaced", sec)

    def test_scratch_entries_not_modified_for_14_days_are_counted(self):
        self.commit()
        d = self.scratch.parent
        d.mkdir(parents=True)
        old, new = d / "af-old", d / "af-new"
        old.mkdir()
        new.mkdir()
        t = (dt.datetime.combine(TODAY, dt.time(12)) - dt.timedelta(days=20)).timestamp()
        os.utime(old, (t, t))
        t2 = (dt.datetime.combine(TODAY, dt.time(12)) - dt.timedelta(days=1)).timestamp()
        os.utime(new, (t2, t2))
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 9)
        self.assertIn("2 entries; 1 not changed", sec)
        self.assertIn("af-old", sec)
        self.assertNotIn("af-new", sec)

    def test_scratch_is_skipped_silently_when_its_folder_is_absent_and_models_when_the_file_is(self):
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        text = self.report()
        self.assertNotIn("Scratch", text)
        self.assertNotIn("Model facts", text)

    def test_model_facts_older_than_60_days_are_said_to_be_old(self):
        self.put("library/schema/models.md", f"---\nchecked: {ago(61)}\n---\ntext\n")
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        self.assertIn("reads Anthropic's model pages again", self.section(self.report(), 10))


class PagesAndLoopTests(Base):
    TIDY = ("system/sessions/system/a/handoff.md: 120 of 1200 words\n  clean\n"
            "system/sessions/system/b/handoff.md: 300 of 1200 words\n  [fix] Done: 12 items, more than 10\n  [review] Rules: stale\n")

    def test_only_pages_with_issues_show(self):
        self.commit()
        self.fake.tidy, self.fake.tidy_code = self.TIDY, 1
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 2)
        self.assertIn("(2)", sec.splitlines()[0])
        self.assertIn("system/b/handoff.md", sec)
        self.assertNotIn("system/a/handoff.md", sec)
        self.assertEqual(self.fake.calls[0], ["session", "tidy", "--all", "--check"])

    def test_a_clean_page_list_says_nothing(self):
        self.commit()
        self.fake.tidy = "system/sessions/system/a/handoff.md: 120 of 1200 words\n  clean\n"
        self.assertEqual(self.run_review(), 0, self.err)
        self.assertTrue(self.section(self.report(), 2).rstrip().endswith("nothing"))

    def test_no_af_on_the_path_is_said_and_the_review_goes_on(self):
        self.commit()
        self.fake.tidy_code = 127
        self.assertEqual(self.run_review(), 0, self.err)
        self.assertIn("af is not on the path", self.section(self.report(), 2))
        self.assertIn("## 3.", self.report())

    def test_one_loop_is_raised_and_not_a_second_while_one_is_open(self):
        self.commit()
        self.git("branch", "merged-one")
        self.assertEqual(self.run_review(), 0, self.err)
        adds = self.fake.adds()
        self.assertEqual(len(adds), 1)
        self.assertEqual(adds[0][adds[0].index("--from") + 1], "harness-review")
        self.assertRegex(adds[0][2], r"^Harness review: 1 items to look at \(system/logs/harness-review-2026-11-14\.md\); dispatch a builder chat for what should go$")
        self.assertEqual(self.run_review("--force", today=TODAY + dt.timedelta(days=15)), 0)
        self.assertEqual(len(self.fake.adds()), 1, "the open loop stands; no second one")
        page = self.root / "workspace/life/open.md"
        page.write_text("# Open loops\n", encoding="utf-8")
        self.assertEqual(self.run_review("--force", today=TODAY + dt.timedelta(days=30)), 0)
        self.assertEqual(len(self.fake.adds()), 2, "once it is closed a new report may raise another")

    def test_no_loop_when_nothing_is_worth_a_look(self):
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        self.assertEqual(self.fake.adds(), [])
        self.assertIn("0 items worth a look", self.report().splitlines()[0])

    def test_a_loop_that_cannot_be_raised_is_an_error_and_leaves_the_review_due(self):
        self.commit()
        self.git("branch", "merged-one")
        self.fake.add_code = 1
        self.assertEqual(self.run_review(), 1)
        self.assertEqual(self.run_review(fake=FakeAf(self.root)), 0)
        self.assertIn("wrote", self.out, "not saved as done, so the next run tries again")

    def test_an_error_in_git_is_a_non_zero_exit(self):
        self.root.joinpath(".git").rename(self.base / "gone")
        self.assertEqual(self.run_review(), 2)


class SafetyAndSizeTests(Base):
    def snapshot(self):
        out = {}
        for p in sorted(self.root.rglob("*")):
            rel = p.relative_to(self.root).as_posix()
            if rel.startswith(".git/") or rel == ".git" or rel == "system/logs" or rel.startswith("workspace") or rel.startswith("system/logs/harness-review"):
                continue
            out[rel] = p.read_bytes() if p.is_file() else None
        return out

    def test_it_writes_nothing_outside_its_report_and_state(self):
        self.put(".claude/skills/orphan-skill/SKILL.md", "---\nname: orphan-skill\n---\nBody\n")
        self.put("system/tools/lonely_tool.py", "x = 1\n")
        self.put("library/process/README.md", "catalog\n")
        self.put("library/process/lone.md", "x\n")
        self.put("system/backlog.md", f"## BB-{ago(90)}-01 · old\nSurfaced: main, {ago(90)}\n")
        self.registry(row("system/old", status=f"ended {ago(40)}T10:00 exit"))
        self.put("system/sessions/system/old/handoff.md", "page\n")
        self.commit()
        self.git("branch", "merged-one")
        before = self.snapshot()
        listing = self.git("ls-files")
        self.assertEqual(self.run_review(), 0, self.err)
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(len(self.fake.adds()), 1, "the only other write is af's own loop, through the verb")
        self.assertEqual(self.git("ls-files"), listing)
        self.assertEqual(self.git("status", "--porcelain").strip(), "", "the logs folder is git-ignored; nothing else changed")
        written = {p.name for p in (self.root / "system/logs").iterdir()}
        self.assertEqual(written, {"harness-review-2026-11-14.md", "harness-review-state.json"})
        self.assertEqual(json.loads((self.root / hr.STATE).read_text(encoding="utf-8"))["date"], "2026-11-14")

    def test_the_report_stays_short_when_every_list_is_long(self):
        for i in range(40):
            self.put(f"library/process/p{i:02}.md", "x\n")
            self.put(f".claude/skills/s{i:02}/SKILL.md", "---\nname: s\n---\nb\n")
            self.put(f"system/tools/t{i:02}_tool.py", "x = 1\n")
        self.put("library/process/README.md", "catalog\n")
        self.commit()
        for i in range(40):
            self.git("branch", f"merged-{i:02}")
        self.assertEqual(self.run_review(), 0, self.err)
        text = self.report()
        self.assertIn("and 25 more", text)
        self.assertLess(len(text.split()), 1500)


class RepositoryDocTests(unittest.TestCase):
    """The four stale lines, and the cap the first one states."""

    def text(self, rel):
        return (ROOT / rel).read_text(encoding="utf-8")

    def test_the_authoring_doc_states_the_cap_the_doctor_enforces(self):
        budget = int(re.search(r"PERSONA_WORD_BUDGET: usize = (\d+)", self.text("system/af/src/doctor.rs")).group(1))
        self.assertIn(f"{budget:,} words for `AGENTS.builder.md` and `AGENTS.operator.md`", self.text("library/process/authoring.md"))

    def test_the_builder_agent_asks_for_no_isolation_and_says_the_caller_makes_the_worktree(self):
        text = self.text(".claude/agents/builder.md")
        front = text.split("---")[1]
        self.assertNotIn("isolation", front)
        self.assertIn("The caller makes your worktree from local master", text)

    def test_the_skill_table_lives_only_in_the_skills_readme(self):
        readme = self.text("README.md")
        self.assertNotIn("| Skill |", readme)
        self.assertIn("](.claude/skills/README.md)", readme)
        self.assertIn("| Skill |", self.text(".claude/skills/README.md"))

    def test_the_readme_describes_what_settings_json_holds(self):
        settings = json.loads(self.text(".claude/settings.json"))
        self.assertNotIn("compaction window", self.text("README.md"))
        self.assertTrue({"hooks", "permissions", "worktree"} <= set(settings))
        self.assertIn("settings.json         # hook wiring, the push confirmation rule, worktree setting", self.text("README.md"))

    def test_the_process_catalog_names_the_review_and_where_its_report_lands(self):
        text = self.text("library/process/README.md")
        self.assertIn("system/tools/harness_review.py", text)
        self.assertIn("system/logs/harness-review-<date>.md", text)
        self.assertLessEqual(len(text.split()), 1500)

    def test_the_batch_run_starts_the_review_last_among_the_morning_steps(self):
        import watch_batch as wb
        self.assertEqual(wb.MORNING[-1], ("harness-review", "harness_review.py", 2))
        self.assertTrue((ROOT / "system/tools/harness_review.py").is_file())


if __name__ == "__main__":
    unittest.main()
