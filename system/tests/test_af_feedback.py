"""`af feedback`: the feedback log's unharvested tail, the harvest watermark, and the doctor note.

The log holds lessons that outlive one deliverable. A harvest closes with a
`## harvested YYYY-MM-DD` heading; everything below the last heading is what the
next drafting session (and the next harvest) reads. Doctor counts that tail on
active projects only.
"""
import contextlib
import io
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

from system import af


class FeedbackLogTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.projects = os.path.join(self._tmp.name, "workspace", "projects")
        self._patch_projects = patch.object(af, "PROJECTS", self.projects)
        self._patch_projects.start()
        self.addCleanup(self._patch_projects.stop)
        self.addCleanup(self._tmp.cleanup)
        self.run_cli("new-project", "fixture", "--domain", "project-mgmt", "--flow", "open-flow")
        self.cdir = os.path.join(self.projects, "fixture")
        self.log = os.path.join(self.cdir, "feedback-log.md")

    def run_cli(self, *args):
        stdout = io.StringIO()
        with patch.object(sys, "argv", ["af", *args]), contextlib.redirect_stdout(stdout):
            af.main()
        return stdout.getvalue()

    def write_log(self, text):
        af.write(self.log, text)

    # -- tail -----------------------------------------------------------------

    def test_scaffold_leaves_an_empty_log_with_nothing_pending(self):
        tail, count, last = af.feedback_tail(self.cdir)
        self.assertEqual(([], 0, None), (tail, count, last))
        self.assertIn("0 unharvested lesson(s), never harvested", self.run_cli("feedback", "fixture"))

    def test_without_a_watermark_the_whole_log_is_the_tail(self):
        self.write_log(
            "- 2026-09-01 — Lesson one. Evidence one.\n"
            "- 2026-09-02 — Lesson two. Evidence two.\n"
            "- 2026-09-03 — Lesson three. Evidence three.\n"
        )
        tail, count, last = af.feedback_tail(self.cdir)
        self.assertEqual(3, count)
        self.assertIsNone(last)
        out = self.run_cli("feedback", "fixture")
        self.assertIn("3 unharvested lesson(s), never harvested", out)
        self.assertIn("Lesson one", out)
        self.assertIn("Lesson three", out)

    def test_the_last_watermark_splits_the_tail(self):
        self.write_log(
            "- 2026-08-01 — Old lesson. Mined already.\n"
            "\n## harvested 2026-08-07 — voice system\n\n"
            "- 2026-08-19 — Middle lesson. Mined too.\n"
            "\n## harvested 2026-09-01\n\n"
            "- 2026-09-05 — New lesson. Not yet mined.\n"
        )
        tail, count, last = af.feedback_tail(self.cdir)
        self.assertEqual(1, count)
        self.assertEqual("2026-09-01", last)
        out = self.run_cli("feedback", "fixture")
        self.assertIn("1 unharvested lesson(s), since the 2026-09-01 harvest", out)
        self.assertIn("New lesson", out)
        self.assertNotIn("Old lesson", out)
        self.assertNotIn("Middle lesson", out)

    def test_legacy_entry_shapes_still_count(self):
        # The older logs mix dated headings, bold bullets, and bare dated lines.
        self.write_log(
            "# Feedback Log — Fixture\n\nPreamble prose that is not an entry.\n\n"
            "## 2026-05-03 — Post 5 session\n\n"
            "- **Body copy over-duplicated.** Detail.\n"
            "- **Slide role overlap.** Detail.\n\n"
            "2026-08-05 — NAMED ANTI-PATTERN: the significance tail. Detail.\n"
        )
        _, count, _ = af.feedback_tail(self.cdir)
        self.assertEqual(4, count)

    # -- watermark ------------------------------------------------------------

    def test_mark_harvested_appends_the_heading_and_an_activity_line(self):
        self.write_log("- 2026-09-01 — Lesson one. Evidence.\n- 2026-09-02 — Lesson two. Evidence.\n")
        out = self.run_cli("feedback", "fixture", "--mark-harvested", "--note", "two pairs, one template patch")
        self.assertIn("watermark written", out)
        self.assertIn("2 lesson(s) now below it", out)
        text = af.read(self.log)
        self.assertTrue(text.endswith(f"\n\n## harvested {af.today()} — two pairs, one template patch\n"), text)
        self.assertIn("Lesson one", text)  # entries are never deleted, only marked
        activity = af.read(os.path.join(self.cdir, "activity.md"))
        self.assertIn("feedback_harvested: 2 lesson(s) above the watermark mined; two pairs, one template patch", activity)
        tail, count, last = af.feedback_tail(self.cdir)
        self.assertEqual(([], 0, af.today()), (tail, count, last))

    def test_mark_harvested_on_an_empty_log_writes_a_bare_heading(self):
        self.run_cli("feedback", "fixture", "--mark-harvested")
        self.assertEqual(f"## harvested {af.today()}\n", af.read(self.log))

    def test_mark_harvested_refuses_when_the_log_is_missing(self):
        os.remove(self.log)
        with patch.object(sys, "argv", ["af", "feedback", "fixture", "--mark-harvested"]), \
             contextlib.redirect_stderr(io.StringIO()) as err, self.assertRaises(SystemExit):
            af.main()
        self.assertIn("nothing to mark", err.getvalue())

    # -- doctor ---------------------------------------------------------------

    def test_doctor_notes_unharvested_lessons_on_active_projects(self):
        self.write_log("- 2026-09-01 — Lesson one. Evidence.\n")
        notes = af.feedback_notes(self.cdir)
        self.assertEqual(1, len(notes))
        self.assertIn("feedback-log.md 1 unharvested lesson(s), never harvested", notes[0])
        self.assertIn("af feedback fixture", notes[0])
        self.assertIn("1 unharvested lesson(s)", self.run_cli("doctor", "fixture"))

    def test_doctor_stays_quiet_once_the_tail_is_marked_or_the_project_completes(self):
        self.write_log("- 2026-09-01 — Lesson one. Evidence.\n")
        self.run_cli("feedback", "fixture", "--mark-harvested")
        self.assertEqual([], af.feedback_notes(self.cdir))

        self.write_log("- 2026-09-01 — Lesson one. Evidence.\n")  # pending again, but the project is done
        cpath = os.path.join(self.cdir, "project.md")
        cfm, cbody = af.split_fm(af.read(cpath), "project.md")
        cfm = af.set_scalar(cfm, "status", "complete")
        af.write(cpath, af.join_fm(cfm, cbody))
        self.assertEqual([], af.feedback_notes(self.cdir))


if __name__ == "__main__":
    unittest.main()
