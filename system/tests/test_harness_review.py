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
from unittest import mock

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
        self.git("config", "core.quotepath", "true")
        self.put("AGENTS.md", "main router words\n")
        self.put("README.md", "readme\n")
        self.put(".gitignore", "system/logs/\nsystem/sessions/\nworkspace/\n")
        self.fake = FakeAf(self.root)
        self.scratch = self.base / "scratch" / "af-*"

    def git(self, *args, cwd=None, env=None):
        r = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@example.test", *args], cwd=cwd or self.root,
                           capture_output=True, text=True, encoding="utf-8", env={**os.environ, **(env or {})})
        self.assertEqual(r.returncode, 0, r.stderr)
        return r.stdout

    def put(self, rel, text, tracked=True):
        p = self.root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8", newline="")
        return p

    def commit(self, msg="c", days_ago=None):
        stamp = f"{ago(days_ago)}T12:00:00" if days_ago is not None else None
        self.git("add", "-A")
        self.git("commit", "-m", msg, "--allow-empty",
                 env={"GIT_AUTHOR_DATE": stamp, "GIT_COMMITTER_DATE": stamp} if stamp else None)

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
    def blocks(self):
        """The four lists of the git section, in order: free branches, branches in use, removable worktrees, unmerged."""
        sec = self.section(self.report(), 3)
        parts = re.split(r"\n(?=\*\*)", sec.split("\n", 2)[2])
        self.assertEqual(len(parts), 4, sec)
        return parts

    def test_a_merged_branch_is_listed_and_an_unmerged_branch_is_not(self):
        self.commit("first", days_ago=20)
        self.git("branch", "merged-one")
        self.git("checkout", "-b", "unmerged-two")
        self.put("x.md", "x\n")
        self.commit("work")
        self.git("checkout", "master")
        self.assertEqual(self.run_review(), 0, self.err)
        free, busy, removable, unmerged = self.blocks()
        self.assertTrue(free.startswith("**Merged into master (no commits of its own) (1)**"), free)
        self.assertIn("merged-one", free)
        self.assertNotIn("unmerged-two", free)
        self.assertNotRegex(free.replace("merged-one", ""), r"- master\b")
        self.assertIn("unmerged-two (last commit", unmerged)

    def test_a_branch_checked_out_in_a_worktree_is_never_listed_as_free_and_neither_is_its_worktree_while_fresh(self):
        self.commit("first", days_ago=20)
        self.git("worktree", "add", str(self.base / "wt-busy"), "-b", "merged-busy")
        self.commit("second", days_ago=2)
        self.git("worktree", "add", str(self.base / "wt-fresh"), "-b", "merged-fresh")
        self.assertEqual(self.run_review(), 0, self.err)
        free, busy, removable, _ = self.blocks()
        self.assertNotIn("merged-busy", free)
        self.assertNotIn("merged-fresh", free)
        self.assertRegex(busy, r"merged-busy \(in use by worktree .*wt-busy\)")
        self.assertRegex(busy, r"merged-fresh \(in use by worktree .*wt-fresh\)")
        self.assertIn("wt-busy", removable, "merged, clean, its last commit 20 days old")
        self.assertNotIn("wt-fresh", removable, "its last commit is 2 days old: still in use")

    def test_a_worktree_is_removable_only_when_clean_and_its_last_commit_is_over_7_days_old(self):
        self.commit("old", days_ago=20)
        self.git("worktree", "add", str(self.base / "wt-clean"), "-b", "b-clean")
        self.git("worktree", "add", str(self.base / "wt-dirty"), "-b", "b-dirty")
        (self.base / "wt-dirty" / "edit.txt").write_text("unsaved\n", encoding="utf-8")
        self.git("worktree", "add", str(self.base / "wt-clean-too"), "-b", "b-clean-too")
        self.assertEqual(self.run_review(), 0, self.err)
        removable = self.blocks()[2]
        self.assertIn("wt-clean", removable)
        self.assertNotIn("wt-dirty", removable, "a worktree with changes is not listed")
        self.assertIn("(2)", removable.splitlines()[0])

    def test_a_worktree_whose_last_commit_is_exactly_7_days_old_is_not_removable_and_8_days_is(self):
        self.commit("eight", days_ago=8)
        self.git("worktree", "add", str(self.base / "wt-eight"), "-b", "b-eight")
        self.commit("seven", days_ago=7)
        self.git("worktree", "add", str(self.base / "wt-seven"), "-b", "b-seven")
        self.assertEqual(self.run_review(), 0, self.err)
        removable = self.blocks()[2]
        self.assertIn("wt-eight", removable)
        self.assertNotIn("wt-seven", removable)

    def test_a_detached_head_is_not_a_branch(self):
        self.commit("first", days_ago=20)
        self.git("checkout", "--detach")
        self.assertEqual(self.run_review(), 0, self.err)
        text = self.report()
        self.assertNotIn("(HEAD", text)
        self.assertNotIn("detached", text)
        self.assertNotIn("- at\n", text)

    def test_every_git_call_runs_with_optional_locks_off(self):
        self.commit("first", days_ago=20)
        self.git("worktree", "add", str(self.base / "wt-a"), "-b", "b-a")
        seen, real = [], subprocess.run

        def spy(cmd, *a, **kw):
            seen.append((list(cmd), kw.get("env") or {}))
            return real(cmd, *a, **kw)

        with mock.patch.object(hr.subprocess, "run", spy):
            self.assertEqual(self.run_review(), 0, self.err)
        calls = [c for c in seen if c[0][0] == "git"]
        self.assertGreaterEqual(len(calls), 5)
        for cmd, env in calls:
            self.assertEqual(cmd[1], "--no-optional-locks", cmd)
            self.assertEqual(env.get("GIT_OPTIONAL_LOCKS"), "0", cmd)

    def test_a_path_with_non_ascii_characters_is_read(self):
        self.put(".claude/skills/solo-skill/SKILL.md", "---\nname: solo-skill\n---\nBody\n")
        self.put("notes/caf\u00e9 notes.md", "Load solo-skill when asked.\n")
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        self.assertNotIn("solo-skill", self.section(self.report(), 5).split("| Skill")[0])


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


