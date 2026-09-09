"""Buttons report on themselves.

Every run lands in af_runs whatever the outcome; a broken indexer never blocks a state
transition; the messages that used to send agents into a draft/adopt retry loop name the
route; an assembly-record row is not schema drift; and the doctor buttons note reads the log.
"""
import builtins
import contextlib
import datetime
import io
import os
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

from system import af
from system.audit import writer

EPOCH = "1970-01-01T00:00:00+00:00"


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    af.write(path, text)


def quiet(fn, *a, **kw):
    with contextlib.redirect_stdout(io.StringIO()):
        return fn(*a, **kw)


class TempDB(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.db = os.path.join(self._tmp.name, "audit.db")
        p = patch.object(af, "AUDIT_DB", self.db)
        p.start()
        self.addCleanup(p.stop)
        self.addCleanup(self._tmp.cleanup)

    def runs(self):
        return writer.query_af_runs(db_path=self.db, since=EPOCH)

    def add(self, verb, exit_code, error=None, days_ago=0):
        at = (datetime.datetime.now(datetime.timezone.utc)
              - datetime.timedelta(days=days_ago)).replace(microsecond=0).isoformat()
        writer.append_af_run(db_path=self.db, verb=verb, argv=verb, exit_code=exit_code,
                             error=error, created_at=at)


class RunLog(TempDB):
    def test_refusal_lands_with_its_message(self):
        with patch.object(sys, "argv", ["af", "ready", "no-such-project", "x"]), \
             contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            af.main()
        rows = self.runs()
        self.assertEqual(len(rows), 1)
        self.assertEqual((rows[0]["verb"], rows[0]["exit_code"]), ("ready", 1))
        self.assertTrue(rows[0]["error"])
        self.assertNotIn("af: ERROR", rows[0]["error"])

    def test_success_lands_with_exit_zero(self):
        with patch.object(af, "cmd_new_project"), patch.object(af, "check_mode_gate"), \
             patch.object(sys, "argv", ["af", "new-project", "anything"]):
            af.main()
        row = self.runs()[0]
        self.assertEqual((row["verb"], row["exit_code"], row["error"]), ("new-project", 0, None))
        self.assertGreaterEqual(row["duration_ms"], 0)

    def test_usage_error_is_labelled_usage(self):
        with patch.object(sys, "argv", ["af", "draft", "p", "d"]), \
             contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            af.main()
        row = self.runs()[0]
        self.assertEqual((row["verb"], row["exit_code"], row["error"]), ("draft", 2, "usage"))

    def test_crash_is_recorded_and_still_raised(self):
        def boom(args):
            raise RuntimeError("kaboom")
        with patch.object(af, "cmd_new_project", boom), patch.object(af, "check_mode_gate"), \
             patch.object(sys, "argv", ["af", "new-project", "anything"]), \
             self.assertRaises(RuntimeError):
            af.main()
        row = self.runs()[0]
        self.assertEqual(row["exit_code"], 2)
        self.assertEqual(row["error"], "RuntimeError: kaboom")

    def test_logging_failure_never_changes_the_outcome(self):
        with patch.object(af, "_audit_writer", side_effect=RuntimeError("db down")), \
             patch.object(af, "cmd_new_project"), patch.object(af, "check_mode_gate"), \
             patch.object(sys, "argv", ["af", "new-project", "anything"]):
            af.main()  # no exception escapes

    def test_help_is_not_logged(self):
        with patch.object(sys, "argv", ["af", "--help"]), \
             contextlib.redirect_stdout(io.StringIO()), self.assertRaises(SystemExit):
            af.main()
        self.assertEqual(self.runs(), [])

    def test_subcommand_verbs_keep_their_second_token(self):
        self.assertEqual(af.verb_of(["pipe", "stage", "acme", "applied"]), "pipe stage")
        self.assertEqual(af.verb_of(["draft", "p", "d", "--file", "x-v1.md"]), "draft")
        self.assertEqual(af.verb_of(["board", "list", "--json"]), "board list")
        self.assertEqual(af.verb_of([]), "?")


class LazyIndexer(unittest.TestCase):
    def test_af_no_longer_binds_the_indexer_at_import(self):
        self.assertFalse(hasattr(af, "indexer"))

    def test_broken_indexer_is_a_clean_refusal_not_a_traceback(self):
        real_import = builtins.__import__

        def fake(name, globals=None, locals=None, fromlist=(), level=0):
            if name == "indexer" or (name == "system" and "indexer" in (fromlist or ())):
                raise SyntaxError("mid-edit")
            return real_import(name, globals, locals, fromlist, level)

        err = io.StringIO()
        with patch.object(builtins, "__import__", fake), contextlib.redirect_stderr(err), \
             self.assertRaises(SystemExit):
            af._indexer()
        self.assertIn("retrieval index unavailable", err.getvalue())
        self.assertIn("SyntaxError", err.getvalue())
        self.assertIn("state buttons are unaffected", err.getvalue())


class TempProject(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.projects = os.path.join(self._tmp.name, "workspace", "projects")
        p = patch.object(af, "PROJECTS", self.projects)
        p.start()
        self.addCleanup(p.stop)
        self.addCleanup(self._tmp.cleanup)
        self.cdir = os.path.join(self.projects, "campaign")

    def write_project(self, rows, domain="marketing"):
        os.makedirs(self.cdir, exist_ok=True)
        write(os.path.join(self.cdir, "project.md"),
                 "---\nname: campaign\nslug: campaign\nschema_version: 2026-07-19-v2\n"
                 f"created_at: 2026-09-01\ndomain: {domain}\nstatus: active\ncurrent_phase: active\n"
                 "flow: open-flow\nlast_activity: 2026-09-09T10:00:00-07:00\n"
                 "deliverables:\n" + rows + "---\n")

    def fm(self):
        return af.split_fm(af.read(os.path.join(self.cdir, "project.md")), "project.md")[0]

    def refusal(self, fn, args):
        err = io.StringIO()
        with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()), \
             self.assertRaises(SystemExit):
            fn(args)
        return err.getvalue()


class DraftAdoptRouting(TempProject):
    """The three refusals that sent four sessions into a draft/adopt retry loop now say where to go."""

    def test_existing_v1_names_adopt(self):
        self.write_project("")
        write(os.path.join(self.cdir, "brief", "brief-v1.md"), "---\nstatus: drafting\n---\nbody\n")
        msg = self.refusal(af.cmd_draft, types.SimpleNamespace(
            project="campaign", deliverable="brief", artifact=None, file="brief/brief-v1.md"))
        self.assertIn("af adopt campaign brief --file", msg)

    def test_row_with_an_artifact_names_version(self):
        self.write_project("  brief:\n    status: drafting\n    file: brief/brief-v1.md\n"
                           "    last_updated: 2026-09-01\n")
        write(os.path.join(self.cdir, "brief", "brief-v1.md"), "---\nstatus: drafting\n---\nbody\n")
        msg = self.refusal(af.cmd_draft, types.SimpleNamespace(
            project="campaign", deliverable="brief", artifact=None, file="brief/brief-v1.md"))
        self.assertIn("af version campaign brief", msg)

    def test_non_canonical_name_says_adopt_registers_existing_files(self):
        self.write_project("")
        msg = self.refusal(af.cmd_draft, types.SimpleNamespace(
            project="campaign", deliverable="brief", artifact=None, file="brief/_seed.md"))
        self.assertIn("<slug>-v1.md", msg)
        self.assertIn("af adopt", msg)

    def test_missing_parent_row_names_the_file_route_not_a_hand_edit(self):
        self.write_project("")
        msg = self.refusal(af.cmd_draft, types.SimpleNamespace(
            project="campaign", deliverable="post-9", artifact="body-copy", file=None))
        self.assertIn("af draft campaign post-9 --file", msg)
        self.assertIn("Never hand-write the row", msg)


class AdoptStampsBookkeeping(TempProject):
    def test_missing_last_updated_is_stamped_and_version_then_works(self):
        self.write_project("")
        path = os.path.join(self.cdir, "spine", "spine-v1.md")
        write(path, "---\nstatus: drafting\n---\n# Spine\n\nbody\n")
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            af.cmd_adopt(types.SimpleNamespace(project="campaign", deliverable="spine",
                                               file="spine/spine-v1.md", workstream=None,
                                               export=None, notes=None))
        self.assertIn("last_updated stamped", out.getvalue())
        afm, body = af.split_fm(af.read(path), "spine-v1.md")
        self.assertEqual(af.get_scalar(afm, "last_updated"), af.today())
        self.assertIn("# Spine", body)
        quiet(af.cmd_version, types.SimpleNamespace(project="campaign", deliverable="spine", artifact=None))
        self.assertTrue(os.path.isfile(os.path.join(self.cdir, "spine", "spine-v2.md")))

    def test_present_last_updated_is_left_alone(self):
        self.write_project("")
        path = os.path.join(self.cdir, "spine", "spine-v1.md")
        write(path, "---\nstatus: drafting\nlast_updated: 2026-01-01\n---\nbody\n")
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            af.cmd_adopt(types.SimpleNamespace(project="campaign", deliverable="spine",
                                               file="spine/spine-v1.md", workstream=None,
                                               export=None, notes=None))
        self.assertNotIn("stamped", out.getvalue())
        afm, _ = af.split_fm(af.read(path), "spine-v1.md")
        self.assertEqual(af.get_scalar(afm, "last_updated"), "2026-01-01")


class AssemblyRecordRows(TempProject):
    def _row(self, slug):
        return (f"  {slug}:\n    status: drafting\n    file: posts/{slug}/post-FINAL.md\n"
                f"    last_updated: 2026-09-07\n")

    def _issues(self, domain):
        self.write_project(self._row("post-2-banyan"), domain=domain)
        write(os.path.join(self.cdir, "posts", "post-2-banyan", "post-FINAL.md"),
                 "---\nstatus: drafting\nlast_updated: 2026-09-07\n---\n")
        return [i for i in af.check_project(self.cdir) if "numeric version head" in i]

    def test_any_marketing_row_on_an_assembly_record_is_clean(self):
        self.assertEqual(self._issues("marketing"), [])

    def test_outside_marketing_the_check_still_fires(self):
        self.assertEqual(len(self._issues("project-mgmt")), 1)


class ButtonNotes(TempDB):
    def test_silent_when_nothing_failed(self):
        self.add("draft", 0)
        self.add("doctor", 1)             # findings, not a refusal
        self.add("sync-harnesses", 1)
        self.assertEqual(af.button_notes(), [])

    def test_counts_refusals_crashes_and_slips_by_verb(self):
        self.add("draft", 1, "already has a version chain")
        self.add("draft", 1, "repo-root-relative")
        self.add("adopt", 1, "status must be drafting")
        self.add("draft", 2, "SyntaxError: mid-edit")
        self.add("version", 2, "usage")
        self.add("pipe board", 1)
        notes = af.button_notes()
        self.assertEqual(len(notes), 1)
        self.assertIn("3 refusal(s): draft 2, adopt 1", notes[0])
        self.assertIn("1 crash(es): draft 1", notes[0])
        self.assertIn("1 usage slip(s)", notes[0])
        self.assertIn("needs a row", notes[0])

    def test_old_rows_fall_out_of_the_window(self):
        self.add("draft", 1, "old", days_ago=af.BUTTON_WINDOW_DAYS + 1)
        self.assertEqual(af.button_notes(), [])

    def test_open_backlog_rows_naming_af_are_counted(self):
        root = os.path.join(self._tmp.name, "root")
        write(os.path.join(root, "system", "builder-backlog.md"),
                 "# Builder Backlog\n\nprose mentioning af.py that is not a row\n\n## Active entries\n\n"
                 "## BB-2026-09-01-01 — prose only\nnothing about buttons here\n\n"
                 "## BB-2026-09-01-02 — `af ready` refuses a shape\nthe button dies on it\n\n"
                 "```yaml\n- id: BB-2026-09-01-03\n  one_line: af.py draft path bug\n```\n")
        with patch.object(af, "ROOT", root):
            self.assertEqual(af.open_backlog_rows_naming_af(), 2)


if __name__ == "__main__":
    unittest.main()
