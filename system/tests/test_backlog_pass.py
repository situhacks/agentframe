"""Contract tests for the scheduled backlog pass and the privacy sweep main runs on a branch.

The pass reads the ready rows and raises one loop. It starts no builder and runs no git, which is
pinned here on the commands it actually issues, not on what it says about itself.
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

import backlog_pass  # noqa: E402
import privacy_sweep  # noqa: E402

ROWS = [
    {"id": "BB-2026-01-01-01", "title": "a row", "text": "## BB-2026-01-01-01 · a row\nFix: x\nVerify: a test"},
    {"id": "BB-2026-01-02-01", "title": "another", "text": "## BB-2026-01-02-01 · another\nFix: y\nVerify: a test"},
]


class PassTests(unittest.TestCase):
    def call(self, argv, rows, open_text=None, code=0):
        """Run the pass over a temp vault with subprocess faked; returns (exit, stdout, every command)."""
        commands = []

        def fake_run(cmd, **_):
            commands.append(cmd)
            stdout = json.dumps(rows) if cmd[:3] == ["af", "backlog", "ready"] else ""
            return mock.Mock(returncode=code if cmd[:3] == ["af", "open", "add"] else 0, stdout=stdout, stderr="boom")

        out = io.StringIO()
        with tempfile.TemporaryDirectory() as d:
            vault = Path(d)
            if open_text is not None:
                (vault / "workspace/life").mkdir(parents=True)
                (vault / "workspace/life/open.md").write_text(open_text, encoding="utf-8")
            with mock.patch.object(subprocess, "run", fake_run), contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
                result = backlog_pass.main(argv, vault=vault, today=dt.date(2026, 11, 8))
        return result, out.getvalue(), commands

    def test_a_pass_raises_one_loop_naming_the_rows_for_a_reviewed_builder(self):
        result, _, commands = self.call([], ROWS)
        self.assertEqual(result, 0)
        add = [c for c in commands if c[:3] == ["af", "open", "add"]]
        self.assertEqual(len(add), 1)
        self.assertIn("reviewed builder", add[0][3])
        self.assertIn("BB-2026-01-01-01 a row", add[0][3])
        self.assertIn("BB-2026-01-02-01 another", add[0][3])
        self.assertEqual(add[0][add[0].index("--nudge") + 1], "2026-11-09")

    def test_every_command_the_pass_issues_is_af_and_none_starts_work(self):
        _, _, commands = self.call([], ROWS)
        self.assertTrue(commands)
        self.assertEqual({c[0] for c in commands}, {"af"})
        self.assertEqual({tuple(c[1:3]) for c in commands}, {("backlog", "ready"), ("open", "add")})

    def test_the_source_starts_no_builder_and_runs_no_git(self):
        for name in ("backlog_pass.py", "backlog-pass.ps1", "register-backlog-pass.ps1"):
            text = (ROOT / "system" / "tools" / name).read_text(encoding="utf-8")
            self.assertNotRegex(text, r"""["'\s]claude["'\s]""", name)
            self.assertNotRegex(text, r"""["'\s]git["'\s]""", name)
        text = (ROOT / "system" / "tools" / "backlog_pass.py").read_text(encoding="utf-8")
        self.assertEqual(re.findall(r"subprocess\.\w+", text), ["subprocess.CompletedProcess", "subprocess.run"])

    def test_no_ready_row_raises_nothing(self):
        result, out, commands = self.call([], [])
        self.assertEqual(result, 0)
        self.assertIn("no row is ready", out)
        self.assertEqual([c for c in commands if c[:3] == ["af", "open", "add"]], [])

    def test_a_dry_run_names_the_rows_and_raises_nothing(self):
        result, out, commands = self.call(["--dry-run"], ROWS)
        self.assertEqual(result, 0)
        self.assertIn("BB-2026-01-01-01", out)
        self.assertEqual([c for c in commands if c[:3] == ["af", "open", "add"]], [])

    def test_a_loop_still_open_is_not_raised_twice(self):
        page = f"# Open loops\n\n- [ ] 2026-11-01 · {backlog_pass.LOOP_PREFIX} BB-2026-01-01-01 a row\n"
        result, out, commands = self.call([], ROWS, open_text=page)
        self.assertEqual(result, 0)
        self.assertIn("still open", out)
        self.assertEqual([c for c in commands if c[:3] == ["af", "open", "add"]], [])

    def test_a_failed_loop_is_a_failed_pass(self):
        result, _, _ = self.call([], ROWS, code=1)
        self.assertEqual(result, 1)

    def test_the_loop_lands_on_the_next_monday(self):
        self.assertEqual(backlog_pass.next_monday(dt.date(2026, 11, 8)), dt.date(2026, 11, 9))
        self.assertEqual(backlog_pass.next_monday(dt.date(2026, 11, 9)), dt.date(2026, 11, 16))

    def test_the_task_runs_after_the_weekly_consolidation_and_is_only_a_register_script(self):
        register = (ROOT / "system" / "tools" / "register-backlog-pass.ps1").read_text(encoding="utf-8")
        weekly = (ROOT / "system" / "tools" / "register-weekly-consolidation.ps1").read_text(encoding="utf-8")
        self.assertIn("-At '18:00'", weekly)
        self.assertIn("-At '20:00'", register)


class RouterLineTests(unittest.TestCase):
    def test_the_worker_line_starts_with_no_connector(self):
        schema = (ROOT / "library" / "schema" / "sessions-registry.md").read_text(encoding="utf-8")
        line = next(l for l in schema.splitlines() if l.startswith('claude -p "<goal>"'))
        self.assertIn("--strict-mcp-config", line)

    def test_a_builder_stops_at_a_refused_path(self):
        builder = (ROOT / ".claude" / "agents" / "builder.md").read_text(encoding="utf-8")
        self.assertIn("When a tool refuses a path, stop and report it; never go around the refusal with a script.", builder)

    def test_main_tags_only_through_the_button(self):
        agents = (ROOT / "AGENTS.md").read_text(encoding="utf-8")
        self.assertIn("Tags only via `af tag match` or `propose`", agents)


class SweepTests(unittest.TestCase):
    def test_a_private_term_in_an_added_line_is_a_hit_and_a_removed_line_is_not(self):
        diff = "+++ b/a.md\n+ met the acme-corp people\n-an acme-corp line going away\n ok acme-corp context\n"
        hits = privacy_sweep.sweep(privacy_sweep.added_lines(diff), {"acme-corp"}, set())
        self.assertEqual(len(hits), 1)
        self.assertIn("private term 'acme-corp'", hits[0])

    def test_a_term_at_the_end_of_another_slug_is_not_a_hit(self):
        lines = [("a.md", "a not-acme here")]
        self.assertEqual(privacy_sweep.sweep(lines, {"acme"}, set()), [])

    def test_a_term_that_starts_a_longer_slug_is_a_hit(self):
        hits = privacy_sweep.sweep([("a.py", '"park", "acme-software",')], {"acme"}, set())
        self.assertEqual(len(hits), 1, hits)
        self.assertIn("private term 'acme'", hits[0])

    def test_a_credential_shape_and_a_workspace_path_are_hits(self):
        fake = "api_key = " + chr(34) + "abcdefghijklmnopqrstuvwxyz" + chr(34)
        hits = privacy_sweep.sweep([("a.py", fake), ("workspace/life/open.md", "anything")], set(), set())
        self.assertTrue(any("secret assignment" in h for h in hits), hits)
        self.assertTrue(any("a private path" in h for h in hits), hits)

    def test_allow_skips_a_term(self):
        self.assertEqual(privacy_sweep.sweep([("a.md", "acme-corp")], {"acme-corp"}, {"acme-corp"}), [])

    def test_the_common_words_are_dictionary_words_never_a_slug(self):
        # A slug here is a private name in a public file, and it switches off the check for that name.
        slugs = sorted(t for t in privacy_sweep.COMMON if not t.isalpha())
        self.assertEqual(slugs, [], "COMMON holds plain words; pass a slug with --allow on the one run that needs it")

    def test_a_project_slug_is_a_term_whatever_its_shape(self):
        with tempfile.TemporaryDirectory() as d:
            vault = Path(d)
            (vault / "workspace/projects/two-word").mkdir(parents=True)
            self.assertIn("two-word", privacy_sweep.vault_terms(vault))

    def test_the_sweep_reads_the_diff_of_the_folder_it_runs_in_so_the_base_copy_can_check_a_branch(self):
        with tempfile.TemporaryDirectory() as d:
            repo = Path(d)
            env = dict(
                os.environ,
                GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@e.invalid", GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@e.invalid",
            )
            subprocess.run(["git", "init", "-q", "-b", "base"], cwd=repo, env=env, check=True)
            (repo / "a.md").write_text("clean\n", encoding="utf-8")
            subprocess.run(["git", "add", "-A"], cwd=repo, env=env, check=True)
            subprocess.run(["git", "commit", "-q", "-m", "base"], cwd=repo, env=env, check=True)
            (repo / "workspace/projects/acme-rollout").mkdir(parents=True)
            (repo / "b.md").write_text("the acme-rollout client\n", encoding="utf-8")
            script = str(ROOT / "system" / "tools" / "privacy_sweep.py")
            r = subprocess.run(
                [sys.executable, script, "--base", "base", "--vault", str(repo)],
                cwd=repo, env=env, capture_output=True, text=True, encoding="utf-8", errors="replace",
            )
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("private term 'acme-rollout'", r.stdout)

    def test_terms_come_from_the_vault_folders_and_skip_short_and_common_words(self):
        with tempfile.TemporaryDirectory() as d:
            vault = Path(d)
            (vault / "workspace/projects/acme-rollout").mkdir(parents=True)
            (vault / "workspace/projects/completed").mkdir(parents=True)
            (vault / "workspace/life/people").mkdir(parents=True)
            (vault / "workspace/life/people/bo.md").write_text("---\nname: Bo\n---\n# Bo\n", encoding="utf-8")
            (vault / "workspace/life/people/jane-roe.md").write_text("---\nname: Jane Roe\n---\n# Jane Roe\n", encoding="utf-8")
            terms = privacy_sweep.vault_terms(vault)
        self.assertIn("acme-rollout", terms)
        self.assertIn("jane-roe", terms)
        self.assertIn("jane roe", terms)
        self.assertNotIn("bo", terms)
        self.assertNotIn("completed", terms)


if __name__ == "__main__":
    unittest.main()