class RegistryShapeTests(Base):
    def test_a_table_with_unexpected_columns_gets_one_line_not_a_list_of_every_folder(self):
        self.put("system/sessions/registry.md", "| name | role | extra | session_id | model | started | last_seen | status | handoff |\n"
                 "|---|---|---|---|---|---|---|---|---|\n| system/a | builder | x | 1 | opus | s | l | idle | h |\n")
        self.put("system/sessions/system/a/handoff.md", "page\n")
        self.put("system/sessions/system/b/brief.md", "brief\n")
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 4)
        self.assertIn("(0)", sec.splitlines()[0])
        self.assertIn("holds a table but no row could be read from it (unexpected columns)", sec)
        self.assertNotIn("system/a", sec)
        self.assertNotIn("system/b", sec)

    def test_a_registry_with_only_its_header_is_a_normal_empty_one(self):
        self.registry()
        self.put("system/sessions/system/b/brief.md", "brief\n")
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 4)
        self.assertNotIn("no row could be read", sec)
        self.assertIn("system/b", sec)


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

    def reports(self):
        return sorted((self.root / "system/logs").glob("harness-review-2*.md"))

    def state(self):
        return json.loads((self.root / hr.STATE).read_text(encoding="utf-8"))

    def test_a_failed_loop_is_retried_without_writing_a_new_report(self):
        self.commit()
        self.git("branch", "merged-one")
        bad = FakeAf(self.root, add_code=1)
        self.assertEqual(self.run_review(fake=bad), 1)
        self.assertEqual((self.state()["date"], bool(self.state()["loop_pending"])), ("2026-11-14", True))
        self.assertEqual(self.run_review(today=TODAY + dt.timedelta(days=1), fake=bad), 1)
        self.assertEqual(len(self.reports()), 1, "two failing runs, one report")
        self.assertEqual(len(bad.adds()), 2, "the second run only retried the loop")
        good = FakeAf(self.root)
        self.assertEqual(self.run_review(today=TODAY + dt.timedelta(days=2), fake=good), 0, self.err)
        self.assertEqual(len(self.reports()), 1)
        self.assertEqual(len(good.adds()), 1, "the loop is raised once, for the report already written")
        self.assertIn("system/logs/harness-review-2026-11-14.md", good.adds()[0][2])
        self.assertFalse(self.state()["loop_pending"])
        self.assertEqual(self.state()["date"], "2026-11-14", "the period still counts from the report")
        self.assertEqual(self.run_review(today=TODAY + dt.timedelta(days=3), fake=good), 0)
        self.assertIn("not due", self.out)
        self.assertEqual(len(good.adds()), 1)

    def test_an_af_missing_from_the_path_is_a_failure_to_retry_and_never_a_crash(self):
        self.commit()
        self.git("branch", "merged-one")

        def missing(args, root):
            raise FileNotFoundError("af")

        self.assertEqual(self.run_review(fake=missing), 1)
        self.assertTrue(self.state()["loop_pending"])
        self.assertEqual(self.run_review(today=TODAY + dt.timedelta(days=1), fake=missing), 1)
        self.assertEqual(len(self.reports()), 1)
        self.assertEqual(self.run_review(today=TODAY + dt.timedelta(days=1), fake=FakeAf(self.root)), 0)
        self.assertFalse(self.state()["loop_pending"])

    def test_the_real_helper_reports_a_missing_af_as_exit_127(self):
        with mock.patch.object(hr.subprocess, "run", side_effect=FileNotFoundError("af")):
            self.assertEqual(hr.af(["session", "tidy"], self.root)[0], 127)
        with mock.patch.object(hr.subprocess, "run", side_effect=PermissionError("denied")):
            self.assertEqual(hr.af(["session", "tidy"], self.root)[0], 127)

    def test_a_pending_loop_already_raised_by_hand_clears_the_flag(self):
        self.commit()
        self.git("branch", "merged-one")
        self.assertEqual(self.run_review(fake=FakeAf(self.root, add_code=1)), 1)
        page = self.root / "workspace/life/open.md"
        page.parent.mkdir(parents=True, exist_ok=True)
        page.write_text(f"- [ ] {TODAY} · by hand · from: harness-review\n", encoding="utf-8")
        good = FakeAf(self.root)
        self.assertEqual(self.run_review(today=TODAY + dt.timedelta(days=1), fake=good), 0)
        self.assertEqual(good.adds(), [])
        self.assertFalse(self.state()["loop_pending"])

    def test_an_error_in_git_is_a_non_zero_exit(self):
        self.root.joinpath(".git").rename(self.base / "gone")
        self.assertEqual(self.run_review(), 2)


