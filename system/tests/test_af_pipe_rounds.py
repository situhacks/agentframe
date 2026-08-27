"""Inbound entry, interview rounds, and application archiving."""

import contextlib
import io
import os
from types import SimpleNamespace

from system import af
from system.tests.test_af_pipe import PipeBase, quiet, write


def capture(fn, *a, **kw):
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        fn(*a, **kw)
    return buf.getvalue()


class RoundBase(PipeBase):
    def round(self, slug, n, **kw):
        quiet(af.cmd_pipe_round, SimpleNamespace(
            slug=slug, number=n, person=kw.get("person"), type=kw.get("type"),
            at=kw.get("at"), folder=kw.get("folder")))

    def archive(self, slug):
        quiet(af.cmd_pipe_archive, SimpleNamespace(slug=slug))

    def unarchive(self, slug):
        quiet(af.cmd_pipe_unarchive, SimpleNamespace(slug=slug))

    def started(self, slug="acme-ai-pm"):
        self.save(slug)
        self.start(slug)
        write(os.path.join(af.app_dir(slug), "jd.md"), "verbatim JD")
        return slug


class TestStartScaffoldsInputs(RoundBase):
    def test_sources_correspondence_people_created(self):
        slug = self.started()
        adir = af.app_dir(slug)
        for sub in ("sources", "correspondence", "people"):
            self.assertTrue(os.path.isdir(os.path.join(adir, sub)), sub)

    def test_sources_index_written_from_pack_skeleton(self):
        slug = self.started()
        idx = af.read(os.path.join(af.app_dir(slug), "sources", "INDEX.md"))
        self.assertIn("Acme - AI PM", idx)
        self.assertIn("Never prep from this folder", idx)
        self.assertNotIn("{name}", idx)


class TestInboundSkipsApplied(RoundBase):
    def test_preparing_to_interviewing_is_legal(self):
        slug = self.started()
        self.stage(slug, "interviewing")
        fm = self.board_fm()
        self.assertEqual(af.row_get(fm, slug, "stage"), "interviewing")
        self.assertIsNone(af.row_get(fm, slug, "applied"))
        self.assertIsNone(af.row_get(fm, slug, "shipped"))

    def test_skip_is_named_in_output(self):
        slug = self.started()
        out = capture(af.cmd_pipe_stage, SimpleNamespace(slug=slug, stage="interviewing"))
        self.assertIn("submitted_by", out)

    def test_saved_to_interviewing_still_refused(self):
        slug = self.save()
        with self.assertRaises(SystemExit):
            self.stage(slug, "interviewing")


class TestPipeRound(RoundBase):
    def test_folder_and_readme_from_pack_skeleton(self):
        slug = self.started()
        self.round(slug, 2, person=["Charles Duckworth, Senior Director"],
                   type="hiring-manager", at="2026-09-02T10:00:00-07:00")
        rdir = os.path.join(af.app_dir(slug), "round-2-charles")
        self.assertTrue(os.path.isdir(rdir))
        readme = af.read(os.path.join(rdir, "README.md"))
        self.assertIn("Charles Duckworth, Senior Director", readme)
        self.assertIn("hiring-manager", readme)
        self.assertIn("round 1", readme)          # {prev} resolves to the prior round
        self.assertNotIn("{people}", readme)

    def test_panel_lists_every_person(self):
        slug = self.started()
        self.round(slug, 3, person=["Dave McKellar, Partner", "Jeff O'Neill, Senior Manager"],
                   type="panel")
        readme = af.read(os.path.join(af.app_dir(slug), "round-3-dave", "README.md"))
        self.assertIn("Dave McKellar", readme)
        self.assertIn("Jeff O'Neill", readme)

    def test_unnamed_interviewer_is_called_out(self):
        slug = self.started()
        self.round(slug, 1, type="recruiter-screen")
        readme = af.read(os.path.join(af.app_dir(slug), "round-1-recruiter-screen", "README.md"))
        self.assertIn("No interviewer named yet", readme)

    def test_first_round_prev_points_at_application_root(self):
        slug = self.started()
        self.round(slug, 1, person=["Mike Duffy, Recruiter"])
        readme = af.read(os.path.join(af.app_dir(slug), "round-1-mike", "README.md"))
        self.assertIn("the application root", readme)

    def test_duplicate_round_refused(self):
        slug = self.started()
        self.round(slug, 1, person=["Mike Duffy, Recruiter"])
        with self.assertRaises(SystemExit):
            self.round(slug, 1, person=["Mike Duffy, Recruiter"])

    def test_terminal_stage_refused(self):
        slug = self.started()
        self.stage(slug, "dropped")
        with self.assertRaises(SystemExit):
            self.round(slug, 1, person=["Mike Duffy, Recruiter"])

    def test_missing_application_folder_refused(self):
        slug = self.save("no-folder-yet")
        with self.assertRaises(SystemExit):
            self.round(slug, 1, person=["Mike Duffy, Recruiter"])


