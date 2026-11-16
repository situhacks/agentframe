"""Tests for the transcript archive: a gzip copy of the vault's Claude Code transcripts.

Every test builds its own temporary vault and Claude Code folder; none touches the real `~/.claude`.
"""

from __future__ import annotations

import contextlib
import gzip
import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "system" / "tools"))

import transcript_archive as ta  # noqa: E402

SECOND = 10**9


class Base(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        base = Path(self.dir.name)
        self.root = base / "Vault One"
        self.root.mkdir()
        self.config = base / "cfg"
        self.projects = self.config / "projects"
        self.projects.mkdir(parents=True)
        self.name = ta.derive(os.path.abspath(self.root))
        self.archive = self.root / ta.ARCHIVE

    def source(self, rel, text='{"a": 1}\n', folder=None, mtime=None) -> Path:
        p = self.projects / (folder or self.name) / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8", newline="\n")
        if mtime is not None:
            os.utime(p, ns=(mtime, mtime))
        return p

    def run_tool(self):
        return ta.run(self.root, self.config)

    def copy(self, rel, folder=None) -> Path:
        return self.archive / (folder or self.name) / (rel + ".gz")

    def read_copy(self, rel, folder=None) -> str:
        with gzip.open(self.copy(rel, folder), "rt", encoding="utf-8") as f:
            return f.read()


class NamingTests(unittest.TestCase):
    def test_a_path_becomes_the_folder_name_claude_code_makes(self):
        self.assertEqual(ta.derive("c:\\Obsidian Vaults\\AgentFrame"), "c--Obsidian-Vaults-AgentFrame")
        self.assertEqual(ta.derive("C:\\Obsidian Vaults\\AgentFrame\\.claude\\worktrees\\agent-a7"),
                         "C--Obsidian-Vaults-AgentFrame--claude-worktrees-agent-a7")
        self.assertEqual(ta.derive("/home/me/my_proj.v2"), "-home-me-my-proj-v2")

    def test_the_vault_and_its_worktrees_belong_and_another_project_with_the_same_start_does_not(self):
        v = "c--Obsidian-Vaults-AgentFrame"
        for windows in (False, True):
            self.assertTrue(ta.belongs(v, v, windows))
            self.assertTrue(ta.belongs(v + "--claude-worktrees-agent-x", v, windows))
            self.assertFalse(ta.belongs(v + "-Marketing", v, windows), "a different project that starts the same")
            self.assertFalse(ta.belongs("c--Obsidian-Vaults-CareerOS", v, windows))
            self.assertFalse(ta.belongs(v + "--claude-worktree", v, windows))

    def test_windows_matches_ignoring_case_and_elsewhere_it_does_not(self):
        v = "c--Obsidian-Vaults-AgentFrame"
        self.assertTrue(ta.belongs("C--Obsidian-Vaults-AgentFrame--Claude-Worktrees-x", v, True))
        self.assertTrue(ta.belongs("C--OBSIDIAN-VAULTS-AGENTFRAME", v, True))
        self.assertFalse(ta.belongs("C--Obsidian-Vaults-AgentFrame", v, False))

    def test_the_claude_folder_is_the_config_dir_variable_else_the_users_dot_claude(self):
        with mock.patch.dict(os.environ, {"CLAUDE_CONFIG_DIR": "X:/cfg", "USERPROFILE": "Y:/home", "HOME": "Z:/h"}):
            self.assertEqual(ta.config_dir(), Path("X:/cfg"))
        with mock.patch.dict(os.environ, {"CLAUDE_CONFIG_DIR": "", "USERPROFILE": "Y:/home", "HOME": "Z:/h"}):
            self.assertEqual(ta.config_dir(), Path("Y:/home/.claude"))


class CopyTests(Base):
    def test_a_new_transcript_is_archived_gzip_readable_at_the_layout_path_with_the_ignore_file(self):
        self.source("s1.jsonl", "line one\nline two\n")
        self.source("s1/subagents/agent-a.jsonl", "sub\n")
        res = self.run_tool()
        self.assertEqual((res.new, res.updated, res.unchanged, res.failed), (2, 0, 0, 0))
        self.assertEqual(self.read_copy("s1.jsonl"), "line one\nline two\n")
        self.assertEqual(self.read_copy("s1/subagents/agent-a.jsonl"), "sub\n")
        self.assertEqual((self.archive / ".gitignore").read_text(encoding="utf-8").split(), ["*"])
        self.assertTrue((self.root / ta.STATE).is_file(), "the state file is under system/logs")

    def test_an_unchanged_file_is_skipped(self):
        self.source("s1.jsonl")
        self.run_tool()
        with mock.patch.object(ta, "write_copy") as write:
            res = self.run_tool()
        write.assert_not_called()
        self.assertEqual((res.new, res.updated, res.unchanged), (0, 0, 1))

    def test_a_grown_file_replaces_its_copy(self):
        p = self.source("s1.jsonl", "one\n")
        self.run_tool()
        with p.open("a", encoding="utf-8", newline="\n") as f:
            f.write("two\n")
        res = self.run_tool()
        self.assertEqual((res.new, res.updated, res.unchanged), (0, 1, 0))
        self.assertEqual(self.read_copy("s1.jsonl"), "one\ntwo\n")
        self.assertFalse(list(self.archive.rglob("*.tmp")), "no temporary file is left")

    def test_the_copy_is_kept_when_the_source_is_deleted(self):
        p = self.source("s1.jsonl", "one\n")
        self.run_tool()
        p.unlink()
        res = self.run_tool()
        self.assertEqual((res.new, res.updated, res.unchanged), (0, 0, 0))
        self.assertEqual(self.read_copy("s1.jsonl"), "one\n")
        self.assertTrue(json.loads((self.root / ta.STATE).read_text(encoding="utf-8"))["files"], "its record stays too")

    def test_the_source_is_never_touched(self):
        p = self.source("s1.jsonl", "one\n", mtime=1_700_000_000 * SECOND)
        before = (p.read_bytes(), p.stat().st_mtime_ns, p.stat().st_size)
        self.run_tool()
        self.assertEqual((p.read_bytes(), p.stat().st_mtime_ns, p.stat().st_size), before)

    def test_worktree_folders_are_included_and_other_projects_are_not(self):
        wt = self.name + "--claude-worktrees-agent-x"
        self.source("a.jsonl", "main\n")
        self.source("b.jsonl", "worktree\n", folder=wt)
        self.source("c.jsonl", "sibling\n", folder=self.name + "-Marketing")
        self.source("d.jsonl", "other\n", folder="c--Somewhere-Else")
        res = self.run_tool()
        self.assertEqual(res.new, 2)
        self.assertEqual(self.read_copy("b.jsonl", wt), "worktree\n")
        self.assertEqual(sorted(p.name for p in self.archive.iterdir() if p.is_dir()), sorted([self.name, wt]))

    def test_a_temp_folder_run_of_the_batch_is_not_archived(self):
        self.source("a.jsonl", "a\n", folder="C--Users-Me-AppData-Local-Temp-agentframe-loop-watch")
        self.assertEqual(self.run_tool().new, 0)

    def test_everything_in_a_session_folder_is_copied_not_only_the_transcript(self):
        self.source("s1.jsonl", "t\n")
        self.source("s1/tool-results/out.txt", "saved output\n")
        self.source("s1/tool-results/page.pdf", "%PDF fake\n")
        self.source("s1/subagents/agent-a.json", "{}\n")
        self.source("s1/subagents/agent-a.jsonl", "sub\n")
        res = self.run_tool()
        self.assertEqual(res.new, 5)
        self.assertEqual(self.read_copy("s1/tool-results/out.txt"), "saved output\n")
        self.assertEqual(self.read_copy("s1/tool-results/page.pdf"), "%PDF fake\n")
        self.assertEqual(self.read_copy("s1/subagents/agent-a.json"), "{}\n")

    def test_wakatime_lock_and_temporary_files_are_left_out(self):
        self.source("s1.jsonl", "t\n")
        for name in ("s1.jsonl.wakatime", "s1.lock", "a.tmp", "b.temp", "c.lck", "d.swp", "~e.jsonl"):
            self.source(name, "x\n")
        res = self.run_tool()
        self.assertEqual(res.new, 1)
        self.assertEqual([p.name for p in (self.archive / self.name).iterdir()], ["s1.jsonl.gz"])
    def test_one_file_that_cannot_be_written_is_counted_and_the_rest_go_on(self):
        self.source("a.jsonl", "a\n")
        self.source("b.jsonl", "b\n")
        real = ta.write_copy

        def flaky(src, st, copy):
            if src.name == "a.jsonl":
                raise PermissionError("locked")
            real(src, st, copy)
        with mock.patch.object(ta, "write_copy", flaky):
            res = self.run_tool()
        self.assertEqual((res.new, res.failed), (1, 1))
        self.assertEqual(self.read_copy("b.jsonl"), "b\n")
        self.assertTrue(res.line().endswith(" \u00b7 1 failed"))
        res = self.run_tool()
        self.assertEqual((res.new, res.unchanged, res.failed), (1, 1, 0), "the failed one is tried again")


class FirstRunTests(Base):
    """No state file, and a copy already there (the one-off copy): refresh it if the source is newer or larger,
    keep it otherwise, never delete it."""

    def prior_copy(self, rel, text, mtime_ns):
        c = self.copy(rel)
        c.parent.mkdir(parents=True, exist_ok=True)
        with gzip.open(c, "wt", encoding="utf-8", newline="\n") as f:
            f.write(text)
        os.utime(c, ns=(mtime_ns, mtime_ns))

    def test_a_larger_source_refreshes_the_copy(self):
        self.source("s.jsonl", "one\ntwo\n", mtime=1_700_000_000 * SECOND)
        self.prior_copy("s.jsonl", "one\n", 1_800_000_000 * SECOND)   # the copy is the newer file; the source is larger
        res = self.run_tool()
        self.assertEqual((res.new, res.updated, res.unchanged), (0, 1, 0))
        self.assertEqual(self.read_copy("s.jsonl"), "one\ntwo\n")

    def test_a_newer_source_refreshes_the_copy(self):
        self.source("s.jsonl", "abc\n", mtime=1_800_000_000 * SECOND)
        self.prior_copy("s.jsonl", "xyz\n", 1_700_000_000 * SECOND)   # same size, older copy
        res = self.run_tool()
        self.assertEqual((res.new, res.updated, res.unchanged), (0, 1, 0))
        self.assertEqual(self.read_copy("s.jsonl"), "abc\n")

    def test_a_source_neither_newer_nor_larger_keeps_its_copy_as_it_is(self):
        self.source("s.jsonl", "abc\n", mtime=1_700_000_000 * SECOND)
        self.prior_copy("s.jsonl", "abc\n", 1_800_000_000 * SECOND)
        with mock.patch.object(ta, "write_copy") as write:
            res = self.run_tool()
        write.assert_not_called()
        self.assertEqual((res.new, res.updated, res.unchanged), (0, 0, 1))

    def test_a_copy_whose_source_is_gone_is_never_deleted_on_the_first_run(self):
        self.prior_copy("gone.jsonl", "old\n", 1_800_000_000 * SECOND)
        self.source("other.jsonl")
        self.run_tool()
        self.assertEqual(self.read_copy("gone.jsonl"), "old\n")

    def test_losing_the_state_file_does_not_recopy_what_the_tool_itself_copied(self):
        self.source("s.jsonl", "abc\n")
        self.run_tool()
        (self.root / ta.STATE).unlink()
        with mock.patch.object(ta, "write_copy") as write:
            res = self.run_tool()
        write.assert_not_called()
        self.assertEqual((res.new, res.updated, res.unchanged), (0, 0, 1))


class ShrinkTests(Base):
    """A source smaller than the copy already holds never replaces it: the new content is saved beside it."""

    def stamp(self, p: Path) -> str:
        import datetime as dt
        return dt.datetime.fromtimestamp(p.stat().st_mtime).strftime("%Y%m%d%H%M%S")

    def test_a_source_that_shrank_keeps_the_old_copy_and_saves_the_new_beside_it_counted_as_kept(self):
        big = "x" * 5000 + "\n"
        p = self.source("s.jsonl", big, mtime=1_700_000_000 * SECOND)
        self.run_tool()
        p.write_text("", encoding="utf-8")
        os.utime(p, ns=(1_800_000_000 * SECOND, 1_800_000_000 * SECOND))
        res = self.run_tool()
        self.assertEqual((res.new, res.updated, res.unchanged, res.kept), (0, 0, 0, 1))
        self.assertEqual(self.read_copy("s.jsonl"), big, "the old copy is whole")
        beside = self.archive / self.name / f"s.jsonl.{self.stamp(p)}.gz"
        with gzip.open(beside, "rt", encoding="utf-8") as f:
            self.assertEqual(f.read(), "")
        self.assertIn(", 1 kept", res.line())
        res = self.run_tool()
        self.assertEqual((res.unchanged, res.kept), (1, 0), "recorded, so not saved again")
        self.assertNotIn("kept", res.line())

    def test_the_same_with_no_state_file_and_a_source_that_is_newer_but_smaller(self):
        p = self.source("s.jsonl", "tiny\n", mtime=1_800_000_000 * SECOND)
        c = self.copy("s.jsonl")
        c.parent.mkdir(parents=True, exist_ok=True)
        with gzip.open(c, "wt", encoding="utf-8") as f:
            f.write("y" * 5000 + "\n")
        os.utime(c, ns=(1_700_000_000 * SECOND, 1_700_000_000 * SECOND))
        res = self.run_tool()
        self.assertEqual((res.updated, res.kept), (0, 1))
        self.assertEqual(self.read_copy("s.jsonl"), "y" * 5000 + "\n")
        self.assertTrue((self.archive / self.name / f"s.jsonl.{self.stamp(p)}.gz").is_file())

    def test_a_source_that_grew_still_replaces_its_copy(self):
        p = self.source("s.jsonl", "one\n")
        self.run_tool()
        p.write_text("one\ntwo\nthree\n", encoding="utf-8", newline="\n")
        res = self.run_tool()
        self.assertEqual((res.updated, res.kept), (1, 0))
        self.assertEqual(self.read_copy("s.jsonl"), "one\ntwo\nthree\n")


class ExitCodeTests(Base):
    def flaky(self, names):
        real = ta.write_copy

        def go(src, st, copy):
            if src.name in names:
                raise PermissionError("locked")
            real(src, st, copy)
        return mock.patch.object(ta, "write_copy", go)

    def main(self):
        out = io.StringIO()
        with mock.patch.dict(os.environ, {"AF_ROOT": str(self.root), "CLAUDE_CONFIG_DIR": str(self.config)}), \
                contextlib.redirect_stdout(out):
            code = ta.main()
        return code, out.getvalue()

    def test_the_codes(self):
        self.assertEqual(ta.Result().code(), 0)
        self.assertEqual(ta.Result(new=1, failed=1).code(), ta.EXIT_PARTIAL)
        self.assertEqual(ta.Result(unchanged=3, failed=1).code(), 3)
        self.assertEqual(ta.Result(failed=2).code(), 1)
        self.assertEqual(ta.Result(new=4, fatal="why").code(), 1)
        self.assertEqual(ta.EXIT_PARTIAL, 3)

    def test_some_files_failing_and_the_rest_copied_is_exit_3(self):
        self.source("a.jsonl", "a\n")
        self.source("b.jsonl", "b\n")
        with self.flaky({"a.jsonl"}):
            code, out = self.main()
        self.assertEqual(code, 3)
        self.assertIn("1 new", out)
        self.assertIn("1 failed", out)

    def test_every_file_failing_is_exit_1(self):
        self.source("a.jsonl", "a\n")
        with self.flaky({"a.jsonl"}):
            code, _ = self.main()
        self.assertEqual(code, 1)

    def test_an_unwritable_archive_folder_is_exit_1_and_nothing_is_copied(self):
        self.source("a.jsonl", "a\n")
        (self.root / "workspace").write_text("a file where the folder should be", encoding="utf-8")
        code, out = self.main()
        self.assertEqual(code, 1)
        self.assertIn("transcript-archive: failed: the archive folder is not writable", out)
        self.assertFalse((self.root / ta.STATE).exists())

    def test_a_gitignore_that_cannot_be_written_stops_the_run_before_any_copy_and_leaves_no_empty_file(self):
        self.source("a.jsonl", "a\n")
        real = os.replace

        def replace(src, dst):
            if Path(dst).name == ".gitignore":
                raise OSError("disk full")
            real(src, dst)
        with mock.patch.object(ta.os, "replace", replace):
            res = self.run_tool()
        self.assertTrue(res.fatal)
        self.assertEqual(res.code(), 1)
        self.assertFalse((self.archive / ".gitignore").exists(), "never a half-written ignore file")
        self.assertEqual(list(self.archive.rglob("*.tmp")), [])
        self.assertEqual(list(self.archive.rglob("*.gz")), [], "nothing is copied without the ignore file")

    def test_no_readable_project_folder_of_the_vault_is_exit_1(self):
        self.source("a.jsonl", "a\n")
        real = os.walk

        def walk(top, onerror=None, **kw):
            if onerror is None or Path(top) == self.archive:
                return real(top, **kw)
            onerror(PermissionError(13, "denied", str(top)))
            return iter(())
        with mock.patch.object(ta.os, "walk", walk):
            res = self.run_tool()
        self.assertEqual((res.code(), res.new), (1, 0))
        self.assertIn("failed:", res.line())

    def test_an_unreadable_subfolder_is_a_failed_file_not_a_whole_failure(self):
        self.source("a.jsonl", "a\n")
        real = os.walk

        def walk(top, onerror=None, **kw):
            if onerror is not None:
                onerror(PermissionError(13, "denied", str(Path(top) / "sub")))
            return real(top, onerror=onerror, **kw)
        with mock.patch.object(ta.os, "walk", walk):
            res = self.run_tool()
        self.assertEqual((res.new, res.failed, res.code()), (1, 1, 3))


class FailingFileLoopTests(Base):
    """A file that keeps failing on 3 or more days gets one loop for main; its record clears when it copies."""

    DAY = ta.dt.date(2026, 11, 10)

    def setUp(self):
        super().setUp()
        self.source("a.jsonl", "a\n")
        self.loops = []
        self.fail = True
        self.af_code = 0
        real = ta.write_copy

        def go(src, st, copy):
            if self.fail and src.name == "a.jsonl":
                raise PermissionError("locked")
            real(src, st, copy)
        p = mock.patch.object(ta, "write_copy", go)
        p.start()
        self.addCleanup(p.stop)

    def add(self, root, text, today):
        self.loops.append((root, text, today))
        return self.af_code

    def day(self, n):
        return ta.run(self.root, self.config, self.DAY + ta.dt.timedelta(days=n), self.add)

    def failing(self):
        return json.loads((self.root / ta.STATE).read_text(encoding="utf-8"))["failing"]

    def test_the_third_day_raises_one_loop_naming_the_file_the_start_and_the_thirty_days(self):
        self.day(0)
        self.day(1)
        self.assertEqual(self.loops, [])
        self.day(2)
        (root, text, today), = self.loops
        self.assertEqual((root, today), (self.root, self.DAY + ta.dt.timedelta(days=2)))
        self.assertIn(f"{self.name}/a.jsonl", text)
        self.assertIn("since 2026-11-10", text)
        self.assertIn("30 days", text)
        self.day(3)
        self.day(4)
        self.assertEqual(len(self.loops), 1, "once per file")

    def test_two_runs_on_one_day_count_as_one_day(self):
        for _ in range(4):
            self.day(0)
        self.day(1)
        self.assertEqual(self.loops, [])
        self.assertEqual(self.failing()[f"{self.name}/a.jsonl"]["days"], ["2026-11-10", "2026-11-11"])

    def test_a_copy_that_succeeds_clears_the_record_and_the_count_starts_again(self):
        self.day(0)
        self.day(1)
        self.fail = False
        res = self.day(2)
        self.assertEqual((res.new, res.failed), (1, 0))
        self.assertEqual(self.failing(), {})
        self.fail = True
        (self.projects / self.name / "a.jsonl").write_text("a bit more\n", encoding="utf-8")
        self.day(3)
        self.day(4)
        self.assertEqual(self.loops, [], "two days since the last success, not four")

    def test_when_af_fails_the_loop_is_tried_again_next_run(self):
        self.af_code = 1
        for n in range(4):
            self.day(n)
        self.assertEqual(len(self.loops), 2, "day 3 and day 4")
        self.af_code = 0
        self.day(4)
        self.day(5)
        self.assertEqual(len(self.loops), 3, "raised once it works, then not again")

    def test_a_file_that_is_gone_drops_its_record(self):
        self.day(0)
        (self.projects / self.name / "a.jsonl").unlink()
        self.day(1)
        self.assertEqual(self.failing(), {})

    def test_the_default_loop_is_af_open_add_for_main_in_the_vault(self):
        done = mock.Mock(return_value=mock.Mock(returncode=0))
        with mock.patch.object(ta.subprocess, "run", done):
            self.assertEqual(ta.raise_loop(self.root, "the text", self.DAY), 0)
        args, kwargs = done.call_args
        self.assertEqual(args[0][:4], ["af", "open", "add", "the text"])
        self.assertEqual(args[0][args[0].index("--from") + 1], "transcript-archive")
        self.assertEqual(args[0][args[0].index("--section") + 1], ta.LOOP_SECTION)
        self.assertEqual(args[0][args[0].index("--nudge") + 1], "2026-11-10")
        self.assertEqual(kwargs["env"]["AF_ROOT"], str(self.root))
        with mock.patch.object(ta.subprocess, "run", side_effect=FileNotFoundError("af")):
            self.assertEqual(ta.raise_loop(self.root, "t", self.DAY), 127)


class MainTests(Base):
    def main(self):
        out = io.StringIO()
        env = {"AF_ROOT": str(self.root), "CLAUDE_CONFIG_DIR": str(self.config)}
        with mock.patch.dict(os.environ, env), contextlib.redirect_stdout(out):
            code = ta.main()
        return code, out.getvalue()

    def test_it_prints_the_one_summary_line_and_exits_0(self):
        self.source("a.jsonl", "a" * 1000 + "\n")
        self.source("b.jsonl", "b\n")
        code, out = self.main()
        self.assertEqual(code, 0)
        self.assertRegex(out, r"\Atranscript-archive: 2 new, 0 updated, 0 unchanged \u00b7 \d+\.\d MB archived\n\Z")
        code, out = self.main()
        self.assertTrue(out.startswith("transcript-archive: 0 new, 0 updated, 2 unchanged \u00b7 "), out)

    def test_the_megabytes_are_the_size_of_the_archive_on_disk(self):
        self.source("a.jsonl", "".join(f"{i} some text that repeats\n" for i in range(200_000)))
        res = self.run_tool()
        on_disk = sum(p.stat().st_size for p in self.archive.rglob("*.gz"))
        self.assertGreater(on_disk, 100_000)
        self.assertGreaterEqual(res.megabytes * 1e6, on_disk)
        self.assertLess(res.megabytes * 1e6, on_disk + 10_000, "only the archive's files, plus the tiny ignore file")

    def test_no_claude_project_folder_is_a_whole_failure(self):
        out = io.StringIO()
        with mock.patch.dict(os.environ, {"AF_ROOT": str(self.root), "CLAUDE_CONFIG_DIR": str(self.config / "nope")}), \
                contextlib.redirect_stdout(out):
            code = ta.main()
        self.assertEqual(code, 1)
        self.assertTrue(out.getvalue().startswith("transcript-archive: failed: no Claude Code project folder"), out.getvalue())

    def test_a_vault_with_no_chats_yet_is_nothing_to_do(self):
        code, out = self.main()
        self.assertEqual(code, 0)
        self.assertTrue(out.startswith("transcript-archive: 0 new, 0 updated, 0 unchanged"))


class WiringTests(unittest.TestCase):
    def test_the_batch_runs_it_as_a_morning_step_with_a_limit(self):
        import watch_batch as wb
        self.assertIn(("transcript-archive", "transcript_archive.py", 15), wb.MORNING)
        self.assertTrue((ROOT / "system" / "tools" / "transcript_archive.py").is_file())


if __name__ == "__main__":
    unittest.main()