class SafetyAndSizeTests(Base):
    def snapshot(self):
        out = {}
        for p in sorted(self.root.rglob("*")):
            rel = p.relative_to(self.root).as_posix()
            if rel.startswith(".git/") or rel == ".git" or rel == "system/logs" or rel.startswith("system/logs/harness-review"):
                continue
            if rel == "workspace/life/open.md":
                continue   # the one file af's own verb may add a loop line to
            out[rel] = p.read_bytes() if p.is_file() else None
        return out

    def indexes(self):
        paths = [self.root / ".git/index", *sorted((self.root / ".git/worktrees").glob("*/index"))]
        return {p.relative_to(self.root).as_posix(): p.read_bytes() for p in paths}

    def test_it_writes_nothing_outside_its_report_and_state(self):
        self.put(".claude/skills/orphan-skill/SKILL.md", "---\nname: orphan-skill\n---\nBody\n")
        self.put("system/tools/lonely_tool.py", "x = 1\n")
        self.put("library/process/README.md", "catalog\n")
        self.put("library/process/lone.md", "x\n")
        self.put("system/backlog.md", f"## BB-{ago(90)}-01 · old\nSurfaced: main, {ago(90)}\n")
        self.registry(row("system/old", status=f"ended {ago(40)}T10:00 exit"))
        self.put("system/sessions/system/old/handoff.md", "page\n")
        self.put("workspace/life/life.md", "life words\n")
        self.put("workspace/projects/p/project.md", "project\n")
        self.commit(days_ago=30)
        self.git("branch", "merged-one")
        wt = self.base / "wt-index"
        self.git("worktree", "add", str(wt), "-b", "merged-wt")
        stamp = (wt / "AGENTS.md").stat().st_mtime + 100
        os.utime(wt / "AGENTS.md", (stamp, stamp))   # a plain `git status` would refresh this worktree's index
        os.utime(self.root / "AGENTS.md", (stamp, stamp))
        before = self.snapshot()
        indexes = self.indexes()
        self.assertEqual(len(indexes), 2, "the main index and the worktree's")
        listing = self.git("ls-files")
        self.assertEqual(self.run_review(), 0, self.err)
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(self.indexes(), indexes, "no worktree's index file was rewritten")
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