class TestArchive(RoundBase):
    def test_moves_folder_and_keeps_board_row(self):
        slug = self.started()
        self.stage(slug, "rejected")
        self.archive(slug)
        self.assertFalse(os.path.isdir(os.path.join(af.apps_root(), slug)))
        self.assertTrue(os.path.isfile(
            os.path.join(af.apps_root(), af.APPS_ARCHIVE, slug, "application.md")))
        self.assertEqual(af.row_get(self.board_fm(), slug, "stage"), "rejected")

    def test_app_dir_resolves_archived(self):
        slug = self.started()
        self.stage(slug, "rejected")
        self.archive(slug)
        self.assertTrue(af.app_is_archived(slug))
        self.assertTrue(af.app_dir(slug).endswith(os.path.join(af.APPS_ARCHIVE, slug)))

    def test_active_stage_refused(self):
        """No override exists: retiring a live row means staging it, which keeps the record honest."""
        slug = self.started()
        with self.assertRaises(SystemExit):
            self.archive(slug)
        self.stage(slug, "dropped")
        self.archive(slug)
        self.assertTrue(af.app_is_archived(slug))

    def test_unarchive_returns_the_folder(self):
        slug = self.started()
        self.stage(slug, "ghosted")
        self.archive(slug)
        self.unarchive(slug)
        self.assertFalse(af.app_is_archived(slug))
        self.assertTrue(os.path.isfile(os.path.join(af.apps_root(), slug, "application.md")))

    def test_unarchive_refused_when_not_archived(self):
        slug = self.started()
        with self.assertRaises(SystemExit):
            self.unarchive(slug)

    def test_archived_application_stays_clean_in_doctor(self):
        slug = self.started()
        self.stage(slug, "rejected")
        self.archive(slug)
        issues, _ = af.check_pipeline()
        self.assertEqual(issues, [])


class TestRoundDoctorNotes(RoundBase):
    def _round_files(self, slug, held, debrief=None):
        rdir = os.path.join(af.app_dir(slug), "round-1-jeff")
        os.makedirs(rdir, exist_ok=True)
        write(os.path.join(rdir, "README.md"), f"---\nheld_at: {held}\n---\n\n# Round 1\n")
        if debrief is not None:
            write(os.path.join(rdir, "debrief.md"), debrief)
        return rdir

    def test_held_round_without_debrief_is_a_note(self):
        slug = self.started()
        self._round_files(slug, "2026-01-05T10:00:00-07:00")
        _, notes = af.check_pipeline()
        self.assertTrue(any("has no debrief.md" in n for n in notes), notes)

    def test_future_round_without_debrief_is_quiet(self):
        slug = self.started()
        self._round_files(slug, "2099-01-05T10:00:00-07:00")
        _, notes = af.check_pipeline()
        self.assertFalse(any("debrief.md" in n for n in notes), notes)

    def test_unpromoted_debrief_is_a_note(self):
        slug = self.started()
        self._round_files(slug, "2026-01-05T10:00:00-07:00",
                          "---\nstatus: ready\ncompleteness: full\npromoted: false\n---\n\n# D\n")
        _, notes = af.check_pipeline()
        self.assertTrue(any("have not received this round's facts" in n for n in notes), notes)

    def test_partial_completeness_is_a_note(self):
        slug = self.started()
        self._round_files(slug, "2026-01-05T10:00:00-07:00",
                          "---\nstatus: drafting\ncompleteness: partial - operator has not reported\n"
                          "promoted: false\n---\n\n# D\n")
        _, notes = af.check_pipeline()
        self.assertTrue(any("completeness is partial" in n for n in notes), notes)

    def test_promoted_full_debrief_is_quiet(self):
        slug = self.started()
        self._round_files(slug, "2026-01-05T10:00:00-07:00",
                          "---\nstatus: ready\ncompleteness: full\npromoted: true\n---\n\n# D\n")
        _, notes = af.check_pipeline()
        self.assertFalse(any("debrief.md" in n for n in notes), notes)
