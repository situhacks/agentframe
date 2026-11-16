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
        self.source("notes.txt", "not a transcript")
        res = self.run_tool()
        self.assertEqual((res.new, res.updated, res.unchanged, res.failed), (2, 0, 0, 0))
        self.assertEqual(self.read_copy("s1.jsonl"), "line one\nline two\n")
        self.assertEqual(self.read_copy("s1/subagents/agent-a.jsonl"), "sub\n")
        self.assertFalse((self.archive / self.name / "notes.txt.gz").exists())
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

    def test_no_claude_folder_is_nothing_to_do_not_an_error(self):
        out = io.StringIO()
        with mock.patch.dict(os.environ, {"AF_ROOT": str(self.root), "CLAUDE_CONFIG_DIR": str(self.config / "nope")}), \
                contextlib.redirect_stdout(out):
            code = ta.main()
        self.assertEqual(code, 0)
        self.assertTrue(out.getvalue().startswith("transcript-archive: 0 new, 0 updated, 0 unchanged"))


class WiringTests(unittest.TestCase):
    def test_the_batch_runs_it_as_a_morning_step_with_a_limit(self):
        import watch_batch as wb
        self.assertIn(("transcript-archive", "transcript_archive.py", 15), wb.MORNING)
        self.assertTrue((ROOT / "system" / "tools" / "transcript_archive.py").is_file())


if __name__ == "__main__":
    unittest.main()