def bb(bid, surfaced, surfaces, done=None, title="a row"):
    text = f"## {bid} · {title}\nSurfaced: main, {surfaced}, evidence\nFix: a fix\nSurfaces: {surfaces}\nVerify: a test\n"
    return text + (f"Done: {done} · abc1234\n" if done else "") + "\n"


def learning(lid, title, guard):
    return f"## {lid} · {title}\nSeen: somewhere\nRoot cause: a reason\nGuard: {guard}\nWatch: `system/x.rs`\n\n"


class CameBackTests(Base):
    """Section 11: rows in the window over the paths of a row completed before it, and learnings whose guard is gone."""

    def came_back(self):
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        return self.section(self.report(), 11)

    def test_a_row_in_the_window_on_the_paths_of_an_earlier_completed_row_is_listed_with_both_ids_and_the_shared_path(self):
        self.put("system/backlog-completed.md", "# Completed\n\n" + bb("BB-2026-10-01-01", "2026-10-01", "system/af/src/open.rs, library/a.md", done="2026-10-05"))
        self.put("system/backlog.md", "# Backlog\n\n" + bb("BB-2026-11-10-02", ago(4), "`system/af/src/open.rs`, `system/other.md`"))
        sec = self.came_back()
        line = next(x for x in sec.splitlines() if "BB-2026-11-10-02" in x)
        self.assertIn("BB-2026-10-01-01", line)
        self.assertIn("`system/af/src/open.rs`", line)
        self.assertNotIn("library/a.md", line)
        self.assertIn("open, surfaced 2026-11-10", line)
        self.assertRegex(sec, r"## 11\. Came back \(1\)")

    def test_a_completed_row_in_the_window_counts_and_says_completed(self):
        self.put("system/backlog-completed.md", "# Completed\n\n" + bb("BB-2026-10-01-01", "2026-10-01", "system/a.rs", done="2026-10-05")
                 + bb("BB-2026-11-08-01", "2026-11-08", "system/a.rs", done="2026-11-09"))
        line = next(x for x in self.came_back().splitlines() if "BB-2026-11-08-01" in x)
        self.assertIn("completed, surfaced 2026-11-08", line)
        self.assertIn("BB-2026-10-01-01 (done 2026-10-05)", line)

    def test_rows_outside_the_pairing_rules_are_not_listed(self):
        self.put("system/backlog-completed.md", "# Completed\n\n"
                 + bb("BB-2026-10-01-01", "2026-10-01", "system/a.rs", done="2026-10-05")
                 + bb("BB-2026-11-05-01", "2026-11-05", "system/b.rs", done="2026-11-06"))
        self.put("system/backlog.md", "# Backlog\n\n"
                 + bb("BB-2026-10-02-01", "2026-10-02", "system/a.rs")        # surfaced before the window
                 + bb("BB-2026-11-09-01", ago(5), "system/c.rs")              # no shared path
                 + bb("BB-2026-11-12-01", ago(2), "system/b.rs"))             # the earlier row was done inside the window
        sec = self.came_back()
        for bid in ("BB-2026-10-02-01", "BB-2026-11-09-01", "BB-2026-11-12-01"):
            self.assertNotIn(bid, sec)
        self.assertRegex(sec, r"## 11\. Came back \(0\)")

    def test_a_row_never_pairs_with_itself_and_an_open_copy_of_a_done_row_is_ignored(self):
        self.put("system/backlog-completed.md", "# Completed\n\n" + bb("BB-2026-10-01-01", "2026-10-01", "system/a.rs", done="2026-10-05"))
        self.put("system/backlog.md", "# Backlog\n\n" + bb("BB-2026-10-01-01", "2026-10-01", "system/a.rs"))
        self.assertRegex(self.came_back(), r"## 11\. Came back \(0\)")

    def test_a_folder_path_covers_the_files_under_it_and_backslashes_match(self):
        self.put("system/backlog-completed.md", "# Completed\n\n" + bb("BB-2026-10-01-01", "2026-10-01", "system/af/src/hooks/", done="2026-10-05"))
        back = "\\".join(["system", "af", "src", "hooks", "source_guard.rs"])
        self.put("system/backlog.md", "# Backlog\n\n" + bb("BB-2026-11-10-01", ago(4), back))
        self.assertIn("`system/af/src/hooks/source_guard.rs`", self.came_back())

    def test_paths_every_fix_touches_are_not_a_reason_to_pair_two_rows(self):
        shared = "system/tests/test_x.py, system/af/tests/, README.md, CHANGELOG.md, system/af/src/cli.rs, system/af/src/lib.rs, library/schema/README.md"
        self.put("system/backlog-completed.md", "# Completed\n\n" + bb("BB-2026-10-01-01", "2026-10-01", shared + ", system/a.rs", done="2026-10-05"))
        self.put("system/backlog.md", "# Backlog\n\n" + bb("BB-2026-11-10-01", ago(4), shared + ", system/b.rs"))
        sec = self.came_back()
        self.assertRegex(sec, r"## 11\. Came back \(0\)")
        for p in ("system/tests/", "system/af/tests/", "README.md", "CHANGELOG.md", "system/af/src/cli.rs"):
            self.assertIn(f"`{p}`", sec.split("**")[0], "the header line names what is left out")

    def test_a_repeated_router_is_the_classic_came_back_and_is_listed(self):
        self.put("system/backlog-completed.md", "# Completed\n\n" + bb("BB-2026-10-01-01", "2026-10-01", "AGENTS.builder.md, system/a.rs", done="2026-10-05"))
        self.put("system/backlog.md", "# Backlog\n\n" + bb("BB-2026-11-10-01", ago(4), "AGENTS.builder.md"))
        self.assertIn("`AGENTS.builder.md`", self.came_back())

    def test_surfaces_lines_shaped_like_the_real_backlog_give_only_paths(self):
        sp = hr.surface_paths
        self.assertEqual(sp("system/af/src/open.rs, library/schema/open-loops.md, system/tools/notify.py (new, lifted from groceries), "
                            "system/tools/push-due-loops.ps1 and register-daily-reminders.ps1 (new), AGENTS.md"),
                         ["system/af/src/open.rs", "library/schema/open-loops.md", "system/tools/notify.py",
                          "system/tools/push-due-loops.ps1", "register-daily-reminders.ps1", "AGENTS.md"])
        self.assertEqual(sp("AGENTS.md (dispatch section, the `claude --name` lines), system/af (session register / list, the hook output), system/sessions/registry.md"),
                         ["AGENTS.md", "system/af", "system/sessions/registry.md"])
        self.assertEqual(sp("system/af/src/cli.rs, system/af/src/life.rs or a new places.rs, system/af/tests/, .claude/agents/README.md if present"),
                         ["system/af/src/cli.rs", "system/af/src/life.rs", "places.rs", "system/af/tests/", ".claude/agents/README.md"])
        self.assertEqual(sp("`system/af/src/open.rs`, system/other.md, `library/a.md` (and the schema)"),
                         ["system/af/src/open.rs", "system/other.md", "library/a.md"], "backticks and bare paths mix")
        self.assertEqual(sp("the old way, e.g. a rewrite, and/or more"), [])

    def test_a_folder_without_its_trailing_slash_covers_the_files_under_it(self):
        self.put("system/backlog-completed.md", "# Completed\n\n" + bb("BB-2026-10-01-01", "2026-10-01", "system/af (session verbs)", done="2026-10-05"))
        self.put("system/backlog.md", "# Backlog\n\n" + bb("BB-2026-11-10-01", ago(4), "system/af/src/sessions.rs"))
        self.assertIn("`system/af/src/sessions.rs`", self.came_back())
        self.assertEqual(hr.shared_paths(["system/afx/y.rs"], ["system/af"]), [], "a name that only starts the same is not under it")

    def test_a_learnings_entry_whose_guard_path_is_gone_is_listed_and_one_that_exists_is_not(self):
        self.put("system/tools/real.py", "x = 1\n")
        self.put("system/learnings.md", "# Learnings\n\n"
                 + learning("L-2026-11-01-01", "kept", "`system/tools/real.py::test_it` and a test named in prose")
                 + learning("L-2026-11-02-01", "lost", "`system/tools/gone.py`, `system/tools/real.py`")
                 + learning("L-2026-11-03-01", "prose one", "prose: `AGENTS.builder.md`"))
        sec = self.came_back()
        self.assertIn("L-2026-11-02-01 lost: guard path gone: `system/tools/gone.py`", sec)
        self.assertNotIn("L-2026-11-01-01", sec)
        self.assertIn("L-2026-11-03-01 prose one: guard path gone: `AGENTS.builder.md`", sec, "the file is absent in this repo")
        self.assertRegex(sec, r"## 11\. Came back \(2\)")

    def test_no_backlog_and_no_learnings_file_is_a_section_of_nothing_and_no_loop(self):
        self.commit()
        self.assertEqual(self.run_review(), 0, self.err)
        sec = self.section(self.report(), 11)
        self.assertRegex(sec, r"Came back \(0\)")
        self.assertEqual(sec.count("nothing"), 2)

    def test_the_count_joins_the_reports_total(self):
        self.put("system/backlog-completed.md", "# Completed\n\n" + bb("BB-2026-10-01-01", "2026-10-01", "system/a.rs", done="2026-10-05"))
        self.put("system/backlog.md", "# Backlog\n\n" + bb("BB-2026-11-10-01", ago(4), "system/a.rs"))
        self.came_back()
        self.assertIn("worth a look", self.report().splitlines()[0])
        self.assertEqual(len(self.fake.adds()), 1, "a came-back item raises the loop like any other")

    def test_the_real_learnings_file_parses_and_every_guard_path_exists(self):
        entries = hr.parse_learnings((ROOT / "system/learnings.md").read_text(encoding="utf-8"))
        self.assertTrue(5 <= len(entries) <= 8, [e["id"] for e in entries])
        for e in entries:
            self.assertRegex(e["id"], r"^L-\d{4}-\d{2}-\d{2}-\d{2}$")
            self.assertTrue(e["paths"], f"{e['id']} names no guard path")
            for path in e["paths"]:
                self.assertTrue((ROOT / path).exists(), f"{e['id']}: {path}")


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

    def test_the_upgrade_skill_makes_the_builders_worktree_from_local_master(self):
        text = self.text(".claude/skills/upgrade/SKILL.md")
        make = "git worktree add .claude/worktrees/<name> -b build/<name> master"
        self.assertIn(make, text)
        self.assertLess(text.index(make), text.index("dispatch the `builder` role"), "the worktree exists before the dispatch")
        self.assertIn("the worktree path", text.split("Its brief carries:")[1].split("4.")[0])

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
